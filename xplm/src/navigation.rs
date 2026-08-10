//! Navigation database, legacy FMS, and GPS receiver access
//! (`XPLMNavigation.h`). The newer per-device FMS/GPS flight-plan API
//! (`XPLMNavFlightPlan`, XPLM410+) lives alongside it as [`FlightPlan`].

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use xplm_sys::{
    xplm_Nav_Airport, xplm_Nav_DME, xplm_Nav_Fix, xplm_Nav_GlideSlope, xplm_Nav_ILS,
    xplm_Nav_InnerMarker, xplm_Nav_LatLon, xplm_Nav_Localizer, xplm_Nav_MiddleMarker, xplm_Nav_NDB,
    xplm_Nav_OuterMarker, xplm_Nav_TACAN, xplm_Nav_Unknown, xplm_Nav_VOR, XPLMClearFMSEntry,
    XPLMCountFMSEntries, XPLMFindFirstNavAidOfType, XPLMFindLastNavAidOfType, XPLMFindNavAid,
    XPLMGetDestinationFMSEntry, XPLMGetDisplayedFMSEntry, XPLMGetFMSEntryInfo, XPLMGetFirstNavAid,
    XPLMGetGPSDestination, XPLMGetGPSDestinationType, XPLMGetNavAidInfo, XPLMGetNextNavAid,
    XPLMNavRef, XPLMNavType, XPLMSetDestinationFMSEntry, XPLMSetDisplayedFMSEntry,
    XPLMSetFMSEntryInfo, XPLMSetFMSEntryLatLon, XPLM_NAV_NOT_FOUND,
};

#[cfg(feature = "XPLM410")]
use xplm_sys::{
    xplm_Fpl_CoPilot_Approach, xplm_Fpl_CoPilot_Primary, xplm_Fpl_CoPilot_Temporary,
    xplm_Fpl_Pilot_Approach, xplm_Fpl_Pilot_Primary, xplm_Fpl_Pilot_Temporary,
    XPLMClearFMSFlightPlanEntry, XPLMCountFMSFlightPlanEntries,
    XPLMGetDestinationFMSFlightPlanEntry, XPLMGetDisplayedFMSFlightPlanEntry,
    XPLMGetFMSFlightPlanEntryInfo, XPLMLoadFMSFlightPlan, XPLMNavFlightPlan, XPLMSaveFMSFlightPlan,
    XPLMSetDestinationFMSFlightPlanEntry, XPLMSetDirectToFMSFlightPlanEntry,
    XPLMSetDisplayedFMSFlightPlanEntry, XPLMSetFMSFlightPlanEntryInfo,
    XPLMSetFMSFlightPlanEntryLatLon, XPLMSetFMSFlightPlanEntryLatLonWithId,
};

