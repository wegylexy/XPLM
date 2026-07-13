//! DataRef access (`XPLMDataAccess.h`), gated by access level at compile
//! time instead of a runtime writability check.
//!
//! `DataRef<T, Access>` is the core generic type; `ReadOnly<T>`/`ReadWrite<T>`
//! are the ergonomic aliases most call sites use (matching the target
//! architecture's `#[dataref = "..."] pub throttle: ReadWrite<f32>` shape).
//! `get()` is defined for both; `set()` only compiles when `Access: Writable`
//! — there is no runtime check to forget, because there is no runtime check.

use std::cell::RefCell;
use std::ffi::CString;
use std::marker::PhantomData;
use std::os::raw::{c_char, c_void};

use xplm_sys::{
    xplmType_Data, xplmType_Double, xplmType_Float, xplmType_FloatArray, xplmType_Int,
    xplmType_IntArray, XPLMDataRef, XPLMGetDatab, XPLMRegisterDataAccessor, XPLMSetDatab,
    XPLMUnregisterDataAccessor,
};

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

// X-Plane datarefs are only ever read/written from the sim's single main
// thread — same rationale as `FlightLoop`/`Window`/`Menu`'s `unsafe impl
// Send`. Plugin state holding one still needs to live in a `static
// Mutex<Option<_>>` (see `register_plugin!`), which requires `Send`.
unsafe impl<T: Scalar, A: Access> Send for DataRef<T, A> {}

impl<T: Scalar, A: Access> DataRef<T, A> {
    /// Looks up `name` via `XPLMFindDataRef`. Returns `None` if no dataref
    /// with that path is currently registered. The access marker is chosen
    /// by the caller (or a `#[dataref = "..."]` field's declared type, via
    /// `#[derive(DataRefContainer)]`) — it is not derived from
    /// `XPLMCanWriteDataRef`, since that's a runtime property (and can
    /// change if a providing plugin unloads) while `Access` is a
    /// compile-time contract about what code you're allowed to write, not a
    /// claim about the sim's current state.
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

// See `DataRef`'s identical `unsafe impl Send` above for the rationale.
unsafe impl<T: ArrayElement, A: Access> Send for ArrayDataRef<T, A> {}

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

