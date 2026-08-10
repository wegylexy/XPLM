//! Customizing built-in avionics displays, or creating entirely new
//! glass-cockpit devices (`XPLMDisplay.h`'s avionics API, XPLM410+).
//!
//! Two independent creation paths share one query/mutate handle
//! ([`AvionicsHandle`]):
//! - [`Avionics::customize`] hooks a *built-in* device (a GNS430, a G1000
//!   screen, ...) — you can draw before/after X-Plane's own drawing and
//!   intercept its bezel/screen mouse, wheel, cursor, and keyboard events.
//! - [`CustomAvionics::builder`] creates a brand-new device from scratch —
//!   you own the whole screen and bezel, drawing both yourself.
//!
//! Neither the SDK's per-event callbacks nor this wrapper's boxed-closure
//! equivalents receive the device handle back (only [`Device`]/coordinates/
//! event data) — capture the surrounding [`AvionicsHandle`] or
//! [`CustomAvionics`] in the closure yourself if a callback needs it.

use std::cell::RefCell;
use std::ffi::{c_void, CString};
use std::os::raw::{c_char, c_int};

use xplm_sys::{
    xplm_device_CDU739_1, xplm_device_CDU739_2, xplm_device_CDU815_1, xplm_device_CDU815_2,
    xplm_device_G1000_MFD, xplm_device_G1000_PFD_1, xplm_device_G1000_PFD_2, xplm_device_GNS430_1,
    xplm_device_GNS430_2, xplm_device_GNS530_1, xplm_device_GNS530_2, xplm_device_MCDU_1,
    xplm_device_MCDU_2, xplm_device_Primus_MFD_1, xplm_device_Primus_MFD_2,
    xplm_device_Primus_MFD_3, xplm_device_Primus_PFD_1, xplm_device_Primus_PFD_2,
    xplm_device_Primus_RMU_1, xplm_device_Primus_RMU_2, XPLMAvionicsID, XPLMAvionicsNeedsDrawing,
    XPLMCreateAvionicsEx, XPLMCreateAvionics_t, XPLMCustomizeAvionics_t, XPLMDestroyAvionics,
    XPLMDeviceID, XPLMGetAvionicsBrightnessRheo, XPLMGetAvionicsBusVoltsRatio,
    XPLMGetAvionicsGeometry, XPLMGetAvionicsGeometryOS, XPLMGetAvionicsHandle,
    XPLMHasAvionicsKeyboardFocus, XPLMIsAvionicsBound, XPLMIsAvionicsPoppedOut,
    XPLMIsAvionicsPopupVisible, XPLMIsCursorOverAvionics, XPLMPopOutAvionics,
    XPLMRegisterAvionicsCallbacksEx, XPLMSetAvionicsBrightnessRheo, XPLMSetAvionicsGeometry,
    XPLMSetAvionicsGeometryOS, XPLMSetAvionicsPopupVisible, XPLMTakeAvionicsKeyboardFocus,
    XPLMUnregisterAvionicsCallbacks,
};

use crate::window::{CursorStatus, KeyFlags, MouseStatus};

/// A built-in cockpit device that [`Avionics::customize`]/[`Avionics::handle_for`]
/// can target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Gns430Pilot,
    Gns430Copilot,
    Gns530Pilot,
    Gns530Copilot,
    Cdu739Pilot,
    Cdu739Copilot,
    G1000PfdPilot,
    G1000Mfd,
    G1000PfdCopilot,
    Cdu815Pilot,
    Cdu815Copilot,
    PrimusPfdPilot,
    PrimusPfdCopilot,
    PrimusMfdPilot,
    PrimusMfdCopilot,
    PrimusMfdCenter,
    PrimusRmuPilot,
    PrimusRmuCopilot,
    McduPilot,
    McduCopilot,
}

