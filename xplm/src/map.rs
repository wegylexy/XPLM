//! Custom map layers (`XPLMMap.h`, XPLM300+): plugin-drawn OpenGL/icon/label
//! content overlaid on X-Plane's built-in map/IOS windows, plus the
//! lat-lon-to-map-coordinate projection API needed to draw at a specific
//! place.

use std::cell::{Cell, RefCell};
use std::ffi::{c_void, CStr, CString};
use std::os::raw::{c_char, c_int};
use std::rc::Rc;

use xplm_sys::{
    xplm_MapLayer_Fill, xplm_MapLayer_Markings, xplm_MapOrientation_Map, xplm_MapOrientation_UI,
    xplm_MapStyle_IFR_HighEnroute, xplm_MapStyle_IFR_LowEnroute, xplm_MapStyle_VFR_Sectional,
    XPLMCreateMapLayer, XPLMCreateMapLayer_t, XPLMDestroyMapLayer, XPLMDrawMapIconFromSheet,
    XPLMDrawMapLabel, XPLMMapExists, XPLMMapGetNorthHeading, XPLMMapLayerID, XPLMMapLayerType,
    XPLMMapOrientation, XPLMMapProject, XPLMMapProjectionID, XPLMMapScaleMeter, XPLMMapStyle,
    XPLMMapUnproject, XPLMRegisterMapCreationHook, XPLM_MAP_IOS, XPLM_MAP_USER_INTERFACE,
};

/// Which built-in map window a [`MapLayer`] is created in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapTarget {
    /// X-Plane's normal map UI.
    UserInterface,
    /// The Instructor Operator Station.
    Ios,
}

impl MapTarget {
    /// `XPLM_MAP_USER_INTERFACE`/`XPLM_MAP_IOS` already come from `xplm-sys`
    /// as NUL-terminated byte arrays, so this borrows their pointer directly
    /// rather than allocating a fresh `CString`.
    fn as_c_ptr(self) -> *const c_char {
        match self {
            MapTarget::UserInterface => XPLM_MAP_USER_INTERFACE.as_ptr() as *const c_char,
            MapTarget::Ios => XPLM_MAP_IOS.as_ptr() as *const c_char,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            MapTarget::UserInterface => "XPLM_MAP_USER_INTERFACE",
            MapTarget::Ios => "XPLM_MAP_IOS",
        }
    }

    /// Whether this map currently exists — safe to call before knowing
    /// whether the user has the map/IOS window open at all.
    pub fn exists(self) -> bool {
        map_exists(self.as_str())
    }
}

/// Whether a map with the given identifier currently exists in X-Plane. Use
/// [`MapTarget::exists`] for the two well-known targets, or this directly
/// for an identifier obtained from [`register_map_creation_hook`].
pub fn map_exists(map_identifier: &str) -> bool {
    let Ok(c_id) = CString::new(map_identifier) else {
        return false;
    };
    unsafe { XPLMMapExists(c_id.as_ptr()) != 0 }
}

/// Registers `callback(map_identifier)` to run each time a new map is
/// constructed — the right time to create a [`MapLayer`] in it via
/// [`MapLayerBuilder::build`]. Only fires for maps created *after* this
/// call; use [`MapTarget::exists`]/[`map_exists`] to check for ones that
/// already exist.
///
/// The SDK provides no way to unregister this hook, so (like every other
/// XPLM registration with no matching unregister call) the closure is
/// intentionally leaked for the plugin's lifetime — call this at most once
/// per distinct piece of setup logic, typically from `start()`.
pub fn register_map_creation_hook(callback: impl FnMut(&str) + 'static) {
    unsafe extern "C" fn trampoline(map_identifier: *const c_char, refcon: *mut c_void) {
        crate::guard(|| {
            let callback: &mut Box<dyn FnMut(&str)> = unsafe { &mut *(refcon as *mut _) };
            let id = unsafe { CStr::from_ptr(map_identifier) }.to_string_lossy();
            callback(&id);
        });
    }

    let boxed: Box<dyn FnMut(&str)> = Box::new(callback);
    let refcon = Box::into_raw(Box::new(boxed)) as *mut c_void;
    unsafe { XPLMRegisterMapCreationHook(Some(trampoline), refcon) };
}

