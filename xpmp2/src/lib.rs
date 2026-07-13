//! Safe wrapper over `xpmp2-sys`: RAII lifecycle ([`Multiplayer`]) and an
//! [`Aircraft`] trait + RAII [`Plane`] handle over the C++ shim's
//! `XPMP2ShimAircraft`, mirroring the shape `xplm`'s own wrappers use
//! (RAII owns cleanup, no leaky panics across the FFI boundary).

use std::ffi::{c_void, CStr, CString};
use std::marker::PhantomData;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(any(feature = "csl-offline", feature = "csl-on-demand"))]
use xpmp2_sys::XPMPLoadCSLPackage;
use xpmp2_sys::{
    xpmp2_shim_aircraft_camera_bearing, xpmp2_shim_aircraft_camera_dist,
    xpmp2_shim_aircraft_change_model, xpmp2_shim_aircraft_contrail_remove,
    xpmp2_shim_aircraft_contrail_request, xpmp2_shim_aircraft_contrail_trigger,
    xpmp2_shim_aircraft_create, xpmp2_shim_aircraft_dataref_count, xpmp2_shim_aircraft_destroy,
    xpmp2_shim_aircraft_get_dataref, xpmp2_shim_aircraft_get_flight_id,
    xpmp2_shim_aircraft_get_location, xpmp2_shim_aircraft_get_model_name,
    xpmp2_shim_aircraft_get_vert_ofs, xpmp2_shim_aircraft_ground_speed_kn,
    xpmp2_shim_aircraft_is_glider, xpmp2_shim_aircraft_is_ground_vehicle,
    xpmp2_shim_aircraft_is_related_to, xpmp2_shim_aircraft_is_rendered,
    xpmp2_shim_aircraft_is_shown_as_ai, xpmp2_shim_aircraft_is_shown_as_tcas_target,
    xpmp2_shim_aircraft_is_sound_muted, xpmp2_shim_aircraft_is_valid,
    xpmp2_shim_aircraft_is_visible, xpmp2_shim_aircraft_match_quality,
    xpmp2_shim_aircraft_mode_s_id, xpmp2_shim_aircraft_rematch_model,
    xpmp2_shim_aircraft_set_ai_priority, xpmp2_shim_aircraft_set_clamp_to_ground,
    xpmp2_shim_aircraft_set_dataref, xpmp2_shim_aircraft_set_gear_deflect_ratio,
    xpmp2_shim_aircraft_set_heading, xpmp2_shim_aircraft_set_info_texts,
    xpmp2_shim_aircraft_set_label, xpmp2_shim_aircraft_set_label_color,
    xpmp2_shim_aircraft_set_label_drawn, xpmp2_shim_aircraft_set_local_loc,
    xpmp2_shim_aircraft_set_location, xpmp2_shim_aircraft_set_mass,
    xpmp2_shim_aircraft_set_on_ground, xpmp2_shim_aircraft_set_pitch,
    xpmp2_shim_aircraft_set_radar, xpmp2_shim_aircraft_set_render, xpmp2_shim_aircraft_set_roll,
    xpmp2_shim_aircraft_set_sound_min_dist, xpmp2_shim_aircraft_set_sound_muted,
    xpmp2_shim_aircraft_set_velocity, xpmp2_shim_aircraft_set_vert_ofs_ratio,
    xpmp2_shim_aircraft_set_visible, xpmp2_shim_aircraft_set_wing_area,
    xpmp2_shim_aircraft_set_wing_span, xpmp2_shim_aircraft_show_as_ai_plane,
    xpmp2_shim_aircraft_tcas_target_idx, xpmp2_shim_aircraft_wake_apply_defaults,
    XPMP2ShimAircraft, XPMPMultiplayerCleanup, XPMPMultiplayerInit,
};

// Mutually exclusive on purpose: `csl-offline` (bulk-load a local CSL
// library up front) and `csl-on-demand` (fetch + load exactly one model's
// package at a time) are two different loading strategies for the same
// underlying `XPMPLoadCSLPackage` call, not layers meant to compose. Picking
// both in one build is almost certainly a Cargo.toml mistake (most likely:
// enabling `csl-on-demand` without also turning off this crate's default
// features, which include `csl-offline`) rather than an intentional hybrid,
// so it's a hard compile error instead of silently allowing both APIs.
#[cfg(all(feature = "csl-offline", feature = "csl-on-demand"))]
compile_error!(
    "xpmp2's `csl-offline` and `csl-on-demand` features are mutually exclusive — \
     enable only the one matching your plugin's loading strategy. This is most \
     often a Cargo feature-unification accident in a workspace: if one crate \
     depends on xpmp2 with `csl-offline` and another with `csl-on-demand`, \
     building both together unifies onto both being enabled. See CLAUDE.md's \
     \"CSL package loading\" note for why these are two independent loading \
     strategies, not a dependency chain."
);

