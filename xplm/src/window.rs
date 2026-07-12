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
use std::ffi::c_void;
#[cfg(feature = "XPLM300")]
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use xplm_sys::{
    XPLMBringWindowToFront, XPLMCreateWindow_t, XPLMCreateWindowEx, XPLMCursorStatus,
    XPLMDestroyWindow, XPLMGetWindowGeometry, XPLMGetWindowIsVisible, XPLMHasKeyboardFocus,
    XPLMIsWindowInFront, XPLMKeyFlags, XPLMMouseStatus, XPLMSetWindowGeometry,
    XPLMSetWindowIsVisible, XPLMTakeKeyboardFocus, XPLMWindowID, xplm_CursorArrow,
    xplm_CursorCustom, xplm_CursorDefault, xplm_CursorHidden, xplm_MouseDown, xplm_MouseUp,
};

#[cfg(feature = "XPLM300")]
use xplm_sys::{XPLMSetWindowTitle, XPLMWindowLayer, xplm_WindowLayerFloatingWindows};

#[cfg(feature = "XPLM301")]
use xplm_sys::{XPLMWindowDecoration, xplm_WindowDecorationRoundRectangle};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