/// Whether a [`MapLayer`] draws "fill" (large-area background, like terrain
/// or weather) or "markings" (small, dense features, like navaid icons) —
/// all fill layers are drawn beneath all markings layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapLayerType {
    Fill,
    Markings,
}

impl From<MapLayerType> for XPLMMapLayerType {
    fn from(t: MapLayerType) -> Self {
        match t {
            MapLayerType::Fill => xplm_MapLayer_Fill,
            MapLayerType::Markings => xplm_MapLayer_Markings,
        }
    }
}

/// The visual style X-Plane is currently drawing the map in — some layers
/// only make sense in certain styles (e.g. localizers in IFR low-enroute).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapStyle {
    VfrSectional,
    IfrLowEnroute,
    IfrHighEnroute,
    /// A style newer than this crate knows about.
    Other(XPLMMapStyle),
}

impl From<XPLMMapStyle> for MapStyle {
    fn from(raw: XPLMMapStyle) -> Self {
        if raw == xplm_MapStyle_VFR_Sectional {
            MapStyle::VfrSectional
        } else if raw == xplm_MapStyle_IFR_LowEnroute {
            MapStyle::IfrLowEnroute
        } else if raw == xplm_MapStyle_IFR_HighEnroute {
            MapStyle::IfrHighEnroute
        } else {
            MapStyle::Other(raw)
        }
    }
}

/// Whether a drawn icon/label's rotation is relative to the map's own north
/// (which may itself be rotated to match the user's heading) or to the
/// screen/user-interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapOrientation {
    Map,
    Ui,
}

impl From<MapOrientation> for XPLMMapOrientation {
    fn from(o: MapOrientation) -> Self {
        match o {
            MapOrientation::Map => xplm_MapOrientation_Map,
            MapOrientation::Ui => xplm_MapOrientation_UI,
        }
    }
}

/// `(left, top, right, bottom)` map-coordinate bounds passed to every
/// [`MapLayer`] callback.
pub type MapBounds = (f32, f32, f32, f32);

fn read_bounds(ptr: *const f32) -> MapBounds {
    if ptr.is_null() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let b = unsafe { std::slice::from_raw_parts(ptr, 4) };
    (b[0], b[1], b[2], b[3])
}

/// An opaque map projection, valid only for the duration of the callback it
/// was handed to — translates between latitude/longitude and the map's own
/// 2D coordinate space, and reports the map's current scale/rotation.
/// Cheap to copy (it's just the SDK's raw handle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapProjection(XPLMMapProjectionID);

impl MapProjection {
    /// Projects `(latitude, longitude)` into `(map_x, map_y)`.
    pub fn project(&self, latitude: f64, longitude: f64) -> (f32, f32) {
        let (mut x, mut y) = (0.0f32, 0.0f32);
        unsafe { XPLMMapProject(self.0, latitude, longitude, &mut x, &mut y) };
        (x, y)
    }

    /// The inverse of [`Self::project`].
    pub fn unproject(&self, map_x: f32, map_y: f32) -> (f64, f64) {
        let (mut latitude, mut longitude) = (0.0f64, 0.0f64);
        unsafe { XPLMMapUnproject(self.0, map_x, map_y, &mut latitude, &mut longitude) };
        (latitude, longitude)
    }

    /// How many map units correspond to one meter at `(map_x, map_y)`.
    pub fn scale_meters(&self, map_x: f32, map_y: f32) -> f32 {
        unsafe { XPLMMapScaleMeter(self.0, map_x, map_y) }
    }

    /// The clockwise rotation (degrees) from the map's own +Y axis to true
    /// north at `(map_x, map_y)` — use to align icons/labels with true
    /// north regardless of the map's own current rotation.
    pub fn north_heading(&self, map_x: f32, map_y: f32) -> f32 {
        unsafe { XPLMMapGetNorthHeading(self.0, map_x, map_y) }
    }
}

