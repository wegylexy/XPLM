//! Camera control (`XPLMCamera.h`). Same boxed-closure-behind-a-refcon
//! trampoline shape as `FlightLoop`/`Menu`, but the SDK exposes only one
//! global camera-control slot (no handle is returned by `XPLMControlCamera`)
//! — see the caveat on `CameraControl::take`'s `Drop` impl.

use std::ffi::c_void;
use std::os::raw::c_int;

use xplm_sys::{
    xplm_ControlCameraForever, xplm_ControlCameraUntilViewChanges, XPLMCameraPosition_t,
    XPLMControlCamera, XPLMDontControlCamera,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub pitch: f32,
    pub heading: f32,
    pub roll: f32,
    pub zoom: f32,
}

impl From<XPLMCameraPosition_t> for CameraPosition {
    fn from(raw: XPLMCameraPosition_t) -> Self {
        Self {
            x: raw.x,
            y: raw.y,
            z: raw.z,
            pitch: raw.pitch,
            heading: raw.heading,
            roll: raw.roll,
            zoom: raw.zoom,
        }
    }
}

impl From<CameraPosition> for XPLMCameraPosition_t {
    fn from(pos: CameraPosition) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            pitch: pos.pitch,
            heading: pos.heading,
            roll: pos.roll,
            zoom: pos.zoom,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraControlDuration {
    UntilViewChanges,
    Forever,
}

impl From<CameraControlDuration> for xplm_sys::XPLMCameraControlDuration {
    fn from(d: CameraControlDuration) -> Self {
        match d {
            CameraControlDuration::UntilViewChanges => xplm_ControlCameraUntilViewChanges,
            CameraControlDuration::Forever => xplm_ControlCameraForever,
        }
    }
}

/// The callback signature: called each drawing cycle while you hold camera
/// control. `is_losing_control` is `true` when X-Plane is about to take the
/// camera back (in which case return `None` regardless). Return `Some(pos)`
/// to reposition the camera, or `None` to surrender control voluntarily.
type Callback = dyn FnMut(bool) -> Option<CameraPosition> + 'static;

/// A held camera-control registration.
///
/// **Caveat**: the SDK has exactly one global camera-control slot with no
/// handle — `XPLMControlCamera` simply replaces whatever was registered
/// before. If you create a second `CameraControl` while a first is still
/// alive, the first's callback is silently orphaned (X-Plane already
/// dropped it), and when the first is later `Drop`ped it will call
/// `XPLMDontControlCamera` and release control the *second* instance
/// thought it still held. Keep at most one `CameraControl` alive at a time.
pub struct CameraControl {
    refcon: *mut Box<Callback>,
}

unsafe impl Send for CameraControl {} // see FlightLoop's identical rationale.

impl CameraControl {
    pub fn take(
        duration: CameraControlDuration,
        callback: impl FnMut(bool) -> Option<CameraPosition> + 'static,
    ) -> Self {
        let boxed: Box<Callback> = Box::new(callback);
        let refcon = Box::into_raw(Box::new(boxed));
        unsafe {
            XPLMControlCamera(duration.into(), Some(trampoline), refcon as *mut c_void);
        }
        Self { refcon }
    }
}

impl Drop for CameraControl {
    fn drop(&mut self) {
        unsafe {
            XPLMDontControlCamera();
            drop(Box::from_raw(self.refcon));
        }
    }
}

unsafe extern "C" fn trampoline(
    out_camera_position: *mut XPLMCameraPosition_t,
    in_is_losing_control: c_int,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let callback: &mut Callback = unsafe { &mut *(*(refcon as *mut Box<Callback>)) };
        match callback(in_is_losing_control != 0) {
            Some(pos) if !out_camera_position.is_null() => {
                unsafe { *out_camera_position = pos.into() };
                1
            }
            _ => 0,
        }
    })
    .unwrap_or(0)
}