impl From<Device> for XPLMDeviceID {
    fn from(d: Device) -> Self {
        match d {
            Device::Gns430Pilot => xplm_device_GNS430_1,
            Device::Gns430Copilot => xplm_device_GNS430_2,
            Device::Gns530Pilot => xplm_device_GNS530_1,
            Device::Gns530Copilot => xplm_device_GNS530_2,
            Device::Cdu739Pilot => xplm_device_CDU739_1,
            Device::Cdu739Copilot => xplm_device_CDU739_2,
            Device::G1000PfdPilot => xplm_device_G1000_PFD_1,
            Device::G1000Mfd => xplm_device_G1000_MFD,
            Device::G1000PfdCopilot => xplm_device_G1000_PFD_2,
            Device::Cdu815Pilot => xplm_device_CDU815_1,
            Device::Cdu815Copilot => xplm_device_CDU815_2,
            Device::PrimusPfdPilot => xplm_device_Primus_PFD_1,
            Device::PrimusPfdCopilot => xplm_device_Primus_PFD_2,
            Device::PrimusMfdPilot => xplm_device_Primus_MFD_1,
            Device::PrimusMfdCopilot => xplm_device_Primus_MFD_2,
            Device::PrimusMfdCenter => xplm_device_Primus_MFD_3,
            Device::PrimusRmuPilot => xplm_device_Primus_RMU_1,
            Device::PrimusRmuCopilot => xplm_device_Primus_RMU_2,
            Device::McduPilot => xplm_device_MCDU_1,
            Device::McduCopilot => xplm_device_MCDU_2,
        }
    }
}

impl From<XPLMDeviceID> for Device {
    fn from(raw: XPLMDeviceID) -> Self {
        match raw {
            v if v == xplm_device_GNS430_1 => Device::Gns430Pilot,
            v if v == xplm_device_GNS430_2 => Device::Gns430Copilot,
            v if v == xplm_device_GNS530_1 => Device::Gns530Pilot,
            v if v == xplm_device_GNS530_2 => Device::Gns530Copilot,
            v if v == xplm_device_CDU739_1 => Device::Cdu739Pilot,
            v if v == xplm_device_CDU739_2 => Device::Cdu739Copilot,
            v if v == xplm_device_G1000_PFD_1 => Device::G1000PfdPilot,
            v if v == xplm_device_G1000_MFD => Device::G1000Mfd,
            v if v == xplm_device_G1000_PFD_2 => Device::G1000PfdCopilot,
            v if v == xplm_device_CDU815_1 => Device::Cdu815Pilot,
            v if v == xplm_device_CDU815_2 => Device::Cdu815Copilot,
            v if v == xplm_device_Primus_PFD_1 => Device::PrimusPfdPilot,
            v if v == xplm_device_Primus_PFD_2 => Device::PrimusPfdCopilot,
            v if v == xplm_device_Primus_MFD_1 => Device::PrimusMfdPilot,
            v if v == xplm_device_Primus_MFD_2 => Device::PrimusMfdCopilot,
            v if v == xplm_device_Primus_MFD_3 => Device::PrimusMfdCenter,
            v if v == xplm_device_Primus_RMU_1 => Device::PrimusRmuPilot,
            v if v == xplm_device_Primus_RMU_2 => Device::PrimusRmuCopilot,
            v if v == xplm_device_MCDU_1 => Device::McduPilot,
            _ => Device::McduCopilot,
        }
    }
}

/// A lightweight, `Copy`able handle to any avionics device (built-in or
/// custom) — the query/mutate API shared by both creation paths. Obtained
/// from [`Avionics::handle_for`], [`Avionics::handle`], or
/// [`CustomAvionics::handle`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvionicsHandle(XPLMAvionicsID);

impl AvionicsHandle {
    /// Whether this device is used by the current aircraft.
    pub fn is_bound(&self) -> bool {
        unsafe { XPLMIsAvionicsBound(self.0) != 0 }
    }

    /// Sets the screen brightness rheostat (`0..1`). Shorthand for the
    /// bound `instrument_brightness_ratio[]` slot if bound; otherwise the
    /// device tracks its own value.
    pub fn set_brightness_rheo(&self, brightness: f32) {
        unsafe { XPLMSetAvionicsBrightnessRheo(self.0, brightness) }
    }

    pub fn brightness_rheo(&self) -> f32 {
        unsafe { XPLMGetAvionicsBrightnessRheo(self.0) }
    }

    /// Ratio of nominal bus voltage (`1.0` = full nominal), or `-1` if not
    /// bound to the current aircraft.
    pub fn bus_volts_ratio(&self) -> f32 {
        unsafe { XPLMGetAvionicsBusVoltsRatio(self.0) }
    }

    /// `Some((x, y))` in device coordinates if the mouse is over this
    /// device's screen.
    pub fn cursor_position(&self) -> Option<(i32, i32)> {
        let (mut x, mut y) = (0, 0);
        let over = unsafe { XPLMIsCursorOverAvionics(self.0, &mut x, &mut y) } != 0;
        over.then_some((x, y))
    }