/// A lightweight, `Copy`able reference to a [`MapLayer`], passed into every
/// callback. Exposes the icon/label drawing calls, each only valid from
/// within the matching callback (icon-drawing from the icon callback, label
/// drawing from the label callback) — calling from the wrong one is a
/// caller error the SDK itself doesn't check for, matching the raw API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapLayerRef(XPLMMapLayerID);

impl MapLayerRef {
    /// Draws a sub-rectangle of a PNG icon sheet at `(map_x, map_y)`. The
    /// sheet is divided into a `ds`×`dt` grid of equally-sized cells;
    /// `(s, t)` selects which cell to draw. `map_width` is the icon's
    /// desired on-screen width in map units. `false` if `png_path` contains
    /// an interior NUL.
    pub fn draw_icon_from_sheet(
        &self,
        png_path: &str,
        s: i32,
        t: i32,
        ds: i32,
        dt: i32,
        map_x: f32,
        map_y: f32,
        orientation: MapOrientation,
        rotation_degrees: f32,
        map_width: f32,
    ) -> bool {
        let Ok(c_path) = CString::new(png_path) else {
            return false;
        };
        unsafe {
            XPLMDrawMapIconFromSheet(
                self.0,
                c_path.as_ptr(),
                s,
                t,
                ds,
                dt,
                map_x,
                map_y,
                orientation.into(),
                rotation_degrees,
                map_width,
            )
        };
        true
    }

    /// Draws `text` at `(map_x, map_y)`. `false` if `text` contains an
    /// interior NUL.
    pub fn draw_label(
        &self,
        text: &str,
        map_x: f32,
        map_y: f32,
        orientation: MapOrientation,
        rotation_degrees: f32,
    ) -> bool {
        let Ok(c_text) = CString::new(text) else {
            return false;
        };
        unsafe {
            XPLMDrawMapLabel(
                self.0,
                c_text.as_ptr(),
                map_x,
                map_y,
                orientation.into(),
                rotation_degrees,
            )
        };
        true
    }
}

type PrepareCacheFn = dyn FnMut(MapLayerRef, MapBounds, MapProjection) + 'static;
type WillBeDeletedFn = dyn FnMut(MapLayerRef) + 'static;
type DrawFn = dyn FnMut(MapLayerRef, MapBounds, f32, f32, MapStyle, MapProjection) + 'static;

struct MapLayerState {
    prepare_cache: RefCell<Box<PrepareCacheFn>>,
    will_be_deleted: RefCell<Box<WillBeDeletedFn>>,
    draw: RefCell<Box<DrawFn>>,
    draw_icons: RefCell<Box<DrawFn>>,
    draw_labels: RefCell<Box<DrawFn>>,
    /// Set inside the `willBeDeletedCallback` trampoline (fired when the
    /// owning map itself is torn down) so `Drop` knows not to call
    /// `XPLMDestroyMapLayer` on an already-gone layer.
    deleted: Rc<Cell<bool>>,
}

/// A plugin-created map layer (`XPLMCreateMapLayer`). Dropping it destroys
/// the layer (unless the owning map already destroyed it first — see
/// [`MapLayerBuilder::on_will_be_deleted`]) and frees its callback closures.
pub struct MapLayer {
    id: XPLMMapLayerID,
    state: *mut MapLayerState,
    deleted: Rc<Cell<bool>>,
}

unsafe impl Send for MapLayer {} // see FlightLoop's identical rationale: main-thread-only callbacks.

impl MapLayer {
    pub fn builder(target: MapTarget, layer_type: MapLayerType, name: &str) -> MapLayerBuilder {
        MapLayerBuilder::new(target, layer_type, name)
    }

    pub fn handle(&self) -> MapLayerRef {
        MapLayerRef(self.id)
    }
}

