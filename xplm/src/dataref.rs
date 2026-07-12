//! DataRef access (`XPLMDataAccess.h`), gated by access level at compile
//! time instead of a runtime writability check.
//!
//! `DataRef<T, Access>` is the core generic type; `ReadOnly<T>`/`ReadWrite<T>`
//! are the ergonomic aliases most call sites use (matching the target
//! architecture's `#[dataref = "..."] pub throttle: ReadWrite<f32>` shape).
//! `get()` is defined for both; `set()` only compiles when `Access: Writable`
//! — there is no runtime check to forget, because there is no runtime check.

use std::ffi::CString;
use std::marker::PhantomData;

use xplm_sys::XPLMDataRef;

mod sealed {
    pub trait Sealed {}
}

/// Marker: a `DataRef<T, ReadOnlyMarker>` (aka `ReadOnly<T>`) only has `get()`.
pub struct ReadOnlyMarker;
/// Marker: a `DataRef<T, ReadWriteMarker>` (aka `ReadWrite<T>`) also has `set()`.
pub struct ReadWriteMarker;

impl sealed::Sealed for ReadOnlyMarker {}
impl sealed::Sealed for ReadWriteMarker {}

/// Implemented by `ReadOnlyMarker`/`ReadWriteMarker` only — sealed so no
/// third access level can be introduced outside this module.
pub trait Access: sealed::Sealed {}
impl Access for ReadOnlyMarker {}
impl Access for ReadWriteMarker {}

/// Implemented only by `ReadWriteMarker`. This is the entire mechanism: a
/// `set()` method with `A: Writable` in its bound simply doesn't exist for
/// `DataRef<T, ReadOnlyMarker>`, so calling it is a compile error, not a
/// caught-at-runtime mistake.
pub trait Writable: Access {}
impl Writable for ReadWriteMarker {}

fn find_raw(name: &str) -> Option<XPLMDataRef> {
    let c_name = CString::new(name).ok()?;
    let raw = unsafe { xplm_sys::XPLMFindDataRef(c_name.as_ptr()) };
    (!raw.is_null()).then_some(raw)
}

/// A scalar type `XPLMGetData*`/`XPLMSetData*` can read/write directly.
/// Sealed to `i32`/`f32`/`f64` — the three scalar dataref types the SDK
/// defines (`xplmType_Int`/`Float`/`Double`).
pub trait Scalar: sealed_scalar::Sealed + Copy {
    #[doc(hidden)]
    unsafe fn get_raw(raw: XPLMDataRef) -> Self;
    #[doc(hidden)]
    unsafe fn set_raw(raw: XPLMDataRef, value: Self);
}

mod sealed_scalar {
    pub trait Sealed {}
    impl Sealed for i32 {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

impl Scalar for i32 {
    unsafe fn get_raw(raw: XPLMDataRef) -> Self {
        unsafe { xplm_sys::XPLMGetDatai(raw) }
    }
    unsafe fn set_raw(raw: XPLMDataRef, value: Self) {
        unsafe { xplm_sys::XPLMSetDatai(raw, value) }
    }
}

impl Scalar for f32 {
    unsafe fn get_raw(raw: XPLMDataRef) -> Self {
        unsafe { xplm_sys::XPLMGetDataf(raw) }
    }
    unsafe fn set_raw(raw: XPLMDataRef, value: Self) {
        unsafe { xplm_sys::XPLMSetDataf(raw, value) }
    }
}

impl Scalar for f64 {
    unsafe fn get_raw(raw: XPLMDataRef) -> Self {
        unsafe { xplm_sys::XPLMGetDatad(raw) }
    }
    unsafe fn set_raw(raw: XPLMDataRef, value: Self) {
        unsafe { xplm_sys::XPLMSetDatad(raw, value) }
    }
}

/// A handle to a scalar (`i32`/`f32`/`f64`) X-Plane dataref, found by path.
/// `Access` (see `ReadOnly`/`ReadWrite` aliases below) gates whether `set()`
/// exists at all.
pub struct DataRef<T: Scalar, A: Access> {
    raw: XPLMDataRef,
    _type: PhantomData<T>,
    _access: PhantomData<A>,
}

impl<T: Scalar, A: Access> DataRef<T, A> {
    /// Looks up `name` via `XPLMFindDataRef`. Returns `None` if no dataref
    /// with that path is currently registered. The access marker is chosen
    /// by the caller (or a `#[dataref = "..."]` field's declared type in
    /// Phase 6) — it is not derived from `XPLMCanWriteDataRef`, since that's
    /// a runtime property (and can change if a providing plugin unloads)
    /// while `Access` is a compile-time contract about what code you're
    /// allowed to write, not a claim about the sim's current state.
    pub fn find(name: &str) -> Option<Self> {
        find_raw(name).map(|raw| Self {
            raw,
            _type: PhantomData,
            _access: PhantomData,
        })
    }