    /// Convenience over [`Self::get`]: reads the whole array into a
    /// freshly-allocated `Vec` — same pairing [`DataBytes::get_all`]/
    /// [`DataStruct::get`] provide for their own types.
    pub fn get_all(&self) -> Vec<T> {
        let mut buf = vec![T::default(); self.len()];
        let n = self.get(0, &mut buf);
        buf.truncate(n);
        buf
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

/// A handle to a byte-string (`xplmType_Data`) X-Plane dataref, found by
/// path — the consumer-side (`XPLMGetDatab`/`XPLMSetDatab`) counterpart to
/// [`PublishedData`] below, same as [`DataRef`]/[`ArrayDataRef`] are the
/// consumer-side counterparts of [`PublishedDataRef`].
pub struct DataBytes<A: Access> {
    raw: XPLMDataRef,
    _access: PhantomData<A>,
}

// See `DataRef`'s identical `unsafe impl Send` above for the rationale.
unsafe impl<A: Access> Send for DataBytes<A> {}

impl<A: Access> DataBytes<A> {
    pub fn find(name: &str) -> Option<Self> {
        find_raw(name).map(|raw| Self {
            raw,
            _access: PhantomData,
        })
    }

    /// The dataref's current byte length, per `XPLMGetDatab`'s
    /// pass-`NULL`-for-`outValue` convention.
    pub fn len(&self) -> usize {
        unsafe { XPLMGetDatab(self.raw, std::ptr::null_mut(), 0, 0) as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copies up to `out.len()` bytes starting at `offset` into `out`,
    /// returning the number actually copied — same offset/max-length
    /// contract as [`ArrayDataRef::get`].
    pub fn get(&self, offset: usize, out: &mut [u8]) -> usize {
        unsafe {
            XPLMGetDatab(
                self.raw,
                out.as_mut_ptr() as *mut c_void,
                offset as i32,
                out.len() as i32,
            ) as usize
        }
    }

    /// Convenience over [`Self::get`]: reads the whole byte string into a
    /// freshly-allocated `Vec`.
    pub fn get_all(&self) -> Vec<u8> {
        let mut buf = vec![0u8; self.len()];
        let n = self.get(0, &mut buf);
        buf.truncate(n);
        buf
    }
}

impl<A: Writable> DataBytes<A> {
    /// Writes `values` starting at `offset`.
    pub fn set(&self, offset: usize, values: &[u8]) {
        unsafe {
            XPLMSetDatab(
                self.raw,
                values.as_ptr() as *mut c_void,
                offset as i32,
                values.len() as i32,
            )
        }
    }
}

pub type ReadOnlyBytes = DataBytes<ReadOnlyMarker>;
pub type ReadWriteBytes = DataBytes<ReadWriteMarker>;

/// Backing state for a [`PublishedDataRef`], boxed so its address is stable
/// across the whole registration's lifetime (the shim/SDK only ever sees the
/// raw pointer, never a copy). `write` is `RefCell`-guarded rather than
/// requiring `&mut self` in the trampoline, since `XPLMSetData*_f` only ever
/// hands back a `*mut c_void` refcon, not a Rust `&mut` borrow.
struct Accessor<T> {
    read: Box<dyn Fn() -> T>,
    write: RefCell<Option<Box<dyn FnMut(T)>>>,
}

/// A scalar type [`PublishedDataRef`] can publish via `XPLMRegisterDataAccessor`.
/// Sealed to `i32`/`f32`/`f64`, same as [`Scalar`] — each impl supplies its
/// own pair of `extern "C"` trampolines, since the SDK's registration call
/// takes a distinct typed function pointer slot per scalar type rather than
/// one generic callback.
pub trait PublishScalar: Scalar + Default + sealed_publish::Sealed {
    #[doc(hidden)]
    unsafe fn register(name: *const c_char, writable: bool, accessor: *mut c_void) -> XPLMDataRef;
}

mod sealed_publish {
    pub trait Sealed {}
    impl Sealed for i32 {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
}

unsafe extern "C" fn read_i32(refcon: *mut c_void) -> i32 {
    crate::guard(|| unsafe { ((&*(refcon as *const Accessor<i32>)).read)() }).unwrap_or_default()
}
unsafe extern "C" fn write_i32(refcon: *mut c_void, value: i32) {
    crate::guard(|| unsafe {
        if let Some(write) = (&*(refcon as *const Accessor<i32>))
            .write
            .borrow_mut()
            .as_mut()
        {
            write(value);
        }
    });
}

impl PublishScalar for i32 {
    unsafe fn register(name: *const c_char, writable: bool, accessor: *mut c_void) -> XPLMDataRef {
        unsafe {
            XPLMRegisterDataAccessor(
                name,
                xplmType_Int,
                writable as i32,
                Some(read_i32),
                writable.then_some(write_i32 as _),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                accessor,
                if writable {
                    accessor
                } else {
                    std::ptr::null_mut()
                },
            )
        }
    }
}

unsafe extern "C" fn read_f32(refcon: *mut c_void) -> f32 {
    crate::guard(|| unsafe { ((&*(refcon as *const Accessor<f32>)).read)() }).unwrap_or_default()
}
unsafe extern "C" fn write_f32(refcon: *mut c_void, value: f32) {
    crate::guard(|| unsafe {
        if let Some(write) = (&*(refcon as *const Accessor<f32>))
            .write
            .borrow_mut()
            .as_mut()
        {
            write(value);
        }
    });
}

impl PublishScalar for f32 {
    unsafe fn register(name: *const c_char, writable: bool, accessor: *mut c_void) -> XPLMDataRef {
        unsafe {
            XPLMRegisterDataAccessor(
                name,
                xplmType_Float,
                writable as i32,
                None,
                None,
                Some(read_f32),
                writable.then_some(write_f32 as _),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                accessor,
                if writable {
                    accessor
                } else {
                    std::ptr::null_mut()
                },
            )
        }
    }
}

unsafe extern "C" fn read_f64(refcon: *mut c_void) -> f64 {
    crate::guard(|| unsafe { ((&*(refcon as *const Accessor<f64>)).read)() }).unwrap_or_default()
}
unsafe extern "C" fn write_f64(refcon: *mut c_void, value: f64) {
    crate::guard(|| unsafe {
        if let Some(write) = (&*(refcon as *const Accessor<f64>))
            .write
            .borrow_mut()
            .as_mut()
        {
            write(value);
        }
    });
}

impl PublishScalar for f64 {
    unsafe fn register(name: *const c_char, writable: bool, accessor: *mut c_void) -> XPLMDataRef {
        unsafe {
            XPLMRegisterDataAccessor(
                name,
                xplmType_Double,
                writable as i32,
                None,
                None,
                None,
                None,
                Some(read_f64),
                writable.then_some(write_f64 as _),
                None,
                None,
                None,
                None,
                None,
                None,
                accessor,
                if writable {
                    accessor
                } else {
                    std::ptr::null_mut()
                },
            )
        }
    }
}

/// A dataref *published* by this plugin (`XPLMRegisterDataAccessor`) for
/// other plugins/scripts to read (and, for `ReadWrite`, write) — the
/// opposite direction from [`DataRef`], which only finds/uses datarefs
/// someone else published. RAII: registers on [`Self::publish`]/
/// [`Self::publish_writable`], unregisters (`XPLMUnregisterDataAccessor`) on
/// `Drop`.
pub struct PublishedDataRef<T: PublishScalar, A: Access> {
    raw: XPLMDataRef,
    // Boxed so the accessor's address is stable — the SDK holds a raw
    // pointer to it (as the refcon) for as long as this dataref stays
    // registered.
    _accessor: Box<Accessor<T>>,
    _type: PhantomData<T>,
    _access: PhantomData<A>,
}

// Same rationale as `DataRef`/`ArrayDataRef` above: X-Plane only ever calls
// the read/write trampolines from its single main thread, but plugin state
// holding one of these still needs to live in a `static Mutex<Option<_>>`,
// which requires `Send`.
unsafe impl<T: PublishScalar, A: Access> Send for PublishedDataRef<T, A> {}

impl<T: PublishScalar> PublishedDataRef<T, ReadOnlyMarker> {
    /// Publishes a read-only dataref at `name`, backed by `read` — called
    /// once per query from another plugin/script/dataref viewer. Returns
    /// `None` if `name` contains a NUL byte or `XPLMRegisterDataAccessor`
    /// itself fails.
    pub fn publish(name: &str, read: impl Fn() -> T + 'static) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut accessor = Box::new(Accessor {
            read: Box::new(read),
            write: RefCell::new(None),
        });
        let accessor_ptr: *mut c_void = &mut *accessor as *mut Accessor<T> as *mut c_void;
        let raw = unsafe { T::register(c_name.as_ptr(), false, accessor_ptr) };
        (!raw.is_null()).then_some(Self {
            raw,
            _accessor: accessor,
            _type: PhantomData,
            _access: PhantomData,
        })
    }
}

impl<T: PublishScalar> PublishedDataRef<T, ReadWriteMarker> {
    /// Publishes a read/write dataref at `name`, backed by `read`/`write` —
    /// `write` runs whenever another plugin/script sets the dataref's
    /// value. Returns `None` under the same conditions as [`Self::publish`].
    pub fn publish_writable(
        name: &str,
        read: impl Fn() -> T + 'static,
        write: impl FnMut(T) + 'static,
    ) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut accessor = Box::new(Accessor {
            read: Box::new(read),
            write: RefCell::new(Some(Box::new(write) as Box<dyn FnMut(T)>)),
        });
        let accessor_ptr: *mut c_void = &mut *accessor as *mut Accessor<T> as *mut c_void;
        let raw = unsafe { T::register(c_name.as_ptr(), true, accessor_ptr) };
        (!raw.is_null()).then_some(Self {
            raw,
            _accessor: accessor,
            _type: PhantomData,
            _access: PhantomData,
        })
    }
}

impl<T: PublishScalar, A: Access> Drop for PublishedDataRef<T, A> {
    fn drop(&mut self) {
        unsafe { XPLMUnregisterDataAccessor(self.raw) }
    }
}

/// `PublishedReadOnly<f64>` etc.
pub type PublishedReadOnly<T> = PublishedDataRef<T, ReadOnlyMarker>;
/// `PublishedReadWrite<f32>` etc.
pub type PublishedReadWrite<T> = PublishedDataRef<T, ReadWriteMarker>;

/// Backing state for [`PublishedData`] — same shape as [`Accessor`], but
/// `read` returns an owned byte string (its length isn't known up front the
/// way a scalar's is) and `write` receives a borrowed slice rather than `T`
/// by value, since copying an arbitrary-length buffer on every write would
/// be wasteful.
struct DataAccessor {
    read: Box<dyn Fn() -> Vec<u8>>,
    write: RefCell<Option<Box<dyn FnMut(&[u8])>>>,
}

/// Mirrors `XPLMGetDatab_f`'s pass-`NULL`-for-`outValue` convention
/// (returns the full length) and its offset/max-length copy semantics
/// otherwise — same contract [`ArrayDataRef::get`] follows for `i32`/`f32`.
unsafe extern "C" fn read_data(
    refcon: *mut c_void,
    out: *mut c_void,
    offset: i32,
    max_length: i32,
) -> i32 {
    crate::guard(|| unsafe {
        let bytes = ((&*(refcon as *const DataAccessor)).read)();
        if out.is_null() {
            return bytes.len() as i32;
        }
        let offset = offset.max(0) as usize;
        let Some(available) = bytes.get(offset..) else {
            return 0;
        };
        let n = available.len().min(max_length.max(0) as usize);
        std::ptr::copy_nonoverlapping(available.as_ptr(), out as *mut u8, n);
        n as i32
    })
    .unwrap_or(0)
}

/// Unlike [`read_data`], this ignores `offset` and always hands `write` the
/// entire `[value, value + length)` buffer as one slice — every known
/// `XPLMSetDatab_f` caller (including X-Plane's own dataref editor) writes
/// the whole byte string in one call at `offset` 0; a `write` closure that
/// needs true partial-write/offset support isn't served by this type.
unsafe extern "C" fn write_data(
    refcon: *mut c_void,
    value: *mut c_void,
    _offset: i32,
    length: i32,
) {
    crate::guard(|| unsafe {
        if let Some(write) = (&*(refcon as *const DataAccessor))
            .write
            .borrow_mut()
            .as_mut()
        {
            let length = length.max(0) as usize;
            let slice = std::slice::from_raw_parts(value as *const u8, length);
            write(slice);
        }
    });
}

fn register_data(name: *const c_char, writable: bool, accessor: *mut c_void) -> XPLMDataRef {
    unsafe {
        XPLMRegisterDataAccessor(
            name,
            xplmType_Data,
            writable as i32,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(read_data),
            writable.then_some(write_data as _),
            accessor,
            if writable {
                accessor
            } else {
                std::ptr::null_mut()
            },
        )
    }
}

/// A published byte-string dataref (`xplmType_Data` — X-Plane's convention
/// for variable-length data such as an ICAO string), the `xplmType_Data`
/// counterpart to [`PublishedDataRef`]'s scalar types. RAII: registers on
/// [`Self::publish`]/[`Self::publish_writable`], unregisters
/// (`XPLMUnregisterDataAccessor`) on `Drop`.
pub struct PublishedData<A: Access> {
    raw: XPLMDataRef,
    _accessor: Box<DataAccessor>,
    _access: PhantomData<A>,
}

// Same rationale as `PublishedDataRef`'s `unsafe impl Send` above.
unsafe impl<A: Access> Send for PublishedData<A> {}

impl PublishedData<ReadOnlyMarker> {
    /// Publishes a read-only byte-string dataref at `name`, backed by
    /// `read` — called once per query, returning the current bytes fresh
    /// each time (e.g. `some_string.into_bytes()`).
    pub fn publish(name: &str, read: impl Fn() -> Vec<u8> + 'static) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut accessor = Box::new(DataAccessor {
            read: Box::new(read),
            write: RefCell::new(None),
        });
        let accessor_ptr: *mut c_void = &mut *accessor as *mut DataAccessor as *mut c_void;
        let raw = register_data(c_name.as_ptr(), false, accessor_ptr);
        (!raw.is_null()).then_some(Self {
            raw,
            _accessor: accessor,
            _access: PhantomData,
        })
    }
}

