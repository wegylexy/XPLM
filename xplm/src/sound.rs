//! Minimal FMOD-backed audio (`XPLMSound.h`, XPLM400+): playing an in-memory
//! PCM16 buffer on one of X-Plane's audio buses, with basic 3D positioning.
//!
//! This deliberately only covers the "basic wrapper" half of the header —
//! `XPLMGetFMODStudio`/`XPLMGetFMODChannelGroup` require linking the real
//! FMOD SDK (they're gated behind `#if defined(_FMOD_COMMON_H)` in the
//! vendored header, which this crate never defines), so they aren't
//! reachable through `xplm-sys` at all.

use std::cell::Cell;
use std::ffi::c_void;
use std::os::raw::c_int;
use std::rc::Rc;

use xplm_sys::{
    xplm_AudioExteriorAircraft, xplm_AudioExteriorEnvironment, xplm_AudioExteriorUnprocessed,
    xplm_AudioGround, xplm_AudioInterior, xplm_AudioRadioCom1, xplm_AudioRadioCom2,
    xplm_AudioRadioCopilot, xplm_AudioRadioPilot, xplm_AudioUI, xplm_Master, XPLMAudioBus,
    XPLMPlayPCMOnBus, XPLMSetAudioCone, XPLMSetAudioFadeDistance, XPLMSetAudioPitch,
    XPLMSetAudioPosition, XPLMSetAudioVolume, XPLMStopAudio, FMOD_RESULT_FMOD_OK,
    FMOD_SOUND_FORMAT_FMOD_SOUND_FORMAT_PCM16, FMOD_VECTOR,
};

/// Which part of the simulated environment a sound belongs to
/// (`XPLMAudioBus`). COM1/COM2/Pilot/Copilot live in a separate FMOD bank
/// (and possibly a separate output device, if the user has a dedicated
/// headset device selected) from the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioBus {
    RadioCom1,
    RadioCom2,
    RadioPilot,
    RadioCopilot,
    ExteriorAircraft,
    ExteriorEnvironment,
    ExteriorUnprocessed,
    Interior,
    Ui,
    Ground,
    /// The master bus. Not normally used directly.
    Master,
}

impl From<AudioBus> for XPLMAudioBus {
    fn from(bus: AudioBus) -> Self {
        match bus {
            AudioBus::RadioCom1 => xplm_AudioRadioCom1,
            AudioBus::RadioCom2 => xplm_AudioRadioCom2,
            AudioBus::RadioPilot => xplm_AudioRadioPilot,
            AudioBus::RadioCopilot => xplm_AudioRadioCopilot,
            AudioBus::ExteriorAircraft => xplm_AudioExteriorAircraft,
            AudioBus::ExteriorEnvironment => xplm_AudioExteriorEnvironment,
            AudioBus::ExteriorUnprocessed => xplm_AudioExteriorUnprocessed,
            AudioBus::Interior => xplm_AudioInterior,
            AudioBus::Ui => xplm_AudioUI,
            AudioBus::Ground => xplm_AudioGround,
            AudioBus::Master => xplm_Master,
        }
    }
}

/// Backing state for an in-flight [`AudioChannel`]'s sample buffer — boxed
/// so its address is stable for as long as X-Plane might still be reading
/// it, and only freed from [`completion_trampoline`], which the SDK
/// guarantees is called exactly once per sound (success, failure, or
/// explicit [`AudioChannel::stop`]).
struct PlaybackState {
    _buffer: Box<[i16]>,
    alive: Rc<Cell<bool>>,
}

unsafe extern "C" fn completion_trampoline(refcon: *mut c_void, _status: xplm_sys::FMOD_RESULT) {
    crate::guard(|| {
        let state = unsafe { Box::from_raw(refcon as *mut PlaybackState) };
        state.alive.set(false);
        // `state` (and its `_buffer`) drops here.
    });
}

/// A handle to a playing (or finished) FMOD channel (`FMOD_CHANNEL*`),
/// returned by [`AudioChannel::play_pcm16`]. There is no `Drop` that stops
/// the sound — dropping the handle just lets it keep playing, per the SDK's
/// own guidance for callers with no further interest in it; the sample
/// buffer is freed automatically once the sound completes.
///
/// The channel pointer becomes invalid the instant the sound finishes (or is
/// stopped) — every method here checks the shared "still alive" flag (set
/// by the completion callback) first and no-ops/returns `false` once it's
/// gone, rather than risk a use-after-free on a dangling `FMOD_CHANNEL*`.
pub struct AudioChannel {
    raw: *mut xplm_sys::FMOD_CHANNEL,
    alive: Rc<Cell<bool>>,
}

