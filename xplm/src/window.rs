//! Windows (`XPLMDisplay.h`'s "modern" window API, `XPLMCreateWindowEx`).
//!
//! The SDK requires all five core callbacks (draw, mouse click, key, cursor,
//! mouse wheel) to be non-null — "if you do not support the cursor or mouse
//! wheel, use functions that return the default values" — so `WindowBuilder`
//! defaults each to a no-op/pass-through closure rather than making every
//! caller supply all five, which would make `Window::builder()` an
//! eight-closure constructor for the common case of "just draw something."
//!
//! Unlike `Menu`, a window has no child-index-space to get stale (there's
//! nothing analogous to menu items here), so the identity-lookup pattern
//! from `xplm::menu` doesn't apply — this is a single boxed-closures-behind-
//! a-refcon trampoline set, same shape as `FlightLoop`.

use std::cell::RefCell;
use std::ffi::{c_void, CString};
use std::os::raw::{c_char, c_int};

use xplm_sys::{
    xplm_CursorArrow, xplm_CursorCustom, xplm_CursorDefault, xplm_CursorHidden, xplm_MouseDown,
    xplm_MouseUp, XPLMBringWindowToFront, XPLMCountHotKeys, XPLMCreateWindowEx, XPLMCreateWindow_t,
    XPLMCursorStatus, XPLMDestroyWindow, XPLMGetHotKeyInfo, XPLMGetNthHotKey,
    XPLMGetWindowGeometry, XPLMGetWindowIsVisible, XPLMHasKeyboardFocus, XPLMHotKeyID,
    XPLMIsWindowInFront, XPLMKeyFlags, XPLMMouseStatus, XPLMPluginID, XPLMRegisterHotKey,
    XPLMRegisterKeySniffer, XPLMSetHotKeyCombination, XPLMSetWindowGeometry,
    XPLMSetWindowIsVisible, XPLMTakeKeyboardFocus, XPLMUnregisterHotKey, XPLMUnregisterKeySniffer,
    XPLMWindowID,
};

#[cfg(feature = "XPLM300")]
use xplm_sys::{xplm_WindowLayerFloatingWindows, XPLMSetWindowTitle, XPLMWindowLayer};