impl PublishedData<ReadWriteMarker> {
    /// Publishes a read/write byte-string dataref at `name`. See
    /// [`write_data`]'s doc comment for `write`'s offset/whole-buffer
    /// contract.
    pub fn publish_writable(
        name: &str,
        read: impl Fn() -> Vec<u8> + 'static,
        write: impl FnMut(&[u8]) + 'static,
    ) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut accessor = Box::new(DataAccessor {
            read: Box::new(read),
            write: RefCell::new(Some(Box::new(write) as Box<dyn FnMut(&[u8])>)),
        });
        let accessor_ptr: *mut c_void = &mut *accessor as *mut DataAccessor as *mut c_void;
        let raw = register_data(c_name.as_ptr(), true, accessor_ptr);
        (!raw.is_null()).then_some(Self {
            raw,
            _accessor: accessor,
            _access: PhantomData,
        })
    }
}

impl<A: Access> Drop for PublishedData<A> {
    fn drop(&mut self) {
        unsafe { XPLMUnregisterDataAccessor(self.raw) }
    }
}

/// `PublishedData<ReadOnlyMarker>`.
pub type PublishedDataReadOnly = PublishedData<ReadOnlyMarker>;
/// `PublishedData<ReadWriteMarker>`.
pub type PublishedDataReadWrite = PublishedData<ReadWriteMarker>;