    /// Requests a redraw before the next frame — only meaningful for a
    /// [`CustomAvionics`] created with `draw_on_demand(true)`; a no-op
    /// otherwise.
    pub fn needs_drawing(&self) {
        unsafe { XPLMAvionicsNeedsDrawing(self.0) }
    }

    pub fn set_popup_visible(&self, visible: bool) {
        unsafe { XPLMSetAvionicsPopupVisible(self.0, visible as c_int) }
    }

    pub fn is_popup_visible(&self) -> bool {
        unsafe { XPLMIsAvionicsPopupVisible(self.0) != 0 }
    }

    pub fn pop_out(&self) {
        unsafe { XPLMPopOutAvionics(self.0) }
    }

    pub fn is_popped_out(&self) -> bool {
        unsafe { XPLMIsAvionicsPoppedOut(self.0) != 0 }
    }

    pub fn take_keyboard_focus(&self) {
        unsafe { XPLMTakeAvionicsKeyboardFocus(self.0) }
    }

    pub fn has_keyboard_focus(&self) -> bool {
        unsafe { XPLMHasAvionicsKeyboardFocus(self.0) != 0 }
    }

    /// `(left, top, right, bottom)` of the popup window, in the X-Plane
    /// coordinate system.
    pub fn geometry(&self) -> (i32, i32, i32, i32) {
        let (mut left, mut top, mut right, mut bottom) = (0, 0, 0, 0);
        unsafe { XPLMGetAvionicsGeometry(self.0, &mut left, &mut top, &mut right, &mut bottom) };
        (left, top, right, bottom)
    }

    pub fn set_geometry(&self, left: i32, top: i32, right: i32, bottom: i32) {
        unsafe { XPLMSetAvionicsGeometry(self.0, left, top, right, bottom) }
    }

    /// `(left, top, right, bottom)` of the popped-out OS window.
    pub fn geometry_os(&self) -> (i32, i32, i32, i32) {
        let (mut left, mut top, mut right, mut bottom) = (0, 0, 0, 0);
        unsafe { XPLMGetAvionicsGeometryOS(self.0, &mut left, &mut top, &mut right, &mut bottom) };
        (left, top, right, bottom)
    }

    pub fn set_geometry_os(&self, left: i32, top: i32, right: i32, bottom: i32) {
        unsafe { XPLMSetAvionicsGeometryOS(self.0, left, top, right, bottom) }
    }
}

type MouseFn = dyn FnMut(i32, i32, MouseStatus) -> bool + 'static;
type WheelFn = dyn FnMut(i32, i32, i32, i32) -> bool + 'static;
type CursorFn = dyn FnMut(i32, i32) -> CursorStatus + 'static;
type KeyFn = dyn FnMut(char, KeyFlags, char, bool) -> bool + 'static;

/// Boxed callbacks shared by both [`Avionics::customize`] and
/// [`CustomAvionics`] — the bezel/screen mouse, wheel, cursor, and keyboard
/// event shape is identical between the two SDK structs.
struct SharedCallbacks {
    bezel_click: RefCell<Box<MouseFn>>,
    bezel_right_click: RefCell<Box<MouseFn>>,
    bezel_scroll: RefCell<Box<WheelFn>>,
    bezel_cursor: RefCell<Box<CursorFn>>,
    screen_touch: RefCell<Box<MouseFn>>,
    screen_right_touch: RefCell<Box<MouseFn>>,
    screen_scroll: RefCell<Box<WheelFn>>,
    screen_cursor: RefCell<Box<CursorFn>>,
    keyboard: RefCell<Box<KeyFn>>,
}

impl SharedCallbacks {
    fn new() -> Self {
        Self {
            bezel_click: RefCell::new(Box::new(|_, _, _| false)),
            bezel_right_click: RefCell::new(Box::new(|_, _, _| false)),
            bezel_scroll: RefCell::new(Box::new(|_, _, _, _| false)),
            bezel_cursor: RefCell::new(Box::new(|_, _| CursorStatus::Default)),
            screen_touch: RefCell::new(Box::new(|_, _, _| false)),
            screen_right_touch: RefCell::new(Box::new(|_, _, _| false)),
            screen_scroll: RefCell::new(Box::new(|_, _, _, _| false)),
            screen_cursor: RefCell::new(Box::new(|_, _| CursorStatus::Default)),
            keyboard: RefCell::new(Box::new(|_, _, _, _| false)),
        }
    }
}

