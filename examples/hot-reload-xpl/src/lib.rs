//! Thin hot-reload loader plugin. X-Plane loads *this* binary once, as
//! `Resources/plugins/hot-reload-xpl/64/win.xpl` (or `mac.xpl`/`lin.xpl` on
//! macOS/Linux — same loader source, `xplm-sys` already builds for all three
//! target OSes), and never again — it just forwards every SDK callback to
//! whatever payload library is currently named in the watch file, and swaps
//! the payload in/out (`libloading::Library` load/drop — `LoadLibraryW`/
//! `FreeLibrary` on Windows, `dlopen`/`dlclose` on macOS/Linux, same API
//! either way) as new builds appear.
//!
//! See `examples/hot-reload-dll` for the swappable payload, and this crate's
//! `.vscode/tasks.json` + `README.md` section for the F5 dev loop that keeps
//! the watch file up to date.
//!
//! This crate deliberately does **not** use `#[xplm::plugin(...)]` /
//! `register_plugin!` — both bake in static `NAME`/`SIGNATURE`/`DESCRIPTION`
//! consts, but this loader's identity is decided dynamically, inside its own
//! `XPluginStart`, from whatever payload it manages to load (see
//! `XPluginStart` below) — X-Plane reads a plugin's name/signature/
//! description exactly once, from that call's out-buffers, with no API to
//! change them later. Every export below is still individually wrapped in
//! `xplm::guard(...)`, same as `register_plugin!` would generate — the
//! panic-safety rule applies regardless of whether the export is macro- or
//! hand-written.

use std::ffi::{c_void, CStr};
use std::fs;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::sync::Mutex;

use libloading::{Library, Symbol};
use xplm::plugin::{write_c_string, XPLMPluginID};
use xplm::processing::{FlightLoop, FlightLoopPhase};

type StartFn = unsafe extern "C" fn(*mut c_char, *mut c_char, *mut c_char) -> c_int;
type EnableFn = unsafe extern "C" fn() -> c_int;
type DisableFn = unsafe extern "C" fn();
type StopFn = unsafe extern "C" fn();
type ReceiveMessageFn = unsafe extern "C" fn(XPLMPluginID, c_int, *mut c_void);

const FALLBACK_NAME: &str = "Hot Reload (missing payload)";
const LOADER_SIGNATURE: &str = "org.xplm-rust.hot-reload-xpl";
/// Fixed for this example. A reusable version of this loader would derive
/// this from a hash of the payload crate's manifest dir instead, so multiple
/// hot-reloaded projects on the same machine don't collide — see this
/// crate's README section.
const WATCH_FILE_NAME: &str = "hot-reload-example.json";

#[derive(serde::Deserialize)]
struct WatchFile {
    payload_path: String,
    build_id: String,
}

/// `dirs::data_local_dir()` resolves to the same per-OS conventions the
/// build task's install-location lookup targets: `%LocalAppData%` on
/// Windows, `~/Library/Application Support` on macOS, `$XDG_DATA_HOME`
/// (or `~/.local/share`) on Linux. Loader and build task must agree on this
/// path exactly — see this crate's `.vscode/tasks.json`.
fn watch_file_path() -> Option<PathBuf> {
    Some(
        dirs::data_local_dir()?
            .join("xplm-hotreload")
            .join(WATCH_FILE_NAME),
    )
}

fn read_watch_file() -> Option<WatchFile> {
    let path = watch_file_path()?;
    let contents = fs::read_to_string(&path)
        .inspect_err(|e| xplm::log(&format!("hot-reload-xpl: no watch file at {path:?}: {e}\n")))
        .ok()?;
    serde_json::from_str(&contents)
        .inspect_err(|e| xplm::log(&format!("hot-reload-xpl: malformed watch file: {e}\n")))
        .ok()
}

