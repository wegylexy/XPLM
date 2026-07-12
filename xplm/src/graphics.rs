//! Graphics (`XPLMGraphics.h`): OpenGL fixed-function state, coordinate
//! conversion, and text drawing. Unlike every other module in this crate,
//! there's no RAII or trampoline here — the whole header is stateless free
//! functions, so this is just a thin, ergonomic layer over them.

use std::ffi::CString;
use std::os::raw::c_int;

use xplm_sys::{
    XPLMBindTexture2d, XPLMDrawNumber, XPLMDrawString, XPLMDrawTranslucentDarkBox, XPLMFontID,
    XPLMGenerateTextureNumbers, XPLMGetFontDimensions, XPLMGetTexture, XPLMLocalToWorld,
    XPLMSetGraphicsState, XPLMTextureID, XPLMWorldToLocal, xplmFont_Basic, xplm_Tex_GeneralInterface,
};

#[cfg(feature = "XPLM200")]
use xplm_sys::{XPLMMeasureString, xplmFont_Proportional};

/// The OpenGL fixed-function state `XPLMSetGraphicsState` controls. Prefer
/// setting all fields explicitly (there is no "default" state, per the SDK
/// docs) rather than relying on `Default` meaning anything in particular —
/// `Default` here is all-off/one-texture-unit purely as a starting point to
/// `..` from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphicsState {
    pub fog: bool,
    /// 0 disables 2D texturing entirely; otherwise enables this many
    /// sequential texturing units starting at unit 0.
    pub texture_units: i32,
    pub lighting: bool,
    pub alpha_testing: bool,
    pub alpha_blending: bool,
    pub depth_testing: bool,
    pub depth_writing: bool,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            fog: false,
            texture_units: 1,
            lighting: false,
            alpha_testing: false,
            alpha_blending: false,
            depth_testing: false,
            depth_writing: false,
        }
    }
}

impl GraphicsState {
    pub fn apply(&self) {
        unsafe {
            XPLMSetGraphicsState(
                self.fog as c_int,
                self.texture_units,
                self.lighting as c_int,
                self.alpha_testing as c_int,
                self.alpha_blending as c_int,
                self.depth_testing as c_int,
                self.depth_writing as c_int,
            )
        }
    }
}

/// Changes the bound 2D texture on `texture_unit` (0-based, up to 4 units).
/// Prefer this over a raw `glBindTexture` call — it's cached across plugins
/// to skip redundant rebinding.
pub fn bind_texture_2d(texture_num: i32, texture_unit: i32) {
    unsafe { XPLMBindTexture2d(texture_num, texture_unit) }
}

/// Generates `count` new OpenGL texture object IDs, reserved so plugins
/// don't collide with IDs X-Plane uses internally. Prefer this over
/// `glGenTextures`.
pub fn generate_texture_numbers(count: usize) -> Vec<i32> {
    let mut ids = vec![0; count];
    unsafe { XPLMGenerateTextureNumbers(ids.as_mut_ptr(), count as c_int) };
    ids
}

/// A well-known X-Plane texture, nameable via [`get_texture`]. Per the SDK's
/// own warning, everything except [`Texture::GeneralInterface`] is
/// deprecated/legacy or narrowly scoped (radar instruments) — this crate
/// only exposes the non-deprecated variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Texture {
    /// Window outlines, button outlines, fonts, etc.
    GeneralInterface,
    #[cfg(feature = "XPLM420")]
    RadarPilot,
    #[cfg(feature = "XPLM420")]
    RadarCopilot,
    /// The exterior paint for the user's aircraft (daytime). Requires the
    /// `deprecated` feature (off by default) — get this via the Widgets
    /// library instead, per the SDK header.
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "use the Widgets library instead, per XPLMGraphics.h")]
    AircraftPaint,
    /// The exterior light map for the user's aircraft. Same caveats as
    /// [`Texture::AircraftPaint`].
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "use the Widgets library instead, per XPLMGraphics.h")]
    AircraftLiteMap,
}