#[cfg(feature = "csl-on-demand")]
pub mod csl_on_demand;

/// Only one [`Multiplayer`] may be live at a time — `XPMPMultiplayerInit`/
/// `XPMPMultiplayerCleanup` are process-wide, not per-handle, same as the
/// underlying XPMP2 library itself assumes (a plugin calls each exactly
/// once, from `XPluginStart`/`XPluginStop`).
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// RAII handle over `XPMPMultiplayerInit`/`XPMPMultiplayerCleanup` — call
/// [`Multiplayer::init`] from `XPluginStart` (after XPLM itself is up) and
/// hold the returned value for as long as the plugin wants multiplayer
/// planes; `XPMPMultiplayerCleanup` runs automatically on drop.
pub struct Multiplayer {
    // Blocks auto-derived Sync (see the `unsafe impl Send` below) — XPMP2,
    // like the rest of the XPLM SDK, is only ever safe to call from
    // X-Plane's main thread, so it must never be shared across threads.
    _not_sync: PhantomData<*const ()>,
}

// X-Plane only ever calls into a plugin (XPluginStart/Enable/Disable/Stop,
// every registered callback) from its single main thread, so there's never
// concurrent access to a `Multiplayer` — but plugin state holding one still
// needs to live in a `static Mutex<Option<_>>` (see `register_plugin!`),
// which requires `Send`. Same rationale as `xplm::dataref::DataRef`/
// `xplm::processing::FlightLoop`/`xplm::window::Window`/`xplm::menu::Menu`.
// Not Sync: nothing here supports being read from two threads at once.
unsafe impl Send for Multiplayer {}

impl Multiplayer {
    /// `plugin_name` is used as the map layer name and in logging.
    /// `resource_dir` must contain XPMP2's supplemental files (`Doc8643.txt`,
    /// `MapIcons.png`, `related.txt`, optionally `Obj8DataRefs.txt`).
    /// `default_icao` is a fallback aircraft type when none can otherwise be
    /// deduced; `log_acronym` is a short tag for log output, defaulting to
    /// `plugin_name` if `None`.
    ///
    /// Returns `Err` with XPMP2's human-readable message on failure (it
    /// returns an empty string on success, matching `XPMPMultiplayerInit`'s
    /// own documented contract).
    ///
    /// # Panics
    /// Panics if a [`Multiplayer`] is already live — `XPMPMultiplayerInit`
    /// is process-wide global state, not something a second handle could
    /// independently own.
    pub fn init(
        plugin_name: &str,
        resource_dir: &str,
        default_icao: Option<&str>,
        log_acronym: Option<&str>,
    ) -> Result<Self, String> {
        if INITIALIZED.swap(true, Ordering::SeqCst) {
            panic!("xpmp2::Multiplayer is already initialized — only one instance may be live at a time");
        }
        let plugin_name = CString::new(plugin_name).expect("plugin_name contains a NUL byte");
        let resource_dir = CString::new(resource_dir).expect("resource_dir contains a NUL byte");
        let default_icao =
            default_icao.map(|s| CString::new(s).expect("default_icao contains a NUL byte"));
        let log_acronym =
            log_acronym.map(|s| CString::new(s).expect("log_acronym contains a NUL byte"));
        let result = unsafe {
            XPMPMultiplayerInit(
                plugin_name.as_ptr(),
                resource_dir.as_ptr(),
                None,
                default_icao
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
                log_acronym
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
            )
        };
        let message = unsafe { CStr::from_ptr(result) }
            .to_string_lossy()
            .into_owned();
        if message.is_empty() {
            Ok(Self {
                _not_sync: PhantomData,
            })
        } else {
            INITIALIZED.store(false, Ordering::SeqCst);
            Err(message)
        }
    }

