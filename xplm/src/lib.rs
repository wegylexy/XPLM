//! Safe, idiomatic wrappers over `xplm-sys`: dataref access, plugin
//! lifecycle, menus, windows/widgets, and the rest of the XPLM SDK surface.

/// Attribute macro for a plugin's top-level state struct — see
/// [`xplm_macros::plugin`] for the full example. Sugar over
/// [`register_plugin!`]'s metadata-argument form.
pub use xplm_macros::plugin;
/// Derives `find() -> Option<Self>` for a `#[dataref = "..."]`-tagged
/// struct — see [`xplm_macros::DataRefContainer`] for the full example.
pub use xplm_macros::DataRefContainer;
/// Derives `publish(shared) -> Option<FooHandles>` for a struct of plain
/// values backing published datarefs — see
/// [`xplm_macros::PublishedDataRefContainer`] for the full example.
pub use xplm_macros::PublishedDataRefContainer;

pub mod aircraft;
pub mod camera;
pub mod command;
pub mod dataref;
pub mod graphics;
pub mod instance;
pub mod menu;
pub mod plugin;
pub mod scenery;
pub mod utilities;
#[cfg(feature = "widgets")]
pub mod widget;
// FlightLoop's underlying XPLMCreateFlightLoop/XPLMDestroyFlightLoop/
// XPLMScheduleFlightLoop are all `#if defined(XPLM210)` in XPLMProcessing.h
// (only the legacy XPLMRegisterFlightLoopCallback predates that) — default
// features always include XPLM210, so this gap only surfaces building a
// lower feature set explicitly (see CLAUDE.md's "test minimal features").
#[cfg(feature = "XPLM210")]
pub mod processing;
pub mod window;

#[cfg(not(test))]
use std::ffi::CString;

/// Runs `f`, catching any panic at the FFI boundary so it can never unwind
/// into the X-Plane host process (which would be undefined behavior).
///
/// On panic, logs the payload via `XPLMDebugString` (best-effort; a second
/// panic while formatting the message is swallowed) and returns `None`.
/// Every `extern "C"` trampoline that X-Plane calls back into MUST go
/// through this.
pub fn guard<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) -> Option<R> {
    match std::panic::catch_unwind(f) {
        Ok(r) => Some(r),
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic across xplm FFI boundary".to_string());
            log(&format!("xplm: caught panic at FFI boundary: {msg}\n"));
            None
        }
    }
}

/// Writes a line to X-Plane's `Log.txt` via `XPLMDebugString`. Never panics;
/// a message containing an interior NUL is truncated at the NUL instead.
///
/// Under `cfg(test)` this writes to stderr instead: `XPLMDebugString` (like
/// every other XPLM function) assumes it's being called from inside a
/// plugin hosted by a running X-Plane process, and segfaults when called
/// from a bare `cargo test` binary — the DLL exists and loads (delay-load
/// resolves it against a local X-Plane install), it just isn't operating in
/// the environment it expects.
pub fn log(msg: &str) {
    #[cfg(test)]
    {
        eprint!("{msg}");
    }
    #[cfg(not(test))]
    {
        let c_msg = CString::new(msg).unwrap_or_else(|e| {
            let nul_position = e.nul_position();
            CString::new(&e.into_vec()[..nul_position]).unwrap_or_default()
        });
        unsafe {
            xplm_sys::XPLMDebugString(c_msg.as_ptr());
        }
    }
}
