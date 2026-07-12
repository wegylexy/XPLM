//! Raw `bindgen` FFI over XPMP2's flat-C surface only
//! (`inc/XPMPMultiplayer.h`) — no safety, no ergonomics; see `xpmp2` for
//! that. `XPMP2::Aircraft` (the C++ class in `inc/XPMPAircraft.h`) isn't
//! bound here yet — see `PHASES.md` in this workspace for the C++ shim
//! that's still needed for it.

#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    /// Not called (would crash outside a hosted X-Plane process, same
    /// limitation as `xplm-sys`) — just takes each function's address, which
    /// is enough to force the linker to actually resolve every symbol
    /// against the compiled XPMP2 static lib (and, transitively, whatever
    /// XPLM/Winsock symbols XPMP2 itself calls), rather than silently
    /// dropping the whole archive because nothing referenced it.
    #[test]
    fn symbols_resolve_at_link_time() {
        let _: unsafe extern "C" fn() = XPMPMultiplayerCleanup;
        let _: unsafe extern "C" fn(
            *const std::os::raw::c_char,
            *const std::os::raw::c_char,
            XPMPIntPrefsFuncTy,
            *const std::os::raw::c_char,
            *const std::os::raw::c_char,
        ) -> *const std::os::raw::c_char = XPMPMultiplayerInit;
        let _: unsafe extern "C" fn(*const std::os::raw::c_char) -> *const std::os::raw::c_char =
            XPMPLoadCSLPackage;
        let _: unsafe extern "C" fn(bool) -> bool = XPMPSoundEnable;
    }
}