impl From<Texture> for XPLMTextureID {
    #[allow(deprecated)]
    fn from(t: Texture) -> Self {
        match t {
            Texture::GeneralInterface => xplm_Tex_GeneralInterface,
            #[cfg(feature = "XPLM420")]
            Texture::RadarPilot => xplm_sys::xplm_Tex_Radar_Pilot,
            #[cfg(feature = "XPLM420")]
            Texture::RadarCopilot => xplm_sys::xplm_Tex_Radar_Copilot,
            #[cfg(feature = "deprecated")]
            Texture::AircraftPaint => xplm_sys::xplm_Tex_AircraftPaint,
            #[cfg(feature = "deprecated")]
            Texture::AircraftLiteMap => xplm_sys::xplm_Tex_AircraftLiteMap,
        }
    }
}

/// Returns the OpenGL texture ID backing a well-known X-Plane texture.
pub fn get_texture(texture: Texture) -> i32 {
    unsafe { XPLMGetTexture(texture.into()) }
}

/// Converts (latitude, longitude, altitude-MSL-meters) to local OpenGL
/// (x, y, z) meters. Do not implement this conversion yourself — the sim's
/// notion of "round" may not match a naive formula, and users can customize
/// planet physics.
pub fn world_to_local(latitude: f64, longitude: f64, altitude: f64) -> (f64, f64, f64) {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    unsafe { XPLMWorldToLocal(latitude, longitude, altitude, &mut x, &mut y, &mut z) };
    (x, y, z)
}

/// The inverse of [`world_to_local`]. Less precise than local coordinates —
/// avoid round-tripping local -> world -> local.
pub fn local_to_world(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let (mut latitude, mut longitude, mut altitude) = (0.0, 0.0, 0.0);
    unsafe { XPLMLocalToWorld(x, y, z, &mut latitude, &mut longitude, &mut altitude) };
    (latitude, longitude, altitude)
}

/// Draws a translucent dark box — the same primitive X-Plane uses behind
/// its own text display, handy for making overlaid text readable.
pub fn draw_translucent_dark_box(left: i32, top: i32, right: i32, bottom: i32) {
    unsafe { XPLMDrawTranslucentDarkBox(left, top, right, bottom) }
}

/// One of X-Plane's built-in fixed-character fonts. The two non-deprecated
/// variants are always available; the rest require the `deprecated` feature
/// (off by default) and are `#[deprecated]` themselves — the SDK header's
/// own warning is "Deprecated, do not use" for every one of them, without
/// exception, so this crate doesn't second-guess which ones are "probably
/// still fine."
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    /// Mono-spaced UI font, available in every SDK version.
    Basic,
    #[cfg(feature = "XPLM200")]
    Proportional,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    Menus,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    Metal,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    Led,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    LedWide,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    PanelHud,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    PanelEfis,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    PanelGps,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    RadiosGa,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    RadiosBc,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    RadiosHm,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    RadiosGaNarrow,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    RadiosBcNarrow,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    RadiosHmNarrow,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    Timer,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    FullRound,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    SmallRound,
    #[cfg(feature = "deprecated")]
    #[deprecated(note = "per XPLMGraphics.h: deprecated, do not use")]
    MenusLocalized,
}