    pub fn get(&self) -> T {
        unsafe { T::get_raw(self.raw) }
    }
}

impl<T: Scalar, A: Writable> DataRef<T, A> {
    pub fn set(&self, value: T) {
        unsafe { T::set_raw(self.raw, value) }
    }
}

/// `ReadOnly<f64>` etc. — the ergonomic alias matching the target
/// architecture's field-type shorthand.
pub type ReadOnly<T> = DataRef<T, ReadOnlyMarker>;
/// `ReadWrite<f32>` etc.
pub type ReadWrite<T> = DataRef<T, ReadWriteMarker>;

/// An array element type `XPLMGetDatav*`/`XPLMSetDatav*` can read/write.
/// Sealed to `i32`/`f32` — the two array dataref types the SDK defines
/// (`xplmType_IntArray`/`FloatArray`). Byte-array (`xplmType_Data`) datarefs
/// are a separate, untyped API and not covered here.
pub trait ArrayElement: sealed_array::Sealed + Copy + Default {
    #[doc(hidden)]
    unsafe fn get_raw(raw: XPLMDataRef, out: Option<&mut [Self]>, offset: i32) -> i32;
    #[doc(hidden)]
    unsafe fn set_raw(raw: XPLMDataRef, values: &[Self], offset: i32);
}

mod sealed_array {
    pub trait Sealed {}
    impl Sealed for i32 {}
    impl Sealed for f32 {}
}

impl ArrayElement for i32 {
    unsafe fn get_raw(raw: XPLMDataRef, out: Option<&mut [Self]>, offset: i32) -> i32 {
        match out {
            Some(buf) => unsafe {
                xplm_sys::XPLMGetDatavi(raw, buf.as_mut_ptr(), offset, buf.len() as i32)
            },
            None => unsafe { xplm_sys::XPLMGetDatavi(raw, std::ptr::null_mut(), 0, 0) },
        }
    }
    unsafe fn set_raw(raw: XPLMDataRef, values: &[Self], offset: i32) {
        unsafe {
            xplm_sys::XPLMSetDatavi(
                raw,
                values.as_ptr() as *mut i32,
                offset,
                values.len() as i32,
            )
        }
    }
}

impl ArrayElement for f32 {
    unsafe fn get_raw(raw: XPLMDataRef, out: Option<&mut [Self]>, offset: i32) -> i32 {
        match out {
            Some(buf) => unsafe {
                xplm_sys::XPLMGetDatavf(raw, buf.as_mut_ptr(), offset, buf.len() as i32)
            },
            None => unsafe { xplm_sys::XPLMGetDatavf(raw, std::ptr::null_mut(), 0, 0) },
        }
    }
    unsafe fn set_raw(raw: XPLMDataRef, values: &[Self], offset: i32) {
        unsafe {
            xplm_sys::XPLMSetDatavf(
                raw,
                values.as_ptr() as *mut f32,
                offset,
                values.len() as i32,
            )
        }
    }
}

/// A handle to an array (`i32`/`f32`) X-Plane dataref, found by path.
pub struct ArrayDataRef<T: ArrayElement, A: Access> {
    raw: XPLMDataRef,
    _type: PhantomData<T>,
    _access: PhantomData<A>,
}

impl<T: ArrayElement, A: Access> ArrayDataRef<T, A> {
    pub fn find(name: &str) -> Option<Self> {
        find_raw(name).map(|raw| Self {
            raw,
            _type: PhantomData,
            _access: PhantomData,
        })
    }

    /// The dataref's current array length, per `XPLMGetDatav*`'s
    /// pass-`NULL`-for-`outValues` convention.
    pub fn len(&self) -> usize {
        unsafe { T::get_raw(self.raw, None, 0) as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copies up to `out.len()` values starting at `offset` into `out`,
    /// returning the number actually copied (may be less than `out.len()`
    /// if `offset + out.len()` runs past the end of the array).
    pub fn get(&self, offset: usize, out: &mut [T]) -> usize {
        unsafe { T::get_raw(self.raw, Some(out), offset as i32) as usize }
    }
}

impl<T: ArrayElement, A: Writable> ArrayDataRef<T, A> {
    /// Writes `values` starting at `offset`. Silently writes fewer than
    /// `values.len()` items if that would run past the end of the array —
    /// this is the underlying `XPLMSetDatav*` semantics, not a bug here.
    pub fn set(&self, offset: usize, values: &[T]) {
        unsafe { T::set_raw(self.raw, values, offset as i32) }
    }
}

pub type ReadOnlyArray<T> = ArrayDataRef<T, ReadOnlyMarker>;
pub type ReadWriteArray<T> = ArrayDataRef<T, ReadWriteMarker>;