/// Marker for a type safe to publish/read as a fixed-size byte-string
/// dataref (`xplmType_Data`) via raw memory reinterpretation, through
/// [`DataStruct`]/[`PublishedStruct`] — implement it yourself
/// (`unsafe impl PackedDataRef for MyStruct {}`) only for a `#[repr(C)]` (or
/// `#[repr(C, packed)]`) type with:
/// - no padding bytes you care about being deterministic (they're read back
///   as whatever bytes happen to be there),
/// - no pointers/references/`Box`/`Vec`/anything with an invariant tied to a
///   specific address or allocation,
/// - no niches that make some bit patterns invalid anywhere in the layout
///   (no `bool`, `char`, fieldless enums with unreachable discriminants,
///   `NonZero*`, etc.) — an arbitrary byte pattern (whatever another plugin,
///   a Lua script, or the dataref editor happens to write) must be a valid
///   `Self`.
///
/// This mirrors `bytemuck::Pod`'s contract without pulling in that
/// dependency; reach for `bytemuck` directly instead if you already depend
/// on it.
///
/// # Safety
/// Implementing this for a type that doesn't meet the above is undefined
/// behavior the moment its bytes are read back as `Self` — [`DataStruct`]/
/// [`PublishedStruct`] have no way to check any of it, only that the byte
/// *length* matches `size_of::<Self>()`.
pub unsafe trait PackedDataRef: Copy {}