impl Drop for MapLayer {
    fn drop(&mut self) {
        unsafe {
            if !self.deleted.get() {
                XPLMDestroyMapLayer(self.id);
            }
            drop(Box::from_raw(self.state));
        }
    }
}

/// Builds a [`MapLayer`]. Every callback defaults to a no-op — set only the
/// ones you need, matching [`crate::window::WindowBuilder`]'s shape.
pub struct MapLayerBuilder {
    target: MapTarget,
    layer_type: MapLayerType,
    name: String,
    show_ui_toggle: bool,
    prepare_cache: Box<PrepareCacheFn>,
    will_be_deleted: Box<WillBeDeletedFn>,
    draw: Box<DrawFn>,
    draw_icons: Box<DrawFn>,
    draw_labels: Box<DrawFn>,
}

impl MapLayerBuilder {
    fn new(target: MapTarget, layer_type: MapLayerType, name: &str) -> Self {
        Self {
            target,
            layer_type,
            name: name.to_string(),
            show_ui_toggle: false,
            prepare_cache: Box::new(|_, _, _| {}),
            will_be_deleted: Box::new(|_| {}),
            draw: Box::new(|_, _, _, _, _, _| {}),
            draw_icons: Box::new(|_, _, _, _, _, _| {}),
            draw_labels: Box::new(|_, _, _, _, _, _| {}),
        }
    }

    /// Whether the map UI should offer a checkbox to toggle this layer.
    pub fn show_ui_toggle(mut self, show: bool) -> Self {
        self.show_ui_toggle = show;
        self
    }

    /// Called whenever the map's total bounds change (e.g. new DSFs
    /// loaded); cache whatever your draw calls need here so they can stay
    /// cheap.
    pub fn on_prepare_cache(
        mut self,
        f: impl FnMut(MapLayerRef, MapBounds, MapProjection) + 'static,
    ) -> Self {
        self.prepare_cache = Box::new(f);
        self
    }

    /// Called just before this layer is deleted because its owning map was
    /// torn down.
    pub fn on_will_be_deleted(mut self, f: impl FnMut(MapLayerRef) + 'static) -> Self {
        self.will_be_deleted = Box::new(f);
        self
    }

    /// Arbitrary OpenGL drawing — no Z-buffer changes permitted. Drawn
    /// beneath all icons/labels.
    pub fn on_draw(
        mut self,
        f: impl FnMut(MapLayerRef, MapBounds, f32, f32, MapStyle, MapProjection) + 'static,
    ) -> Self {
        self.draw = Box::new(f);
        self
    }

    /// Call [`MapLayerRef::draw_icon_from_sheet`] from here (and only from
    /// here) any number of times.
    pub fn on_draw_icons(
        mut self,
        f: impl FnMut(MapLayerRef, MapBounds, f32, f32, MapStyle, MapProjection) + 'static,
    ) -> Self {
        self.draw_icons = Box::new(f);
        self
    }

    /// Call [`MapLayerRef::draw_label`] from here (and only from here) any
    /// number of times.
    pub fn on_draw_labels(
        mut self,
        f: impl FnMut(MapLayerRef, MapBounds, f32, f32, MapStyle, MapProjection) + 'static,
    ) -> Self {
        self.draw_labels = Box::new(f);
        self
    }

