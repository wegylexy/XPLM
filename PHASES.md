# Remaining phases

The pure-XPLM SDK surface — `xplm-sys`/`xplm`/`xplm-macros`, covering every
`SDK/CHeaders/XPLM/*.h` header plus Widgets — is fully ported and has two
acceptance-test example plugins (`examples/hello-plugin`,
`examples/xpl-template`). See git log for how that was built; it isn't
repeated here.

What's left is XPMP2 multiplayer support, numbered fresh from here. Each
phase should land as its own commit(s) and be testable before moving to the
next.

## What XPMP2 actually exposes (checked against current upstream headers)

The original C# project's `XPMP2/LegacyAircraft.cs` (recovered from git
history, same source as `examples/xpl-template`'s port) called
`XPMPCreatePlaneWithModelName`/`XPMPDestroyPlane` — plain `extern "C"`
functions in `inc/XPMPMultiplayer.h`. But checking upstream
(https://github.com/TwinFan/XPMP2) directly rather than trusting that old
usage as current guidance: those specific functions are now marked
`[[deprecated]]`, with a doc comment pointing at subclassing
`XPMP2::Aircraft` instead. (`inc/XPCAircraft.h`, despite its name suggesting
a flat-C legacy API, is actually itself a deprecated *C++* class —
`class XPCAircraft : public XPMP2::Aircraft` — not flat C at all.)

So the plane-creation path this crate should target is the modern C++
`XPMP2::Aircraft` class from the start, not the deprecated C callback
function. Everything else in `inc/XPMPMultiplayer.h` — init/cleanup
(`XPMPMultiplayerInit`/`XPMPMultiplayerCleanup`), CSL package loading,
preference callbacks, sound (`XPMPSoundEnable`/`XPMPSoundAdd`/...), contrail
queries — remains plain, non-deprecated `extern "C"`, and `bindgen` handles
it exactly like every `XPLM*.h` header already ported.

## Phase 1 — XPMP2 sys crate: flat-C lifecycle/sound/CSL + C++ shim (done)

- Added `external/XPMP2` as a git submodule (https://github.com/TwinFan/XPMP2.git),
  pinned to `master`/`v3.6.1` (same commit).
- **Build approach changed from the original plan**: rather than vendoring
  prebuilt per-platform libraries (what the old C# project did), `xpmp2-sys`'s
  `build.rs` compiles XPMP2's own C++ sources directly via the `cc` crate —
  the same ~14 `.cpp` files upstream's `CMakeLists.txt` lists (everything
  except `SoundFMOD.cpp`, gated behind `INCLUDE_FMOD_SOUND` upstream too and
  needing a separate vendored FMOD SDK this crate doesn't carry), as a
  static lib, matching XPMP2's own default (`XPMP2_BUILD_SHARED_LIBS` is
  off by default upstream, Windows-only when on). This sidesteps needing to
  source/vendor prebuilt binaries at all, and — since XPMP2's own CMake
  build already targets Windows/macOS/Linux uniformly via the same
  `APL`/`IBM`/`LIN` convention `xplm-sys` uses — means the exact same
  `build.rs` logic should work on all three once the (pre-existing, tracked
  separately) Linux/macOS `xplm-sys` link-step gap is closed.
- `bindgen` over `inc/XPMPMultiplayer.h` — mostly plain `extern "C"`
  functions taking `const char*`/`bool`/`int`, as expected, but a handful
  (`XPMPSoundGetAudioDeviceName`, `XPMPSoundSetAudioDeviceName`,
  `XPMPSoundGetActiveAudioDevice`, `XPMPGetModelInfo2`,
  `XPMPAddModelDataRef`) take/return `std::string&`/`std::string*` instead —
  bindgen can't safely lay out MSVC's `std::string` ABI, so those are
  `blocklist_function`-ed (each has a plain-C-typed equivalent already in
  the same header, e.g. `XPMPGetModelInfo`, except the `std::string*`
  overload of `XPMPSoundGetActiveAudioDevice`/`XPMPAddModelDataRef` — left
  unexposed until something needs them enough to justify a tiny shim).
  `XPMPGetAircraft` (returns `XPMP2::Aircraft*`) is blocklisted too — that's
  the C++ class this phase doesn't bind.
- **Verified for real, not just "it compiled"**: a `cargo test` in
  `xpmp2-sys` takes the address of several bound functions (without calling
  them — same can't-call-outside-a-hosted-process limitation as `xplm-sys`)
  specifically to force the linker to actually resolve every symbol against
  the compiled XPMP2 static lib and its own XPLM/Winsock (`Ws2_32`/
  `Iphlpapi`) dependencies — a `cargo build` alone proved nothing here,
  since with zero Rust code referencing any XPMP2 symbol the linker was
  dropping the whole static archive silently.
- **The C++ shim**: `xpmp2-sys/shim/shim.h`+`shim.cpp` define one concrete
  `ShimAircraft : public XPMP2::Aircraft` whose `UpdatePosition` (the one
  pure-virtual method) forwards into a plain C function pointer + `void
  *refcon`, compiled into the same static lib as XPMP2 itself. Exposed as
  `extern "C"` functions operating on an opaque `XPMP2ShimAircraft*`:
  create/destroy, mode-S-id/valid/visible getters+setters, location (both
  world lat/lon/alt-ft via `SetLocation`, and local x/y/z directly),
  pitch/heading/roll, on-ground, cartesian velocity, label, and indexed
  get/set over the CSL model's animation dataRef array (`XPMP2::Aircraft::v`)
  — covers everything `UpdatePosition` needs to fill per XPMP2's own docs,
  not yet every overridable virtual (`GetFlightId`/`GetAoA`/`SoundGetName`/
  etc. aren't forwarded — add as needed once `xpmp2`'s `Aircraft` trait
  design in Phase 2 clarifies which of those are worth exposing). The
  constructor's `std::string` parameters and the `v` vector are handled
  entirely inside the shim (`const char*` in, `std::string`/`std::vector`
  never crossing into Rust) — bindgen only ever sees `shim.h`'s plain C
  types. Constructor exceptions (`XPMP2::XPMP2Error` on invalid/duplicate
  mode-S-id or no CSL match) are caught in the shim and turned into a `NULL`
  return, so no C++ exception ever crosses into Rust — the mirror image of
  why every Rust-side trampoline in this workspace goes through
  `xplm::guard()`.
- Considered and rejected: the `cxx` crate's bridge macros for the shim — a
  heavier build-time dependency and a different FFI idiom than every other
  crate here uses; the hand-shim keeps `xpmp2-sys` built the same way as
  `xplm-sys` (a C/C++ compiler via `cc`, `bindgen`, nothing more).
- **Verified the shim for real too**: a second link-time-resolution test,
  separate from the flat-C one — `shim.cpp` is a distinct translation unit
  within the same static archive, and most linkers pull in `.lib`/`.a`
  members per-symbol, so resolving an `XPMPMultiplayer.cpp` symbol doesn't
  prove `shim.cpp`'s object file got linked too.

## Phase 2 — XPMP2 safe wrapper

- New crate `xpmp2` (safe wrappers, mirrors `xplm`'s shape), depending on
  `xpmp2-sys` and `xplm`.
- `Multiplayer` (or similar): RAII over `XPMPMultiplayerInit`/
  `XPMPMultiplayerCleanup` — `init()` returns `Result`/`Option` (the C#
  original surfaces `XPMPMultiplayerInit`'s error string; this crate's
  version should too, rather than swallowing it), `Drop` calls cleanup.
  Sound/contrail/preference/CSL-loading functions hang off this handle or as
  plain free functions, matching whichever of `xplm::camera`'s
  single-global-slot pattern or plain free functions fits
  `XPMPMultiplayerInit`'s own single-instance contract (needs confirming
  against the header before committing to a shape).
- An `Aircraft` trait (object-safe, `update_position` at minimum, matching
  `XPMP2::Aircraft`'s pure-virtual method, with default-implemented methods
  for the overridable ones) that a caller implements on their own plane
  state struct; a `Plane` RAII handle registers one through Phase 1's C++
  shim and destroys it on `Drop` — same RAII shape as every other subsystem
  in this crate, even though the FFI plumbing underneath (a real C++ object)
  is different. Naming TBD for both — avoid colliding with `xplm::aircraft`,
  which is XPLM's own user/AI-aircraft API, a different subsystem entirely.
- README gains a "Multiplayer (XPMP2)" section once the API stabilizes,
  type-checked the same way as every other section in this crate (`xpmp2`'s
  own `tests/readme_examples.rs`, since calling into it also needs a hosted
  X-Plane process).
- Not started.

## Phase 3 — CSL package streaming/cache client (feature-gated, in front of XPMP2)

**Superseded by a real reference implementation** — see
https://github.com/wegylexy/csl-on-demand (C# original) and its `rust-port`
branch (a from-scratch Rust port of the same server, scaffolded but
uncommitted, living in its own sibling repo/clone rather than in this
workspace — it has zero dependency on `xplm`/XPMP2). That project's actual
wire protocol is simpler than what's sketched below: `GET /match?icao=&
airline=&livery=` → 302 to `/pack/{root}/{id}` → `multipart/mixed` (model
matching happens server-side, standard `Accept-Encoding` negotiation
including zstd via `tower-http`, no bespoke manifest/hash format needed).
The client-side piece described below (fetch, parse the multipart response,
materialize into a directory `XPMPLoadCSLPackage` can read) still belongs
here in `xpmp2` — treat the design below as superseded in its *protocol*
assumptions (no manifest/BLAKE3/CDN design needed, just speak
`csl-on-demand`'s existing routes) but still roughly right in its
*integration point* (materialize, then hand off to XPMP2 unchanged).

Motivation: CSL packages (the actual 3D models XPMP2 draws for multiplayer
traffic) run into the GBs, and XPMP2's own `XPMPLoadCSLPackage` only knows
how to read them from a local directory it's handed — every install
duplicates that in full. Goal: fetch/cache/decompress on demand from a
public origin, deduplicated by content, and hand XPMP2 a normal-looking
local directory it never has to know is synthesized. Default behavior is
unchanged (caller still points `xpmp2` at a local CSL directory exactly like
upstream XPMP2 expects); this is entirely opt-in.

- **Don't build a bespoke server or streaming protocol.** A public origin
  implies auth/bandwidth/abuse concerns that are a deployment problem, not a
  protocol-design problem — solve it by putting a **hash-verified manifest +
  content-addressed, compressed blobs** behind any static HTTP host/CDN
  (GitHub Releases, S3/R2, Cloudflare, etc.), and writing a plain HTTPS
  client in Rust. This gets range requests, caching headers, CDN edge
  caching, and (if the host requires it) bearer-token auth for free, instead
  of reinventing them.
- **Manifest**: a signed/hashed JSON (or similar) document per CSL package
  listing its files and their content hashes (e.g. BLAKE3) and the
  compressed blob's URL + size. Signature/hash verification means even an
  untrusted mirror/CDN can't serve tampered content silently.
- **Content-addressed local cache**: blobs are stored once, keyed by hash,
  in a shared local directory (not per-plugin-install) — identical textures/
  objects reused across CSL packages (common in practice — many liveries
  share the base model) are only ever fetched/decompressed/stored once.
  Compression at rest (likely zstd) shrinks the on-disk footprint further.
- **Materialization, not a new loader**: for each CSL package XPMP2 needs,
  this layer verifies/fetches/decompresses the referenced blobs into a
  directory shaped exactly like a normal CSL package (real files or
  hardlinks/symlinks from the content-addressed store — avoiding a second
  full copy per package), then hands that directory to
  `XPMPLoadCSLPackage` unchanged. XPMP2's own model-matching logic keeps
  working exactly as today; this phase never touches it.
- **Feature flags** (on `xpmp2`, both default-off so plain upstream behavior
  is what you get with no extra dependencies pulled in):
  - `csl-cache`: the fetch/verify/decompress/materialize client described
    above.
  - `csl-cache-remote-matching` (depends on `csl-cache`; see Phase 4): the
    deeper "don't materialize full packages, match directly off blobs"
    mode — kept as a separate, later flag since it's substantially more
    work and reopens the "reimplement matching logic" question.
- New crate (`xpmp2-csl-cache` or similar; naming TBD) — kept out of
  `xpmp2` itself so the network/compression/hashing dependencies (an async
  HTTP client, a hashing crate, zstd bindings) are only pulled in by callers
  who opt into `csl-cache`.
- Explicitly out of scope for this repo: hosting the actual public origin
  server, auth/rate-limiting policy, and building the manifests for any
  specific CSL package set — those are deployment/content concerns, not
  something a Cargo crate should own. This phase only needs a documented
  manifest schema and a client that speaks it.
- Not started; this needs its own design pass (manifest schema, exact
  hashing/compression choices, cache eviction policy) before code, same as
  Phases 1-2 got before implementation.

## Phase 4 — direct CSL matching against cached blobs (optional, future)

- Only worth doing if Phase 3's materialize-then-hand-to-XPMP2 approach
  proves too slow/heavy in practice (e.g. cold-start latency materializing
  a large package before first draw).
- Would mean reimplementing (a subset of) XPMP2's own CSL model-matching
  logic in Rust to select and stream in only the specific model actually
  needed, rather than materializing a whole package — a real reimplementation
  of business logic, not an FFI wrapper, similar in spirit to (though
  smaller than) the pure-Rust-port question already declined for XPMP2 as a
  whole. Revisit only with a concrete performance problem in hand, not
  speculatively.
- Not started.