/// A handle to a byte-string (`xplmType_Data`) X-Plane dataref reinterpreted
/// as a fixed-size `#[repr(C)]` struct `T` (see [`PackedDataRef`]), found by
/// path — the consumer-side counterpart to [`PublishedStruct`], same as
/// [`DataBytes`] is to [`PublishedData`].
pub struct DataStruct<T: PackedDataRef, A: Access> {
    raw: XPLMDataRef,
    _type: PhantomData<T>,
    _access: PhantomData<A>,
}

// See `DataRef`'s identical `unsafe impl Send` above for the rationale.
unsafe impl<T: PackedDataRef, A: Access> Send for DataStruct<T, A> {}

impl<T: PackedDataRef, A: Access> DataStruct<T, A> {
    pub fn find(name: &str) -> Option<Self> {
        find_raw(name).map(|raw| Self {
            raw,
            _type: PhantomData,
            _access: PhantomData,
        })
    }

    /// Reads the dataref's current bytes and reinterprets them as `T`.
    /// Returns `None` if the dataref's current byte length doesn't match
    /// `size_of::<T>()` — e.g. it's the wrong dataref, or was last written
    /// by a different version of this packed struct.
    pub fn get(&self) -> Option<T> {
        let size = std::mem::size_of::<T>();
        let len = unsafe { XPLMGetDatab(self.raw, std::ptr::null_mut(), 0, 0) as usize };
        if len != size {
            return None;
        }
        let mut buf = vec![0u8; size];
        let n = unsafe {
            XPLMGetDatab(self.raw, buf.as_mut_ptr() as *mut c_void, 0, size as i32) as usize
        };
        if n != size {
            return None;
        }
        // SAFETY: `PackedDataRef`'s contract guarantees any byte pattern of
        // this length is a valid `T`; `read_unaligned` accounts for `buf`
        // not necessarily matching `T`'s alignment.
        Some(unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const T) })
    }

    /// Reads `out.len()` raw bytes starting at `byte_offset` — for reading
    /// a single field of `T` without transferring the whole struct. Pair
    /// with `std::mem::offset_of!(T, field)` for `byte_offset`. Returns the
    /// number of bytes actually copied (may be less than `out.len()` if
    /// `byte_offset + out.len()` runs past the end of `T`).
    pub fn get_bytes(&self, byte_offset: usize, out: &mut [u8]) -> usize {
        unsafe {
            XPLMGetDatab(
                self.raw,
                out.as_mut_ptr() as *mut c_void,
                byte_offset as i32,
                out.len() as i32,
            ) as usize
        }
    }

    /// [`Self::get_bytes`], reinterpreted as `F` — same `offset_of!`
    /// pairing, but for a specific field's type rather than raw bytes.
    /// Returns `None` if fewer than `size_of::<F>()` bytes were available at
    /// `byte_offset` (e.g. `byte_offset` runs past `T`'s end).
    ///
    /// `F` has no `PackedDataRef` bound — it's a sub-field's type (`i32`,
    /// `f32`, ...), not necessarily a type you'd publish/find as its own
    /// top-level packed-struct dataref. The same byte-pattern-validity
    /// requirement `PackedDataRef` documents still applies to `F` here;
    /// this method just doesn't require you to assert it via the trait for
    /// every field type you ever read this way.
    pub fn get_field<F: Copy>(&self, byte_offset: usize) -> Option<F> {
        let size = std::mem::size_of::<F>();
        let mut buf = vec![0u8; size];
        if self.get_bytes(byte_offset, &mut buf) != size {
            return None;
        }
        Some(unsafe { std::ptr::read_unaligned(buf.as_ptr() as *const F) })
    }
}

impl<T: PackedDataRef, A: Writable> DataStruct<T, A> {
    /// Writes `value`'s raw bytes as the dataref's whole byte string.
    pub fn set(&self, value: T) {
        self.set_bytes(0, unsafe {
            std::slice::from_raw_parts(&value as *const T as *const u8, std::mem::size_of::<T>())
        });
    }

    /// Writes `bytes` starting at `byte_offset` — for writing a single
    /// field of `T` without transferring the whole struct. Pair with
    /// `std::mem::offset_of!(T, field)` for `byte_offset`. Silently writes
    /// fewer than `bytes.len()` bytes if that would run past the end of
    /// `T` — same convention [`ArrayDataRef::set`] documents.
    pub fn set_bytes(&self, byte_offset: usize, bytes: &[u8]) {
        unsafe {
            XPLMSetDatab(
                self.raw,
                bytes.as_ptr() as *mut c_void,
                byte_offset as i32,
                bytes.len() as i32,
            )
        }
    }

    /// [`Self::set_bytes`], from a typed `value` instead of raw bytes — see
    /// [`Self::get_field`]'s doc comment for `F`'s requirements.
    pub fn set_field<F: Copy>(&self, byte_offset: usize, value: F) {
        self.set_bytes(byte_offset, unsafe {
            std::slice::from_raw_parts(&value as *const F as *const u8, std::mem::size_of::<F>())
        });
    }
}

