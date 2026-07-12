//! Safe wrapper over `xpmp2-sys`: RAII lifecycle ([`Multiplayer`]) and an
//! [`Aircraft`] trait + RAII [`Plane`] handle over the C++ shim's
//! `XPMP2ShimAircraft`, mirroring the shape `xplm`'s own wrappers use
//! (RAII owns cleanup, no leaky panics across the FFI boundary).

use std::ffi::{c_void, CStr, CString};
use std::marker::PhantomData;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, Ordering};

use xpmp2_sys::{
    xpmp2_shim_aircraft_create, xpmp2_shim_aircraft_dataref_count, xpmp2_shim_aircraft_destroy,
    xpmp2_shim_aircraft_get_dataref, xpmp2_shim_aircraft_get_location, xpmp2_shim_aircraft_is_valid,
    xpmp2_shim_aircraft_is_visible, xpmp2_shim_aircraft_mode_s_id, xpmp2_shim_aircraft_set_dataref,
    xpmp2_shim_aircraft_set_heading, xpmp2_shim_aircraft_set_label,
    xpmp2_shim_aircraft_set_label_drawn, xpmp2_shim_aircraft_set_local_loc,
    xpmp2_shim_aircraft_set_location, xpmp2_shim_aircraft_set_on_ground,
    xpmp2_shim_aircraft_set_pitch, xpmp2_shim_aircraft_set_roll, xpmp2_shim_aircraft_set_velocity,
    xpmp2_shim_aircraft_set_visible, XPMP2ShimAircraft, XPMPMultiplayerCleanup, XPMPMultiplayerInit,
};

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
    // Neither Send nor Sync: XPMP2, like the rest of the XPLM SDK, is only
    // ever safe to call from X-Plane's main thread.
    _not_send_sync: PhantomData<*const ()>,
}

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
        let default_icao = default_icao.map(|s| CString::new(s).expect("default_icao contains a NUL byte"));
        let log_acronym = log_acronym.map(|s| CString::new(s).expect("log_acronym contains a NUL byte"));
        let result = unsafe {
            XPMPMultiplayerInit(
                plugin_name.as_ptr(),
                resource_dir.as_ptr(),
                None,
                default_icao.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                log_acronym.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            )
        };
        let message = unsafe { CStr::from_ptr(result) }.to_string_lossy().into_owned();
        if message.is_empty() {
            Ok(Self {
                _not_send_sync: PhantomData,
            })
        } else {
            INITIALIZED.store(false, Ordering::SeqCst);
            Err(message)
        }
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
pub trait Aircraft {
    /// Called once per drawing cycle (`XPMP2::Aircraft::UpdatePosition`).
    /// Update location/attitude/velocity/labels/dataRefs on the `plane`
    /// handle passed in — there is no other way to reach the underlying
    /// `XPMP2ShimAircraft` from here.
    fn update_position(&mut self, plane: &PlaneHandle, elapsed_since_last_call: f32, fl_counter: i32);
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
        refcon.aircraft.update_position(&plane, elapsed_since_last_call, fl_counter);
    });
}

/// RAII handle over a plane created through the C++ shim
/// (`xpmp2_shim_aircraft_create`/`_destroy`). Requires a live [`Multiplayer`]
/// (enforced by borrowing one) for as long as the plane exists.
pub struct Plane<'m> {
    raw: *mut XPMP2ShimAircraft,
    // Owns the boxed Refcon for this plane's whole lifetime; the shim only
    // stores the pointer, so this Box must outlive every future callback.
    _refcon: Box<Refcon>,
    _multiplayer: PhantomData<&'m Multiplayer>,
}

impl<'m> Plane<'m> {
    /// `mode_s_id` of `0` lets XPMP2 assign one. Returns `None` if XPMP2
    /// rejected the plane (invalid/duplicate `mode_s_id`, or no CSL model
    /// matched `icao_type`/`icao_airline`/`livery`).
    pub fn new(
        _multiplayer: &'m Multiplayer,
        icao_type: &str,
        icao_airline: &str,
        livery: &str,
        mode_s_id: u32,
        csl_id: &str,
        aircraft: impl Aircraft + 'static,
    ) -> Option<Self> {
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
            _multiplayer: PhantomData,
        })
    }

    pub fn handle(&self) -> PlaneHandle<'_> {
        PlaneHandle {
            raw: self.raw,
            _marker: PhantomData,
        }
    }
}

impl Drop for Plane<'_> {
    fn drop(&mut self) {
        unsafe {
            xpmp2_shim_aircraft_destroy(self.raw);
        }
    }
}
