//! Safe, idiomatic wrappers over `xplm-sys`.
//!
//! Phase 2+ modules (dataref, plugin lifecycle, menus, ...) land here.

pub mod camera;
pub mod dataref;
pub mod menu;
pub mod plugin;
pub mod processing;

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
