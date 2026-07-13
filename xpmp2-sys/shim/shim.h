/// Hand-written C shim over `XPMP2::Aircraft` (`inc/XPMPAircraft.h`) — a
/// plain `extern "C"` surface `bindgen` can wrap directly, since `bindgen`
/// itself cannot let Rust override a C++ virtual method or synthesize a
/// vtable. `XPMP2ShimAircraft` is always heap-allocated by
/// `xpmp2_shim_aircraft_create` and must be freed exactly once via
/// `xpmp2_shim_aircraft_destroy` — never `free()`/`delete` from the Rust
/// side directly, since it's a real C++ object underneath.
///
/// Every setter here is safe to call at any time, but only meaningful to
/// call from within (or before scheduling further calls to) the
/// `update_position` callback — same contract as overriding
/// `XPMP2::Aircraft::UpdatePosition` directly would have.
#ifndef XPMP2_SYS_SHIM_H
#define XPMP2_SYS_SHIM_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct XPMP2ShimAircraft XPMP2ShimAircraft;

/// Mirrors `XPMP2::Aircraft::UpdatePosition`'s parameters exactly.
typedef void (*XPMP2ShimUpdatePositionFn)(
    void* refcon,
    float elapsed_since_last_call,
    int fl_counter
);

/// Creates a plane. Returns `NULL` on failure (invalid/duplicate
/// `mode_s_id`, or no CSL model could be matched) — mirrors
/// `XPMP2::Aircraft`'s constructor throwing `XPMP2::XPMP2Error`, caught
/// here so no C++ exception ever tries to cross into Rust.
///
/// `icao_type`/`icao_airline`/`livery`/`csl_id` are NUL-terminated, UTF-8
/// (treated as empty if NULL). `mode_s_id` of `0` lets XPMP2 assign one.
/// `update_position`/`refcon` are stored for the aircraft's whole lifetime
/// and called once per drawing cycle; `refcon` is never interpreted by
/// this shim, only handed back verbatim.
XPMP2ShimAircraft* xpmp2_shim_aircraft_create(
    const char* icao_type,
    const char* icao_airline,
    const char* livery,
    unsigned int mode_s_id,
    const char* csl_id,
    XPMP2ShimUpdatePositionFn update_position,
    void* refcon
);

