//! `XPLMUtilities.h`'s stateless free functions and data-file/directory
//! helpers. Key sniffers and hotkeys are declared in `XPLMDisplay.h`
//! instead, so they live in `xplm::window` alongside the rest of that
//! header. Commands (also `XPLMUtilities.h`) get their own module,
//! `xplm::command`, since they need the RAII + trampoline treatment.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[cfg(feature = "XPLM200")]
use xplm_sys::{xplm_DataFile_ReplayMovie, xplm_DataFile_Situation, XPLMDataFileType};
use xplm_sys::{
    XPLMGetDirectoryContents, XPLMGetPrefsPath, XPLMGetSystemPath, XPLMGetVersions,
    XPLMHostApplicationID, XPLMReloadScenery, XPLMSpeakString,
};
#[cfg(feature = "XPLM200")]
use xplm_sys::{XPLMLoadDataFile, XPLMSaveDataFile};

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

/// The per-page scratch buffer size `DirectoryEntries` fetches with — small
/// and fixed, since a page is re-fetched (not grown) whenever it runs out;
/// see [`directory_entries`].
const DIRECTORY_PAGE_BUF_SIZE: usize = 1024;

/// A lazy, pull-based enumeration of a directory's entries (the `Iterator`
/// analogue of C#'s `IEnumerable<string>` over `Directory.EnumerateFiles`) —
/// nothing is read until [`Iterator::next`] is actually called, and only one
/// small page of names is held at a time. Internally, each page is one
/// `XPLMGetDirectoryContents` call using its `inFirstReturn` paging support;
/// the next page is only fetched once the current one is exhausted, so
/// listing a huge directory never requires buffering it all in memory at
/// once — memory use stays bounded by the page size regardless of how many
/// entries the directory has.
pub struct DirectoryEntries {
    path: CString,
    next_index: i32,
    page: Vec<u8>,
    pos: usize,
    /// How many NUL-terminated names in `page` (from `pos` on) are still
    /// unread — tracked from the SDK's own `outReturnedFiles` for this page,
    /// rather than scanned for a sentinel, since a page's unused tail is
    /// already zero-filled and would otherwise look like more empty entries.
    remaining_in_page: i32,
    exhausted: bool,
}

impl DirectoryEntries {
    fn fetch_next_page(&mut self) {
        let mut page = vec![0u8; DIRECTORY_PAGE_BUF_SIZE];
        let mut returned_files: i32 = 0;
        unsafe {
            XPLMGetDirectoryContents(
                self.path.as_ptr(),
                self.next_index,
                page.as_mut_ptr() as *mut c_char,
                DIRECTORY_PAGE_BUF_SIZE as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut returned_files,
            );
        }
        if returned_files <= 0 {
            self.exhausted = true;
            return;
        }
        self.next_index += returned_files;
        self.remaining_in_page = returned_files;
        self.page = page;
        self.pos = 0;
    }
}

impl Iterator for DirectoryEntries {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        loop {
            if self.remaining_in_page > 0 {
                let end = self.page[self.pos..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|i| self.pos + i)
                    .unwrap_or(self.page.len());
                let name = String::from_utf8_lossy(&self.page[self.pos..end]).into_owned();
                self.pos = end + 1;
                self.remaining_in_page -= 1;
                return Some(name);
            }
            if self.exhausted {
                return None;
            }
            self.fetch_next_page();
        }
    }
}

/// Lazily enumerates the entries of `directory_path` — see
/// [`DirectoryEntries`] for why this pulls one small page at a time rather
/// than reading the whole directory upfront. `None` if `directory_path`
/// contains an interior NUL.
pub fn directory_entries(directory_path: &str) -> Option<DirectoryEntries> {
    let path = CString::new(directory_path).ok()?;
    Some(DirectoryEntries {
        path,
        next_index: 0,
        page: Vec::new(),
        pos: 0,
        remaining_in_page: 0,
        exhausted: false,
    })
}

/// Which kind of data file `load_data_file`/`save_data_file` operate on.
#[cfg(feature = "XPLM200")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFileType {
    Situation,
    ReplayMovie,
}

#[cfg(feature = "XPLM200")]
impl From<DataFileType> for XPLMDataFileType {
    fn from(t: DataFileType) -> Self {
        match t {
            DataFileType::Situation => xplm_DataFile_Situation,
            DataFileType::ReplayMovie => xplm_DataFile_ReplayMovie,
        }
    }
}

/// Loads a `.sit`/replay file (`file_path` relative to the X-System folder).
/// Passing `None` only makes sense for `DataFileType::ReplayMovie`, to clear
/// the current replay. Returns `false` on failure (including `file_path`
/// containing an interior NUL).
#[cfg(feature = "XPLM200")]
pub fn load_data_file(file_type: DataFileType, file_path: Option<&str>) -> bool {
    let c_path = match file_path {
        Some(p) => match CString::new(p) {
            Ok(p) => Some(p),
            Err(_) => return false,
        },
        None => None,
    };
    let ptr = c_path.as_deref().map_or(std::ptr::null(), CStr::as_ptr);
    unsafe { XPLMLoadDataFile(file_type.into(), ptr) != 0 }
}

/// Saves a `.sit`/replay file (`file_path` relative to the X-System folder).
/// Returns `false` on failure (including `file_path` containing an interior
/// NUL).
#[cfg(feature = "XPLM200")]
pub fn save_data_file(file_type: DataFileType, file_path: &str) -> bool {
    let Ok(c_path) = CString::new(file_path) else {
        return false;
    };
    unsafe { XPLMSaveDataFile(file_type.into(), c_path.as_ptr()) != 0 }
}