    /// Loads a CSL package directory you already have on disk (containing an
    /// `xsb_aircraft.txt`) — for a plugin that ships/installs its whole CSL
    /// library locally and loads it up front. Safe to call more than once
    /// (each additional package's models are added to what's already
    /// loaded).
    ///
    /// This is *not* what [`csl_on_demand`] uses — fetching one model at a
    /// time and loading only that model's package the moment it's needed is
    /// the whole point of "on demand"; see [`csl_on_demand::CslCache::request`],
    /// which loads its fetched package itself rather than routing through
    /// this method. `load_csl_package` exists purely for the bulk/local-
    /// library case, which is why it's gated behind its own `csl-offline`
    /// feature rather than being unconditionally available.
    ///
    /// Returns `Err` with XPMP2's human-readable message on failure (empty
    /// string means success, same convention as [`Multiplayer::init`]).
    #[cfg(feature = "csl-offline")]
    pub fn load_csl_package(&self, csl_folder: &str) -> Result<(), String> {
        load_csl_package_raw(csl_folder)
    }
}

/// `XPMPLoadCSLPackage`, shared by [`Multiplayer::load_csl_package`] (behind
/// `csl-offline`) and [`csl_on_demand::CslCache::request`] (behind
/// `csl-on-demand`) — deliberately not feature-gated itself, since
/// on-demand loading must work with `csl-offline` disabled: on-demand means
/// never bulk-loading a local library, not depending on the API that does.
#[cfg(any(feature = "csl-offline", feature = "csl-on-demand"))]
pub(crate) fn load_csl_package_raw(csl_folder: &str) -> Result<(), String> {
    let csl_folder = CString::new(csl_folder).expect("csl_folder contains a NUL byte");
    let result = unsafe { XPMPLoadCSLPackage(csl_folder.as_ptr()) };
    let message = unsafe { CStr::from_ptr(result) }
        .to_string_lossy()
        .into_owned();
    if message.is_empty() {
        Ok(())
    } else {
        Err(message)
    }
}

impl Drop for Multiplayer {
    fn drop(&mut self) {
        unsafe {
            XPMPMultiplayerCleanup();
        }
        INITIALIZED.store(false, Ordering::SeqCst);
    }
}

/// A multiplayer plane's per-frame behavior — implement this and pass an
/// instance to [`Plane::new`]. Mirrors overriding `XPMP2::Aircraft` directly
/// in C++, but through the shim's callback instead of a vtable.
///
/// `Any` is a supertrait so callers holding a `&dyn Aircraft`/`&mut dyn
/// Aircraft` (from [`Plane::aircraft`]/[`Plane::aircraft_mut`]) can recover
/// their own concrete type via trait-object upcasting
/// (`(aircraft as &mut dyn Any).downcast_mut::<T>()`) instead of an `unsafe`
/// pointer-reinterpret cast — `Plane::new`'s own `impl Aircraft + 'static`
/// bound already requires every implementor to be `'static`, which is all
/// `Any` needs, so this costs existing implementors nothing.
pub trait Aircraft: std::any::Any {
    /// Called once per drawing cycle (`XPMP2::Aircraft::UpdatePosition`).
    /// Update location/attitude/velocity/labels/dataRefs on the `plane`
    /// handle passed in — there is no other way to reach the underlying
    /// `XPMP2ShimAircraft` from here.
    fn update_position(
        &mut self,
        plane: &PlaneHandle,
        elapsed_since_last_call: f32,
        fl_counter: i32,
    );
}

/// Mirrors `XPMPTransponderMode` (`XPMPMultiplayer.h`) exactly — the mode
/// half of `XPMP2::Aircraft::acRadar`, forwarded by XPMP2 to this aircraft's
/// own TCAS target slot (`sim/cockpit2/tcas/targets/ssr_mode`).
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransponderMode {
    Off = 0,
    Standby,
    ModeA,
    ModeC,
    Test,
    ModeSGround,
    ModeSTaOnly,
    ModeSTaRa,
}

