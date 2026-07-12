//! Plugin lifecycle (`XPLMPlugin.h` + the five required exported callbacks
//! every `.xpl` must define). `register_plugin!` generates the
//! `extern "C"` trampolines; `#[xplm::plugin(...)]` is sugar over the same
//! mechanism.

use std::ffi::c_void;
use std::os::raw::{c_char, c_int};

pub use xplm_sys::XPLMPluginID;

/// Implement this for your plugin's top-level state struct, then call
/// `xplm::register_plugin!(YourType)` once at crate root to wire up the
/// five `extern "C"` exports X-Plane requires (`XPluginStart/Stop/Enable/
/// Disable/ReceiveMessage`).
pub trait XPlanePlugin: Sized + 'static {
    /// Human-readable plugin name, shown in X-Plane's Plugin Admin window.
    /// Defaulted to empty rather than required, so `#[xplm::plugin(name =
    /// ..., signature = ..., description = ...)]` can supply this instead
    /// without forcing a redundant override here — see `register_plugin!`'s
    /// two forms.
    const NAME: &'static str = "";
    /// Unique reverse-DNS-style identifier (e.g. `"com.example.myplugin"`).
    const SIGNATURE: &'static str = "";
    const DESCRIPTION: &'static str = "";

    /// Called once when the plugin is loaded. Corresponds to `XPluginStart`
    /// returning success (1) — unlike the raw SDK, there's no way to signal
    /// startup failure here; if `start()` needs to fail, panic (`guard()`
    /// catches it, logs it, and the plugin simply won't have a live
    /// instance for `enable`/`disable`/`receive_message` to no-op against).
    fn start() -> Self;

    /// Return `false` to refuse to enable (`XPluginEnable` returning 0).
    fn enable(&mut self) -> bool {
        true
    }

    fn disable(&mut self) {}

    /// Called once when the plugin is about to be unloaded; takes `self` by
    /// value so `Drop` runs naturally afterward — no separate cleanup step
    /// to remember, unlike the C# original's explicit `Dispose()`.
    fn stop(self) {}

    fn receive_message(
        &mut self,
        from_plugin_id: XPLMPluginID,
        message: c_int,
        param: *mut c_void,
    ) {
        let _ = (from_plugin_id, message, param);
    }
}

/// Writes up to 255 bytes of `value` plus a NUL terminator into `dest`,
/// truncating if longer. `dest` is one of the `out_name`/`out_sig`/
/// `out_desc` buffers X-Plane passes to `XPluginStart`, guaranteed by the
/// SDK to be at least 256 bytes.
///
/// # Safety
/// `dest` must be valid for writes of at least 256 bytes, or null (in which
/// case this is a no-op).
pub unsafe fn write_c_string(dest: *mut c_char, value: &str) {
    if dest.is_null() {
        return;
    }
    const CAPACITY: usize = 256;
    let bytes = value.as_bytes();
    let len = bytes.len().min(CAPACITY - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, dest, len);
        *dest.add(len) = 0;
    }
}

/// Generates the five `extern "C"` exports X-Plane loads a plugin by
/// (`XPluginStart`, `XPluginStop`, `XPluginEnable`, `XPluginDisable`,
/// `XPluginReceiveMessage`) for `$t: XPlanePlugin`. Call this exactly once,
/// at the root of your plugin's `cdylib` crate.
///
/// Two forms:
/// - `register_plugin!(MyPlugin)` reads `NAME`/`SIGNATURE`/`DESCRIPTION` off
///   `MyPlugin`'s `impl XPlanePlugin` (override the trait's defaults there).
/// - `register_plugin!(MyPlugin, name = "...", signature = "...", description
///   = "...")` supplies them directly instead — this is what
///   `#[xplm::plugin(...)]` expands to, so a plugin using that attribute
///   doesn't need to (and shouldn't) also override the consts.
///
/// Every trampoline is wrapped in [`crate::guard`] — a panic in any
/// lifecycle method is caught and logged rather than unwinding into
/// X-Plane.
#[macro_export]
macro_rules! register_plugin {
    ($t:ty) => {
        $crate::register_plugin!(
            @impl
            $t,
            <$t as $crate::plugin::XPlanePlugin>::NAME,
            <$t as $crate::plugin::XPlanePlugin>::SIGNATURE,
            <$t as $crate::plugin::XPlanePlugin>::DESCRIPTION
        );
    };
    ($t:ty, name = $name:expr, signature = $signature:expr, description = $description:expr) => {
        $crate::register_plugin!(@impl $t, $name, $signature, $description);
    };
    (@impl $t:ty, $name:expr, $signature:expr, $description:expr) => {
        static XPLM_PLUGIN_STATE: ::std::sync::Mutex<::std::option::Option<$t>> =
            ::std::sync::Mutex::new(::std::option::Option::None);

        #[no_mangle]
        pub extern "C" fn XPluginStart(
            out_name: *mut ::std::os::raw::c_char,
            out_sig: *mut ::std::os::raw::c_char,
            out_desc: *mut ::std::os::raw::c_char,
        ) -> ::std::os::raw::c_int {
            unsafe {
                $crate::plugin::write_c_string(out_name, $name);
                $crate::plugin::write_c_string(out_sig, $signature);
                $crate::plugin::write_c_string(out_desc, $description);
            }
            $crate::guard(|| {
                let plugin = <$t as $crate::plugin::XPlanePlugin>::start();
                *XPLM_PLUGIN_STATE.lock().unwrap() = ::std::option::Option::Some(plugin);
            });
            1
        }

        #[no_mangle]
        pub extern "C" fn XPluginStop() {
            $crate::guard(|| {
                if let ::std::option::Option::Some(plugin) =
                    XPLM_PLUGIN_STATE.lock().unwrap().take()
                {
                    <$t as $crate::plugin::XPlanePlugin>::stop(plugin);
                }
            });
        }

        #[no_mangle]
        pub extern "C" fn XPluginEnable() -> ::std::os::raw::c_int {
            $crate::guard(|| match XPLM_PLUGIN_STATE.lock().unwrap().as_mut() {
                ::std::option::Option::Some(plugin) => {
                    <$t as $crate::plugin::XPlanePlugin>::enable(plugin) as ::std::os::raw::c_int
                }
                ::std::option::Option::None => 0,
            })
            .unwrap_or(0)
        }

        #[no_mangle]
        pub extern "C" fn XPluginDisable() {
            $crate::guard(|| {
                if let ::std::option::Option::Some(plugin) =
                    XPLM_PLUGIN_STATE.lock().unwrap().as_mut()
                {
                    <$t as $crate::plugin::XPlanePlugin>::disable(plugin);
                }
            });
        }

        #[no_mangle]
        pub extern "C" fn XPluginReceiveMessage(
            from_who: $crate::plugin::XPLMPluginID,
            message: ::std::os::raw::c_int,
            param: *mut ::std::os::raw::c_void,
        ) {
            $crate::guard(|| {
                if let ::std::option::Option::Some(plugin) =
                    XPLM_PLUGIN_STATE.lock().unwrap().as_mut()
                {
                    <$t as $crate::plugin::XPlanePlugin>::receive_message(
                        plugin, from_who, message, param,
                    );
                }
            });
        }
    };
}