// --- Customizing a built-in device -----------------------------------

type DrawBeforeFn = dyn FnMut(Device) -> bool + 'static;
type DrawAfterFn = dyn FnMut(Device) + 'static;

struct CustomizeState {
    draw_before: RefCell<Box<DrawBeforeFn>>,
    draw_after: RefCell<Box<DrawAfterFn>>,
    shared: SharedCallbacks,
}

/// Hooks into (or just queries) a built-in cockpit device.
pub struct Avionics;

impl Avionics {
    /// A non-owning handle for programmatic interaction with `device`
    /// (popup visibility, geometry, brightness, ...) without intercepting
    /// its drawing or input — equivalent to calling
    /// [`Self::customize`] with every callback left at its default no-op.
    pub fn handle_for(device: Device) -> AvionicsHandle {
        AvionicsHandle(unsafe { XPLMGetAvionicsHandle(device.into()) })
    }

    pub fn customize(device: Device) -> AvionicsCustomizationBuilder {
        AvionicsCustomizationBuilder::new(device)
    }
}

/// Builds a [`CustomizedAvionics`] registration. Every callback defaults to
/// a no-op/pass-through, matching [`crate::window::WindowBuilder`]'s shape.
pub struct AvionicsCustomizationBuilder {
    device: Device,
    draw_before: Box<DrawBeforeFn>,
    draw_after: Box<DrawAfterFn>,
    shared: SharedCallbacks,
}

impl AvionicsCustomizationBuilder {
    fn new(device: Device) -> Self {
        Self {
            device,
            draw_before: Box::new(|_| true),
            draw_after: Box::new(|_| {}),
            shared: SharedCallbacks::new(),
        }
    }

    /// Called before X-Plane draws `device`. Return `true` to let X-Plane's
    /// own drawing continue, `false` to suppress it.
    pub fn on_draw_before(mut self, f: impl FnMut(Device) -> bool + 'static) -> Self {
        self.draw_before = Box::new(f);
        self
    }

    /// Called after X-Plane draws `device`.
    pub fn on_draw_after(mut self, f: impl FnMut(Device) + 'static) -> Self {
        self.draw_after = Box::new(f);
        self
    }