/// Mirrors `DR_VALS` (`XPMPAircraft.h`) exactly — indexes into
/// [`PlaneHandle::dataref`]/[`PlaneHandle::set_dataref`]'s generic `v[]`
/// array, which XPMP2 pre-populates with these entries (a CSL model's own
/// animation datarefs, if any, start immediately after [`Self::Count`]).
/// Kept as one enum rather than ~25 dedicated shim getter/setter pairs —
/// every one of these is a plain `float` slot in the same array the generic
/// accessors already reach, so a shim round trip per field would add
/// nothing but boilerplate on both sides of the FFI boundary.
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRefIndex {
    ControlsGearRatio = 0,
    ControlsNwsRatio,
    ControlsFlapRatio,
    ControlsSpoilerRatio,
    ControlsSpeedBrakeRatio,
    ControlsSlatRatio,
    ControlsWingSweepRatio,
    ControlsThrustRatio,
    ControlsYokePitchRatio,
    ControlsYokeHeadingRatio,
    ControlsYokeRollRatio,
    ControlsThrustRevers,

    ControlsTaxiLitesOn,
    ControlsLandingLitesOn,
    ControlsBeaconLitesOn,
    ControlsStrobeLitesOn,
    ControlsNavLitesOn,

    GearNoseGearDeflectionMtr,
    GearTireVerticalDeflectionMtr,
    GearTireRotationAngleDeg,
    GearTireRotationSpeedRpm,
    GearTireRotationSpeedRadSec,

    EnginesEngineRotationAngleDeg,
    EnginesEngineRotationSpeedRpm,
    EnginesEngineRotationSpeedRadSec,
    EnginesPropRotationAngleDeg,
    EnginesPropRotationSpeedRpm,
    EnginesPropRotationSpeedRadSec,
    EnginesThrustReverserDeployRatio,

    EnginesEngineRotationAngleDeg1,
    EnginesEngineRotationAngleDeg2,
    EnginesEngineRotationAngleDeg3,
    EnginesEngineRotationAngleDeg4,
    EnginesEngineRotationSpeedRpm1,
    EnginesEngineRotationSpeedRpm2,
    EnginesEngineRotationSpeedRpm3,
    EnginesEngineRotationSpeedRpm4,
    EnginesEngineRotationSpeedRadSec1,
    EnginesEngineRotationSpeedRadSec2,
    EnginesEngineRotationSpeedRadSec3,
    EnginesEngineRotationSpeedRadSec4,

    MiscTouchDown,
}

/// The subset of a [`Plane`]'s operations meaningful to call from inside
/// [`Aircraft::update_position`] — borrowed in, not owned, so an
/// implementation can't accidentally destroy its own plane mid-callback.
pub struct PlaneHandle<'a> {
    raw: *mut XPMP2ShimAircraft,
    _marker: PhantomData<&'a ()>,
}

