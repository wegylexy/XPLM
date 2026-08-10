//! Scenery (`XPLMScenery.h`): terrain probing, magnetic variation, and OBJ
//! loading. `Instance` (`xplm::instance`) builds on [`Object`]/[`DrawInfo`]
//! from here.

use std::ffi::c_void;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

use xplm_sys::{
    xplm_ProbeHitTerrain, xplm_ProbeMissed, xplm_ProbeY, XPLMCreateProbe, XPLMDestroyProbe,
    XPLMLoadObject, XPLMObjectRef, XPLMProbeInfo_t, XPLMProbeRef, XPLMProbeTerrainXYZ,
    XPLMUnloadObject,
};

#[cfg(feature = "XPLM210")]
use xplm_sys::XPLMLoadObjectAsync;

#[cfg(feature = "XPLM200")]
use xplm_sys::XPLMLookupObjects;

#[cfg(feature = "XPLM300")]
use xplm_sys::{XPLMDegMagneticToDegTrue, XPLMDegTrueToDegMagnetic, XPLMGetMagneticVariation};

/// `(x, y, z)` in local OpenGL meters.
pub type Vec3 = (f32, f32, f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeHit {
    pub location: Vec3,
    pub normal: Vec3,
    pub velocity: Vec3,
    pub is_wet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProbeOutcome {
    Hit(ProbeHit),
    /// The point is outside the loaded scenery area, or otherwise has no
    /// terrain under it.
    Missed,
    /// Bad probe/struct size, or a type mismatch for this query — an API
    /// misuse, not a "no terrain here" result.
    Error,
}

/// A terrain probe (`XPLMProbeRef`). The SDK recommends reusing the same
/// probe for nearby points rather than creating one per query — probing is
/// relatively expensive, and probes cache query information internally.
pub struct TerrainProbe {
    raw: XPLMProbeRef,
}

unsafe impl Send for TerrainProbe {} // see FlightLoop's identical rationale.

impl TerrainProbe {
    pub fn new() -> Self {
        let raw = unsafe { XPLMCreateProbe(xplm_ProbeY) };
        Self { raw }
    }

    /// Finds the highest physical scenery along the Y axis through
    /// `(x, y, z)` (local OpenGL coordinates).
    pub fn probe_terrain(&self, x: f32, y: f32, z: f32) -> ProbeOutcome {
        let mut info = XPLMProbeInfo_t {
            structSize: std::mem::size_of::<XPLMProbeInfo_t>() as c_int,
            locationX: 0.0,
            locationY: 0.0,
            locationZ: 0.0,
            normalX: 0.0,
            normalY: 0.0,
            normalZ: 0.0,
            velocityX: 0.0,
            velocityY: 0.0,
            velocityZ: 0.0,
            is_wet: 0,
        };
        let result = unsafe { XPLMProbeTerrainXYZ(self.raw, x, y, z, &mut info) };
        if result == xplm_ProbeHitTerrain {
            ProbeOutcome::Hit(ProbeHit {
                location: (info.locationX, info.locationY, info.locationZ),
                normal: (info.normalX, info.normalY, info.normalZ),
                velocity: (info.velocityX, info.velocityY, info.velocityZ),
                is_wet: info.is_wet != 0,
            })
        } else if result == xplm_ProbeMissed {
            ProbeOutcome::Missed
        } else {
            ProbeOutcome::Error
        }
    }
}

impl Default for TerrainProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TerrainProbe {
    fn drop(&mut self) {
        unsafe { XPLMDestroyProbe(self.raw) }
    }
}

/// The magnetic variation (declination) at `(latitude, longitude)`, in
/// degrees.
#[cfg(feature = "XPLM300")]
pub fn magnetic_variation(latitude: f64, longitude: f64) -> f32 {
    unsafe { XPLMGetMagneticVariation(latitude, longitude) }
}

/// Converts a true-north heading to magnetic-north, at the user's current
/// location.
#[cfg(feature = "XPLM300")]
pub fn true_to_magnetic(heading_degrees_true: f32) -> f32 {
    unsafe { XPLMDegTrueToDegMagnetic(heading_degrees_true) }
}

/// The inverse of [`true_to_magnetic`].
#[cfg(feature = "XPLM300")]
pub fn magnetic_to_true(heading_degrees_magnetic: f32) -> f32 {
    unsafe { XPLMDegMagneticToDegTrue(heading_degrees_magnetic) }
}

/// Positioning info for drawing an object — used by
/// `xplm::instance::Instance::set_position`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawInfo {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Degrees, positive is nose up.
    pub pitch: f32,
    /// Degrees, clockwise.
    pub heading: f32,
    pub roll: f32,
}

impl From<DrawInfo> for xplm_sys::XPLMDrawInfo_t {
    fn from(d: DrawInfo) -> Self {
        Self {
            structSize: std::mem::size_of::<xplm_sys::XPLMDrawInfo_t>() as c_int,
            x: d.x,
            y: d.y,
            z: d.z,
            pitch: d.pitch,
            heading: d.heading,
            roll: d.roll,
        }
    }
}

/// A loaded `.obj` file (`XPLMObjectRef`), reference-counted by X-Plane
/// itself. Dropping this calls `XPLMUnloadObject`, balancing the load.
pub struct Object {
    pub(crate) raw: XPLMObjectRef,
}

unsafe impl Send for Object {} // see FlightLoop's identical rationale.

impl Object {
    /// Loads synchronously; `path` is relative to the X-System folder.
    /// Returns `None` if the object can't be found or is malformed.
    ///
    /// Any custom datarefs the object animates with must already be
    /// registered before it's loaded — you may need to defer loading until
    /// the sim has fully started.
    pub fn load(path: &str) -> Option<Self> {
        let c_path = CString::new(path).ok()?;
        let raw = unsafe { XPLMLoadObject(c_path.as_ptr()) };
        (!raw.is_null()).then_some(Self { raw })
    }

    /// Loads asynchronously; `callback` runs once loading finishes (with
    /// `None` if it failed), without blocking the sim. There is no way to
    /// cancel an in-flight load. Returns `false` (without starting a load)
    /// if `path` contains an interior NUL.
    #[cfg(feature = "XPLM210")]
    pub fn load_async(path: &str, callback: impl FnOnce(Option<Self>) + 'static) -> bool {
        let Ok(c_path) = CString::new(path) else {
            return false;
        };
        let boxed: Box<AsyncCallback> = Box::new(callback);
        let refcon = Box::into_raw(Box::new(boxed));
        unsafe {
            XPLMLoadObjectAsync(
                c_path.as_ptr(),
                Some(load_object_trampoline),
                refcon as *mut c_void,
            );
        }
        true
    }

    /// Convenience shorthand for
    /// `Instance::new(&self, datarefs)` — see [`crate::instance::Instance::new`]
    /// for the full contract.
    pub fn new_instance(&self, datarefs: &[&str]) -> Option<crate::instance::Instance> {
        crate::instance::Instance::new(self, datarefs)
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        unsafe { XPLMUnloadObject(self.raw) }
    }
}

#[cfg(feature = "XPLM210")]
type AsyncCallback = dyn FnOnce(Option<Object>) + 'static;

/// Looks up `virtual_path` in X-Plane's library system (relative to the
/// X-System folder) and calls `callback` once per matching object's file
/// path — one virtual path may resolve to several concrete objects,
/// selected/weighted by `(latitude, longitude)`. Returns the number of
/// matches found, or `0` if `virtual_path` contains an interior NUL.
#[cfg(feature = "XPLM200")]
pub fn lookup_objects(
    virtual_path: &str,
    latitude: f32,
    longitude: f32,
    mut callback: impl FnMut(&str),
) -> i32 {
    unsafe extern "C" fn trampoline(file_path: *const c_char, refcon: *mut c_void) {
        crate::guard(|| {
            let callback: &mut &mut dyn FnMut(&str) = unsafe { &mut *(refcon as *mut _) };
            let path = unsafe { CStr::from_ptr(file_path) }.to_string_lossy();
            callback(&path);
        });
    }

    let Ok(c_path) = CString::new(virtual_path) else {
        return 0;
    };
    let mut trait_obj: &mut dyn FnMut(&str) = &mut callback;
    let refcon = &mut trait_obj as *mut &mut dyn FnMut(&str) as *mut c_void;
    unsafe {
        XPLMLookupObjects(
            c_path.as_ptr(),
            latitude,
            longitude,
            Some(trampoline),
            refcon,
        )
    }
}

#[cfg(feature = "XPLM210")]
unsafe extern "C" fn load_object_trampoline(object: XPLMObjectRef, refcon: *mut c_void) {
    crate::guard(|| {
        let callback: Box<AsyncCallback> =
            *unsafe { Box::from_raw(refcon as *mut Box<AsyncCallback>) };
        let loaded = (!object.is_null()).then_some(Object { raw: object });
        callback(loaded);
    });
}