/// The currently loaded payload DLL and its resolved lifecycle symbols.
/// Symbols are re-resolved by name on each call rather than cached, since
/// calls here are rare (once per swap, or once per Enable/Disable toggle) —
/// simplicity over micro-optimizing a cold path.
struct LoadedPayload {
    library: Library,
    /// The staged DLL/dylib/so this payload was loaded from — kept so it can
    /// be deleted once `library` drops and releases the file lock, instead
    /// of leaving every past build's staging file behind forever.
    path: PathBuf,
    build_id: String,
    enabled: bool,
}

impl LoadedPayload {
    unsafe fn symbol<T>(&self, name: &[u8]) -> Option<Symbol<'_, T>> {
        self.library.get(name).ok()
    }

    fn enable(&mut self) {
        if self.enabled {
            return;
        }
        unsafe {
            if let Some(f) = self.symbol::<EnableFn>(b"XPluginEnable\0") {
                f();
            }
        }
        self.enabled = true;
    }

    fn disable(&mut self) {
        if !self.enabled {
            return;
        }
        unsafe {
            if let Some(f) = self.symbol::<DisableFn>(b"XPluginDisable\0") {
                f();
            }
        }
        self.enabled = false;
    }

    /// Disables (if needed), calls the payload's `XPluginStop`, then drops
    /// `library` — the `FreeLibrary` equivalent, releasing the file lock on
    /// the old build's DLL — and finally deletes the now-unlocked file.
    /// Best-effort: a delete failure (e.g. still transiently locked on some
    /// platform/timing edge case) is logged, not fatal — an orphaned staging
    /// file is a minor disk-space nit, not a correctness problem.
    fn stop(mut self) {
        self.disable();
        unsafe {
            if let Some(f) = self.symbol::<StopFn>(b"XPluginStop\0") {
                f();
            }
        }
        let path = self.path.clone();
        drop(self.library);
        if let Err(e) = fs::remove_file(&path) {
            xplm::log(&format!(
                "hot-reload-xpl: failed to clean up old payload file {path:?}: {e}\n"
            ));
        } else {
            xplm::log(&format!(
                "hot-reload-xpl: cleaned up old payload file {path:?}\n"
            ));
        }
    }
}

struct LoaderState {
    payload: Option<LoadedPayload>,
    loader_enabled: bool,
    // Kept alive only so `Drop` unregisters it on `XPluginStop`.
    _poll: FlightLoop,
}

static LOADER_STATE: Mutex<Option<LoaderState>> = Mutex::new(None);

/// Loads `payload_path`, calls its `XPluginStart`, and returns the loaded
/// payload plus the name/signature/description *it* reported — used both to
/// seed the loader's own identity on first start, and to log what got loaded
/// on later swaps.
fn load_payload(
    payload_path: &str,
    build_id: &str,
) -> Option<(LoadedPayload, String, String, String)> {
    let library = match unsafe { Library::new(payload_path) } {
        Ok(library) => library,
        Err(e) => {
            xplm::log(&format!(
                "hot-reload-xpl: failed to load payload {payload_path}: {e}\n"
            ));
            return None;
        }
    };

    let mut name_buf: [c_char; 256] = [0; 256];
    let mut sig_buf: [c_char; 256] = [0; 256];
    let mut desc_buf: [c_char; 256] = [0; 256];
    unsafe {
        let start: Symbol<StartFn> = match library.get(b"XPluginStart\0") {
            Ok(start) => start,
            Err(e) => {
                xplm::log(&format!(
                    "hot-reload-xpl: payload {payload_path} missing XPluginStart: {e}\n"
                ));
                return None;
            }
        };
        start(
            name_buf.as_mut_ptr(),
            sig_buf.as_mut_ptr(),
            desc_buf.as_mut_ptr(),
        );
    }

    let to_string = |buf: &[c_char; 256]| unsafe {
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    };

    Some((
        LoadedPayload {
            library,
            path: PathBuf::from(payload_path),
            build_id: build_id.to_string(),
            enabled: false,
        },
        to_string(&name_buf),
        to_string(&sig_buf),
        to_string(&desc_buf),
    ))
}