    /// `f(x, y, status) -> consumed`.
    pub fn on_bezel_click(
        mut self,
        f: impl FnMut(i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.shared.bezel_click = RefCell::new(Box::new(f));
        self
    }

    pub fn on_bezel_right_click(
        mut self,
        f: impl FnMut(i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.shared.bezel_right_click = RefCell::new(Box::new(f));
        self
    }

    /// `f(x, y, wheel_axis, clicks) -> consumed`.
    pub fn on_bezel_scroll(mut self, f: impl FnMut(i32, i32, i32, i32) -> bool + 'static) -> Self {
        self.shared.bezel_scroll = RefCell::new(Box::new(f));
        self
    }

    pub fn on_bezel_cursor(mut self, f: impl FnMut(i32, i32) -> CursorStatus + 'static) -> Self {
        self.shared.bezel_cursor = RefCell::new(Box::new(f));
        self
    }

    pub fn on_screen_touch(
        mut self,
        f: impl FnMut(i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.shared.screen_touch = RefCell::new(Box::new(f));
        self
    }

    pub fn on_screen_right_touch(
        mut self,
        f: impl FnMut(i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.shared.screen_right_touch = RefCell::new(Box::new(f));
        self
    }

    pub fn on_screen_scroll(mut self, f: impl FnMut(i32, i32, i32, i32) -> bool + 'static) -> Self {
        self.shared.screen_scroll = RefCell::new(Box::new(f));
        self
    }

    pub fn on_screen_cursor(mut self, f: impl FnMut(i32, i32) -> CursorStatus + 'static) -> Self {
        self.shared.screen_cursor = RefCell::new(Box::new(f));
        self
    }

    /// `f(key, flags, virtual_key, losing_focus) -> consumed`.
    pub fn on_keyboard(
        mut self,
        f: impl FnMut(char, KeyFlags, char, bool) -> bool + 'static,
    ) -> Self {
        self.shared.keyboard = RefCell::new(Box::new(f));
        self
    }

    /// Registers the customization. `None` if `XPLMRegisterAvionicsCallbacksEx`
    /// fails (most likely a struct-size mismatch for the linked SDK version).
    pub fn build(self) -> Option<CustomizedAvionics> {
        let state = Box::into_raw(Box::new(CustomizeState {
            draw_before: RefCell::new(self.draw_before),
            draw_after: RefCell::new(self.draw_after),
            shared: self.shared,
        }));

        let mut params = XPLMCustomizeAvionics_t {
            structSize: std::mem::size_of::<XPLMCustomizeAvionics_t>() as c_int,
            deviceId: self.device.into(),
            drawCallbackBefore: Some(customize_draw_before_trampoline),
            drawCallbackAfter: Some(customize_draw_after_trampoline),
            bezelClickCallback: Some(customize_bezel_click_trampoline),
            bezelRightClickCallback: Some(customize_bezel_right_click_trampoline),
            bezelScrollCallback: Some(customize_bezel_scroll_trampoline),
            bezelCursorCallback: Some(customize_bezel_cursor_trampoline),
            screenTouchCallback: Some(customize_screen_touch_trampoline),
            screenRightTouchCallback: Some(customize_screen_right_touch_trampoline),
            screenScrollCallback: Some(customize_screen_scroll_trampoline),
            screenCursorCallback: Some(customize_screen_cursor_trampoline),
            keyboardCallback: Some(customize_keyboard_trampoline),
            refcon: state as *mut c_void,
        };
        let id = unsafe { XPLMRegisterAvionicsCallbacksEx(&mut params) };
        if id.is_null() {
            unsafe { drop(Box::from_raw(state)) };
            return None;
        }
        Some(CustomizedAvionics { id, state })
    }
}

/// An active built-in-device customization. Dropping it unregisters the
/// callbacks (`XPLMUnregisterAvionicsCallbacks`) and frees the closures.
pub struct CustomizedAvionics {
    id: XPLMAvionicsID,
    state: *mut CustomizeState,
}

unsafe impl Send for CustomizedAvionics {} // see FlightLoop's identical rationale: main-thread-only.

impl CustomizedAvionics {
    pub fn handle(&self) -> AvionicsHandle {
        AvionicsHandle(self.id)
    }
}

impl Drop for CustomizedAvionics {
    fn drop(&mut self) {
        unsafe {
            XPLMUnregisterAvionicsCallbacks(self.id);
            drop(Box::from_raw(self.state));
        }
    }
}

unsafe extern "C" fn customize_draw_before_trampoline(
    device_id: XPLMDeviceID,
    _is_before: c_int,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.draw_before.borrow_mut())(device_id.into())
    })
    .unwrap_or(true) as c_int
}

unsafe extern "C" fn customize_draw_after_trampoline(
    device_id: XPLMDeviceID,
    _is_before: c_int,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.draw_after.borrow_mut())(device_id.into());
    });
    1
}

unsafe extern "C" fn customize_bezel_click_trampoline(
    x: c_int,
    y: c_int,
    mouse: xplm_sys::XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.bezel_click.borrow_mut())(x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn customize_bezel_right_click_trampoline(
    x: c_int,
    y: c_int,
    mouse: xplm_sys::XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.bezel_right_click.borrow_mut())(x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn customize_bezel_scroll_trampoline(
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.bezel_scroll.borrow_mut())(x, y, wheel, clicks)
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn customize_bezel_cursor_trampoline(
    x: c_int,
    y: c_int,
    refcon: *mut c_void,
) -> xplm_sys::XPLMCursorStatus {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.bezel_cursor.borrow_mut())(x, y)
    })
    .unwrap_or(CursorStatus::Default)
    .into()
}

unsafe extern "C" fn customize_screen_touch_trampoline(
    x: c_int,
    y: c_int,
    mouse: xplm_sys::XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.screen_touch.borrow_mut())(x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn customize_screen_right_touch_trampoline(
    x: c_int,
    y: c_int,
    mouse: xplm_sys::XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.screen_right_touch.borrow_mut())(x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn customize_screen_scroll_trampoline(
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.screen_scroll.borrow_mut())(x, y, wheel, clicks)
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn customize_screen_cursor_trampoline(
    x: c_int,
    y: c_int,
    refcon: *mut c_void,
) -> xplm_sys::XPLMCursorStatus {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.screen_cursor.borrow_mut())(x, y)
    })
    .unwrap_or(CursorStatus::Default)
    .into()
}