#[cfg(feature = "XPLM301")]
use xplm_sys::{xplm_WindowDecorationRoundRectangle, XPLMWindowDecoration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseStatus {
    Down,
    Drag,
    Up,
}

impl From<XPLMMouseStatus> for MouseStatus {
    fn from(raw: XPLMMouseStatus) -> Self {
        if raw == xplm_MouseDown {
            MouseStatus::Down
        } else if raw == xplm_MouseUp {
            MouseStatus::Up
        } else {
            MouseStatus::Drag
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorStatus {
    /// Let X-Plane (or a lower window) manage the cursor.
    Default,
    Hidden,
    Arrow,
    /// Show the cursor but let the plugin pick an OS-specific cursor image.
    Custom,
}

impl From<CursorStatus> for XPLMCursorStatus {
    fn from(status: CursorStatus) -> Self {
        match status {
            CursorStatus::Default => xplm_CursorDefault,
            CursorStatus::Hidden => xplm_CursorHidden,
            CursorStatus::Arrow => xplm_CursorArrow,
            CursorStatus::Custom => xplm_CursorCustom,
        }
    }
}

/// `XPLMKeyFlags`' bits (`XPLMDefs.h`): which modifiers were held, and
/// whether this is a key-down or key-up event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyFlags(XPLMKeyFlags);

impl KeyFlags {
    pub fn shift(self) -> bool {
        self.0 & 1 != 0
    }
    pub fn option_alt(self) -> bool {
        self.0 & 2 != 0
    }
    pub fn control(self) -> bool {
        self.0 & 4 != 0
    }
    pub fn down(self) -> bool {
        self.0 & 8 != 0
    }
    pub fn up(self) -> bool {
        self.0 & 16 != 0
    }
}

#[cfg(feature = "XPLM301")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowDecoration {
    None,
    RoundRectangle,
    SelfDecorated,
    SelfDecoratedResizable,
}

#[cfg(feature = "XPLM301")]
impl From<WindowDecoration> for XPLMWindowDecoration {
    fn from(d: WindowDecoration) -> Self {
        match d {
            WindowDecoration::None => xplm_sys::xplm_WindowDecorationNone,
            WindowDecoration::RoundRectangle => xplm_WindowDecorationRoundRectangle,
            WindowDecoration::SelfDecorated => xplm_sys::xplm_WindowDecorationSelfDecorated,
            WindowDecoration::SelfDecoratedResizable => {
                xplm_sys::xplm_WindowDecorationSelfDecoratedResizable
            }
        }
    }
}

#[cfg(feature = "XPLM300")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowLayer {
    FlightOverlay,
    FloatingWindows,
    Modal,
    GrowlNotifications,
}

#[cfg(feature = "XPLM300")]
impl From<WindowLayer> for XPLMWindowLayer {
    fn from(l: WindowLayer) -> Self {
        match l {
            WindowLayer::FlightOverlay => xplm_sys::xplm_WindowLayerFlightOverlay,
            WindowLayer::FloatingWindows => xplm_WindowLayerFloatingWindows,
            WindowLayer::Modal => xplm_sys::xplm_WindowLayerModal,
            WindowLayer::GrowlNotifications => xplm_sys::xplm_WindowLayerGrowlNotifications,
        }
    }
}

/// A lightweight, `Copy`able reference to a window, passed into every
/// callback and returned by [`Window::handle`]. Exposes the non-callback
/// query/mutate API (geometry, visibility, title, focus, front-ness); does
/// not own the window (dropping it does nothing — use [`Window`] for that).
#[derive(Clone, Copy)]
pub struct WindowRef(XPLMWindowID);

impl WindowRef {
    #[cfg(feature = "widgets")]
    pub(crate) fn from_raw(id: XPLMWindowID) -> Self {
        Self(id)
    }

    /// `(left, top, right, bottom)` — boxels for a window created via
    /// [`WindowBuilder`], pixels for a legacy `XPLMCreateWindow` one (not
    /// exposed by this crate).
    pub fn geometry(&self) -> (i32, i32, i32, i32) {
        let (mut left, mut top, mut right, mut bottom) = (0, 0, 0, 0);
        unsafe { XPLMGetWindowGeometry(self.0, &mut left, &mut top, &mut right, &mut bottom) };
        (left, top, right, bottom)
    }

    pub fn set_geometry(&self, left: i32, top: i32, right: i32, bottom: i32) {
        unsafe { XPLMSetWindowGeometry(self.0, left, top, right, bottom) }
    }

    pub fn is_visible(&self) -> bool {
        unsafe { XPLMGetWindowIsVisible(self.0) != 0 }
    }

    pub fn set_visible(&self, visible: bool) {
        unsafe { XPLMSetWindowIsVisible(self.0, visible as c_int) }
    }

    /// Only takes effect for windows created with
    /// [`WindowDecoration::RoundRectangle`].
    #[cfg(feature = "XPLM300")]
    pub fn set_title(&self, title: &str) {
        let Ok(c_title) = CString::new(title) else {
            return;
        };
        unsafe { XPLMSetWindowTitle(self.0, c_title.as_ptr()) }
    }

    pub fn take_keyboard_focus(&self) {
        unsafe { XPLMTakeKeyboardFocus(self.0) }
    }

    pub fn has_keyboard_focus(&self) -> bool {
        unsafe { XPLMHasKeyboardFocus(self.0) != 0 }
    }

    pub fn bring_to_front(&self) {
        unsafe { XPLMBringWindowToFront(self.0) }
    }

    pub fn is_in_front(&self) -> bool {
        unsafe { XPLMIsWindowInFront(self.0) != 0 }
    }
}

type DrawFn = dyn FnMut(WindowRef) + 'static;
type MouseClickFn = dyn FnMut(WindowRef, i32, i32, MouseStatus) -> bool + 'static;
type KeyFn = dyn FnMut(WindowRef, char, KeyFlags, char, bool) + 'static;
type CursorFn = dyn FnMut(WindowRef, i32, i32) -> CursorStatus + 'static;
type MouseWheelFn = dyn FnMut(WindowRef, i32, i32, i32, i32) -> bool + 'static;

struct WindowState {
    draw: RefCell<Box<DrawFn>>,
    mouse_click: RefCell<Box<MouseClickFn>>,
    key: RefCell<Box<KeyFn>>,
    cursor: RefCell<Box<CursorFn>>,
    mouse_wheel: RefCell<Box<MouseWheelFn>>,
    #[cfg(feature = "XPLM300")]
    right_click: RefCell<Box<MouseClickFn>>,
}

/// A window your plugin created. Dropping it destroys the native window and
/// frees the callback closures.
pub struct Window {
    id: XPLMWindowID,
    state: *mut WindowState,
}

unsafe impl Send for Window {} // see FlightLoop's identical rationale: main-thread-only callbacks.

impl Window {
    pub fn builder(left: i32, top: i32, right: i32, bottom: i32) -> WindowBuilder {
        WindowBuilder::new(left, top, right, bottom)
    }

    pub fn handle(&self) -> WindowRef {
        WindowRef(self.id)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            XPLMDestroyWindow(self.id);
            drop(Box::from_raw(self.state));
        }
    }
}

/// Builds a [`Window`]. Every callback defaults to a no-op/pass-through
/// (draw: nothing; mouse click/wheel/right-click: don't consume the event;
/// cursor: `CursorStatus::Default`; key: ignore) so you only need to set the
/// ones you actually care about — the SDK itself forbids leaving any of the
/// five core callbacks null.
pub struct WindowBuilder {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    visible: bool,
    draw: Box<DrawFn>,
    mouse_click: Box<MouseClickFn>,
    key: Box<KeyFn>,
    cursor: Box<CursorFn>,
    mouse_wheel: Box<MouseWheelFn>,
    #[cfg(feature = "XPLM300")]
    right_click: Box<MouseClickFn>,
    #[cfg(feature = "XPLM301")]
    decoration: WindowDecoration,
    #[cfg(feature = "XPLM300")]
    layer: WindowLayer,
}

impl WindowBuilder {
    fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
            visible: true,
            draw: Box::new(|_| {}),
            mouse_click: Box::new(|_, _, _, _| false),
            key: Box::new(|_, _, _, _, _| {}),
            cursor: Box::new(|_, _, _| CursorStatus::Default),
            mouse_wheel: Box::new(|_, _, _, _, _| false),
            #[cfg(feature = "XPLM300")]
            right_click: Box::new(|_, _, _, _| false),
            #[cfg(feature = "XPLM301")]
            decoration: WindowDecoration::RoundRectangle,
            #[cfg(feature = "XPLM300")]
            layer: WindowLayer::FloatingWindows,
        }
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn on_draw(mut self, f: impl FnMut(WindowRef) + 'static) -> Self {
        self.draw = Box::new(f);
        self
    }

    /// Consumed clicks (return `true`) don't pass through to lower windows.
    /// WARNING (from the SDK): passing clicks through has known mouse
    /// tracking problems in X-Plane; avoid returning `false` in practice.
    pub fn on_mouse_click(
        mut self,
        f: impl FnMut(WindowRef, i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.mouse_click = Box::new(f);
        self
    }

    #[cfg(feature = "XPLM300")]
    pub fn on_right_click(
        mut self,
        f: impl FnMut(WindowRef, i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.right_click = Box::new(f);
        self
    }

    /// `f(window, key, flags, virtual_key, losing_focus)`.
    pub fn on_key(
        mut self,
        f: impl FnMut(WindowRef, char, KeyFlags, char, bool) + 'static,
    ) -> Self {
        self.key = Box::new(f);
        self
    }

    pub fn on_cursor(
        mut self,
        f: impl FnMut(WindowRef, i32, i32) -> CursorStatus + 'static,
    ) -> Self {
        self.cursor = Box::new(f);
        self
    }

    /// `f(window, x, y, wheel, clicks) -> consumed`.
    pub fn on_mouse_wheel(
        mut self,
        f: impl FnMut(WindowRef, i32, i32, i32, i32) -> bool + 'static,
    ) -> Self {
        self.mouse_wheel = Box::new(f);
        self
    }

    #[cfg(feature = "XPLM301")]
    pub fn decoration(mut self, decoration: WindowDecoration) -> Self {
        self.decoration = decoration;
        self
    }

    #[cfg(feature = "XPLM300")]
    pub fn layer(mut self, layer: WindowLayer) -> Self {
        self.layer = layer;
        self
    }

    pub fn build(self) -> Option<Window> {
        let state = Box::into_raw(Box::new(WindowState {
            draw: RefCell::new(self.draw),
            mouse_click: RefCell::new(self.mouse_click),
            key: RefCell::new(self.key),
            cursor: RefCell::new(self.cursor),
            mouse_wheel: RefCell::new(self.mouse_wheel),
            #[cfg(feature = "XPLM300")]
            right_click: RefCell::new(self.right_click),
        }));

        let mut params = XPLMCreateWindow_t {
            structSize: std::mem::size_of::<XPLMCreateWindow_t>() as c_int,
            left: self.left,
            top: self.top,
            right: self.right,
            bottom: self.bottom,
            visible: self.visible as c_int,
            drawWindowFunc: Some(draw_trampoline),
            handleMouseClickFunc: Some(mouse_click_trampoline),
            handleKeyFunc: Some(key_trampoline),
            handleCursorFunc: Some(cursor_trampoline),
            handleMouseWheelFunc: Some(mouse_wheel_trampoline),
            refcon: state as *mut c_void,
            #[cfg(feature = "XPLM301")]
            decorateAsFloatingWindow: self.decoration.into(),
            #[cfg(feature = "XPLM300")]
            layer: self.layer.into(),
            #[cfg(feature = "XPLM300")]
            handleRightClickFunc: Some(right_click_trampoline),
        };

        let id = unsafe { XPLMCreateWindowEx(&mut params) };
        if id.is_null() {
            unsafe { drop(Box::from_raw(state)) };
            return None;
        }
        Some(Window { id, state })
    }
}

unsafe extern "C" fn draw_trampoline(window_id: XPLMWindowID, refcon: *mut c_void) {
    crate::guard(|| {
        let state: &WindowState = unsafe { &*(refcon as *const WindowState) };
        (state.draw.borrow_mut())(WindowRef(window_id));
    });
}

unsafe extern "C" fn mouse_click_trampoline(
    window_id: XPLMWindowID,
    x: c_int,
    y: c_int,
    mouse: XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &WindowState = unsafe { &*(refcon as *const WindowState) };
        (state.mouse_click.borrow_mut())(WindowRef(window_id), x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

#[cfg(feature = "XPLM300")]
unsafe extern "C" fn right_click_trampoline(
    window_id: XPLMWindowID,
    x: c_int,
    y: c_int,
    mouse: XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &WindowState = unsafe { &*(refcon as *const WindowState) };
        (state.right_click.borrow_mut())(WindowRef(window_id), x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn key_trampoline(
    window_id: XPLMWindowID,
    key: c_char,
    flags: XPLMKeyFlags,
    virtual_key: c_char,
    refcon: *mut c_void,
    losing_focus: c_int,
) {
    crate::guard(|| {
        let state: &WindowState = unsafe { &*(refcon as *const WindowState) };
        (state.key.borrow_mut())(
            WindowRef(window_id),
            key as u8 as char,
            KeyFlags(flags),
            virtual_key as u8 as char,
            losing_focus != 0,
        );
    });
}

unsafe extern "C" fn cursor_trampoline(
    window_id: XPLMWindowID,
    x: c_int,
    y: c_int,
    refcon: *mut c_void,
) -> XPLMCursorStatus {
    crate::guard(|| {
        let state: &WindowState = unsafe { &*(refcon as *const WindowState) };
        (state.cursor.borrow_mut())(WindowRef(window_id), x, y)
    })
    .unwrap_or(CursorStatus::Default)
    .into()
}

unsafe extern "C" fn mouse_wheel_trampoline(
    window_id: XPLMWindowID,
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &WindowState = unsafe { &*(refcon as *const WindowState) };
        (state.mouse_wheel.borrow_mut())(WindowRef(window_id), x, y, wheel, clicks)
    })
    .unwrap_or(false) as c_int
}

type KeySnifferFn = dyn FnMut(char, KeyFlags, char) -> bool + 'static;

/// A registered key sniffer (`XPLMRegisterKeySniffer`). Sees every keystroke
/// while alive, before or after the window system depending on how it was
/// registered; return `true` from the callback to let the key continue
/// downstream, `false` to consume it. Dropping this unregisters it.
pub struct KeySniffer {
    before_windows: bool,
    state: *mut RefCell<Box<KeySnifferFn>>,
}

unsafe impl Send for KeySniffer {} // see FlightLoop's identical rationale: main-thread-only callbacks.

/// Registers `callback` to see every keystroke. `before_windows` matches the
/// SDK's `inBeforeWindows`: sniff before the window system gets the key, or
/// after. Returns `None` if the SDK refuses the registration.
pub fn register_key_sniffer(
    before_windows: bool,
    callback: impl FnMut(char, KeyFlags, char) -> bool + 'static,
) -> Option<KeySniffer> {
    let state = Box::into_raw(Box::new(RefCell::new(
        Box::new(callback) as Box<KeySnifferFn>
    )));
    let ok = unsafe {
        XPLMRegisterKeySniffer(
            Some(key_sniffer_trampoline),
            before_windows as c_int,
            state as *mut c_void,
        )
    };
    if ok == 0 {
        unsafe { drop(Box::from_raw(state)) };
        return None;
    }
    Some(KeySniffer {
        before_windows,
        state,
    })
}

impl Drop for KeySniffer {
    fn drop(&mut self) {
        unsafe {
            XPLMUnregisterKeySniffer(
                Some(key_sniffer_trampoline),
                self.before_windows as c_int,
                self.state as *mut c_void,
            );
            drop(Box::from_raw(self.state));
        }
    }
}

unsafe extern "C" fn key_sniffer_trampoline(
    in_char: c_char,
    in_flags: XPLMKeyFlags,
    in_virtual_key: c_char,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &RefCell<Box<KeySnifferFn>> =
            unsafe { &*(refcon as *const RefCell<Box<KeySnifferFn>>) };
        (state.borrow_mut())(
            in_char as u8 as char,
            KeyFlags(in_flags),
            in_virtual_key as u8 as char,
        )
    })
    .unwrap_or(true) as c_int
}

/// A hot key your plugin registered (`XPLMRegisterHotKey`). Dropping this
/// unregisters it — the SDK only lets a plugin unregister its own hot keys,
/// unlike [`HotKeyId::set_combination`], which can remap any plugin's.
pub struct HotKey {
    id: XPLMHotKeyID,
    state: *mut RefCell<Box<dyn FnMut() + 'static>>,
}

unsafe impl Send for HotKey {} // see FlightLoop's identical rationale: main-thread-only callbacks.

impl HotKey {
    /// A stable, `Copy`able reference to this hot key, usable with the same
    /// query/mutate API every other plugin's hot keys are exposed through
    /// ([`hot_key_count`]/[`nth_hot_key`]).
    pub fn id(&self) -> HotKeyId {
        HotKeyId(self.id)
    }
}

/// Registers a hot key: `virtual_key`/`flags` is the key combination (see
/// `XPLMDefs.h`'s `XPLM_VK_*` constants for `virtual_key`), `description`
/// shows up in X-Plane's key-binding UI. Returns `None` if `description`
/// contains an interior NUL.
pub fn register_hot_key(
    virtual_key: char,
    flags: KeyFlags,
    description: &str,
    callback: impl FnMut() + 'static,
) -> Option<HotKey> {
    let c_description = CString::new(description).ok()?;
    let state = Box::into_raw(Box::new(RefCell::new(
        Box::new(callback) as Box<dyn FnMut() + 'static>
    )));
    let id = unsafe {
        XPLMRegisterHotKey(
            virtual_key as u8 as c_char,
            flags.0,
            c_description.as_ptr(),
            Some(hot_key_trampoline),
            state as *mut c_void,
        )
    };
    if id.is_null() {
        unsafe { drop(Box::from_raw(state)) };
        return None;
    }
    Some(HotKey { id, state })
}

impl Drop for HotKey {
    fn drop(&mut self) {
        unsafe {
            XPLMUnregisterHotKey(self.id);
            drop(Box::from_raw(self.state));
        }
    }
}

unsafe extern "C" fn hot_key_trampoline(refcon: *mut c_void) {
    crate::guard(|| {
        let state: &RefCell<Box<dyn FnMut() + 'static>> =
            unsafe { &*(refcon as *const RefCell<Box<dyn FnMut() + 'static>>) };
        (state.borrow_mut())();
    });
}

/// A stable handle to a registered hot key (`XPLMHotKeyID`) — possibly one
/// registered by another plugin. Unlike [`crate::menu::MenuItem`], this
/// isn't an index into a reindexed list, so it's safe to hold onto; only
/// [`hot_key_count`]/[`nth_hot_key`]'s *enumeration order* shifts as hot keys
/// are (un)registered, which is why this crate doesn't cache a position for
/// one either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HotKeyId(XPLMHotKeyID);

pub struct HotKeyInfo {
    pub virtual_key: char,
    pub flags: KeyFlags,
    pub description: String,
    pub plugin: XPLMPluginID,
}

impl HotKeyId {
    pub fn info(&self) -> HotKeyInfo {
        let (mut virtual_key, mut flags, mut plugin): (c_char, XPLMKeyFlags, XPLMPluginID) =
            (0, 0, 0);
        let mut description = [0u8; 512];
        unsafe {
            XPLMGetHotKeyInfo(
                self.0,
                &mut virtual_key,
                &mut flags,
                description.as_mut_ptr() as *mut c_char,
                &mut plugin,
            );
        }
        let end = description
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(description.len());
        HotKeyInfo {
            virtual_key: virtual_key as u8 as char,
            flags: KeyFlags(flags),
            description: String::from_utf8_lossy(&description[..end]).into_owned(),
            plugin,
        }
    }

    /// Remaps this hot key's combination — the SDK allows this for *any*
    /// plugin's hot key, not just your own, so a caller can offer a
    /// key-rebinding UI.
    pub fn set_combination(&self, virtual_key: char, flags: KeyFlags) {
        unsafe { XPLMSetHotKeyCombination(self.0, virtual_key as u8 as c_char, flags.0) }
    }
}

/// The number of hot keys currently registered, across every plugin.
pub fn hot_key_count() -> i32 {
    unsafe { XPLMCountHotKeys() }
}

/// The hot key at `index` (`0..hot_key_count()`) in the current enumeration
/// order — re-derive this on every call rather than caching it, since
/// (un)registering any hot key (by any plugin) shifts the positions after
/// it.
pub fn nth_hot_key(index: i32) -> Option<HotKeyId> {
    let id = unsafe { XPLMGetNthHotKey(index) };
    (!id.is_null()).then_some(HotKeyId(id))
}
