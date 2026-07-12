//! Aircraft control (`XPLMPlanes.h`). Free functions for the user's own
//! aircraft; [`AircraftAccess`] is the RAII wrapper for the "exclusive
//! access to AI/multiplayer aircraft" half, which only one plugin may hold
//! at a time — same single-global-slot shape as `xplm::camera::CameraControl`,
//! but this one at least tells you whether you got it.
//!
//! Per the header's own warning: don't call the flight-initialization
//! functions here from a drawing callback, a dataref get/set handler, or
//! the post-flightmodel processing callback — only from command/menu/UI
//! handlers or the pre-flightmodel callback.

use std::ffi::{c_void, CString};
use std::os::raw::c_char;

use xplm_sys::{
    XPLMAcquirePlanes, XPLMCountAircraft, XPLMDisableAIForPlane, XPLMGetNthAircraftModel,
    XPLMPlaceUserAtAirport, XPLMPluginID, XPLMReleasePlanes, XPLMSetActiveAircraftCount,
    XPLMSetAircraftModel, XPLMSetUsersAircraft,
};

#[cfg(feature = "XPLM300")]
use xplm_sys::XPLMPlaceUserAtLocation;

/// Index of the user's own aircraft, for use with [`nth_aircraft_model`]
/// (`XPLM_USER_AIRCRAFT`).
pub const USER_AIRCRAFT: i32 = 0;

// XPLMInitFlight/XPLMUpdateFlight (the JSON-based flight-init API) are
// `#if defined(XPLM430)` — this crate caps its version features at
// `XPLM420` (see CLAUDE.md/PHASES.md), so they're intentionally not wrapped
// here, not merely deferred.

/// Changes the user's aircraft to the `.acf` at `path` (a full, not
/// relative, path) and reinitializes at the nearest airport's first
/// runway. No equivalent full-control API is wrapped here (see XPLM430 note above).
pub fn set_users_aircraft(path: &str) {
    let Ok(c_path) = CString::new(path) else {
        return;
    };
    unsafe { XPLMSetUsersAircraft(c_path.as_ptr()) }
}

/// Places the user at the airport with the given X-Plane airport ID (e.g.
/// `"KBOS"`). No equivalent full-control API is wrapped here (see XPLM430 note above).
pub fn place_user_at_airport(airport_code: &str) {
    let Ok(c_code) = CString::new(airport_code) else {
        return;
    };
    unsafe { XPLMPlaceUserAtAirport(c_code.as_ptr()) }
}

/// Places the user at a specific location (loading scenery as needed).
/// Always starts with engines running, regardless of the user's startup
/// preference. No equivalent full-control API is wrapped here (see XPLM430 note above).
#[cfg(feature = "XPLM300")]
pub fn place_user_at_location(
    latitude_degrees: f64,
    longitude_degrees: f64,
    elevation_meters_msl: f32,
    heading_degrees_true: f32,
    speed_meters_per_second: f32,
) {
    unsafe {
        XPLMPlaceUserAtLocation(
            latitude_degrees,
            longitude_degrees,
            elevation_meters_msl,
            heading_degrees_true,
            speed_meters_per_second,
        )
    }
}

/// `(total_aircraft, active_aircraft, controlling_plugin)` — how many
/// aircraft slots X-Plane has, how many are active, and which plugin (if
/// any) currently controls them via [`AircraftAccess`].
pub fn aircraft_count() -> (i32, i32, XPLMPluginID) {
    let (mut total, mut active, mut controller) = (0, 0, 0);
    unsafe { XPLMCountAircraft(&mut total, &mut active, &mut controller) };
    (total, active, controller)
}

/// `(file_name, path)` for the aircraft at `index` (0 is always the user's
/// own aircraft).
pub fn nth_aircraft_model(index: i32) -> (String, String) {
    let mut file_name = [0u8; 256];
    let mut path = [0u8; 512];
    unsafe {
        XPLMGetNthAircraftModel(
            index,
            file_name.as_mut_ptr() as *mut c_char,
            path.as_mut_ptr() as *mut c_char,
        )
    };
    (read_c_buf(&file_name), read_c_buf(&path))
}

