//! Widgets (`SDK/CHeaders/Widgets` — `XPWidgets.h`/`XPWidgetDefs.h`). A
//! lighter-weight, immediate-mode-ish UI toolkit built on top of
//! `xplm::window::Window`, with its own tree of `XPWidgetID`s.
//!
//! Unlike `xplm::menu::MenuItem` (this crate's template for index-addressed,
//! X-Plane-managed lists — see the standing invariant in `CLAUDE.md`), every
//! widget API here — `XPDestroyWidget`, `XPSetWidgetProperty`,
//! `XPPlaceWidgetWithin`, etc. — addresses a widget by its raw `XPWidgetID`
//! pointer directly, which stays valid for the widget's whole lifetime.
//! Index-addressing only shows up in *enumeration*
//! (`XPGetNthChildWidget`/`XPCountChildWidgets`), and [`WidgetRef::children`]
//! re-derives that on every call rather than caching a position — so there's
//! no cached-index hazard to guard against for the handles themselves, only
//! for anyone iterating children while mutating the tree concurrently (the
//! same hazard as mutating any collection mid-iteration, not specific to
//! this SDK).
//!
//! Standard widget classes' own per-class property/message IDs
//! (`XPStandardWidgets.h`) and `XPUIGraphics.h`'s native-look-and-feel
//! drawing helpers aren't wrapped yet — [`WidgetPropertyId::new`] and
//! [`WidgetMessage::Other`] let a caller reach them by raw ID in the
//! meantime.

use std::cell::RefCell;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use xplm_sys::{
    xpMsg_AcceptChild, xpMsg_AcceptParent, xpMsg_Create, xpMsg_CursorAdjust,
    xpMsg_DescriptorChanged, xpMsg_Destroy, xpMsg_Draw, xpMsg_ExposedChanged, xpMsg_Hidden,
    xpMsg_KeyLoseFocus, xpMsg_KeyPress, xpMsg_KeyTakeFocus, xpMsg_LoseChild, xpMsg_MouseDown,
    xpMsg_MouseDrag, xpMsg_MouseUp, xpMsg_MouseWheel, xpMsg_None as xp_msg_none, xpMsg_Paint,
    xpMsg_PropertyChanged, xpMsg_Reshape, xpMsg_Shown, xpProperty_Clip, xpProperty_DragXOff,
    xpProperty_DragYOff, xpProperty_Dragging, xpProperty_Enabled, xpProperty_Hilited,
    xpProperty_Object, xpProperty_Refcon, xpWidgetClass_Button, xpWidgetClass_Caption,
    xpWidgetClass_GeneralGraphics, xpWidgetClass_MainWindow, xpWidgetClass_Progress,
    xpWidgetClass_ScrollBar, xpWidgetClass_SubWindow, xpWidgetClass_TextField, XPAddWidgetCallback,
    XPBringRootWidgetToFront, XPCountChildWidgets, XPCreateCustomWidget, XPCreateWidget,
    XPDestroyWidget, XPDispatchMode, XPFindRootWidget, XPGetNthChildWidget, XPGetParentWidget,
    XPGetWidgetDescriptor, XPGetWidgetForLocation, XPGetWidgetGeometry, XPGetWidgetProperty,
    XPGetWidgetUnderlyingWindow, XPGetWidgetWithFocus, XPHideWidget, XPIsWidgetInFront,
    XPIsWidgetVisible, XPLoseKeyboardFocus, XPPlaceWidgetWithin, XPSendMessageToWidget,
    XPSetKeyboardFocus, XPSetWidgetDescriptor, XPSetWidgetGeometry, XPSetWidgetProperty,
    XPShowWidget, XPWidgetClass, XPWidgetID, XPWidgetMessage, XPWidgetPropertyID,
};

use crate::window::WindowRef;

/// A built-in widget class (`XPWidgetClass`) for [`create_widget`]. Custom
/// behavior instead goes through [`create_custom_widget`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetClass {
    MainWindow,
    SubWindow,
    Button,
    TextField,
    ScrollBar,
    Caption,
    GeneralGraphics,
    Progress,
}