    /// Creates the layer. `None` if `XPLMCreateMapLayer` fails — most often
    /// because `target` doesn't currently exist (see [`MapTarget::exists`]);
    /// use [`register_map_creation_hook`] to build layers as maps appear.
    pub fn build(self) -> Option<MapLayer> {
        let c_name = CString::new(self.name).ok()?;
        let deleted = Rc::new(Cell::new(false));
        let state = Box::into_raw(Box::new(MapLayerState {
            prepare_cache: RefCell::new(self.prepare_cache),
            will_be_deleted: RefCell::new(self.will_be_deleted),
            draw: RefCell::new(self.draw),
            draw_icons: RefCell::new(self.draw_icons),
            draw_labels: RefCell::new(self.draw_labels),
            deleted: deleted.clone(),
        }));

        let mut params = XPLMCreateMapLayer_t {
            structSize: std::mem::size_of::<XPLMCreateMapLayer_t>() as c_int,
            mapToCreateLayerIn: self.target.as_c_ptr(),
            layerType: self.layer_type.into(),
            willBeDeletedCallback: Some(will_be_deleted_trampoline),
            prepCacheCallback: Some(prepare_cache_trampoline),
            drawCallback: Some(draw_trampoline),
            iconCallback: Some(icon_trampoline),
            labelCallback: Some(label_trampoline),
            showUiToggle: self.show_ui_toggle as c_int,
            layerName: c_name.as_ptr(),
            refcon: state as *mut c_void,
        };
        let id = unsafe { XPLMCreateMapLayer(&mut params) };
        if id.is_null() {
            unsafe { drop(Box::from_raw(state)) };
            return None;
        }
        Some(MapLayer { id, state, deleted })
    }
}

unsafe extern "C" fn prepare_cache_trampoline(
    in_layer: XPLMMapLayerID,
    in_total_map_bounds: *const f32,
    projection: XPLMMapProjectionID,
    refcon: *mut c_void,
) {
    crate::guard(|| {
        let state: &MapLayerState = unsafe { &*(refcon as *const MapLayerState) };
        (state.prepare_cache.borrow_mut())(
            MapLayerRef(in_layer),
            read_bounds(in_total_map_bounds),
            MapProjection(projection),
        );
    });
}

unsafe extern "C" fn will_be_deleted_trampoline(in_layer: XPLMMapLayerID, refcon: *mut c_void) {
    crate::guard(|| {
        let state: &MapLayerState = unsafe { &*(refcon as *const MapLayerState) };
        state.deleted.set(true);
        (state.will_be_deleted.borrow_mut())(MapLayerRef(in_layer));
    });
}

unsafe extern "C" fn draw_trampoline(
    in_layer: XPLMMapLayerID,
    in_map_bounds: *const f32,
    zoom_ratio: f32,
    map_units_per_ui_unit: f32,
    map_style: XPLMMapStyle,
    projection: XPLMMapProjectionID,
    refcon: *mut c_void,
) {
    crate::guard(|| {
        let state: &MapLayerState = unsafe { &*(refcon as *const MapLayerState) };
        (state.draw.borrow_mut())(
            MapLayerRef(in_layer),
            read_bounds(in_map_bounds),
            zoom_ratio,
            map_units_per_ui_unit,
            map_style.into(),
            MapProjection(projection),
        );
    });
}

unsafe extern "C" fn icon_trampoline(
    in_layer: XPLMMapLayerID,
    in_map_bounds: *const f32,
    zoom_ratio: f32,
    map_units_per_ui_unit: f32,
    map_style: XPLMMapStyle,
    projection: XPLMMapProjectionID,
    refcon: *mut c_void,
) {
    crate::guard(|| {
        let state: &MapLayerState = unsafe { &*(refcon as *const MapLayerState) };
        (state.draw_icons.borrow_mut())(
            MapLayerRef(in_layer),
            read_bounds(in_map_bounds),
            zoom_ratio,
            map_units_per_ui_unit,
            map_style.into(),
            MapProjection(projection),
        );
    });
}

unsafe extern "C" fn label_trampoline(
    in_layer: XPLMMapLayerID,
    in_map_bounds: *const f32,
    zoom_ratio: f32,
    map_units_per_ui_unit: f32,
    map_style: XPLMMapStyle,
    projection: XPLMMapProjectionID,
    refcon: *mut c_void,
) {
    crate::guard(|| {
        let state: &MapLayerState = unsafe { &*(refcon as *const MapLayerState) };
        (state.draw_labels.borrow_mut())(
            MapLayerRef(in_layer),
            read_bounds(in_map_bounds),
            zoom_ratio,
            map_units_per_ui_unit,
            map_style.into(),
            MapProjection(projection),
        );
    });
}