fn read_c_buf(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

type AvailableCallback = dyn FnOnce() + 'static;

/// Exclusive access to the AI/multiplayer aircraft — only one plugin may
/// hold this at a time. Dropping it releases access (`XPLMReleasePlanes`).
pub struct AircraftAccess {
    _private: (),
}

unsafe impl Send for AircraftAccess {} // see FlightLoop's identical rationale.

impl AircraftAccess {
    /// Attempts to acquire exclusive access. `aircraft_paths`, if given, are
    /// full `.acf` paths to load into each slot (an empty string leaves a
    /// slot unloaded); `None` loads no planes.
    ///
    /// Returns `Some` if access was granted immediately. If not, and
    /// `on_available` was given, it runs later once access is available —
    /// but you must call `acquire` again *then* to actually take it; a
    /// notification isn't a grant, matching the SDK's own two-step shape
    /// (the same "single-shot callback, not a Future" reasoning as
    /// `Object::load_async`; see `PHASES.md`/README for why `xplm` doesn't
    /// wrap these in `async fn`).
    pub fn acquire(
        aircraft_paths: Option<&[&str]>,
        on_available: Option<impl FnOnce() + 'static>,
    ) -> Option<Self> {
        let c_strings: Option<Vec<CString>> = aircraft_paths
            .map(|paths| paths.iter().map(|p| CString::new(*p)).collect())
            .transpose()
            .ok()
            .flatten();
        let mut ptrs: Option<Vec<*mut c_char>> = c_strings.as_ref().map(|strings| {
            let mut ptrs: Vec<*mut c_char> =
                strings.iter().map(|s| s.as_ptr() as *mut c_char).collect();
            ptrs.push(std::ptr::null_mut());
            ptrs
        });
        let paths_ptr = ptrs
            .as_mut()
            .map_or(std::ptr::null_mut(), |p| p.as_mut_ptr());

        let refcon: *mut c_void = match on_available {
            Some(callback) => {
                let boxed: Box<AvailableCallback> = Box::new(callback);
                Box::into_raw(Box::new(boxed)) as *mut c_void
            }
            None => std::ptr::null_mut(),
        };
        let trampoline = if refcon.is_null() {
            None
        } else {
            Some(planes_available_trampoline as _)
        };

        let granted = unsafe { XPLMAcquirePlanes(paths_ptr, trampoline, refcon) };
        if granted != 0 {
            if !refcon.is_null() {
                // Granted immediately: the callback will never fire, so we
                // must reclaim it ourselves instead of leaking it.
                unsafe { drop(Box::from_raw(refcon as *mut Box<AvailableCallback>)) };
            }
            Some(Self { _private: () })
        } else {
            None
        }
    }

    pub fn set_active_aircraft_count(&self, count: i32) {
        unsafe { XPLMSetActiveAircraftCount(count) }
    }

    /// `index` must not be 0 (the user's aircraft) — use
    /// [`set_users_aircraft`] for that.
    pub fn set_aircraft_model(&self, index: i32, aircraft_path: &str) {
        let Ok(c_path) = CString::new(aircraft_path) else {
            return;
        };
        unsafe { XPLMSetAircraftModel(index, c_path.as_ptr()) }
    }

    pub fn disable_ai_for_plane(&self, plane_index: i32) {
        unsafe { XPLMDisableAIForPlane(plane_index) }
    }
}

impl Drop for AircraftAccess {
    fn drop(&mut self) {
        unsafe { XPLMReleasePlanes() }
    }
}

unsafe extern "C" fn planes_available_trampoline(refcon: *mut c_void) {
    crate::guard(|| {
        let callback: Box<AvailableCallback> =
            *unsafe { Box::from_raw(refcon as *mut Box<AvailableCallback>) };
        callback();
    });
}
