//! Plugin lifecycle (`XPLMPlugin.h` + the five required exported callbacks
//! every `.xpl` must define). `register_plugin!` generates the
//! `extern "C"` trampolines; `#[xplm::plugin(...)]` is sugar over the same
//! mechanism.

use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_int};

use xplm_sys::{
    XPLMCountPlugins, XPLMDisablePlugin, XPLMEnableFeature, XPLMEnablePlugin,
    XPLMEnumerateFeatures, XPLMFindPluginByPath, XPLMFindPluginBySignature, XPLMGetMyID,
    XPLMGetNthPlugin, XPLMGetPluginInfo, XPLMHasFeature, XPLMIsFeatureEnabled, XPLMIsPluginEnabled,
    XPLMReloadPlugins, XPLMSendMessageToPlugin,
};

pub use xplm_sys::XPLMPluginID;

/// `XPLM_NO_PLUGIN_ID` (`-1`) — accepted by [`Plugin::send_message`] to
/// broadcast to every enabled plugin with a message handler.
pub const NO_PLUGIN_ID: XPLMPluginID = -1;

/// A handle to a loaded plugin (this one or another), found via [`my_id`],
/// [`nth`], [`find_by_path`], or [`find_by_signature`] (`XPLMPlugin.h`).
/// Cheap to copy — it's just the SDK's opaque `XPLMPluginID`, re-validated
/// against the sim on every call rather than cached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plugin(XPLMPluginID);

/// `(name, file_path, signature, description)` for a [`Plugin`], per
/// `XPLMGetPluginInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInfo {
    pub name: String,
    pub file_path: String,
    pub signature: String,
    pub description: String,
}

fn read_c_buf(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

impl Plugin {
    pub fn id(&self) -> XPLMPluginID {
        self.0
    }

    /// Name, absolute file path, signature, and human-readable description.
    pub fn info(&self) -> PluginInfo {
        const BUF_LEN: usize = 256;
        let mut name = [0u8; BUF_LEN];
        let mut file_path = [0u8; BUF_LEN];
        let mut signature = [0u8; BUF_LEN];
        let mut description = [0u8; BUF_LEN];
        unsafe {
            XPLMGetPluginInfo(
                self.0,
                name.as_mut_ptr() as *mut c_char,
                file_path.as_mut_ptr() as *mut c_char,
                signature.as_mut_ptr() as *mut c_char,
                description.as_mut_ptr() as *mut c_char,
            );
        }
        PluginInfo {
            name: read_c_buf(&name),
            file_path: read_c_buf(&file_path),
            signature: read_c_buf(&signature),
            description: read_c_buf(&description),
        }
    }

    pub fn is_enabled(&self) -> bool {
        unsafe { XPLMIsPluginEnabled(self.0) != 0 }
    }

    /// Enables this plugin if not already enabled. Returns `false` if it
    /// refused to enable (its own `XPluginEnable` returned 0).
    pub fn enable(&self) -> bool {
        unsafe { XPLMEnablePlugin(self.0) != 0 }
    }

    pub fn disable(&self) {
        unsafe { XPLMDisablePlugin(self.0) }
    }

    /// Sends `message` (with an opaque `param`, meaning defined by the
    /// message itself) to this plugin, or broadcasts to every enabled
    /// plugin with a message handler if this is [`NO_PLUGIN_ID`].
    ///
    /// # Safety
    /// `param`'s validity contract is entirely defined by `message` — the
    /// same as the raw `XPLMSendMessageToPlugin` call.
    pub unsafe fn send_message(&self, message: i32, param: *mut c_void) {
        unsafe { XPLMSendMessageToPlugin(self.0, message, param) }
    }
}

/// This plugin's own handle, per `XPLMGetMyID`.
pub fn my_id() -> Plugin {
    Plugin(unsafe { XPLMGetMyID() })
}

/// How many plugins (enabled or not) are currently loaded.
pub fn count() -> i32 {
    unsafe { XPLMCountPlugins() }
}

/// The plugin at `index` (`0..count()`), in the SDK's own (unspecified, but
/// stable within a session) enumeration order.
pub fn nth(index: i32) -> Plugin {
    Plugin(unsafe { XPLMGetNthPlugin(index) })
}

/// Looks up a loaded plugin by its absolute file path. `None` if `path`
/// contains an interior NUL or no such plugin is loaded.
pub fn find_by_path(path: &str) -> Option<Plugin> {
    let c_path = CString::new(path).ok()?;
    let id = unsafe { XPLMFindPluginByPath(c_path.as_ptr()) };
    (id != NO_PLUGIN_ID).then_some(Plugin(id))
}

/// Looks up a loaded plugin by its unique signature (e.g.
/// `"com.example.myplugin"`) — the recommended way to find another plugin
/// your own plugin interoperates with. `None` if `signature` contains an
/// interior NUL or no such plugin is loaded.
pub fn find_by_signature(signature: &str) -> Option<Plugin> {
    let c_sig = CString::new(signature).ok()?;
    let id = unsafe { XPLMFindPluginBySignature(c_sig.as_ptr()) };
    (id != NO_PLUGIN_ID).then_some(Plugin(id))
}

/// Reloads every plugin. Once the callback you're inside returns, this
/// plugin will receive `disable`/`stop` and be unloaded, then reloaded as if
/// X-Plane were starting up again.
pub fn reload_plugins() {
    unsafe { XPLMReloadPlugins() }
}

/// Whether this X-Plane installation supports the named optional feature
/// (e.g. `"XPLM_USE_NATIVE_PATHS"`) — see `XPLMPlugin.h`'s Plugin Features
/// API docs for the well-known feature strings. `false` if `feature`
/// contains an interior NUL.
pub fn has_feature(feature: &str) -> bool {
    let Ok(c_feature) = CString::new(feature) else {
        return false;
    };
    unsafe { XPLMHasFeature(c_feature.as_ptr()) != 0 }
}

/// Whether `feature` is currently enabled for this plugin. It's an error
/// (per the SDK) to call this for a feature [`has_feature`] doesn't report
/// as supported. `false` if `feature` contains an interior NUL.
pub fn is_feature_enabled(feature: &str) -> bool {
    let Ok(c_feature) = CString::new(feature) else {
        return false;
    };
    unsafe { XPLMIsFeatureEnabled(c_feature.as_ptr()) != 0 }
}

/// Enables or disables `feature` for this plugin. Silently does nothing if
/// `feature` contains an interior NUL.
pub fn enable_feature(feature: &str, enable: bool) {
    let Ok(c_feature) = CString::new(feature) else {
        return;
    };
    unsafe { XPLMEnableFeature(c_feature.as_ptr(), enable as c_int) }
}

unsafe extern "C" fn feature_enumerator_trampoline(feature: *const c_char, refcon: *mut c_void) {
    crate::guard(|| {
        let callback: &mut &mut dyn FnMut(&str) = unsafe { &mut *(refcon as *mut _) };
        let name = unsafe { CStr::from_ptr(feature) }.to_string_lossy();
        callback(&name);
    });
}

/// Calls `callback` once per feature this running version of X-Plane
/// supports.
pub fn enumerate_features(mut callback: impl FnMut(&str)) {
    let mut trait_obj: &mut dyn FnMut(&str) = &mut callback;
    let refcon = &mut trait_obj as *mut &mut dyn FnMut(&str) as *mut c_void;
    unsafe { XPLMEnumerateFeatures(Some(feature_enumerator_trampoline), refcon) }
}

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