impl From<WidgetClass> for XPWidgetClass {
    fn from(c: WidgetClass) -> Self {
        (match c {
            WidgetClass::MainWindow => xpWidgetClass_MainWindow,
            WidgetClass::SubWindow => xpWidgetClass_SubWindow,
            WidgetClass::Button => xpWidgetClass_Button,
            WidgetClass::TextField => xpWidgetClass_TextField,
            WidgetClass::ScrollBar => xpWidgetClass_ScrollBar,
            WidgetClass::Caption => xpWidgetClass_Caption,
            WidgetClass::GeneralGraphics => xpWidgetClass_GeneralGraphics,
            WidgetClass::Progress => xpWidgetClass_Progress,
        }) as XPWidgetClass
    }
}

/// How far a [`WidgetRef::send_message`] call propagates (`XPDispatchMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchMode {
    /// Only the target widget's own callback(s).
    Direct,
    /// The target, then its parent, grandparent, etc. until one handles it.
    UpChain,
    /// The target and every descendant, depth-first.
    Recursive,
    /// Like `Recursive`, but every callback on every widget runs regardless
    /// of whether an earlier one reported the message as handled.
    DirectAllCallbacks,
    /// Only the single most-recently-added callback on the target widget.
    Once,
}

impl From<DispatchMode> for XPDispatchMode {
    fn from(m: DispatchMode) -> Self {
        (match m {
            DispatchMode::Direct => xplm_sys::xpMode_Direct,
            DispatchMode::UpChain => xplm_sys::xpMode_UpChain,
            DispatchMode::Recursive => xplm_sys::xpMode_Recursive,
            DispatchMode::DirectAllCallbacks => xplm_sys::xpMode_DirectAllCallbacks,
            DispatchMode::Once => xplm_sys::xpMode_Once,
        }) as XPDispatchMode
    }
}

/// A widget message (`XPWidgetMessage`) — passed to a custom widget's
/// callback, and to [`WidgetRef::send_message`]. `Other` carries any
/// message this crate doesn't have a variant for yet, including standard
/// widget classes' own extended messages (`XPStandardWidgets.h`) and
/// `xpMsg_UserStart`-based plugin-defined ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetMessage {
    None,
    Create,
    Destroy,
    Paint,
    Draw,
    KeyPress,
    KeyTakeFocus,
    KeyLoseFocus,
    MouseDown,
    MouseDrag,
    MouseUp,
    Reshape,
    ExposedChanged,
    AcceptChild,
    LoseChild,
    AcceptParent,
    Shown,
    Hidden,
    DescriptorChanged,
    PropertyChanged,
    MouseWheel,
    CursorAdjust,
    Other(XPWidgetMessage),
}

impl From<XPWidgetMessage> for WidgetMessage {
    fn from(raw: XPWidgetMessage) -> Self {
        match raw {
            v if v == xp_msg_none => WidgetMessage::None,
            v if v == xpMsg_Create => WidgetMessage::Create,
            v if v == xpMsg_Destroy => WidgetMessage::Destroy,
            v if v == xpMsg_Paint => WidgetMessage::Paint,
            v if v == xpMsg_Draw => WidgetMessage::Draw,
            v if v == xpMsg_KeyPress => WidgetMessage::KeyPress,
            v if v == xpMsg_KeyTakeFocus => WidgetMessage::KeyTakeFocus,
            v if v == xpMsg_KeyLoseFocus => WidgetMessage::KeyLoseFocus,
            v if v == xpMsg_MouseDown => WidgetMessage::MouseDown,
            v if v == xpMsg_MouseDrag => WidgetMessage::MouseDrag,
            v if v == xpMsg_MouseUp => WidgetMessage::MouseUp,
            v if v == xpMsg_Reshape => WidgetMessage::Reshape,
            v if v == xpMsg_ExposedChanged => WidgetMessage::ExposedChanged,
            v if v == xpMsg_AcceptChild => WidgetMessage::AcceptChild,
            v if v == xpMsg_LoseChild => WidgetMessage::LoseChild,
            v if v == xpMsg_AcceptParent => WidgetMessage::AcceptParent,
            v if v == xpMsg_Shown => WidgetMessage::Shown,
            v if v == xpMsg_Hidden => WidgetMessage::Hidden,
            v if v == xpMsg_DescriptorChanged => WidgetMessage::DescriptorChanged,
            v if v == xpMsg_PropertyChanged => WidgetMessage::PropertyChanged,
            v if v == xpMsg_MouseWheel => WidgetMessage::MouseWheel,
            v if v == xpMsg_CursorAdjust => WidgetMessage::CursorAdjust,
            _ => WidgetMessage::Other(raw),
        }
    }
}