impl PlaneHandle<'_> {
    pub fn mode_s_id(&self) -> u32 {
        unsafe { xpmp2_shim_aircraft_mode_s_id(self.raw) }
    }

    pub fn is_valid(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_is_valid(self.raw) != 0 }
    }

    pub fn set_visible(&self, visible: bool) {
        unsafe { xpmp2_shim_aircraft_set_visible(self.raw, visible as c_int) }
    }

    pub fn is_visible(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_is_visible(self.raw) != 0 }
    }

    /// Sets world coordinates (latitude/longitude degrees, altitude feet);
    /// XPMP2 converts to local OpenGL coordinates internally.
    pub fn set_location(&self, lat: f64, lon: f64, alt_ft: f64) {
        unsafe { xpmp2_shim_aircraft_set_location(self.raw, lat, lon, alt_ft) }
    }

    /// Returns `(lat, lon, alt_ft)`.
    pub fn location(&self) -> (f64, f64, f64) {
        let (mut lat, mut lon, mut alt_ft) = (0.0, 0.0, 0.0);
        unsafe { xpmp2_shim_aircraft_get_location(self.raw, &mut lat, &mut lon, &mut alt_ft) }
        (lat, lon, alt_ft)
    }

    /// Sets local OpenGL coordinates directly, skipping the world-to-local
    /// conversion [`Self::set_location`] does.
    pub fn set_local_location(&self, x: f32, y: f32, z: f32) {
        unsafe { xpmp2_shim_aircraft_set_local_loc(self.raw, x, y, z) }
    }

    pub fn set_pitch(&self, degrees: f32) {
        unsafe { xpmp2_shim_aircraft_set_pitch(self.raw, degrees) }
    }

    pub fn set_heading(&self, degrees: f32) {
        unsafe { xpmp2_shim_aircraft_set_heading(self.raw, degrees) }
    }

    pub fn set_roll(&self, degrees: f32) {
        unsafe { xpmp2_shim_aircraft_set_roll(self.raw, degrees) }
    }

    pub fn set_on_ground(&self, on_ground: bool) {
        unsafe { xpmp2_shim_aircraft_set_on_ground(self.raw, on_ground as c_int) }
    }

    /// Cartesian velocity in m/s.
    pub fn set_velocity(&self, vx: f32, vy: f32, vz: f32) {
        unsafe { xpmp2_shim_aircraft_set_velocity(self.raw, vx, vy, vz) }
    }

    pub fn set_label(&self, label: &str) {
        let label = CString::new(label).unwrap_or_default();
        unsafe { xpmp2_shim_aircraft_set_label(self.raw, label.as_ptr()) }
    }

    pub fn set_label_drawn(&self, drawn: bool) {
        unsafe { xpmp2_shim_aircraft_set_label_drawn(self.raw, drawn as c_int) }
    }

    /// The CSL model's animation dataRef array length.
    pub fn dataref_count(&self) -> usize {
        unsafe { xpmp2_shim_aircraft_dataref_count(self.raw) }
    }

    /// Returns `0.0` if `index` is out of bounds rather than panicking —
    /// mirrors the shim's own out-of-bounds handling.
    pub fn dataref(&self, index: usize) -> f32 {
        unsafe { xpmp2_shim_aircraft_get_dataref(self.raw, index) }
    }

    /// Silently does nothing if `index` is out of bounds.
    pub fn set_dataref(&self, index: usize, value: f32) {
        unsafe { xpmp2_shim_aircraft_set_dataref(self.raw, index, value) }
    }

    /// Sets this plane's transponder code/mode (`XPMP2::Aircraft::acRadar`),
    /// which XPMP2 forwards to its TCAS target slot so other traffic renders
    /// correctly on X-Plane's own TCAS display. `code` is a squawk code
    /// (e.g. `1200`), not validated here — XPMP2/X-Plane treat it the same
    /// as any other transponder code.
    pub fn set_radar(&self, code: u16, mode: TransponderMode) {
        unsafe {
            xpmp2_shim_aircraft_set_radar(self.raw, code as std::os::raw::c_long, mode as c_int)
        }
    }

    /// The label's base RGBA color, each channel `0..1`.
    pub fn set_label_color(&self, r: f32, g: f32, b: f32, a: f32) {
        unsafe { xpmp2_shim_aircraft_set_label_color(self.raw, r, g, b, a) }
    }

    /// `0..1`; phases out the ground/gear vertical offset at higher
    /// altitudes.
    pub fn set_vert_ofs_ratio(&self, ratio: f32) {
        unsafe { xpmp2_shim_aircraft_set_vert_ofs_ratio(self.raw, ratio) }
    }

    /// The vertical offset XPMP2 currently applies on top of the plane's
    /// local `y` coordinate to place the model on the ground.
    pub fn vert_ofs(&self) -> f32 {
        unsafe { xpmp2_shim_aircraft_get_vert_ofs(self.raw) }
    }

    pub fn set_gear_deflect_ratio(&self, ratio: f32) {
        unsafe { xpmp2_shim_aircraft_set_gear_deflect_ratio(self.raw, ratio) }
    }

    pub fn set_clamp_to_ground(&self, clamp: bool) {
        unsafe { xpmp2_shim_aircraft_set_clamp_to_ground(self.raw, clamp as c_int) }
    }

    /// Lower sorts earlier for one of the limited TCAS target slots.
    pub fn set_ai_priority(&self, priority: i32) {
        unsafe { xpmp2_shim_aircraft_set_ai_priority(self.raw, priority) }
    }

    /// Whether the CSL model is drawn in the 3D world — independent of
    /// TCAS/multiplayer visibility (see [`Self::set_visible`]/
    /// [`Self::is_visible`]).
    pub fn set_render(&self, render: bool) {
        unsafe { xpmp2_shim_aircraft_set_render(self.raw, render as c_int) }
    }

    pub fn is_rendered(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_is_rendered(self.raw) != 0 }
    }

    /// 1-based index into `sim/cockpit2/tcas/targets`, or `-1` if this plane
    /// isn't currently occupying a TCAS slot.
    pub fn tcas_target_idx(&self) -> i32 {
        unsafe { xpmp2_shim_aircraft_tcas_target_idx(self.raw) }
    }

    pub fn is_shown_as_tcas_target(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_is_shown_as_tcas_target(self.raw) != 0 }
    }

    pub fn is_shown_as_ai(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_is_shown_as_ai(self.raw) != 0 }
    }

    /// Whether this plane is to be drawn on TCAS (it will if the
    /// transponder isn't switched off).
    pub fn show_as_ai_plane(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_show_as_ai_plane(self.raw) != 0 }
    }

    /// Distance to camera in meters.
    pub fn camera_dist(&self) -> f32 {
        unsafe { xpmp2_shim_aircraft_camera_dist(self.raw) }
    }

    /// Bearing from camera in degrees.
    pub fn camera_bearing(&self) -> f32 {
        unsafe { xpmp2_shim_aircraft_camera_bearing(self.raw) }
    }

    /// Rough ground speed estimate in knots, based on [`Self::set_velocity`].
    pub fn ground_speed_kn(&self) -> f32 {
        unsafe { xpmp2_shim_aircraft_ground_speed_kn(self.raw) }
    }

    /// Is this plane "related" to `icao_type` (named in the same line in
    /// `related.txt`)? E.g. `is_related_to("GLID")` asks if this is a
    /// glider.
    pub fn is_related_to(&self, icao_type: &str) -> bool {
        let icao_type = CString::new(icao_type).unwrap_or_default();
        unsafe { xpmp2_shim_aircraft_is_related_to(self.raw, icao_type.as_ptr()) != 0 }
    }

    pub fn is_ground_vehicle(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_is_ground_vehicle(self.raw) != 0 }
    }

    pub fn is_glider(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_is_glider(self.raw) != 0 }
    }

    /// Re-runs model matching with new type/airline/livery criteria; returns
    /// the match quality (the lower the better).
    pub fn change_model(&self, icao_type: &str, icao_airline: &str, livery: &str) -> i32 {
        let icao_type = CString::new(icao_type).unwrap_or_default();
        let icao_airline = CString::new(icao_airline).unwrap_or_default();
        let livery = CString::new(livery).unwrap_or_default();
        unsafe {
            xpmp2_shim_aircraft_change_model(
                self.raw,
                icao_type.as_ptr(),
                icao_airline.as_ptr(),
                livery.as_ptr(),
            )
        }
    }

    /// Re-runs model matching with this plane's existing criteria, e.g.
    /// after more CSL models have been loaded. Returns the match quality
    /// (the lower the better).
    pub fn rematch_model(&self) -> i32 {
        unsafe { xpmp2_shim_aircraft_rematch_model(self.raw) }
    }

    pub fn match_quality(&self) -> i32 {
        unsafe { xpmp2_shim_aircraft_match_quality(self.raw) }
    }

    /// The name of the CSL model currently in use.
    pub fn model_name(&self) -> String {
        unsafe { read_shim_string(self.raw, xpmp2_shim_aircraft_get_model_name) }
    }

    /// The first non-empty string out of: flight number, registration,
    /// departure/arrival airports.
    pub fn flight_id(&self) -> String {
        unsafe { read_shim_string(self.raw, xpmp2_shim_aircraft_get_flight_id) }
    }

    /// Sets the informational texts forwarded via multiplayer shared
    /// dataRefs. Each field is truncated to `XPMPInfoTexts_t`'s
    /// corresponding fixed-size buffer if longer (tail number/flight
    /// number/airport codes: 9 bytes; ICAO type/ICAO airline: 3-4 bytes;
    /// manufacturer/model/airline name: 39 bytes — see `XPMPMultiplayer.h`).
    #[allow(clippy::too_many_arguments)]
    pub fn set_info_texts(
        &self,
        tail_num: &str,
        icao_ac_type: &str,
        manufacturer: &str,
        model: &str,
        icao_airline: &str,
        airline: &str,
        flight_num: &str,
        apt_from: &str,
        apt_to: &str,
    ) {
        let tail_num = CString::new(tail_num).unwrap_or_default();
        let icao_ac_type = CString::new(icao_ac_type).unwrap_or_default();
        let manufacturer = CString::new(manufacturer).unwrap_or_default();
        let model = CString::new(model).unwrap_or_default();
        let icao_airline = CString::new(icao_airline).unwrap_or_default();
        let airline = CString::new(airline).unwrap_or_default();
        let flight_num = CString::new(flight_num).unwrap_or_default();
        let apt_from = CString::new(apt_from).unwrap_or_default();
        let apt_to = CString::new(apt_to).unwrap_or_default();
        unsafe {
            xpmp2_shim_aircraft_set_info_texts(
                self.raw,
                tail_num.as_ptr(),
                icao_ac_type.as_ptr(),
                manufacturer.as_ptr(),
                model.as_ptr(),
                icao_airline.as_ptr(),
                airline.as_ptr(),
                flight_num.as_ptr(),
                apt_from.as_ptr(),
                apt_to.as_ptr(),
            )
        }
    }

    /// Wake turbulence support data (wingspan/wing area/mass) — leave a
    /// value unset (`NAN`) to let XPMP2 fall back to its Doc8643-derived
    /// default for the aircraft's wake turbulence category.
    pub fn set_wing_span(&self, meters: f32) {
        unsafe { xpmp2_shim_aircraft_set_wing_span(self.raw, meters) }
    }

    pub fn set_wing_area(&self, square_meters: f32) {
        unsafe { xpmp2_shim_aircraft_set_wing_area(self.raw, square_meters) }
    }

    pub fn set_mass(&self, kg: f32) {
        unsafe { xpmp2_shim_aircraft_set_mass(self.raw, kg) }
    }

    /// Fills in default wake turbulence data based on the Doc8643 wake
    /// turbulence category. `overwrite_all` false only fills still-unset
    /// (`NAN`) fields.
    pub fn wake_apply_defaults(&self, overwrite_all: bool) {
        unsafe { xpmp2_shim_aircraft_wake_apply_defaults(self.raw, overwrite_all as c_int) }
    }

    /// Requests contrails. `dist_m`/`life_time_s` of `0` mean "keep the
    /// current value" per XPMP2's own contract; `num` of `0` removes them.
    pub fn contrail_request(&self, num: u32, dist_m: u32, life_time_s: u32) {
        unsafe { xpmp2_shim_aircraft_contrail_request(self.raw, num, dist_m, life_time_s) }
    }

    pub fn contrail_remove(&self) {
        unsafe { xpmp2_shim_aircraft_contrail_remove(self.raw) }
    }

    /// Applies XPMP2's standard per-type contrail heuristic (contrails for
    /// jets only). Returns the number of contrails created.
    pub fn contrail_trigger(&self) -> u32 {
        unsafe { xpmp2_shim_aircraft_contrail_trigger(self.raw) }
    }

    pub fn set_sound_min_dist(&self, meters: i32) {
        unsafe { xpmp2_shim_aircraft_set_sound_min_dist(self.raw, meters) }
    }

    pub fn set_sound_muted(&self, mute: bool) {
        unsafe { xpmp2_shim_aircraft_set_sound_muted(self.raw, mute as c_int) }
    }

    pub fn is_sound_muted(&self) -> bool {
        unsafe { xpmp2_shim_aircraft_is_sound_muted(self.raw) != 0 }
    }
}

