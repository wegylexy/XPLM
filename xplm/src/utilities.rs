//! A slice of `XPLMUtilities.h`'s stateless free functions — the rest
//! (directory listing, data files, key sniffers, hotkeys) is left for a
//! later pass; see `PHASES.md`. Commands (also `XPLMUtilities.h`) get their
//! own module, `xplm::command`, since they need the RAII + trampoline
//! treatment.

use std::ffi::CString;
use std::os::raw::c_char;

use xplm_sys::{
    XPLMGetPrefsPath, XPLMGetSystemPath, XPLMGetVersions, XPLMHostApplicationID, XPLMReloadScenery,
    XPLMSpeakString,
};

/// The SDK's own documented minimum buffer size for
/// `XPLMGetSystemPath`/`XPLMGetPrefsPath`.
const PATH_BUFFER_LEN: usize = 512;

fn read_path(f: unsafe extern "C" fn(*mut c_char)) -> String {
    let mut buf = [0u8; PATH_BUFFER_LEN];
    unsafe { f(buf.as_mut_ptr() as *mut c_char) };
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// The full path to the X-System folder (a directory path, ending in a
/// trailing separator).
pub fn system_path() -> String {
    read_path(XPLMGetSystemPath)
}

/// A path to a file within X-Plane's preferences directory (strip back to
/// the last directory separator to get the directory itself).
pub fn prefs_path() -> String {
    read_path(XPLMGetPrefsPath)
}

/// `(xplane_version, xplm_version, host_id)`. Versions are decimal-encoded
/// (e.g. `606` for X-Plane 6.06).
pub fn versions() -> (i32, i32, XPLMHostApplicationID) {
    let (mut xplane_version, mut xplm_version, mut host_id) = (0, 0, 0);
    unsafe { XPLMGetVersions(&mut xplane_version, &mut xplm_version, &mut host_id) };
    (xplane_version, xplm_version, host_id)
}

/// Uses the OS's text-to-speech to speak `text`, if available. Silently
/// does nothing if `text` contains an interior NUL.
pub fn speak_string(text: &str) {
    let Ok(c_text) = CString::new(text) else {
        return;
    };
    unsafe { XPLMSpeakString(c_text.as_ptr()) }
}

/// Reloads the current scenery, the same as if the user had used the
/// scenery-reload menu item.
pub fn reload_scenery() {
    unsafe { XPLMReloadScenery() }
}