impl From<WidgetMessage> for XPWidgetMessage {
    fn from(m: WidgetMessage) -> Self {
        (match m {
            WidgetMessage::None => xp_msg_none,
            WidgetMessage::Create => xpMsg_Create,
            WidgetMessage::Destroy => xpMsg_Destroy,
            WidgetMessage::Paint => xpMsg_Paint,
            WidgetMessage::Draw => xpMsg_Draw,
            WidgetMessage::KeyPress => xpMsg_KeyPress,
            WidgetMessage::KeyTakeFocus => xpMsg_KeyTakeFocus,
            WidgetMessage::KeyLoseFocus => xpMsg_KeyLoseFocus,
            WidgetMessage::MouseDown => xpMsg_MouseDown,
            WidgetMessage::MouseDrag => xpMsg_MouseDrag,
            WidgetMessage::MouseUp => xpMsg_MouseUp,
            WidgetMessage::Reshape => xpMsg_Reshape,
            WidgetMessage::ExposedChanged => xpMsg_ExposedChanged,
            WidgetMessage::AcceptChild => xpMsg_AcceptChild,
            WidgetMessage::LoseChild => xpMsg_LoseChild,
            WidgetMessage::AcceptParent => xpMsg_AcceptParent,
            WidgetMessage::Shown => xpMsg_Shown,
            WidgetMessage::Hidden => xpMsg_Hidden,
            WidgetMessage::DescriptorChanged => xpMsg_DescriptorChanged,
            WidgetMessage::PropertyChanged => xpMsg_PropertyChanged,
            WidgetMessage::MouseWheel => xpMsg_MouseWheel,
            WidgetMessage::CursorAdjust => xpMsg_CursorAdjust,
            WidgetMessage::Other(raw) => return raw,
        }) as XPWidgetMessage
    }
}

/// A widget property ID (`XPWidgetPropertyID`). The base IDs every widget
/// supports are associated consts; standard widget classes' own IDs
/// (`XPStandardWidgets.h`) or plugin-defined ones (`xpProperty_UserStart`
/// and up) can be reached via [`WidgetPropertyId::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetPropertyId(XPWidgetPropertyID);

impl WidgetPropertyId {
    pub const DRAGGING: Self = Self(xpProperty_Dragging as XPWidgetPropertyID);
    pub const DRAG_X_OFF: Self = Self(xpProperty_DragXOff as XPWidgetPropertyID);
    pub const DRAG_Y_OFF: Self = Self(xpProperty_DragYOff as XPWidgetPropertyID);
    pub const HILITED: Self = Self(xpProperty_Hilited as XPWidgetPropertyID);
    pub const OBJECT: Self = Self(xpProperty_Object as XPWidgetPropertyID);
    pub const CLIP: Self = Self(xpProperty_Clip as XPWidgetPropertyID);
    pub const ENABLED: Self = Self(xpProperty_Enabled as XPWidgetPropertyID);

    /// `xpProperty_Refcon` is reserved by this crate to store a custom
    /// widget's callback closure — reading or writing it directly would
    /// corrupt that, so it's deliberately not exposed as a named const here.
    pub fn new(raw: XPWidgetPropertyID) -> Self {
        Self(raw)
    }
}

