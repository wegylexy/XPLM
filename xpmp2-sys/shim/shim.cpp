#include "shim.h"

#include <cstring>

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

/// Copies `s` into `buf` (up to `buf_len - 1` bytes, NUL-terminated), same
/// truncation contract as BSD `strlcpy`. `buf`/`buf_len` of `0`/`NULL` is a
/// no-op. Returns `s.size()` either way, so callers can detect truncation by
/// comparing the return value against `buf_len`.
size_t copyToBuf(const std::string& s, char* buf, size_t buf_len)
{
    if (buf && buf_len > 0) {
        size_t n = s.size() < buf_len - 1 ? s.size() : buf_len - 1;
        std::memcpy(buf, s.data(), n);
        buf[n] = '\0';
    }
    return s.size();
}

/// `XPMPInfoTexts_t`'s fields are fixed-size `char[]` arrays, not
/// `std::string` — copies `s` in, truncating to fit, always NUL-terminated.
template <size_t N>
void copyToField(const char* s, char (&field)[N])
{
    size_t n = std::strlen(s);
    if (n > N - 1) n = N - 1;
    std::memcpy(field, s, n);
    field[n] = '\0';
}

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

void xpmp2_shim_aircraft_set_radar(XPMP2ShimAircraft* aircraft, long code, int mode)
{
    auto* a = cast(aircraft);
    a->acRadar.code = code;
    a->acRadar.mode = static_cast<XPMPTransponderMode>(mode);
}

void xpmp2_shim_aircraft_set_label_color(XPMP2ShimAircraft* aircraft, float r, float g, float b, float a)
{
    auto* ac = cast(aircraft);
    ac->colLabel[0] = r;
    ac->colLabel[1] = g;
    ac->colLabel[2] = b;
    ac->colLabel[3] = a;
}

void xpmp2_shim_aircraft_set_vert_ofs_ratio(XPMP2ShimAircraft* aircraft, float ratio)
{
    cast(aircraft)->vertOfsRatio = ratio;
}

float xpmp2_shim_aircraft_get_vert_ofs(const XPMP2ShimAircraft* aircraft)
{
    return const_cast<ShimAircraft*>(cast(aircraft))->GetVertOfs();
}

void xpmp2_shim_aircraft_set_gear_deflect_ratio(XPMP2ShimAircraft* aircraft, float ratio)
{
    cast(aircraft)->gearDeflectRatio = ratio;
}

void xpmp2_shim_aircraft_set_clamp_to_ground(XPMP2ShimAircraft* aircraft, int clamp)
{
    cast(aircraft)->bClampToGround = (clamp != 0);
}

void xpmp2_shim_aircraft_set_ai_priority(XPMP2ShimAircraft* aircraft, int priority)
{
    cast(aircraft)->aiPrio = priority;
}

void xpmp2_shim_aircraft_set_render(XPMP2ShimAircraft* aircraft, int render)
{
    cast(aircraft)->SetRender(render != 0);
}

int xpmp2_shim_aircraft_is_rendered(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->IsRendered() ? 1 : 0;
}

int xpmp2_shim_aircraft_tcas_target_idx(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->GetTcasTargetIdx();
}

int xpmp2_shim_aircraft_is_shown_as_tcas_target(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->IsCurrentlyShownAsTcasTarget() ? 1 : 0;
}

int xpmp2_shim_aircraft_is_shown_as_ai(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->IsCurrentlyShownAsAI() ? 1 : 0;
}

int xpmp2_shim_aircraft_show_as_ai_plane(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->ShowAsAIPlane() ? 1 : 0;
}

float xpmp2_shim_aircraft_camera_dist(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->GetCameraDist();
}

float xpmp2_shim_aircraft_camera_bearing(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->GetCameraBearing();
}

float xpmp2_shim_aircraft_ground_speed_kn(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->GetGS_kn();
}

int xpmp2_shim_aircraft_is_related_to(const XPMP2ShimAircraft* aircraft, const char* icao_type)
{
    return cast(aircraft)->IsRelatedTo(orEmpty(icao_type)) ? 1 : 0;
}