pub type ReadOnlyStruct<T> = DataStruct<T, ReadOnlyMarker>;
pub type ReadWriteStruct<T> = DataStruct<T, ReadWriteMarker>;

/// Backing state for [`PublishedStruct`] — same shape as [`DataAccessor`],
/// but `read`/`write` exchange a `T` by value instead of a `Vec<u8>`/`&[u8]`,
/// since `T`'s size is fixed and known at compile time.
struct StructAccessor<T> {
    read: Box<dyn Fn() -> T>,
    write: RefCell<Option<Box<dyn FnMut(T)>>>,
}

/// Unlike [`read_data`], a fixed-size packed struct is always the whole
/// buffer — a short `max_length` (smaller than `size_of::<T>()`) can't
/// return a valid partial `T`, so this returns `0` rather than truncating.
unsafe extern "C" fn read_struct<T: PackedDataRef>(
    refcon: *mut c_void,
    out: *mut c_void,
    offset: i32,
    max_length: i32,
) -> i32 {
    crate::guard(|| unsafe {
        let value = ((&*(refcon as *const StructAccessor<T>)).read)();
        let size = std::mem::size_of::<T>() as i32;
        if out.is_null() {
            return size;
        }
        let offset = offset.max(0);
        if offset >= size {
            return 0;
        }
        let bytes = std::slice::from_raw_parts(&value as *const T as *const u8, size as usize);
        let avail = &bytes[offset as usize..];
        let n = avail.len().min(max_length.max(0) as usize);
        std::ptr::copy_nonoverlapping(avail.as_ptr(), out as *mut u8, n);
        n as i32
    })
    .unwrap_or(0)
}

/// Accepts a write at any in-bounds `(offset, length)`, not just the whole
/// buffer at once — a partial write is applied via read-current/patch/write
/// (calling the accessor's own `read` to get the rest of `T`, splicing in
/// the incoming bytes, then decoding and calling `write` with the merged
/// value), so another plugin/script can update a single field of a
/// published packed struct (see [`DataStruct::set_field`]'s pairing helper
/// on the consumer side) without clobbering the rest. An out-of-bounds
/// `(offset, length)` — reaching past `size_of::<T>()` — is rejected outright
/// rather than silently truncated.
unsafe extern "C" fn write_struct<T: PackedDataRef>(
    refcon: *mut c_void,
    value: *mut c_void,
    offset: i32,
    length: i32,
) {
    crate::guard(|| unsafe {
        let size = std::mem::size_of::<T>();
        let offset = offset.max(0) as usize;
        let length = length.max(0) as usize;
        if offset > size || length > size - offset {
            return;
        }
        let accessor = &*(refcon as *const StructAccessor<T>);
        if accessor.write.borrow().is_none() {
            return; // not writable — skip the read-merge for nothing.
        }
        let current = (accessor.read)();
        // SAFETY: see `DataStruct::get`'s identical justification for
        // reinterpreting `T`'s bytes both ways.
        let mut bytes =
            std::slice::from_raw_parts(&current as *const T as *const u8, size).to_vec();
        let incoming = std::slice::from_raw_parts(value as *const u8, length);
        bytes[offset..offset + length].copy_from_slice(incoming);
        let decoded = std::ptr::read_unaligned(bytes.as_ptr() as *const T);
        if let Some(write) = accessor.write.borrow_mut().as_mut() {
            write(decoded);
        }
    });
}

fn register_struct<T: PackedDataRef>(
    name: *const c_char,
    writable: bool,
    accessor: *mut c_void,
) -> XPLMDataRef {
    unsafe {
        XPLMRegisterDataAccessor(
            name,
            xplmType_Data,
            writable as i32,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(read_struct::<T>),
            if writable {
                Some(write_struct::<T>)
            } else {
                None
            },
            accessor,
            if writable {
                accessor
            } else {
                std::ptr::null_mut()
            },
        )
    }
}

/// A published byte-string dataref (`xplmType_Data`) reinterpreted as a
/// fixed-size `#[repr(C)]` struct `T` (see [`PackedDataRef`]) — the
/// `PackedDataRef` counterpart to [`PublishedData`]'s `Vec<u8>`. RAII:
/// registers on [`Self::publish`]/[`Self::publish_writable`], unregisters
/// (`XPLMUnregisterDataAccessor`) on `Drop`.
pub struct PublishedStruct<T: PackedDataRef, A: Access> {
    raw: XPLMDataRef,
    _accessor: Box<StructAccessor<T>>,
    _access: PhantomData<A>,
}

// Same rationale as `PublishedData`'s `unsafe impl Send` above.
unsafe impl<T: PackedDataRef, A: Access> Send for PublishedStruct<T, A> {}