const REFCON_PROPERTY: XPWidgetPropertyID = xpProperty_Refcon as XPWidgetPropertyID;

/// A lightweight, `Copy`able reference to a widget, passed into every custom
/// callback and returned by [`Widget::handle`]/tree-walking functions.
/// Exposes the non-owning query/mutate API; does not own the widget
/// (dropping it does nothing — use [`Widget`] for that, or destroy an
/// arbitrary widget by dropping the `Widget` that created it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetRef(XPWidgetID);

impl WidgetRef {
    /// `(left, top, right, bottom)`.
    pub fn geometry(&self) -> (i32, i32, i32, i32) {
        let (mut left, mut top, mut right, mut bottom) = (0, 0, 0, 0);
        unsafe { XPGetWidgetGeometry(self.0, &mut left, &mut top, &mut right, &mut bottom) };
        (left, top, right, bottom)
    }

    pub fn set_geometry(&self, left: i32, top: i32, right: i32, bottom: i32) {
        unsafe { XPSetWidgetGeometry(self.0, left, top, right, bottom) }
    }

    pub fn is_visible(&self) -> bool {
        unsafe { XPIsWidgetVisible(self.0) != 0 }
    }

    pub fn show(&self) {
        unsafe { XPShowWidget(self.0) }
    }

    pub fn hide(&self) {
        unsafe { XPHideWidget(self.0) }
    }

    /// Reads up to 255 bytes of this widget's descriptor (its title/caption/
    /// text, depending on class) — truncated if longer.
    pub fn descriptor(&self) -> String {
        let mut buf = [0u8; 256];
        unsafe {
            XPGetWidgetDescriptor(self.0, buf.as_mut_ptr() as *mut c_char, buf.len() as c_int)
        };
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..end]).into_owned()
    }

    /// Does nothing if `descriptor` contains an interior NUL.
    pub fn set_descriptor(&self, descriptor: &str) {
        let Ok(c_descriptor) = CString::new(descriptor) else {
            return;
        };
        unsafe { XPSetWidgetDescriptor(self.0, c_descriptor.as_ptr()) }
    }

    /// `None` if `property` has never been set on this widget.
    pub fn property(&self, property: WidgetPropertyId) -> Option<isize> {
        let mut exists: c_int = 0;
        let value = unsafe { XPGetWidgetProperty(self.0, property.0, &mut exists) };
        (exists != 0).then_some(value)
    }

    pub fn set_property(&self, property: WidgetPropertyId, value: isize) {
        unsafe { XPSetWidgetProperty(self.0, property.0, value) }
    }

    /// The parent this widget was last placed within
    /// ([`create_widget`]/[`create_custom_widget`]'s `container`, or a later
    /// [`Self::place_within`]) — `None` if it has none.
    pub fn parent(&self) -> Option<WidgetRef> {
        let id = unsafe { XPGetParentWidget(self.0) };
        (!id.is_null()).then_some(WidgetRef(id))
    }

    /// Moves this widget into `container` (or detaches it, if `None`),
    /// appending it after `container`'s existing children — the SDK has no
    /// "insert at position" call, only add-to-end.
    pub fn place_within(&self, container: Option<WidgetRef>) {
        unsafe { XPPlaceWidgetWithin(self.0, container.map_or(std::ptr::null_mut(), |c| c.0)) }
    }

    /// How many children this widget currently has — re-fetch this (and
    /// [`Self::nth_child`]) rather than caching either across calls that
    /// might add/remove/reorder children.
    pub fn child_count(&self) -> i32 {
        unsafe { XPCountChildWidgets(self.0) }
    }

    /// The child at `index` (`0..child_count()`) in the current order.
    pub fn nth_child(&self, index: i32) -> Option<WidgetRef> {
        let id = unsafe { XPGetNthChildWidget(self.0, index) };
        (!id.is_null()).then_some(WidgetRef(id))
    }

    /// Iterates this widget's current children. Don't mutate the tree while
    /// iterating — same caveat as mutating any collection mid-iteration, not
    /// specific to this SDK.
    pub fn children(&self) -> impl Iterator<Item = WidgetRef> + '_ {
        (0..self.child_count()).filter_map(move |i| self.nth_child(i))
    }

    /// Walks up to this widget's root (the widget created with
    /// `is_root = true` that this one descends from).
    pub fn find_root(&self) -> WidgetRef {
        WidgetRef(unsafe { XPFindRootWidget(self.0) })
    }

    /// Brings this widget's whole root window in front of other root
    /// windows (only meaningful for a root widget/one of its descendants).
    pub fn bring_root_to_front(&self) {
        unsafe { XPBringRootWidgetToFront(self.0) }
    }

    pub fn is_in_front(&self) -> bool {
        unsafe { XPIsWidgetInFront(self.0) != 0 }
    }

    /// The `xplm::window::Window` this widget is ultimately drawn within.
    pub fn underlying_window(&self) -> WindowRef {
        WindowRef::from_raw(unsafe { XPGetWidgetUnderlyingWindow(self.0) })
    }

    /// Requests keyboard focus; returns the widget that actually ended up
    /// with it (a parent may claim it instead), or `None` if focus went to
    /// X-Plane itself.
    pub fn set_keyboard_focus(&self) -> Option<WidgetRef> {
        let id = unsafe { XPSetKeyboardFocus(self.0) };
        (!id.is_null()).then_some(WidgetRef(id))
    }

    pub fn lose_keyboard_focus(&self) {
        unsafe { XPLoseKeyboardFocus(self.0) }
    }

    /// Sends `message` to this widget via `mode`; `true` if some callback
    /// reported it as handled.
    pub fn send_message(
        &self,
        message: WidgetMessage,
        mode: DispatchMode,
        param1: isize,
        param2: isize,
    ) -> bool {
        unsafe { XPSendMessageToWidget(self.0, message.into(), mode.into(), param1, param2) != 0 }
    }
}