/// Shared by every shim function shaped like `strlcpy` (write into a caller
/// buffer, return the untruncated length): tries a stack buffer first, and
/// only allocates if the string is longer.
unsafe fn read_shim_string(
    raw: *mut XPMP2ShimAircraft,
    f: unsafe extern "C" fn(*const XPMP2ShimAircraft, *mut std::os::raw::c_char, usize) -> usize,
) -> String {
    let mut buf = [0u8; 128];
    let len = f(raw, buf.as_mut_ptr() as *mut _, buf.len());
    if len < buf.len() {
        return String::from_utf8_lossy(&buf[..len]).into_owned();
    }
    let mut buf = vec![0u8; len + 1];
    let len2 = f(raw, buf.as_mut_ptr() as *mut _, buf.len());
    String::from_utf8_lossy(&buf[..len2]).into_owned()
}

/// Per-plane state handed to the shim's `update_position` callback via a
/// boxed refcon: the caller's [`Aircraft`] plus a raw pointer back to the
/// owning `XPMP2ShimAircraft`, so [`Aircraft::update_position`] can be
/// handed a [`PlaneHandle`] without the callback needing its own copy of
/// the pointer threaded through separately.
struct Refcon {
    aircraft: Box<dyn Aircraft>,
    raw: *mut XPMP2ShimAircraft,
}