impl<T: PackedDataRef> PublishedStruct<T, ReadOnlyMarker> {
    /// Publishes a read-only packed-struct dataref at `name`, backed by
    /// `read` — called once per query, returning the current value fresh
    /// each time.
    pub fn publish(name: &str, read: impl Fn() -> T + 'static) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut accessor = Box::new(StructAccessor {
            read: Box::new(read),
            write: RefCell::new(None),
        });
        let accessor_ptr: *mut c_void = &mut *accessor as *mut StructAccessor<T> as *mut c_void;
        let raw = register_struct::<T>(c_name.as_ptr(), false, accessor_ptr);
        (!raw.is_null()).then_some(Self {
            raw,
            _accessor: accessor,
            _access: PhantomData,
        })
    }
}

impl<T: PackedDataRef> PublishedStruct<T, ReadWriteMarker> {
    /// Publishes a read/write packed-struct dataref at `name`. `write` only
    /// runs for a whole-buffer, correctly-sized write — see
    /// [`write_struct`]'s doc comment.
    pub fn publish_writable(
        name: &str,
        read: impl Fn() -> T + 'static,
        write: impl FnMut(T) + 'static,
    ) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut accessor = Box::new(StructAccessor {
            read: Box::new(read),
            write: RefCell::new(Some(Box::new(write) as Box<dyn FnMut(T)>)),
        });
        let accessor_ptr: *mut c_void = &mut *accessor as *mut StructAccessor<T> as *mut c_void;
        let raw = register_struct::<T>(c_name.as_ptr(), true, accessor_ptr);
        (!raw.is_null()).then_some(Self {
            raw,
            _accessor: accessor,
            _access: PhantomData,
        })
    }
}

impl<T: PackedDataRef, A: Access> Drop for PublishedStruct<T, A> {
    fn drop(&mut self) {
        unsafe { XPLMUnregisterDataAccessor(self.raw) }
    }
}

/// `PublishedStruct<T, ReadOnlyMarker>`.
pub type PublishedStructReadOnly<T> = PublishedStruct<T, ReadOnlyMarker>;
/// `PublishedStruct<T, ReadWriteMarker>`.
pub type PublishedStructReadWrite<T> = PublishedStruct<T, ReadWriteMarker>;

/// An array element type [`PublishedArray`] can publish via
/// `XPLMRegisterDataAccessor` — sealed to `i32`/`f32`, the same two types
/// [`ArrayElement`] (its consumer-side counterpart) supports.
pub trait PublishArrayElement: ArrayElement + sealed_publish_array::Sealed {
    #[doc(hidden)]
    unsafe fn register(name: *const c_char, writable: bool, accessor: *mut c_void) -> XPLMDataRef;
}

mod sealed_publish_array {
    pub trait Sealed {}
    impl Sealed for i32 {}
    impl Sealed for f32 {}
}

/// Backing state for [`PublishedArray`] — `read` returns an owned `Vec<T>`
/// (the array's current length isn't fixed at compile time, unlike a
/// scalar's); `write` receives `(offset, &[T])`, mirroring
/// [`ArrayDataRef::set`]'s own signature, so a partial write from another
/// plugin/script can be applied at the right position rather than assumed
/// to always replace the whole array from index `0`.
struct ArrayAccessor<T> {
    read: Box<dyn Fn() -> Vec<T>>,
    write: RefCell<Option<Box<dyn FnMut(usize, &[T])>>>,
}

unsafe extern "C" fn read_array_i32(
    refcon: *mut c_void,
    out_values: *mut i32,
    offset: i32,
    max: i32,
) -> i32 {
    crate::guard(|| unsafe {
        let values = ((&*(refcon as *const ArrayAccessor<i32>)).read)();
        if out_values.is_null() {
            return values.len() as i32;
        }
        let offset = offset.max(0) as usize;
        let Some(avail) = values.get(offset..) else {
            return 0;
        };
        let n = avail.len().min(max.max(0) as usize);
        std::ptr::copy_nonoverlapping(avail.as_ptr(), out_values, n);
        n as i32
    })
    .unwrap_or(0)
}
unsafe extern "C" fn write_array_i32(
    refcon: *mut c_void,
    values: *mut i32,
    offset: i32,
    count: i32,
) {
    crate::guard(|| unsafe {
        if let Some(write) = (&*(refcon as *const ArrayAccessor<i32>))
            .write
            .borrow_mut()
            .as_mut()
        {
            let offset = offset.max(0) as usize;
            let count = count.max(0) as usize;
            let slice = std::slice::from_raw_parts(values, count);
            write(offset, slice);
        }
    });
}

impl PublishArrayElement for i32 {
    unsafe fn register(name: *const c_char, writable: bool, accessor: *mut c_void) -> XPLMDataRef {
        unsafe {
            XPLMRegisterDataAccessor(
                name,
                xplmType_IntArray,
                writable as i32,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(read_array_i32),
                writable.then_some(write_array_i32 as _),
                None,
                None,
                None,
                None,
                accessor,
                if writable {
                    accessor
                } else {
                    std::ptr::null_mut()
                },
            )
        }
    }
}