unsafe extern "C" fn customize_keyboard_trampoline(
    key: c_char,
    flags: xplm_sys::XPLMKeyFlags,
    virtual_key: c_char,
    refcon: *mut c_void,
    losing_focus: c_int,
) -> c_int {
    crate::guard(|| {
        let state: &CustomizeState = unsafe { &*(refcon as *const CustomizeState) };
        (state.shared.keyboard.borrow_mut())(
            key as u8 as char,
            KeyFlags::from(flags),
            virtual_key as u8 as char,
            losing_focus != 0,
        )
    })
    .unwrap_or(false) as c_int
}

// --- Creating a brand-new device ---------------------------------------

type ScreenDrawFn = dyn FnMut() + 'static;
type BezelDrawFn = dyn FnMut(f32, f32, f32) + 'static;
type BrightnessFn = dyn FnMut(f32, f32, f32) -> f32 + 'static;

struct CustomAvionicsState {
    draw: RefCell<Box<ScreenDrawFn>>,
    bezel_draw: RefCell<Box<BezelDrawFn>>,
    brightness: RefCell<Option<Box<BrightnessFn>>>,
    shared: SharedCallbacks,
}

/// A plugin-created glass-cockpit device (`XPLMCreateAvionicsEx`). Dropping
/// it destroys the device (`XPLMDestroyAvionics`) and frees its callback
/// closures.
pub struct CustomAvionics {
    id: XPLMAvionicsID,
    state: *mut CustomAvionicsState,
}

unsafe impl Send for CustomAvionics {} // see FlightLoop's identical rationale: main-thread-only.

impl CustomAvionics {
    /// `device_id` must be unique, contain no spaces, and is what OBJ files
    /// reference via `ATTR_cockpit_device` to display this device in the 3D
    /// cockpit. `device_name` is a user-facing label.
    pub fn builder(
        device_id: &str,
        device_name: &str,
        screen_width: i32,
        screen_height: i32,
    ) -> CustomAvionicsBuilder {
        CustomAvionicsBuilder::new(device_id, device_name, screen_width, screen_height)
    }

    pub fn handle(&self) -> AvionicsHandle {
        AvionicsHandle(self.id)
    }
}

impl Drop for CustomAvionics {
    fn drop(&mut self) {
        unsafe {
            XPLMDestroyAvionics(self.id);
            drop(Box::from_raw(self.state));
        }
    }
}

/// Builds a [`CustomAvionics`] device. Every callback defaults to a no-op,
/// matching [`crate::window::WindowBuilder`]'s shape; `brightness` defaults
/// to `None` (X-Plane's own default brightness behavior).
pub struct CustomAvionicsBuilder {
    device_id: String,
    device_name: String,
    screen_width: i32,
    screen_height: i32,
    bezel_width: i32,
    bezel_height: i32,
    screen_offset_x: i32,
    screen_offset_y: i32,
    draw_on_demand: bool,
    draw: Box<ScreenDrawFn>,
    bezel_draw: Box<BezelDrawFn>,
    brightness: Option<Box<BrightnessFn>>,
    shared: SharedCallbacks,
}

impl CustomAvionicsBuilder {
    fn new(device_id: &str, device_name: &str, screen_width: i32, screen_height: i32) -> Self {
        Self {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            screen_width,
            screen_height,
            bezel_width: 0,
            bezel_height: 0,
            screen_offset_x: 0,
            screen_offset_y: 0,
            draw_on_demand: false,
            draw: Box::new(|| {}),
            bezel_draw: Box::new(|_, _, _| {}),
            brightness: None,
            shared: SharedCallbacks::new(),
        }
    }

    /// Size of the bezel drawn around the screen for the 2D popup.
    pub fn bezel_size(mut self, width: i32, height: i32) -> Self {
        self.bezel_width = width;
        self.bezel_height = height;
        self
    }

    /// The screen's offset into the bezel for the 2D popup.
    pub fn screen_offset(mut self, x: i32, y: i32) -> Self {
        self.screen_offset_x = x;
        self.screen_offset_y = y;
        self
    }

    /// If `true`, X-Plane only calls the draw callback when you request it
    /// via [`AvionicsHandle::needs_drawing`], instead of every frame.
    pub fn draw_on_demand(mut self, on_demand: bool) -> Self {
        self.draw_on_demand = on_demand;
        self
    }

