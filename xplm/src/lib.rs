//! Safe, idiomatic wrappers over `xplm-sys`.
//!
//! Phase 2+ modules (dataref, plugin lifecycle, menus, processing, ...) land here.

/// Runs `f`, catching any panic at the FFI boundary so it can never unwind
/// into the X-Plane host process (which would be undefined behavior).
pub fn guard<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) -> Option<R> {
    std::panic::catch_unwind(f).ok()
}