/// Destroys a plane created by `xpmp2_shim_aircraft_create`. `NULL` is a
/// no-op.
void xpmp2_shim_aircraft_destroy(XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::GetModeS_ID`.
unsigned int xpmp2_shim_aircraft_mode_s_id(const XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::IsValid` — `false` if XPMP2 invalidated this aircraft
/// (e.g. after an internal error); an invalid aircraft should be destroyed,
/// not updated further.
int xpmp2_shim_aircraft_is_valid(const XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::SetVisible`/`IsVisible`.
void xpmp2_shim_aircraft_set_visible(XPMP2ShimAircraft* aircraft, int visible);
int xpmp2_shim_aircraft_is_visible(const XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::SetLocation` — converts world (lat/lon/feet) to local
/// OpenGL coordinates and writes `drawInfo`.
void xpmp2_shim_aircraft_set_location(
    XPMP2ShimAircraft* aircraft,
    double lat,
    double lon,
    double alt_ft
);

/// `XPMP2::Aircraft::GetLocation`.
void xpmp2_shim_aircraft_get_location(
    const XPMP2ShimAircraft* aircraft,
    double* out_lat,
    double* out_lon,
    double* out_alt_ft
);

/// `XPMP2::Aircraft::SetLocalLoc` — sets `drawInfo.x/y/z` directly, for
/// callers that already have local OpenGL coordinates and want to skip the
/// world-to-local conversion `set_location` does.
void xpmp2_shim_aircraft_set_local_loc(XPMP2ShimAircraft* aircraft, float x, float y, float z);

/// Degrees. `XPMP2::Aircraft::SetPitch`/`SetHeading`/`SetRoll`.
void xpmp2_shim_aircraft_set_pitch(XPMP2ShimAircraft* aircraft, float degrees);
void xpmp2_shim_aircraft_set_heading(XPMP2ShimAircraft* aircraft, float degrees);
void xpmp2_shim_aircraft_set_roll(XPMP2ShimAircraft* aircraft, float degrees);

/// `XPMP2::Aircraft::bOnGrnd`.
void xpmp2_shim_aircraft_set_on_ground(XPMP2ShimAircraft* aircraft, int on_ground);

/// Cartesian velocity in m/s, `XPMP2::Aircraft::v_x/v_y/v_z`.
void xpmp2_shim_aircraft_set_velocity(XPMP2ShimAircraft* aircraft, float vx, float vy, float vz);

/// `XPMP2::Aircraft::label` (UTF-8, NUL-terminated; treated as empty if
/// NULL) and `XPMP2::Aircraft::bDrawLabel`.
void xpmp2_shim_aircraft_set_label(XPMP2ShimAircraft* aircraft, const char* label);
void xpmp2_shim_aircraft_set_label_drawn(XPMP2ShimAircraft* aircraft, int drawn);

/// The current length of `XPMP2::Aircraft::v` — the CSL model's animation
/// dataRef array — and per-index get/set. Silently does nothing (get
/// returns `0.0`) if `index >= count`, rather than an out-of-bounds access.
size_t xpmp2_shim_aircraft_dataref_count(const XPMP2ShimAircraft* aircraft);
float xpmp2_shim_aircraft_get_dataref(const XPMP2ShimAircraft* aircraft, size_t index);
void xpmp2_shim_aircraft_set_dataref(XPMP2ShimAircraft* aircraft, size_t index, float value);

/// `XPMP2::Aircraft::acRadar` — the transponder code/mode XPMP2 forwards to
/// this aircraft's own TCAS target slot (`sim/cockpit2/tcas/targets/
/// modeC_code`/`ssr_mode`). `mode` is an `XPMPTransponderMode` value
/// (`xpmpTransponderMode_Off` through `_ModeS_TARA`, see `XPMPMultiplayer.h`).
void xpmp2_shim_aircraft_set_radar(XPMP2ShimAircraft* aircraft, long code, int mode);

/// `XPMP2::Aircraft::colLabel` — the label's base RGBA color, each channel
/// `0..1`.
void xpmp2_shim_aircraft_set_label_color(XPMP2ShimAircraft* aircraft, float r, float g, float b, float a);

/// `XPMP2::Aircraft::vertOfsRatio` — `0..1`, phases out the ground/gear
/// vertical offset at higher altitudes.
void xpmp2_shim_aircraft_set_vert_ofs_ratio(XPMP2ShimAircraft* aircraft, float ratio);

/// `XPMP2::Aircraft::GetVertOfs` — the current vertical offset XPMP2 applies
/// on top of `drawInfo.y` to place the model on the ground.
float xpmp2_shim_aircraft_get_vert_ofs(const XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::gearDeflectRatio`.
void xpmp2_shim_aircraft_set_gear_deflect_ratio(XPMP2ShimAircraft* aircraft, float ratio);

/// `XPMP2::Aircraft::bClampToGround`.
void xpmp2_shim_aircraft_set_clamp_to_ground(XPMP2ShimAircraft* aircraft, int clamp);

/// `XPMP2::Aircraft::aiPrio` — lower sorts earlier for one of the limited
/// TCAS target slots.
void xpmp2_shim_aircraft_set_ai_priority(XPMP2ShimAircraft* aircraft, int priority);

/// `XPMP2::Aircraft::SetRender`/`IsRendered` — whether the CSL model is drawn
/// in the 3D world (independent of TCAS/multiplayer visibility, see
/// `set_visible`/`is_visible`).
void xpmp2_shim_aircraft_set_render(XPMP2ShimAircraft* aircraft, int render);
int xpmp2_shim_aircraft_is_rendered(const XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::GetTcasTargetIdx` — 1-based index into
/// `sim/cockpit2/tcas/targets`, or `-1` if this plane isn't currently
/// occupying a TCAS slot.
int xpmp2_shim_aircraft_tcas_target_idx(const XPMP2ShimAircraft* aircraft);
int xpmp2_shim_aircraft_is_shown_as_tcas_target(const XPMP2ShimAircraft* aircraft);
int xpmp2_shim_aircraft_is_shown_as_ai(const XPMP2ShimAircraft* aircraft);
int xpmp2_shim_aircraft_show_as_ai_plane(const XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::GetCameraDist`/`GetCameraBearing`/`GetGS_kn`.
float xpmp2_shim_aircraft_camera_dist(const XPMP2ShimAircraft* aircraft);
float xpmp2_shim_aircraft_camera_bearing(const XPMP2ShimAircraft* aircraft);
float xpmp2_shim_aircraft_ground_speed_kn(const XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::IsRelatedTo`/`IsGroundVehicle`/`IsGlider`.
int xpmp2_shim_aircraft_is_related_to(const XPMP2ShimAircraft* aircraft, const char* icao_type);
int xpmp2_shim_aircraft_is_ground_vehicle(const XPMP2ShimAircraft* aircraft);
int xpmp2_shim_aircraft_is_glider(const XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::ChangeModel`/`ReMatchModel`/`GetMatchQuality` — re-runs
/// model matching, optionally with new type/airline/livery criteria; returns
/// the match quality (the lower the better). `rematch` reruns with the
/// aircraft's existing criteria.
int xpmp2_shim_aircraft_change_model(
    XPMP2ShimAircraft* aircraft,
    const char* icao_type,
    const char* icao_airline,
    const char* livery
);
int xpmp2_shim_aircraft_rematch_model(XPMP2ShimAircraft* aircraft);
int xpmp2_shim_aircraft_match_quality(const XPMP2ShimAircraft* aircraft);

/// Copies up to `buf_len - 1` bytes of `XPMP2::Aircraft::GetModelName`/
/// `GetFlightId` into `buf` and NUL-terminates it (a no-op, `buf` untouched,
/// if `buf` is NULL or `buf_len` is `0`). Returns the untruncated string's
/// length, mirroring `strlcpy` — compare against `buf_len` to detect
/// truncation.
size_t xpmp2_shim_aircraft_get_model_name(const XPMP2ShimAircraft* aircraft, char* buf, size_t buf_len);
size_t xpmp2_shim_aircraft_get_flight_id(const XPMP2ShimAircraft* aircraft, char* buf, size_t buf_len);

/// `XPMP2::Aircraft::acInfoTexts` — informational texts forwarded via
/// multiplayer shared dataRefs. Each parameter is NUL-terminated, UTF-8
/// (treated as empty if NULL), and truncated to `XPMPInfoTexts_t`'s
/// corresponding fixed-size field if longer.
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
    const char* apt_to
);

/// `XPMP2::Aircraft::wake` — wake turbulence support data; leave a value at
/// `NAN` to let XPMP2 fall back to its Doc8643-derived default for the
/// aircraft's wake turbulence category. `wake_apply_defaults` is
/// `WakeApplyDefaults` (`overwrite_all` false fills only still-`NAN` fields).
void xpmp2_shim_aircraft_set_wing_span(XPMP2ShimAircraft* aircraft, float meters);
void xpmp2_shim_aircraft_set_wing_area(XPMP2ShimAircraft* aircraft, float square_meters);
void xpmp2_shim_aircraft_set_mass(XPMP2ShimAircraft* aircraft, float kg);
void xpmp2_shim_aircraft_wake_apply_defaults(XPMP2ShimAircraft* aircraft, int overwrite_all);

/// `XPMP2::Aircraft::ContrailRequest`/`ContrailRemove`/`ContrailTrigger` —
/// `dist_m`/`life_time` of `0` mean "keep the current value" per XPMP2's own
/// contract. `contrail_trigger` applies XPMP2's standard per-type contrail
/// heuristic instead and returns the number of contrails created.
void xpmp2_shim_aircraft_contrail_request(XPMP2ShimAircraft* aircraft, unsigned num, unsigned dist_m, unsigned life_time_s);
void xpmp2_shim_aircraft_contrail_remove(XPMP2ShimAircraft* aircraft);
unsigned xpmp2_shim_aircraft_contrail_trigger(XPMP2ShimAircraft* aircraft);

/// `XPMP2::Aircraft::sndMinDist`/`SoundMuteAll`/`SoundIsMuted`. XPMP2's
/// higher-level per-channel sound API (`SoundPlay`/`SoundStop`/
/// `SoundVolume`, custom `SoundGetName` overrides) is intentionally not
/// shimmed — it's designed around overriding a C++ virtual per aircraft
/// subclass, which this single-`ShimAircraft`-class shim can't route to
/// per-plane Rust code any more than it already does for `UpdatePosition`.
void xpmp2_shim_aircraft_set_sound_min_dist(XPMP2ShimAircraft* aircraft, int meters);
void xpmp2_shim_aircraft_set_sound_muted(XPMP2ShimAircraft* aircraft, int mute);
int xpmp2_shim_aircraft_is_sound_muted(const XPMP2ShimAircraft* aircraft);

#ifdef __cplusplus
}
#endif

#endif // XPMP2_SYS_SHIM_H