    /// Draws into the device's screen framebuffer. X-Plane does not clear
    /// the screen between calls — call `glClear` yourself if needed.
    pub fn on_draw(mut self, f: impl FnMut() + 'static) -> Self {
        self.draw = Box::new(f);
        self
    }

    /// `f(ambient_r, ambient_g, ambient_b)` — draws the 2D-popup bezel; only
    /// called while the popup is visible.
    pub fn on_bezel_draw(mut self, f: impl FnMut(f32, f32, f32) + 'static) -> Self {
        self.bezel_draw = Box::new(f);
        self
    }

    pub fn on_bezel_click(
        mut self,
        f: impl FnMut(i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.shared.bezel_click = RefCell::new(Box::new(f));
        self
    }

    pub fn on_bezel_right_click(
        mut self,
        f: impl FnMut(i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.shared.bezel_right_click = RefCell::new(Box::new(f));
        self
    }

    pub fn on_bezel_scroll(mut self, f: impl FnMut(i32, i32, i32, i32) -> bool + 'static) -> Self {
        self.shared.bezel_scroll = RefCell::new(Box::new(f));
        self
    }

    pub fn on_bezel_cursor(mut self, f: impl FnMut(i32, i32) -> CursorStatus + 'static) -> Self {
        self.shared.bezel_cursor = RefCell::new(Box::new(f));
        self
    }

    pub fn on_screen_touch(
        mut self,
        f: impl FnMut(i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.shared.screen_touch = RefCell::new(Box::new(f));
        self
    }

    pub fn on_screen_right_touch(
        mut self,
        f: impl FnMut(i32, i32, MouseStatus) -> bool + 'static,
    ) -> Self {
        self.shared.screen_right_touch = RefCell::new(Box::new(f));
        self
    }

    pub fn on_screen_scroll(mut self, f: impl FnMut(i32, i32, i32, i32) -> bool + 'static) -> Self {
        self.shared.screen_scroll = RefCell::new(Box::new(f));
        self
    }

    pub fn on_screen_cursor(mut self, f: impl FnMut(i32, i32) -> CursorStatus + 'static) -> Self {
        self.shared.screen_cursor = RefCell::new(Box::new(f));
        self
    }

    pub fn on_keyboard(
        mut self,
        f: impl FnMut(char, KeyFlags, char, bool) -> bool + 'static,
    ) -> Self {
        self.shared.keyboard = RefCell::new(Box::new(f));
        self
    }

    /// `f(rheo_value, ambient_brightness, bus_volts_ratio) -> brightness`,
    /// all in `0..1` (`bus_volts_ratio` is `-1` if unbound) — return the
    /// screen's brightness ratio. Leave unset to use X-Plane's default
    /// behavior.
    pub fn on_brightness(mut self, f: impl FnMut(f32, f32, f32) -> f32 + 'static) -> Self {
        self.brightness = Some(Box::new(f));
        self
    }

    /// Creates the device. `None` if `device_id`/`device_name` contain an
    /// interior NUL, or `XPLMCreateAvionicsEx` fails (e.g. a struct-size
    /// mismatch, or a non-unique `device_id`).
    pub fn build(self) -> Option<CustomAvionics> {
        let c_device_id = CString::new(self.device_id).ok()?;
        let c_device_name = CString::new(self.device_name).ok()?;
        let has_brightness = self.brightness.is_some();

        let state = Box::into_raw(Box::new(CustomAvionicsState {
            draw: RefCell::new(self.draw),
            bezel_draw: RefCell::new(self.bezel_draw),
            brightness: RefCell::new(self.brightness),
            shared: self.shared,
        }));

        let mut params = XPLMCreateAvionics_t {
            structSize: std::mem::size_of::<XPLMCreateAvionics_t>() as c_int,
            screenWidth: self.screen_width,
            screenHeight: self.screen_height,
            bezelWidth: self.bezel_width,
            bezelHeight: self.bezel_height,
            screenOffsetX: self.screen_offset_x,
            screenOffsetY: self.screen_offset_y,
            drawOnDemand: self.draw_on_demand as c_int,
            bezelDrawCallback: Some(custom_bezel_draw_trampoline),
            drawCallback: Some(custom_screen_draw_trampoline),
            bezelClickCallback: Some(custom_bezel_click_trampoline),
            bezelRightClickCallback: Some(custom_bezel_right_click_trampoline),
            bezelScrollCallback: Some(custom_bezel_scroll_trampoline),
            bezelCursorCallback: Some(custom_bezel_cursor_trampoline),
            screenTouchCallback: Some(custom_screen_touch_trampoline),
            screenRightTouchCallback: Some(custom_screen_right_touch_trampoline),
            screenScrollCallback: Some(custom_screen_scroll_trampoline),
            screenCursorCallback: Some(custom_screen_cursor_trampoline),
            keyboardCallback: Some(custom_keyboard_trampoline),
            brightnessCallback: if has_brightness {
                Some(custom_brightness_trampoline)
            } else {
                None
            },
            deviceID: c_device_id.as_ptr() as *mut c_char,
            deviceName: c_device_name.as_ptr() as *mut c_char,
            refcon: state as *mut c_void,
        };
        let id = unsafe { XPLMCreateAvionicsEx(&mut params) };
        if id.is_null() {
            unsafe { drop(Box::from_raw(state)) };
            return None;
        }
        Some(CustomAvionics { id, state })
    }
}

unsafe extern "C" fn custom_screen_draw_trampoline(refcon: *mut c_void) {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.draw.borrow_mut())();
    });
}

