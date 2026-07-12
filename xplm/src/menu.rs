//! Menus (`XPLMMenus.h`). Same RAII + trampoline shape as `FlightLoop`, plus
//! an internal item list mirroring the C# original's `List<Item>`: a
//! `MenuItem` doesn't cache its own index (that would go stale the moment an
//! earlier item is removed and X-Plane reindexes everything below it) —
//! instead it's found by identity in the owning `Menu`'s item list on
//! demand, the same way the C# code used `Items.IndexOf(this)`.

use std::cell::RefCell;
use std::ffi::{CString, c_void};
use std::os::raw::c_int;
use std::rc::Rc;

use xplm_sys::{
    XPLMAppendMenuItem, XPLMAppendMenuSeparator, XPLMCheckMenuItem, XPLMCheckMenuItemState,
    XPLMClearAllMenuItems, XPLMCreateMenu, XPLMDestroyMenu, XPLMEnableMenuItem,
    XPLMFindPluginsMenu, XPLMMenuCheck, XPLMMenuID, XPLMSetMenuItemName, xplm_Menu_Checked,
    xplm_Menu_NoCheck, xplm_Menu_Unchecked,
};

#[cfg(feature = "XPLM210")]
use xplm_sys::XPLMRemoveMenuItem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCheckState {
    NoCheck,
    Unchecked,
    Checked,
}

impl From<MenuCheckState> for XPLMMenuCheck {
    fn from(state: MenuCheckState) -> Self {
        match state {
            MenuCheckState::NoCheck => xplm_Menu_NoCheck,
            MenuCheckState::Unchecked => xplm_Menu_Unchecked,
            MenuCheckState::Checked => xplm_Menu_Checked,
        }
    }
}

impl From<XPLMMenuCheck> for MenuCheckState {
    fn from(raw: XPLMMenuCheck) -> Self {
        if raw == xplm_Menu_Checked {
            MenuCheckState::Checked
        } else if raw == xplm_Menu_Unchecked {
            MenuCheckState::Unchecked
        } else {
            MenuCheckState::NoCheck
        }
    }
}

/// Identity token for one item. `Rc` clones of the same token live in the
/// owning `Menu`'s item list and in each `MenuItem` handle; identity
/// (`Rc::ptr_eq`) rather than a cached integer is how a `MenuItem` finds its
/// current position — the same reasoning as the C# original's
/// `Items.IndexOf(this)`.
struct ItemToken;

struct MenuState {
    handler: RefCell<Box<dyn FnMut(i32) + 'static>>,
    // One slot per XPLM menu index: `Some` for a real item, `None` for a
    // separator (which still consumes an index slot XPLM-side, so it must
    // occupy a slot here too for positions to line up). `Vec::remove` shifts
    // later positions down automatically, matching XPLM's own reindexing
    // after `XPLMRemoveMenuItem`.
    items: RefCell<Vec<Option<Rc<ItemToken>>>>,
}

/// A menu your plugin created. Dropping it destroys the native menu (and,
/// transitively, any items on it) and frees the handler closure and item
/// bookkeeping.
pub struct Menu {
    id: XPLMMenuID,
    state: *mut MenuState,
}

unsafe impl Send for Menu {} // see FlightLoop's identical rationale: main-thread-only callbacks.

impl Menu {
    /// Creates a submenu of `parent`, anchored at `parent_item`'s current
    /// index (a [`MenuItem`] previously returned by [`Menu::add_item`] on
    /// `parent`), with `handler` called as `handler(item_index)` whenever an
    /// item added via [`Menu::add_item`] on the new submenu is clicked.
    /// Returns `None` if `parent_item` has already been removed.
    pub fn new_submenu(
        parent: &Menu,
        parent_item: &MenuItem<'_>,
        name: &str,
        handler: impl FnMut(i32) + 'static,
    ) -> Option<Self> {
        let index = parent_item.index()?;
        Self::create(parent.id, index, name, handler)
    }

    /// Creates a top-level submenu of X-Plane's Plugins menu — the common
    /// case for a plugin's own menu. Internally this appends the item that
    /// anchors the submenu to the Plugins menu, then creates the submenu on
    /// it, mirroring the two-step dance the SDK requires
    /// (`XPLMAppendMenuItem` + `XPLMCreateMenu`).
    pub fn new_in_plugins_menu(name: &str, handler: impl FnMut(i32) + 'static) -> Option<Self> {
        let plugins_menu = unsafe { XPLMFindPluginsMenu() };
        let c_name = CString::new(name).ok()?;
        let item_index = unsafe {
            XPLMAppendMenuItem(plugins_menu, c_name.as_ptr(), std::ptr::null_mut(), 0)
        };
        if item_index < 0 {
            return None;
        }
        Self::create(plugins_menu, item_index, name, handler)
    }

    fn create(
        parent_id: XPLMMenuID,
        parent_item_index: i32,
        name: &str,
        handler: impl FnMut(i32) + 'static,
    ) -> Option<Self> {
        let c_name = CString::new(name).ok()?;
        let state = Box::into_raw(Box::new(MenuState {
            handler: RefCell::new(Box::new(handler)),
            items: RefCell::new(Vec::new()),
        }));

        let id = unsafe {
            XPLMCreateMenu(
                c_name.as_ptr(),
                parent_id,
                parent_item_index,
                Some(trampoline),
                state as *mut c_void,
            )
        };
        if id.is_null() {
            unsafe { drop(Box::from_raw(state)) };
            return None;
        }
        Some(Self { id, state })
    }

