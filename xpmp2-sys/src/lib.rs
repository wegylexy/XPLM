//! Raw `bindgen` FFI over XPMP2's flat-C surface
//! (`inc/XPMPMultiplayer.h`) plus `shim/shim.h`, a hand-written C++ shim
//! exposing `XPMP2::Aircraft` (the C++ class in `inc/XPMPAircraft.h`) as
//! flat `extern "C"` functions operating on an opaque `XPMP2ShimAircraft*` —
//! see `shim/shim.h`'s doc comment for why a shim, not a direct `bindgen`
//! binding, is what's here. No safety/ergonomics of its own; see `xpmp2`
//! for that.

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

    /// `shim.cpp` is a separate translation unit within the same static
    /// archive as `XPMPMultiplayer.cpp` — resolving a symbol from one
    /// doesn't prove the linker pulled in the other, since most linkers
    /// pull individual object-file members of a `.lib`/`.a` on demand, not
    /// the whole archive at once. Needs its own, separate check.
    #[test]
    fn shim_symbols_resolve_at_link_time() {
        let _: unsafe extern "C" fn(
            *const std::os::raw::c_char,
            *const std::os::raw::c_char,
            *const std::os::raw::c_char,
            std::os::raw::c_uint,
            *const std::os::raw::c_char,
            XPMP2ShimUpdatePositionFn,
            *mut std::os::raw::c_void,
        ) -> *mut XPMP2ShimAircraft = xpmp2_shim_aircraft_create;
        let _: unsafe extern "C" fn(*mut XPMP2ShimAircraft) = xpmp2_shim_aircraft_destroy;
        let _: unsafe extern "C" fn(*mut XPMP2ShimAircraft, f32, f32, f32) =
            xpmp2_shim_aircraft_set_velocity;
        let _: unsafe extern "C" fn(*const XPMP2ShimAircraft) -> usize =
            xpmp2_shim_aircraft_dataref_count;
    }
}
