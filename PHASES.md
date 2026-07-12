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

## Phase 1 — XPMP2 sys crate: flat-C lifecycle/sound/CSL + the C++ shim

- **Vendoring/build**: the original C# project vendored *prebuilt* XPMP2
  static/import libraries directly (`XPMP2-lib/lib/win/XPMP2.lib`,
  `lib/lin/libXPMP2.a`, `lib/XPMP2.framework.zip` for mac) rather than
  compiling XPMP2's C++ sources itself. Do the same here where possible:
  `xpmp2-sys` links a prebuilt library per platform (vendored the same way
  `SDK/` is — gitignored, downloaded per a new README section, not
  committed). A small hand-written C++ shim (see below) still needs
  compiling locally via the `cc` crate, but that's a few shim functions, not
  the whole library.
- `bindgen` over `inc/XPMPMultiplayer.h` (init/cleanup/sound/CSL/contrail —
  all plain `extern "C"`, no shim needed) exactly like the XPLM headers.
- **The C++ shim**, for actually creating/updating a plane: a hand-written
  `extern "C"` C++ file (compiled by `cc::Build` alongside the vendored
  prebuilt lib) that defines one concrete subclass of `XPMP2::Aircraft`
  whose virtual overrides (`UpdatePosition` is pure virtual; others like
  `GetFlightId`/`GetAoA`/`SoundGetName` are overridable with defaults)
  forward into plain C function pointers plus a `void *refcon` —
  `bindgen` cannot let Rust override a C++ virtual method or synthesize a
  vtable on its own, so this shim is unavoidable. Exposed to `xpmp2-sys` as
  `extern "C"` create/destroy/update functions operating on an opaque
  handle, matching the shape of every other SDK surface in this workspace
  (a thin handle + functions, not a C++ type Rust ever names directly).
  Considered and rejected for now: the `cxx` crate's bridge macros — a
  heavier build-time dependency and a different FFI idiom than every other
  crate here uses; the hand-shim stays consistent with how `xplm-sys` itself
  is built (a C compiler via `cc`, nothing more) unless a concrete reason to
  prefer `cxx` shows up.
- New crate `xpmp2-sys` (raw FFI, mirrors `xplm-sys`'s shape: `build.rs`
  locates the vendored library + headers, compiles the shim, `bindgen`
  generates bindings, no safety/ergonomics of its own) as a new workspace
  member, independent of `xplm-sys`/`xplm` (XPMP2 isn't a subsystem of the
  XPLM SDK proper) but depending on `xplm-sys` where its headers reference
  XPLM types directly.
- Not started: no `xpmp2-sys` crate yet, no vendored library, no shim code,
  no README download section.

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