unsafe extern "C" fn update_position_trampoline(
    refcon: *mut c_void,
    elapsed_since_last_call: f32,
    fl_counter: c_int,
) {
    xplm::guard(|| {
        let refcon = &mut *(refcon as *mut Refcon);
        let plane = PlaneHandle {
            raw: refcon.raw,
            _marker: PhantomData,
        };
        refcon
            .aircraft
            .update_position(&plane, elapsed_since_last_call, fl_counter);
    });
}

/// RAII handle over a plane created through the C++ shim
/// (`xpmp2_shim_aircraft_create`/`_destroy`). [`Plane::new`] takes `&
/// Multiplayer` only as proof one is live *at construction time* — it isn't
/// stored (a plugin's top-level state struct commonly holds both a
/// `Multiplayer` and its `Plane`s together, which a borrowed lifetime here
/// would make self-referential and impossible to express safely). Drop
/// every `Plane` before dropping `Multiplayer` if you can (simplest way:
/// declare `Plane`/`Vec<Plane>` fields *before* `Multiplayer` in your struct
/// — Rust drops struct fields in declaration order, top to bottom);
/// `XPMPMultiplayerCleanup` itself documents that it's meant to be the
/// library's last call regardless, so this isn't a hard safety requirement
/// the way a dangling raw pointer would be.
pub struct Plane {
    raw: *mut XPMP2ShimAircraft,
    // Owns the boxed Refcon for this plane's whole lifetime; the shim only
    // stores the pointer, so this Box must outlive every future callback.
    _refcon: Box<Refcon>,
}

