//! Instanced object drawing (`XPLMInstance.h`). Builds on
//! [`crate::scenery::Object`]/[`crate::scenery::DrawInfo`] — depends on
//! `XPLMDrawInfo_t`/`XPLMObjectRef` from `XPLMScenery.h`, which is why this
//! module sits alongside `xplm::scenery` rather than `xplm::window`/
//! `xplm::menu`.

use std::ffi::CString;
use std::os::raw::c_char;

use xplm_sys::{XPLMCreateInstance, XPLMDestroyInstance, XPLMInstanceRef, XPLMInstanceSetPosition};

#[cfg(feature = "XPLM420")]
use xplm_sys::XPLMInstanceSetAutoShift;

use crate::scenery::{DrawInfo, Object};

/// An instanced `.obj`, registered with the list of datarefs you'll drive
/// per-frame via [`Instance::set_position`]. Dropping this calls
/// `XPLMDestroyInstance` (the underlying [`Object`] is unaffected — you're
/// still responsible for its own lifetime separately, per the SDK's own
/// "you can release your OBJ ref after creating the instance" note).
pub struct Instance {
    raw: XPLMInstanceRef,
    dataref_count: usize,
}

unsafe impl Send for Instance {} // see FlightLoop's identical rationale.

impl Instance {
    /// `datarefs` are the (ordered) dataref paths this instance's object
    /// animates with; `set_position`'s `data` slice must supply exactly one
    /// value per entry, in the same order. Returns `None` if any path
    /// contains an interior NUL.
    pub fn new(object: &Object, datarefs: &[&str]) -> Option<Self> {
        let c_strings: Vec<CString> = datarefs
            .iter()
            .map(|s| CString::new(*s))
            .collect::<Result<_, _>>()
            .ok()?;
        let mut ptrs: Vec<*const c_char> = c_strings.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(std::ptr::null());

        let raw = unsafe { XPLMCreateInstance(object.raw, ptrs.as_mut_ptr()) };
        (!raw.is_null()).then_some(Self {
            raw,
            dataref_count: datarefs.len(),
        })
    }

    /// Tells X-Plane to move this instance automatically whenever the sim's
    /// local coordinate system shifts — use for static instances you'd
    /// otherwise have to reposition yourself on every shift.
    #[cfg(feature = "XPLM420")]
    pub fn set_auto_shift(&self) {
        unsafe { XPLMInstanceSetAutoShift(self.raw) }
    }

    /// Updates the instance's position and every dataref it was created
    /// with. Call from a flight loop or UI callback — **not** a drawing
    /// callback (the whole point of instancing is that you don't need one).
    ///
    /// `data.len()` must equal the number of datarefs passed to
    /// [`Instance::new`]; mismatches are only checked in debug builds.
    pub fn set_position(&self, position: DrawInfo, data: &[f32]) {
        debug_assert_eq!(
            data.len(),
            self.dataref_count,
            "Instance::set_position: data.len() must match the dataref count from Instance::new"
        );
        let raw_position = position.into();
        unsafe { XPLMInstanceSetPosition(self.raw, &raw_position, data.as_ptr()) }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { XPLMDestroyInstance(self.raw) }
    }
}