int xpmp2_shim_aircraft_is_ground_vehicle(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->IsGroundVehicle() ? 1 : 0;
}

int xpmp2_shim_aircraft_is_glider(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->IsGlider() ? 1 : 0;
}

int xpmp2_shim_aircraft_change_model(
    XPMP2ShimAircraft* aircraft,
    const char* icao_type,
    const char* icao_airline,
    const char* livery)
{
    return cast(aircraft)->ChangeModel(orEmpty(icao_type), orEmpty(icao_airline), orEmpty(livery));
}

int xpmp2_shim_aircraft_rematch_model(XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->ReMatchModel();
}

int xpmp2_shim_aircraft_match_quality(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->GetMatchQuality();
}

size_t xpmp2_shim_aircraft_get_model_name(const XPMP2ShimAircraft* aircraft, char* buf, size_t buf_len)
{
    return copyToBuf(cast(aircraft)->GetModelName(), buf, buf_len);
}

size_t xpmp2_shim_aircraft_get_flight_id(const XPMP2ShimAircraft* aircraft, char* buf, size_t buf_len)
{
    return copyToBuf(cast(aircraft)->GetFlightId(), buf, buf_len);
}

void xpmp2_shim_aircraft_set_info_texts(
    XPMP2ShimAircraft* aircraft,
    const char* tail_num,
    const char* icao_ac_type,
    const char* manufacturer,
    const char* model,
    const char* icao_airline,
    const char* airline,
    const char* flight_num,
    const char* apt_from,
    const char* apt_to)
{
    auto& t = cast(aircraft)->acInfoTexts;
    copyToField(orEmpty(tail_num), t.tailNum);
    copyToField(orEmpty(icao_ac_type), t.icaoAcType);
    copyToField(orEmpty(manufacturer), t.manufacturer);
    copyToField(orEmpty(model), t.model);
    copyToField(orEmpty(icao_airline), t.icaoAirline);
    copyToField(orEmpty(airline), t.airline);
    copyToField(orEmpty(flight_num), t.flightNum);
    copyToField(orEmpty(apt_from), t.aptFrom);
    copyToField(orEmpty(apt_to), t.aptTo);
}

void xpmp2_shim_aircraft_set_wing_span(XPMP2ShimAircraft* aircraft, float meters)
{
    cast(aircraft)->SetWingSpan(meters);
}

void xpmp2_shim_aircraft_set_wing_area(XPMP2ShimAircraft* aircraft, float square_meters)
{
    cast(aircraft)->SetWingArea(square_meters);
}

void xpmp2_shim_aircraft_set_mass(XPMP2ShimAircraft* aircraft, float kg)
{
    cast(aircraft)->SetMass(kg);
}

void xpmp2_shim_aircraft_wake_apply_defaults(XPMP2ShimAircraft* aircraft, int overwrite_all)
{
    cast(aircraft)->WakeApplyDefaults(overwrite_all != 0);
}

void xpmp2_shim_aircraft_contrail_request(XPMP2ShimAircraft* aircraft, unsigned num, unsigned dist_m, unsigned life_time_s)
{
    cast(aircraft)->ContrailRequest(num, dist_m, life_time_s);
}

void xpmp2_shim_aircraft_contrail_remove(XPMP2ShimAircraft* aircraft)
{
    cast(aircraft)->ContrailRemove();
}

unsigned xpmp2_shim_aircraft_contrail_trigger(XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->ContrailTrigger();
}

void xpmp2_shim_aircraft_set_sound_min_dist(XPMP2ShimAircraft* aircraft, int meters)
{
    cast(aircraft)->sndMinDist = meters;
}

void xpmp2_shim_aircraft_set_sound_muted(XPMP2ShimAircraft* aircraft, int mute)
{
    cast(aircraft)->SoundMuteAll(mute != 0);
}

int xpmp2_shim_aircraft_is_sound_muted(const XPMP2ShimAircraft* aircraft)
{
    return cast(aircraft)->SoundIsMuted() ? 1 : 0;
}

} // extern "C"