// Same rationale as `Multiplayer`'s `unsafe impl Send` above: X-Plane only
// ever calls in from its single main thread, but plugin state holding a
// `Plane` still needs to live in a `static Mutex<Option<_>>`, which
// requires `Send`. Sync is not implemented (nor derivable, since `raw` is a
// raw pointer) — nothing here supports concurrent access.
unsafe impl Send for Plane {}

impl Plane {
    /// `mode_s_id` of `0` lets XPMP2 assign one. Returns `None` if XPMP2
    /// rejected the plane (invalid/duplicate `mode_s_id`, or no CSL model
    /// matched `icao_type`/`icao_airline`/`livery`).
    ///
    /// # Panics
    /// Built with the `csl-on-demand` feature, panics if `csl_id` is empty.
    /// On-demand mode already resolved the model server-side (see
    /// `csl_on_demand::FetchedPackage::csl_id`) — an empty `csl_id` here
    /// would silently fall back to XPMP2's own local Doc8643/ICAO-based
    /// matching, which needs `Doc8643.txt`/`related.txt` files this mode
    /// has no reason to bundle, and which outright fails (returning `None`
    /// from this function, not a panic) the first time ever, before any
    /// package has been loaded. Passing `""` here under `csl-on-demand` is
    /// always a bug, not a valid "let XPMP2 match it" call, so it's caught
    /// immediately instead of failing confusingly later.
    pub fn new(
        _multiplayer: &Multiplayer,
        icao_type: &str,
        icao_airline: &str,
        livery: &str,
        mode_s_id: u32,
        csl_id: &str,
        aircraft: impl Aircraft + 'static,
    ) -> Option<Self> {
        #[cfg(feature = "csl-on-demand")]
        assert!(
            !csl_id.is_empty(),
            "xpmp2::Plane::new: csl_id must not be empty under the csl-on-demand feature \
             — pass csl_on_demand::FetchedPackage::csl_id, not \"\""
        );

        let icao_type = CString::new(icao_type).unwrap_or_default();
        let icao_airline = CString::new(icao_airline).unwrap_or_default();
        let livery = CString::new(livery).unwrap_or_default();
        let csl_id = CString::new(csl_id).unwrap_or_default();

        // Boxed twice over: the outer Box<Refcon> is what the shim's refcon
        // pointer actually points at (stable address, moved once here and
        // never again); `raw` inside it is filled in right after creation
        // since the shim only returns the aircraft pointer after the call.
        let mut refcon = Box::new(Refcon {
            aircraft: Box::new(aircraft),
            raw: std::ptr::null_mut(),
        });
        let refcon_ptr: *mut Refcon = &mut *refcon;

        let raw = unsafe {
            xpmp2_shim_aircraft_create(
                icao_type.as_ptr(),
                icao_airline.as_ptr(),
                livery.as_ptr(),
                mode_s_id,
                csl_id.as_ptr(),
                Some(update_position_trampoline),
                refcon_ptr as *mut c_void,
            )
        };
        if raw.is_null() {
            return None;
        }
        refcon.raw = raw;

        Some(Self {
            raw,
            _refcon: refcon,
        })
    }

    pub fn handle(&self) -> PlaneHandle<'_> {
        PlaneHandle {
            raw: self.raw,
            _marker: PhantomData,
        }
    }

    /// Reaches back into the [`Aircraft`] impl this `Plane` was constructed
    /// with, e.g. to push a newly-arrived position/config update into a
    /// plane that already has a model loaded, from outside
    /// [`Aircraft::update_position`]. Returns `&dyn Aircraft`/`&mut dyn
    /// Aircraft` rather than the concrete type — recover it via
    /// `(aircraft as &mut dyn std::any::Any).downcast_mut::<T>()` (see
    /// [`Aircraft`]'s `Any` supertrait).
    pub fn aircraft(&self) -> &dyn Aircraft {
        &*self._refcon.aircraft
    }

    /// See [`Self::aircraft`].
    pub fn aircraft_mut(&mut self) -> &mut dyn Aircraft {
        &mut *self._refcon.aircraft
    }
}

impl Drop for Plane {
    fn drop(&mut self) {
        unsafe {
            xpmp2_shim_aircraft_destroy(self.raw);
        }
    }
}