unsafe extern "C" fn custom_bezel_draw_trampoline(
    ambient_r: f32,
    ambient_g: f32,
    ambient_b: f32,
    refcon: *mut c_void,
) {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.bezel_draw.borrow_mut())(ambient_r, ambient_g, ambient_b);
    });
}

unsafe extern "C" fn custom_brightness_trampoline(
    rheo_value: f32,
    ambient_brightness: f32,
    bus_volts_ratio: f32,
    refcon: *mut c_void,
) -> f32 {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        match state.brightness.borrow_mut().as_mut() {
            Some(f) => f(rheo_value, ambient_brightness, bus_volts_ratio),
            None => ambient_brightness,
        }
    })
    .unwrap_or(ambient_brightness)
}

unsafe extern "C" fn custom_bezel_click_trampoline(
    x: c_int,
    y: c_int,
    mouse: xplm_sys::XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.bezel_click.borrow_mut())(x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn custom_bezel_right_click_trampoline(
    x: c_int,
    y: c_int,
    mouse: xplm_sys::XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.bezel_right_click.borrow_mut())(x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn custom_bezel_scroll_trampoline(
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.bezel_scroll.borrow_mut())(x, y, wheel, clicks)
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn custom_bezel_cursor_trampoline(
    x: c_int,
    y: c_int,
    refcon: *mut c_void,
) -> xplm_sys::XPLMCursorStatus {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.bezel_cursor.borrow_mut())(x, y)
    })
    .unwrap_or(CursorStatus::Default)
    .into()
}

unsafe extern "C" fn custom_screen_touch_trampoline(
    x: c_int,
    y: c_int,
    mouse: xplm_sys::XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.screen_touch.borrow_mut())(x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn custom_screen_right_touch_trampoline(
    x: c_int,
    y: c_int,
    mouse: xplm_sys::XPLMMouseStatus,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.screen_right_touch.borrow_mut())(x, y, mouse.into())
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn custom_screen_scroll_trampoline(
    x: c_int,
    y: c_int,
    wheel: c_int,
    clicks: c_int,
    refcon: *mut c_void,
) -> c_int {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.screen_scroll.borrow_mut())(x, y, wheel, clicks)
    })
    .unwrap_or(false) as c_int
}

unsafe extern "C" fn custom_screen_cursor_trampoline(
    x: c_int,
    y: c_int,
    refcon: *mut c_void,
) -> xplm_sys::XPLMCursorStatus {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.screen_cursor.borrow_mut())(x, y)
    })
    .unwrap_or(CursorStatus::Default)
    .into()
}

unsafe extern "C" fn custom_keyboard_trampoline(
    key: c_char,
    flags: xplm_sys::XPLMKeyFlags,
    virtual_key: c_char,
    refcon: *mut c_void,
    losing_focus: c_int,
) -> c_int {
    crate::guard(|| {
        let state: &CustomAvionicsState = unsafe { &*(refcon as *const CustomAvionicsState) };
        (state.shared.keyboard.borrow_mut())(
            key as u8 as char,
            KeyFlags::from(flags),
            virtual_key as u8 as char,
            losing_focus != 0,
        )
    })
    .unwrap_or(false) as c_int
}