/// The widget currently holding keyboard focus, if any (`None` means
/// X-Plane itself has it).
pub fn widget_with_focus() -> Option<WidgetRef> {
    let id = unsafe { XPGetWidgetWithFocus() };
    (!id.is_null()).then_some(WidgetRef(id))
}

/// The topmost (or, if `recursive`, deepest) widget under `(x, y)` within
/// `container`; `visible_only` skips hidden widgets.
pub fn widget_for_location(
    container: WidgetRef,
    x: i32,
    y: i32,
    recursive: bool,
    visible_only: bool,
) -> Option<WidgetRef> {
    let id = unsafe {
        XPGetWidgetForLocation(container.0, x, y, recursive as c_int, visible_only as c_int)
    };
    (!id.is_null()).then_some(WidgetRef(id))
}

type WidgetCallback = dyn FnMut(WidgetMessage, WidgetRef, isize, isize) -> bool + 'static;

/// A widget your plugin created. Dropping it destroys the native widget
/// (and every descendant) via `XPDestroyWidget`.
pub struct Widget {
    id: XPWidgetID,
}

unsafe impl Send for Widget {} // see FlightLoop's identical rationale: main-thread-only callbacks.

impl Widget {
    pub fn handle(&self) -> WidgetRef {
        WidgetRef(self.id)
    }
}

impl Drop for Widget {
    fn drop(&mut self) {
        // Triggers `xpMsg_Destroy` synchronously, which the trampoline (for
        // a custom widget) uses to free its boxed closure — see
        // `custom_widget_trampoline`.
        unsafe { XPDestroyWidget(self.id, 1) }
    }
}