/// The loader's 1Hz poll: if the watch file's `build_id` differs from
/// whatever's currently loaded, swap it in. Fail-soft throughout — a bad
/// build or an unreadable watch file logs and leaves the previous payload
/// (if any) running rather than tearing anything down.
fn check_for_new_payload() {
    let Some(watch) = read_watch_file() else {
        return;
    };
    let mut guard = LOADER_STATE.lock().unwrap();
    let Some(state) = guard.as_mut() else {
        return;
    };
    if state.payload.as_ref().map(|p| p.build_id.as_str()) == Some(watch.build_id.as_str()) {
        return; // already running this build
    }

    xplm::log(&format!(
        "hot-reload-xpl: detected build {}, swapping\n",
        watch.build_id
    ));
    if let Some(old) = state.payload.take() {
        old.stop();
    }

    match load_payload(&watch.payload_path, &watch.build_id) {
        Some((mut new_payload, name, _sig, _desc)) => {
            xplm::log(&format!("hot-reload-xpl: loaded payload \"{name}\"\n"));
            if state.loader_enabled {
                new_payload.enable();
            }
            state.payload = Some(new_payload);
        }
        None => xplm::log("hot-reload-xpl: swap failed, no payload loaded\n"),
    }
}

#[no_mangle]
pub extern "C" fn XPluginStart(
    out_name: *mut c_char,
    out_sig: *mut c_char,
    out_desc: *mut c_char,
) -> c_int {
    xplm::guard(|| {
        let watch = read_watch_file();
        let (payload, name, sig, desc) = match watch {
            Some(w) => match load_payload(&w.payload_path, &w.build_id) {
                Some((payload, name, sig, desc)) => (Some(payload), name, sig, desc),
                None => (
                    None,
                    FALLBACK_NAME.to_string(),
                    LOADER_SIGNATURE.to_string(),
                    "Hot-reload loader; payload failed to load.".to_string(),
                ),
            },
            None => (
                None,
                FALLBACK_NAME.to_string(),
                LOADER_SIGNATURE.to_string(),
                "Hot-reload loader; no payload has been built yet.".to_string(),
            ),
        };

        unsafe {
            write_c_string(out_name, &name);
            write_c_string(out_sig, &sig);
            write_c_string(out_desc, &desc);
        }

        let poll = FlightLoop::new(FlightLoopPhase::BeforeFlightModel, |_, _, _| {
            check_for_new_payload();
            1.0 // re-run every second
        });
        poll.schedule(1.0, true);

        *LOADER_STATE.lock().unwrap() = Some(LoaderState {
            payload,
            loader_enabled: false,
            _poll: poll,
        });
    });

    1
}

#[no_mangle]
pub extern "C" fn XPluginStop() {
    xplm::guard(|| {
        if let Some(state) = LOADER_STATE.lock().unwrap().take() {
            if let Some(payload) = state.payload {
                payload.stop();
            }
            // state._poll drops here, unregistering the flight loop.
        }
    });
}

#[no_mangle]
pub extern "C" fn XPluginEnable() -> c_int {
    xplm::guard(|| {
        let mut guard = LOADER_STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            state.loader_enabled = true;
            if let Some(payload) = state.payload.as_mut() {
                payload.enable();
            }
        }
    });
    1
}

#[no_mangle]
pub extern "C" fn XPluginDisable() {
    xplm::guard(|| {
        let mut guard = LOADER_STATE.lock().unwrap();
        if let Some(state) = guard.as_mut() {
            state.loader_enabled = false;
            if let Some(payload) = state.payload.as_mut() {
                payload.disable();
            }
        }
    });
}

#[no_mangle]
pub extern "C" fn XPluginReceiveMessage(
    from_who: XPLMPluginID,
    message: c_int,
    param: *mut c_void,
) {
    xplm::guard(|| {
        let guard = LOADER_STATE.lock().unwrap();
        if let Some(payload) = guard.as_ref().and_then(|state| state.payload.as_ref()) {
            unsafe {
                if let Some(f) = payload.symbol::<ReceiveMessageFn>(b"XPluginReceiveMessage\0") {
                    f(from_who, message, param);
                }
            }
        }
    });
}