unsafe impl Send for AudioChannel {} // see FlightLoop's identical rationale: main-thread-only.

impl AudioChannel {
    /// Plays `samples` (16-bit signed PCM, `num_channels`-interleaved) on
    /// `bus`. The buffer is copied, so `samples` need not outlive the call.
    /// Playback doesn't start instantly — only at X-Plane's next sound
    /// refresh (typically next frame).
    pub fn play_pcm16(
        bus: AudioBus,
        samples: &[i16],
        freq_hz: i32,
        num_channels: i32,
        looped: bool,
    ) -> Self {
        let alive = Rc::new(Cell::new(true));
        let state = Box::new(PlaybackState {
            _buffer: samples.into(),
            alive: alive.clone(),
        });
        let audio_ptr = state._buffer.as_ptr() as *mut c_void;
        let buffer_size = (state._buffer.len() * std::mem::size_of::<i16>()) as u32;
        let refcon = Box::into_raw(state) as *mut c_void;

        let raw = unsafe {
            XPLMPlayPCMOnBus(
                audio_ptr,
                buffer_size,
                FMOD_SOUND_FORMAT_FMOD_SOUND_FORMAT_PCM16,
                freq_hz,
                num_channels,
                looped as c_int,
                bus.into(),
                Some(completion_trampoline),
                refcon,
            )
        };
        Self { raw, alive }
    }

    /// Whether this channel is still valid — `false` once the sound has
    /// finished, been stopped, or failed to start.
    pub fn is_alive(&self) -> bool {
        self.alive.get()
    }

    /// Stops playback. A no-op if the sound has already finished on its
    /// own. Consumes `self` since the channel is never valid afterward.
    pub fn stop(self) {
        if self.alive.get() {
            unsafe { XPLMStopAudio(self.raw) };
        }
    }

    /// Moves this (now-3D) sound to `position`/`velocity` in local
    /// coordinates. `false` if the channel is no longer alive.
    pub fn set_position(&self, position: (f32, f32, f32), velocity: (f32, f32, f32)) -> bool {
        if !self.alive.get() {
            return false;
        }
        let mut pos = FMOD_VECTOR {
            x: position.0,
            y: position.1,
            z: position.2,
        };
        let mut vel = FMOD_VECTOR {
            x: velocity.0,
            y: velocity.1,
            z: velocity.2,
        };
        unsafe { XPLMSetAudioPosition(self.raw, &mut pos, &mut vel) == FMOD_RESULT_FMOD_OK }
    }

    /// Sets this (now-3D) sound's min/max fade distances. Pass two negative
    /// values to set it back to a 2D (non-positional) sound. `false` if the
    /// channel is no longer alive.
    pub fn set_fade_distance(&self, min_distance: f32, max_distance: f32) -> bool {
        if !self.alive.get() {
            return false;
        }
        unsafe {
            XPLMSetAudioFadeDistance(self.raw, min_distance, max_distance) == FMOD_RESULT_FMOD_OK
        }
    }

    /// `volume` of `1.0` is unchanged; above `1.0` artificially amplifies.
    /// `false` if the channel is no longer alive.
    pub fn set_volume(&self, volume: f32) -> bool {
        if !self.alive.get() {
            return false;
        }
        unsafe { XPLMSetAudioVolume(self.raw, volume) == FMOD_RESULT_FMOD_OK }
    }

    /// `false` if the channel is no longer alive.
    pub fn set_pitch(&self, pitch_hz: f32) -> bool {
        if !self.alive.get() {
            return false;
        }
        unsafe { XPLMSetAudioPitch(self.raw, pitch_hz) == FMOD_RESULT_FMOD_OK }
    }

    /// Sets this (now-3D) sound's directional cone; `orientation` is in
    /// local coordinates. `false` if the channel is no longer alive.
    pub fn set_cone(
        &self,
        inside_angle: f32,
        outside_angle: f32,
        outside_volume: f32,
        orientation: (f32, f32, f32),
    ) -> bool {
        if !self.alive.get() {
            return false;
        }
        let mut orientation = FMOD_VECTOR {
            x: orientation.0,
            y: orientation.1,
            z: orientation.2,
        };
        unsafe {
            XPLMSetAudioCone(
                self.raw,
                inside_angle,
                outside_angle,
                outside_volume,
                &mut orientation,
            ) == FMOD_RESULT_FMOD_OK
        }
    }
}
