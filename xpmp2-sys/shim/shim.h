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

#ifdef __cplusplus
}
#endif

#endif // XPMP2_SYS_SHIM_H