    /// Appends an item, returning a [`MenuItem`] handle with getters/setters
    /// for its checked state, enabled state, and name — matching the C#
    /// original's `Item`-returning `AppendItem`. Returns `None` if the name
    /// contains an interior NUL or the append fails.
    pub fn add_item(&self, name: &str) -> Option<MenuItem<'_>> {
        let c_name = CString::new(name).ok()?;
        let state = unsafe { &*self.state };

        let token = Rc::new(ItemToken);
        // The item's refcon is this token's identity pointer; XPLM hands it
        // back unchanged on click; `trampoline` finds the item's *current*
        // position by scanning for that identity rather than trusting a
        // frozen index (see the module docs).
        let item_ref = Rc::as_ptr(&token) as *mut c_void;

        #[cfg(debug_assertions)]
        let predicted_index = state.items.borrow().len() as i32;
        let actual_index = unsafe { XPLMAppendMenuItem(self.id, c_name.as_ptr(), item_ref, 0) };
        if actual_index < 0 {
            return None;
        }
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            actual_index, predicted_index,
            "XPLMAppendMenuItem index didn't match our tracked item count"
        );

        // One extra strong count, reconstructed and dropped in `remove`/
        // `Menu::drop` — this is what "the C side holds a reference" means
        // for an `Rc`: a strong count with no live Rust owner until we
        // explicitly reclaim it.
        std::mem::forget(token.clone());
        state.items.borrow_mut().push(Some(token.clone()));

        Some(MenuItem { menu: self, token })
    }

    pub fn add_separator(&self) {
        unsafe { XPLMAppendMenuSeparator(self.id) };
        let state = unsafe { &*self.state };
        state.items.borrow_mut().push(None);
    }

    /// Removes all items, discarding the list — the menu can be rebuilt
    /// from scratch afterward. Any outstanding `MenuItem` handles become
    /// stale (`index()` will return `None` for them).
    pub fn clear_items(&self) {
        unsafe { XPLMClearAllMenuItems(self.id) };
        let state = unsafe { &*self.state };
        for slot in state.items.borrow_mut().drain(..) {
            if let Some(token) = slot {
                reclaim_leaked_count(&token);
            }
        }
    }
}

/// Reconstructs and immediately drops the one extra `Rc` strong count that
/// `Menu::add_item` leaked to represent "X-Plane holds a reference" —
/// balances exactly one `std::mem::forget(token.clone())` per token.
fn reclaim_leaked_count(token: &Rc<ItemToken>) {
    unsafe { drop(Rc::from_raw(Rc::as_ptr(token))) };
}

impl Drop for Menu {
    fn drop(&mut self) {
        let state = unsafe { &*self.state };
        for slot in state.items.borrow().iter() {
            if let Some(token) = slot {
                reclaim_leaked_count(token);
            }
        }
        unsafe {
            XPLMDestroyMenu(self.id);
            drop(Box::from_raw(self.state));
        }
    }
}

/// A handle to one item on a [`Menu`], returned by [`Menu::add_item`].
/// Its index is never cached — it's looked up by identity in the owning
/// `Menu`'s item list each time, so it stays correct even after earlier
/// items are removed and X-Plane reindexes everything below them.
pub struct MenuItem<'a> {
    menu: &'a Menu,
    token: Rc<ItemToken>,
}

impl MenuItem<'_> {
    /// This item's current XPLM index, or `None` if it's already been
    /// removed. Computed by identity lookup (`Items.IndexOf(this)` in the
    /// C# original), not cached.
    pub fn index(&self) -> Option<i32> {
        let state = unsafe { &*self.menu.state };
        state
            .items
            .borrow()
            .iter()
            .position(|slot| matches!(slot, Some(t) if Rc::ptr_eq(t, &self.token)))
            .map(|i| i as i32)
    }

    pub fn set_checked(&self, state: MenuCheckState) {
        if let Some(index) = self.index() {
            unsafe { XPLMCheckMenuItem(self.menu.id, index, state.into()) }
        }
    }

    pub fn checked(&self) -> Option<MenuCheckState> {
        let index = self.index()?;
        let mut raw: XPLMMenuCheck = 0;
        unsafe { XPLMCheckMenuItemState(self.menu.id, index, &mut raw) };
        Some(raw.into())
    }

    pub fn set_enabled(&self, enabled: bool) {
        if let Some(index) = self.index() {
            unsafe { XPLMEnableMenuItem(self.menu.id, index, enabled as c_int) }
        }
    }

    pub fn set_name(&self, name: &str) {
        let (Some(index), Ok(c_name)) = (self.index(), CString::new(name)) else {
            return;
        };
        unsafe { XPLMSetMenuItemName(self.menu.id, index, c_name.as_ptr(), 0) }
    }

    /// Removes this item. Consumes `self` since, once removed, there is
    /// nothing left for this handle to refer to.
    #[cfg(feature = "XPLM210")]
    pub fn remove(self) {
        let state = unsafe { &*self.menu.state };
        let Some(index) = self.index() else {
            return;
        };
        unsafe { XPLMRemoveMenuItem(self.menu.id, index) };
        let mut items = state.items.borrow_mut();
        if let Some(slot) = items.get_mut(index as usize) {
            if let Some(token) = slot.take() {
                reclaim_leaked_count(&token);
            }
        }
    }
}

unsafe extern "C" fn trampoline(in_menu_ref: *mut c_void, in_item_ref: *mut c_void) {
    crate::guard(|| {
        let state: &MenuState = unsafe { &*(in_menu_ref as *const MenuState) };
        let index = state.items.borrow().iter().position(|slot| {
            matches!(slot, Some(t) if Rc::as_ptr(t) as *mut c_void == in_item_ref)
        });
        if let Some(index) = index {
            (state.handler.borrow_mut())(index as i32);
        }
    });
}
