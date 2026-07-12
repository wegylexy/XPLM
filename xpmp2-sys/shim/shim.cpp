#include "shim.h"

#include "XPMPAircraft.h"

namespace {

/// The one concrete `XPMP2::Aircraft` subclass this crate ever needs —
/// `UpdatePosition` forwards into a plain C function pointer + refcon
/// instead of being overridden per-callsite in C++, so every plane created
/// through this shim, regardless of what Rust code drives it, shares this
/// single class.
class ShimAircraft final : public XPMP2::Aircraft {
public:
    XPMP2ShimUpdatePositionFn updatePositionFn = nullptr;
    void* refcon = nullptr;

    ShimAircraft(const std::string& icaoType,
                 const std::string& icaoAirline,
                 const std::string& livery,
                 XPMPPlaneID modeSId,
                 const std::string& cslId)
        : XPMP2::Aircraft(icaoType, icaoAirline, livery, modeSId, cslId)
    {}

    void UpdatePosition(float elapsedSinceLastCall, int flCounter) override
    {
        if (updatePositionFn) {
            updatePositionFn(refcon, elapsedSinceLastCall, flCounter);
        }
    }
};

inline ShimAircraft* cast(XPMP2ShimAircraft* p) { return reinterpret_cast<ShimAircraft*>(p); }
inline const ShimAircraft* cast(const XPMP2ShimAircraft* p) { return reinterpret_cast<const ShimAircraft*>(p); }

inline const char* orEmpty(const char* s) { return s ? s : ""; }

} // namespace

extern "C" {

XPMP2ShimAircraft* xpmp2_shim_aircraft_create(
    const char* icao_type,
    const char* icao_airline,
    const char* livery,
    unsigned int mode_s_id,
    const char* csl_id,
    XPMP2ShimUpdatePositionFn update_position,
    void* refcon)
{
    // XPMP2::Aircraft's constructor throws XPMP2::XPMP2Error on failure
    // (invalid/duplicate mode_s_id, no CSL model matched) — caught here so
    // no C++ exception ever unwinds across the extern "C" boundary into
    // Rust, which would be undefined behavior (the mirror image of why
    // every Rust-side trampoline in this workspace goes through
    // `xplm::guard()`).
    try {
        auto* a = new ShimAircraft(
            orEmpty(icao_type),
            orEmpty(icao_airline),
            orEmpty(livery),
            static_cast<XPMPPlaneID>(mode_s_id),
            orEmpty(csl_id)
        );
        a->updatePositionFn = update_position;
        a->refcon = refcon;
        return reinterpret_cast<XPMP2ShimAircraft*>(a);
    } catch (...) {
        return nullptr;
    }
}

void xpmp2_shim_aircraft_destroy(XPMP2ShimAircraft* aircraft)
{
    delete cast(aircraft);
}

unsigned int xpmp2_shim_aircraft_mode_s_id(const XPMP2ShimAircraft* aircraft)
{
    return static_cast<unsigned int>(cast(aircraft)->GetModeS_ID());
}

int xpmp2_shim_aircraft_is_valid(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->IsValid() ? 1 : 0;
}

void xpmp2_shim_aircraft_set_visible(XPMP2ShimAircraft* aircraft, int visible)
{
    cast(aircraft)->SetVisible(visible != 0);
}

int xpmp2_shim_aircraft_is_visible(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->IsVisible() ? 1 : 0;
}

void xpmp2_shim_aircraft_set_location(XPMP2ShimAircraft* aircraft, double lat, double lon, double alt_ft)
{
    cast(aircraft)->SetLocation(lat, lon, alt_ft);
}

void xpmp2_shim_aircraft_get_location(const XPMP2ShimAircraft* aircraft, double* out_lat, double* out_lon, double* out_alt_ft)
{
    double lat = 0, lon = 0, alt_ft = 0;
    // GetLocation is non-const in some XPMP2 versions' signature but does
    // not logically mutate; cast away constness locally rather than widen
    // this shim function's own contract.
    const_cast<ShimAircraft*>(cast(aircraft))->GetLocation(lat, lon, alt_ft);
    if (out_lat) *out_lat = lat;
    if (out_lon) *out_lon = lon;
    if (out_alt_ft) *out_alt_ft = alt_ft;
}

void xpmp2_shim_aircraft_set_local_loc(XPMP2ShimAircraft* aircraft, float x, float y, float z)
{
    cast(aircraft)->SetLocalLoc(x, y, z);
}

void xpmp2_shim_aircraft_set_pitch(XPMP2ShimAircraft* aircraft, float degrees)
{
    cast(aircraft)->SetPitch(degrees);
}

void xpmp2_shim_aircraft_set_heading(XPMP2ShimAircraft* aircraft, float degrees)
{
    cast(aircraft)->SetHeading(degrees);
}

void xpmp2_shim_aircraft_set_roll(XPMP2ShimAircraft* aircraft, float degrees)
{
    cast(aircraft)->SetRoll(degrees);
}

void xpmp2_shim_aircraft_set_on_ground(XPMP2ShimAircraft* aircraft, int on_ground)
{
    cast(aircraft)->bOnGrnd = (on_ground != 0);
}

void xpmp2_shim_aircraft_set_velocity(XPMP2ShimAircraft* aircraft, float vx, float vy, float vz)
{
    auto* a = cast(aircraft);
    a->v_x = vx;
    a->v_y = vy;
    a->v_z = vz;
}

void xpmp2_shim_aircraft_set_label(XPMP2ShimAircraft* aircraft, const char* label)
{
    cast(aircraft)->label = orEmpty(label);
}

void xpmp2_shim_aircraft_set_label_drawn(XPMP2ShimAircraft* aircraft, int drawn)
{
    cast(aircraft)->bDrawLabel = (drawn != 0);
}

size_t xpmp2_shim_aircraft_dataref_count(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->v.size();
}

float xpmp2_shim_aircraft_get_dataref(const XPMP2ShimAircraft* aircraft, size_t index)
{
    const auto* a = cast(aircraft);
    return index < a->v.size() ? a->v[index] : 0.0f;
}

void xpmp2_shim_aircraft_set_dataref(XPMP2ShimAircraft* aircraft, size_t index, float value)
{
    auto* a = cast(aircraft);
    if (index < a->v.size()) {
        a->v[index] = value;
    }
}

} // extern "C"