fn read_c_buf(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// `XPLMNavType`'s bits — which kinds of navaid a search should consider.
/// Bitwise-OR (`|`) together to search multiple types at once, as the raw
/// SDK constants do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavTypeMask(XPLMNavType);

impl NavTypeMask {
    pub const NONE: Self = Self(xplm_Nav_Unknown);
    pub const AIRPORT: Self = Self(xplm_Nav_Airport);
    pub const NDB: Self = Self(xplm_Nav_NDB);
    pub const VOR: Self = Self(xplm_Nav_VOR);
    pub const ILS: Self = Self(xplm_Nav_ILS);
    pub const LOCALIZER: Self = Self(xplm_Nav_Localizer);
    pub const GLIDE_SLOPE: Self = Self(xplm_Nav_GlideSlope);
    pub const OUTER_MARKER: Self = Self(xplm_Nav_OuterMarker);
    pub const MIDDLE_MARKER: Self = Self(xplm_Nav_MiddleMarker);
    pub const INNER_MARKER: Self = Self(xplm_Nav_InnerMarker);
    pub const FIX: Self = Self(xplm_Nav_Fix);
    pub const DME: Self = Self(xplm_Nav_DME);
    /// A specific lat/lon coordinate entered into the FMS — never present in
    /// the database itself, only returned when querying an FMS/flight-plan
    /// entry. Use [`Fms::set_entry_lat_lon`] to program one, not a navaid
    /// search.
    pub const LAT_LON: Self = Self(xplm_Nav_LatLon);
    pub const TACAN: Self = Self(xplm_Nav_TACAN);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn raw(self) -> XPLMNavType {
        self.0
    }
}

impl std::ops::BitOr for NavTypeMask {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl From<XPLMNavType> for NavTypeMask {
    fn from(raw: XPLMNavType) -> Self {
        Self(raw)
    }
}

/// Full info about a [`NavAid`], per `XPLMGetNavAidInfo`. Frequencies use
/// the nav.dat convention: NDB frequencies are exact, all others are
/// multiplied by 100.
#[derive(Clone, Debug, PartialEq)]
pub struct NavAidInfo {
    pub nav_type: NavTypeMask,
    pub latitude: f32,
    pub longitude: f32,
    pub height: f32,
    pub frequency: i32,
    pub heading: f32,
    pub id: String,
    pub name: String,
    /// Whether this navaid is within the local "region" of loaded DSFs.
    pub in_local_region: bool,
}

/// A reference into X-Plane's in-memory navigation database
/// (`XPLMNavRef`) — an iterator/handle, not an owned resource; there is
/// nothing to free. Like-typed navaids are grouped together, but the
/// database is not necessarily densely populated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavAid(XPLMNavRef);

impl NavAid {
    /// The very first navaid in the database, for traversing all of it.
    /// `None` if the database is empty.
    pub fn first() -> Option<Self> {
        let r = unsafe { XPLMGetFirstNavAid() };
        (r != XPLM_NAV_NOT_FOUND).then_some(Self(r))
    }

    /// The navaid after this one in the database. `None` if this was the
    /// last one.
    pub fn next(&self) -> Option<Self> {
        let r = unsafe { XPLMGetNextNavAid(self.0) };
        (r != XPLM_NAV_NOT_FOUND).then_some(Self(r))
    }

    /// The first navaid of exactly one given type. `None` if there are none.
    pub fn first_of_type(nav_type: NavTypeMask) -> Option<Self> {
        let r = unsafe { XPLMFindFirstNavAidOfType(nav_type.raw()) };
        (r != XPLM_NAV_NOT_FOUND).then_some(Self(r))
    }

    /// The last navaid of exactly one given type. `None` if there are none.
    pub fn last_of_type(nav_type: NavTypeMask) -> Option<Self> {
        let r = unsafe { XPLMFindLastNavAidOfType(nav_type.raw()) };
        (r != XPLM_NAV_NOT_FOUND).then_some(Self(r))
    }

    /// General-purpose search — see `XPLMFindNavAid`'s docs for how the
    /// optional filters combine: `near` finds the nearest match to a
    /// lat/lon (otherwise the last match found is returned); `frequency`
    /// screens out non-matching radio beacons; the name/ID fragments filter
    /// by substring. `types` may combine multiple [`NavTypeMask`] bits.
    pub fn find(
        name_fragment: Option<&str>,
        id_fragment: Option<&str>,
        near: Option<(f32, f32)>,
        frequency: Option<i32>,
        types: NavTypeMask,
    ) -> Option<Self> {
        let c_name = name_fragment.map(CString::new).transpose().ok()?;
        let c_id = id_fragment.map(CString::new).transpose().ok()?;
        let (mut lat, mut lon) = near.unwrap_or_default();
        let mut freq = frequency.unwrap_or_default();
        let r = unsafe {
            XPLMFindNavAid(
                c_name.as_deref().map_or(std::ptr::null(), CStr::as_ptr),
                c_id.as_deref().map_or(std::ptr::null(), CStr::as_ptr),
                if near.is_some() {
                    &mut lat
                } else {
                    std::ptr::null_mut()
                },
                if near.is_some() {
                    &mut lon
                } else {
                    std::ptr::null_mut()
                },
                if frequency.is_some() {
                    &mut freq
                } else {
                    std::ptr::null_mut()
                },
                types.raw(),
            )
        };
        (r != XPLM_NAV_NOT_FOUND).then_some(Self(r))
    }

    /// This navaid's full info.
    pub fn info(&self) -> NavAidInfo {
        let mut nav_type: XPLMNavType = xplm_Nav_Unknown;
        let (mut latitude, mut longitude, mut height, mut heading) =
            (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let mut frequency: i32 = 0;
        let mut id = [0u8; 32];
        let mut name = [0u8; 256];
        let mut in_region: u8 = 0;
        unsafe {
            XPLMGetNavAidInfo(
                self.0,
                &mut nav_type,
                &mut latitude,
                &mut longitude,
                &mut height,
                &mut frequency,
                &mut heading,
                id.as_mut_ptr() as *mut c_char,
                name.as_mut_ptr() as *mut c_char,
                &mut in_region as *mut u8 as *mut c_char,
            );
        }
        NavAidInfo {
            nav_type: NavTypeMask(nav_type),
            latitude,
            longitude,
            height,
            frequency,
            heading,
            id: read_c_buf(&id),
            name: read_c_buf(&name),
            in_local_region: in_region != 0,
        }
    }
}

/// One entry in the legacy (single-flight-plan) FMS, per [`Fms`].
#[derive(Clone, Debug, PartialEq)]
pub struct FmsEntryInfo {
    pub nav_type: NavTypeMask,
    pub id: String,
    /// `None` if this isn't a navaid/airport entry, or the async database
    /// lookup for it hasn't completed yet.
    pub navaid: Option<NavAid>,
    pub altitude: i32,
    pub latitude: f32,
    pub longitude: f32,
}

/// The legacy, single-flight-plan FMS (`XPLMCountFMSEntries` and friends) —
/// zero-sized, since the SDK has exactly one such FMS; see [`FlightPlan`]
/// for the newer per-device/per-plan API (XPLM410+). Entries are indexed
/// `0..Fms::count()`, and must stay contiguous — clearing an entry at the
/// end shortens the effective flight plan (max 100 waypoints).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fms;

impl Fms {
    pub fn count() -> i32 {
        unsafe { XPLMCountFMSEntries() }
    }

    /// The index of the entry the pilot is currently viewing.
    pub fn displayed_entry() -> i32 {
        unsafe { XPLMGetDisplayedFMSEntry() }
    }

    /// The index of the entry the FMS is flying to (the track runs from
    /// `index - 1` to `index`).
    pub fn destination_entry() -> i32 {
        unsafe { XPLMGetDestinationFMSEntry() }
    }

    pub fn set_displayed_entry(index: i32) {
        unsafe { XPLMSetDisplayedFMSEntry(index) }
    }

    pub fn set_destination_entry(index: i32) {
        unsafe { XPLMSetDestinationFMSEntry(index) }
    }

    /// Info about entry `index`. `navaid` starts as `XPLM_NAV_NOT_FOUND`
    /// before the call, per the SDK's own workaround for a pre-11.31 bug
    /// where it's otherwise left unset rather than cleared on a miss.
    pub fn entry_info(index: i32) -> FmsEntryInfo {
        let mut nav_type: XPLMNavType = xplm_Nav_Unknown;
        let mut id = [0u8; 256];
        let mut nav_ref: XPLMNavRef = XPLM_NAV_NOT_FOUND;
        let mut altitude: i32 = 0;
        let (mut latitude, mut longitude) = (0.0f32, 0.0f32);
        unsafe {
            XPLMGetFMSEntryInfo(
                index,
                &mut nav_type,
                id.as_mut_ptr() as *mut c_char,
                &mut nav_ref,
                &mut altitude,
                &mut latitude,
                &mut longitude,
            );
        }
        FmsEntryInfo {
            nav_type: NavTypeMask(nav_type),
            id: read_c_buf(&id),
            navaid: (nav_ref != XPLM_NAV_NOT_FOUND).then_some(NavAid(nav_ref)),
            altitude,
            latitude,
            longitude,
        }
    }

    /// Sets entry `index` to `navaid` at `altitude` — airports, fixes, VORs,
    /// and NDBs only.
    pub fn set_entry(index: i32, navaid: NavAid, altitude: i32) {
        unsafe { XPLMSetFMSEntryInfo(index, navaid.0, altitude) }
    }

    /// Sets entry `index` to a lat/lon waypoint.
    pub fn set_entry_lat_lon(index: i32, latitude: f32, longitude: f32, altitude: i32) {
        unsafe { XPLMSetFMSEntryLatLon(index, latitude, longitude, altitude) }
    }

    /// Clears entry `index`, potentially shortening the flight plan.
    pub fn clear_entry(index: i32) {
        unsafe { XPLMClearFMSEntry(index) }
    }
}

/// Which per-device/per-purpose flight plan a [`FlightPlan`] call targets
/// (XPLM410+) — an aircraft may have up to two navigation devices (GPS or
/// FMS), each with up to two flight plans (a GPS has enroute/approach; an
/// FMS has active/temporary). Targeting a plan the aircraft doesn't have is
/// a no-op per the SDK.
#[cfg(feature = "XPLM410")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightPlanId {
    PilotPrimary,
    CoPilotPrimary,
    PilotApproach,
    CoPilotApproach,
    PilotTemporary,
    CoPilotTemporary,
}

#[cfg(feature = "XPLM410")]
impl From<FlightPlanId> for XPLMNavFlightPlan {
    fn from(id: FlightPlanId) -> Self {
        match id {
            FlightPlanId::PilotPrimary => xplm_Fpl_Pilot_Primary,
            FlightPlanId::CoPilotPrimary => xplm_Fpl_CoPilot_Primary,
            FlightPlanId::PilotApproach => xplm_Fpl_Pilot_Approach,
            FlightPlanId::CoPilotApproach => xplm_Fpl_CoPilot_Approach,
            FlightPlanId::PilotTemporary => xplm_Fpl_Pilot_Temporary,
            FlightPlanId::CoPilotTemporary => xplm_Fpl_CoPilot_Temporary,
        }
    }
}

/// A handle to one of an aircraft's flight plans (XPLM410+) — the modern,
/// per-device counterpart to [`Fms`]. See [`FlightPlanId`] for which plan
/// each variant targets.
#[cfg(feature = "XPLM410")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlightPlan(XPLMNavFlightPlan);

#[cfg(feature = "XPLM410")]
impl FlightPlan {
    pub fn new(id: FlightPlanId) -> Self {
        Self(id.into())
    }

    pub fn count(&self) -> i32 {
        unsafe { XPLMCountFMSFlightPlanEntries(self.0) }
    }

    pub fn displayed_entry(&self) -> i32 {
        unsafe { XPLMGetDisplayedFMSFlightPlanEntry(self.0) }
    }

    pub fn destination_entry(&self) -> i32 {
        unsafe { XPLMGetDestinationFMSFlightPlanEntry(self.0) }
    }

    pub fn set_displayed_entry(&self, index: i32) {
        unsafe { XPLMSetDisplayedFMSFlightPlanEntry(self.0, index) }
    }

    pub fn set_destination_entry(&self, index: i32) {
        unsafe { XPLMSetDestinationFMSFlightPlanEntry(self.0, index) }
    }

    /// Flies directly from the aircraft's current position to entry
    /// `index`, ignoring the point before it.
    pub fn set_direct_to_entry(&self, index: i32) {
        unsafe { XPLMSetDirectToFMSFlightPlanEntry(self.0, index) }
    }

    pub fn entry_info(&self, index: i32) -> FmsEntryInfo {
        let mut nav_type: XPLMNavType = xplm_Nav_Unknown;
        let mut id = [0u8; 256];
        let mut nav_ref: XPLMNavRef = XPLM_NAV_NOT_FOUND;
        let mut altitude: i32 = 0;
        let (mut latitude, mut longitude) = (0.0f32, 0.0f32);
        unsafe {
            XPLMGetFMSFlightPlanEntryInfo(
                self.0,
                index,
                &mut nav_type,
                id.as_mut_ptr() as *mut c_char,
                &mut nav_ref,
                &mut altitude,
                &mut latitude,
                &mut longitude,
            );
        }
        FmsEntryInfo {
            nav_type: NavTypeMask(nav_type),
            id: read_c_buf(&id),
            navaid: (nav_ref != XPLM_NAV_NOT_FOUND).then_some(NavAid(nav_ref)),
            altitude,
            latitude,
            longitude,
        }
    }

    /// Airports, fixes, and VOR/NDB/TACAN radio beacons only.
    pub fn set_entry(&self, index: i32, navaid: NavAid, altitude: i32) {
        unsafe { XPLMSetFMSFlightPlanEntryInfo(self.0, index, navaid.0, altitude) }
    }

    pub fn set_entry_lat_lon(&self, index: i32, latitude: f32, longitude: f32, altitude: i32) {
        unsafe { XPLMSetFMSFlightPlanEntryLatLon(self.0, index, latitude, longitude, altitude) }
    }

    /// Same as [`Self::set_entry_lat_lon`], but with an explicit display ID
    /// for the waypoint. `false` if `id` contains an interior NUL.
    pub fn set_entry_lat_lon_with_id(
        &self,
        index: i32,
        latitude: f32,
        longitude: f32,
        altitude: i32,
        id: &str,
    ) -> bool {
        let Ok(c_id) = CString::new(id) else {
            return false;
        };
        let bytes = c_id.as_bytes();
        unsafe {
            XPLMSetFMSFlightPlanEntryLatLonWithId(
                self.0,
                index,
                latitude,
                longitude,
                altitude,
                c_id.as_ptr(),
                bytes.len() as u32,
            )
        };
        true
    }

    pub fn clear_entry(&self, index: i32) {
        unsafe { XPLMClearFMSFlightPlanEntry(self.0, index) }
    }
}

/// Loads an X-Plane 11+ formatted flight plan (including instrument
/// procedures) into `device` (`0` for pilot-side, `1` for co-pilot-side).
#[cfg(feature = "XPLM410")]
pub fn load_flight_plan(device: i32, plan_text: &str) {
    let bytes = plan_text.as_bytes();
    unsafe { XPLMLoadFMSFlightPlan(device, bytes.as_ptr() as *const c_char, bytes.len() as u32) }
}

/// Saves `device`'s (`0` pilot-side, `1` co-pilot-side) current flight plan
/// in X-Plane 11+ format.
#[cfg(feature = "XPLM410")]
pub fn save_flight_plan(device: i32) -> String {
    // First call (zero-length buffer) to discover the required length
    // (including NUL) — `Vec::new()`'s pointer is non-null even though
    // there's no backing allocation.
    let mut probe: Vec<u8> = Vec::new();
    let needed = unsafe { XPLMSaveFMSFlightPlan(device, probe.as_mut_ptr() as *mut c_char, 0) };
    if needed == 0 {
        return String::new();
    }
    let mut buf = vec![0u8; needed as usize];
    unsafe { XPLMSaveFMSFlightPlan(device, buf.as_mut_ptr() as *mut c_char, buf.len() as u32) };
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// The type of the currently selected GPS destination (a single type, not a
/// mask).
pub fn gps_destination_type() -> NavTypeMask {
    NavTypeMask(unsafe { XPLMGetGPSDestinationType() })
}

/// The GPS's current destination. `None` if there is none.
pub fn gps_destination() -> Option<NavAid> {
    let r = unsafe { XPLMGetGPSDestination() };
    (r != XPLM_NAV_NOT_FOUND).then_some(NavAid(r))
}