impl From<Font> for XPLMFontID {
    #[allow(deprecated)]
    fn from(f: Font) -> Self {
        match f {
            Font::Basic => xplmFont_Basic,
            #[cfg(feature = "XPLM200")]
            Font::Proportional => xplmFont_Proportional,
            #[cfg(feature = "deprecated")]
            Font::Menus => xplm_sys::xplmFont_Menus,
            #[cfg(feature = "deprecated")]
            Font::Metal => xplm_sys::xplmFont_Metal,
            #[cfg(feature = "deprecated")]
            Font::Led => xplm_sys::xplmFont_Led,
            #[cfg(feature = "deprecated")]
            Font::LedWide => xplm_sys::xplmFont_LedWide,
            #[cfg(feature = "deprecated")]
            Font::PanelHud => xplm_sys::xplmFont_PanelHUD,
            #[cfg(feature = "deprecated")]
            Font::PanelEfis => xplm_sys::xplmFont_PanelEFIS,
            #[cfg(feature = "deprecated")]
            Font::PanelGps => xplm_sys::xplmFont_PanelGPS,
            #[cfg(feature = "deprecated")]
            Font::RadiosGa => xplm_sys::xplmFont_RadiosGA,
            #[cfg(feature = "deprecated")]
            Font::RadiosBc => xplm_sys::xplmFont_RadiosBC,
            #[cfg(feature = "deprecated")]
            Font::RadiosHm => xplm_sys::xplmFont_RadiosHM,
            #[cfg(feature = "deprecated")]
            Font::RadiosGaNarrow => xplm_sys::xplmFont_RadiosGANarrow,
            #[cfg(feature = "deprecated")]
            Font::RadiosBcNarrow => xplm_sys::xplmFont_RadiosBCNarrow,
            #[cfg(feature = "deprecated")]
            Font::RadiosHmNarrow => xplm_sys::xplmFont_RadiosHMNarrow,
            #[cfg(feature = "deprecated")]
            Font::Timer => xplm_sys::xplmFont_Timer,
            #[cfg(feature = "deprecated")]
            Font::FullRound => xplm_sys::xplmFont_FullRound,
            #[cfg(feature = "deprecated")]
            Font::SmallRound => xplm_sys::xplmFont_SmallRound,
            #[cfg(feature = "deprecated")]
            Font::MenusLocalized => xplm_sys::xplmFont_Menus_Localized,
        }
    }
}

/// Draws `text` with its lower-left corner at `(x, y)`, in `color` (RGB,
/// each `0.0..=1.0`). `word_wrap_width`, if given, wraps to that many pixels.
/// Returns the x-offset plus the width of everything drawn.
pub fn draw_string(
    color: [f32; 3],
    x: i32,
    y: i32,
    text: &str,
    word_wrap_width: Option<i32>,
    font: Font,
) {
    let Ok(c_text) = CString::new(text) else {
        return;
    };
    let mut color = color;
    let mut word_wrap_width = word_wrap_width;
    unsafe {
        XPLMDrawString(
            color.as_mut_ptr(),
            x,
            y,
            c_text.as_ptr() as *mut _,
            word_wrap_width
                .as_mut()
                .map_or(std::ptr::null_mut(), |w| w as *mut i32),
            font.into(),
        )
    }
}

/// Draws `value` with its lower-left corner at `(x, y)`, matching
/// PlaneMaker/X-Plane's digit-editing field style: `integer_digits` and
/// `decimal_digits` control how many of each are shown, and `show_sign`
/// forces a leading `+`/`-`.
#[allow(clippy::too_many_arguments)]
pub fn draw_number(
    color: [f32; 3],
    x: i32,
    y: i32,
    value: f64,
    integer_digits: i32,
    decimal_digits: i32,
    show_sign: bool,
    font: Font,
) {
    let mut color = color;
    unsafe {
        XPLMDrawNumber(
            color.as_mut_ptr(),
            x,
            y,
            value,
            integer_digits,
            decimal_digits,
            show_sign as c_int,
            font.into(),
        )
    }
}

/// `(char_width, char_height, digits_only)` for `font`. For a proportional
/// font, `char_width` is an arbitrary average width, not exact.
pub fn font_dimensions(font: Font) -> (i32, i32, bool) {
    let (mut width, mut height, mut digits_only) = (0, 0, 0);
    unsafe { XPLMGetFontDimensions(font.into(), &mut width, &mut height, &mut digits_only) };
    (width, height, digits_only != 0)
}

/// The width in pixels `text` would occupy when drawn in `font`.
#[cfg(feature = "XPLM200")]
pub fn measure_string(font: Font, text: &str) -> f32 {
    // XPLMMeasureString takes a pointer + length rather than a NUL-terminated
    // string, so it works on the UTF-8 bytes directly without a CString.
    unsafe { XPLMMeasureString(font.into(), text.as_ptr() as *const _, text.len() as c_int) }
}
