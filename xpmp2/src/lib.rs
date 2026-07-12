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
    xpmp2_shim_aircraft_create, xpmp2_shim_aircraft_dataref_count, xpmp2_shim_aircraft_destroy,
    xpmp2_shim_aircraft_get_dataref, xpmp2_shim_aircraft_get_location,
    xpmp2_shim_aircraft_is_valid, xpmp2_shim_aircraft_is_visible, xpmp2_shim_aircraft_mode_s_id,
    xpmp2_shim_aircraft_set_dataref, xpmp2_shim_aircraft_set_heading,
    xpmp2_shim_aircraft_set_label, xpmp2_shim_aircraft_set_label_drawn,
    xpmp2_shim_aircraft_set_local_loc, xpmp2_shim_aircraft_set_location,
    xpmp2_shim_aircraft_set_on_ground, xpmp2_shim_aircraft_set_pitch, xpmp2_shim_aircraft_set_roll,
    xpmp2_shim_aircraft_set_velocity, xpmp2_shim_aircraft_set_visible, XPMP2ShimAircraft,
    XPMPMultiplayerCleanup, XPMPMultiplayerInit,
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
pub trait Aircraft {
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
}

impl Drop for Plane {
    fn drop(&mut self) {
        unsafe {
            xpmp2_shim_aircraft_destroy(self.raw);
        }
    }
}