unsafe extern "C" fn read_array_f32(
    refcon: *mut c_void,
    out_values: *mut f32,
    offset: i32,
    max: i32,
) -> i32 {
    crate::guard(|| unsafe {
        let values = ((&*(refcon as *const ArrayAccessor<f32>)).read)();
        if out_values.is_null() {
            return values.len() as i32;
        }
        let offset = offset.max(0) as usize;
        let Some(avail) = values.get(offset..) else {
            return 0;
        };
        let n = avail.len().min(max.max(0) as usize);
        std::ptr::copy_nonoverlapping(avail.as_ptr(), out_values, n);
        n as i32
    })
    .unwrap_or(0)
}
unsafe extern "C" fn write_array_f32(
    refcon: *mut c_void,
    values: *mut f32,
    offset: i32,
    count: i32,
) {
    crate::guard(|| unsafe {
        if let Some(write) = (&*(refcon as *const ArrayAccessor<f32>))
            .write
            .borrow_mut()
            .as_mut()
        {
            let offset = offset.max(0) as usize;
            let count = count.max(0) as usize;
            let slice = std::slice::from_raw_parts(values, count);
            write(offset, slice);
        }
    });
}

impl PublishArrayElement for f32 {
    unsafe fn register(name: *const c_char, writable: bool, accessor: *mut c_void) -> XPLMDataRef {
        unsafe {
            XPLMRegisterDataAccessor(
                name,
                xplmType_FloatArray,
                writable as i32,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(read_array_f32),
                writable.then_some(write_array_f32 as _),
                None,
                None,
                accessor,
                if writable {
                    accessor
                } else {
                    std::ptr::null_mut()
                },
            )
        }
    }
}

/// A published array (`i32`/`f32`) dataref — the publish-side counterpart to
/// [`ArrayDataRef`]/[`ReadOnlyArray`]/[`ReadWriteArray`]. RAII: registers on
/// [`Self::publish`]/[`Self::publish_writable`], unregisters
/// (`XPLMUnregisterDataAccessor`) on `Drop`.
pub struct PublishedArray<T: PublishArrayElement, A: Access> {
    raw: XPLMDataRef,
    _accessor: Box<ArrayAccessor<T>>,
    _access: PhantomData<A>,
}

// Same rationale as `PublishedDataRef`'s `unsafe impl Send` above.
unsafe impl<T: PublishArrayElement, A: Access> Send for PublishedArray<T, A> {}

impl<T: PublishArrayElement> PublishedArray<T, ReadOnlyMarker> {
    /// Publishes a read-only array dataref at `name`, backed by `read` —
    /// called once per query, returning the current array fresh each time.
    pub fn publish(name: &str, read: impl Fn() -> Vec<T> + 'static) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut accessor = Box::new(ArrayAccessor {
            read: Box::new(read),
            write: RefCell::new(None),
        });
        let accessor_ptr: *mut c_void = &mut *accessor as *mut ArrayAccessor<T> as *mut c_void;
        let raw = unsafe { T::register(c_name.as_ptr(), false, accessor_ptr) };
        (!raw.is_null()).then_some(Self {
            raw,
            _accessor: accessor,
            _access: PhantomData,
        })
    }
}

impl<T: PublishArrayElement> PublishedArray<T, ReadWriteMarker> {
    /// Publishes a read/write array dataref at `name`. `write` receives
    /// `(offset, values)` for a partial write starting at `offset` — same
    /// shape as [`ArrayDataRef::set`]'s parameters.
    pub fn publish_writable(
        name: &str,
        read: impl Fn() -> Vec<T> + 'static,
        write: impl FnMut(usize, &[T]) + 'static,
    ) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let mut accessor = Box::new(ArrayAccessor {
            read: Box::new(read),
            write: RefCell::new(Some(Box::new(write) as Box<dyn FnMut(usize, &[T])>)),
        });
        let accessor_ptr: *mut c_void = &mut *accessor as *mut ArrayAccessor<T> as *mut c_void;
        let raw = unsafe { T::register(c_name.as_ptr(), true, accessor_ptr) };
        (!raw.is_null()).then_some(Self {
            raw,
            _accessor: accessor,
            _access: PhantomData,
        })
    }
}

impl<T: PublishArrayElement, A: Access> Drop for PublishedArray<T, A> {
    fn drop(&mut self) {
        unsafe { XPLMUnregisterDataAccessor(self.raw) }
    }
}

/// `PublishedArray<T, ReadOnlyMarker>`.
pub type PublishedArrayReadOnly<T> = PublishedArray<T, ReadOnlyMarker>;
/// `PublishedArray<T, ReadWriteMarker>`.
pub type PublishedArrayReadWrite<T> = PublishedArray<T, ReadWriteMarker>;