/// Creates a widget of one of the SDK's built-in classes ([`WidgetClass`]) —
/// its look and interactive behavior are entirely handled natively; observe
/// or react to it via [`WidgetRef::send_message`]/[`WidgetRef::property`],
/// or wrap it in a [`create_custom_widget`] parent that intercepts messages
/// bubbled `UpChain`. `None` if `descriptor` contains an interior NUL or the
/// SDK refuses the creation.
pub fn create_widget(
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    visible: bool,
    descriptor: &str,
    is_root: bool,
    container: Option<WidgetRef>,
    class: WidgetClass,
) -> Option<Widget> {
    let c_descriptor = CString::new(descriptor).ok()?;
    let id = unsafe {
        XPCreateWidget(
            left,
            top,
            right,
            bottom,
            visible as c_int,
            c_descriptor.as_ptr(),
            is_root as c_int,
            container.map_or(std::ptr::null_mut(), |c| c.0),
            class.into(),
        )
    };
    (!id.is_null()).then_some(Widget { id })
}

/// Creates a widget whose behavior is entirely up to `callback`:
/// `f(message, widget, param1, param2) -> handled`. `None` if `descriptor`
/// contains an interior NUL or the SDK refuses the creation.
pub fn create_custom_widget(
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    visible: bool,
    descriptor: &str,
    is_root: bool,
    container: Option<WidgetRef>,
    callback: impl FnMut(WidgetMessage, WidgetRef, isize, isize) -> bool + 'static,
) -> Option<Widget> {
    let c_descriptor = CString::new(descriptor).ok()?;
    let state: Box<RefCell<Box<WidgetCallback>>> =
        Box::new(RefCell::new(Box::new(callback) as Box<WidgetCallback>));
    let refcon = Box::into_raw(state) as isize;

    let id = unsafe {
        XPCreateCustomWidget(
            left,
            top,
            right,
            bottom,
            visible as c_int,
            c_descriptor.as_ptr(),
            is_root as c_int,
            container.map_or(std::ptr::null_mut(), |c| c.0),
            Some(custom_widget_trampoline),
        )
    };
    if id.is_null() {
        unsafe { drop(Box::from_raw(refcon as *mut RefCell<Box<WidgetCallback>>)) };
        return None;
    }
    unsafe { XPSetWidgetProperty(id, REFCON_PROPERTY, refcon) };
    Some(Widget { id })
}

/// Adds an additional callback on top of an existing widget (including a
/// standard-class one), matching `XPAddWidgetCallback`'s "subclassing"
/// pattern — the SDK dispatches the most-recently-added callback first, and
/// there is no way to remove one short of destroying the widget. Because of
/// that, `callback` must own everything it needs for the widget's entire
/// remaining lifetime; unlike [`create_custom_widget`], its `Box` is
/// intentionally leaked (reclaiming it would need a removal hook the SDK
/// doesn't provide).
pub fn add_widget_callback(
    widget: WidgetRef,
    callback: impl FnMut(WidgetMessage, WidgetRef, isize, isize) -> bool + 'static,
) {
    let state: Box<RefCell<Box<WidgetCallback>>> =
        Box::new(RefCell::new(Box::new(callback) as Box<WidgetCallback>));
    let refcon = Box::into_raw(state);
    unsafe {
        XPSetWidgetProperty(widget.0, REFCON_PROPERTY, refcon as isize);
        XPAddWidgetCallback(widget.0, Some(custom_widget_trampoline));
    }
}

unsafe extern "C" fn custom_widget_trampoline(
    message: XPWidgetMessage,
    widget: XPWidgetID,
    param1: isize,
    param2: isize,
) -> c_int {
    crate::guard(|| {
        let mut exists: c_int = 0;
        let refcon = unsafe { XPGetWidgetProperty(widget, REFCON_PROPERTY, &mut exists) };
        if exists == 0 || refcon == 0 {
            return false;
        }
        let state = refcon as *const RefCell<Box<WidgetCallback>>;
        let wrapped_message = WidgetMessage::from(message);
        let handled =
            (unsafe { &*state }.borrow_mut())(wrapped_message, WidgetRef(widget), param1, param2);
        if wrapped_message == WidgetMessage::Destroy {
            drop(unsafe { Box::from_raw(state as *mut RefCell<Box<WidgetCallback>>) });
        }
        handled
    })
    .unwrap_or(false) as c_int
}
