# Remaining phases

Phases 0 through 6 are done — see git log for full rationale (each commit
message carries the detail that used to live here). Summary below; only
Phases 7-8 are described in full, since those are what's actually left. Each
phase should land as its own commit(s) and be testable before moving to the
next.

## Done (0-6)

- **0/1** — Orphan tree with only the SDK moved over (later made gitignored,
  downloaded per README instead). Versioned `xplm-sys` FFI: Cargo features
  `XPLM200`-`XPLM420`, cascading low-to-high, matching exactly what's in the
  vendored headers (no `XPLM430`). Fixed real MSVC/bindgen build issues along
  the way (libclang needing MSVC's `INCLUDE` paths, `APL`/`IBM`/`LIN` all
  needing explicit values, `XPLM_64.dll` needing delay-load so `cargo test`
  can start without a hosted X-Plane process).
- **2** — `xplm::guard()` (panic boundary, `catch_unwind` + `XPLMDebugString`
  logging). `FlightLoop` proves the RAII + trampoline pattern every later
  module reuses: boxed closure behind a thin refcon pointer, one
  panic-guarded `extern "C"` trampoline, `Drop` unregisters natively.
- **3** — `xplm::dataref::DataRef<T, Access>` / `ArrayDataRef<T, Access>`
  (`T` sealed to `i32`/`f32`/`f64`/array variants). `ReadOnly<T>`/
  `ReadWrite<T>` aliases; `set()` only exists under a `Writable` bound —
  compile-time access gating, not a runtime check.
- **4** — `xplm::plugin::XPlanePlugin` trait + `register_plugin!($t)` macro,
  generating the five required `extern "C"` exports. `examples/hello-plugin`
  created (a real `cdylib`, confirmed exports present in the built DLL).
- **5** — `xplm::menu::Menu`/`MenuItem` (identity-lookup via `Rc<ItemToken>`,
  never a cached index or raw pointer — see the standing invariant in
  `CLAUDE.md`, raised as a real bug once already) and
  `xplm::camera::CameraControl` (documented the SDK's single-global-slot
  caveat on `Drop`). Instance and Display/Graphics were rescoped out (see 5b).
- **5b** — `xplm::window::Window`/`WindowBuilder` (`XPLMCreateWindowEx`, all
  five core callbacks default to no-ops so callers only set what they need)
  and `xplm::graphics` (stateless wrappers: `GraphicsState`, texture/coordinate
  helpers, text drawing). Added an opt-in `deprecated` Cargo feature
  (`xplm-sys` → `xplm`, off by default) exposing the SDK's
  `#if defined(XPLM_DEPRECATED)` symbols (old texture/font constants) —
  previously invisible entirely since `build.rs` never defined that macro;
  now reachable but each variant marked `#[deprecated]` so using one is a
  compiler warning, not a silent trap. Caught and fixed two version-gating
  bugs by testing `--features XPLM200` explicitly, not just the default set
  — now a standing check for every future module.
- **6** — `#[xplm::plugin(name = ..., signature = ..., description = ...)]`
  attribute macro: expands to the annotated struct unchanged plus
  `register_plugin!($t, name = ..., signature = ..., description = ...)`.
  Required loosening Phase 4's `XPlanePlugin::NAME`/`SIGNATURE`/`DESCRIPTION`
  to defaulted (`= ""`) associated consts and adding a second
  `register_plugin!` macro arm taking metadata as explicit arguments —
  otherwise the attribute (which only sees the struct item, not a later
  separate `impl XPlanePlugin for MyPlugin` block) would have no way to
  supply the trait's required consts without the user redundantly
  re-declaring them. `#[derive(xplm::DataRefContainer)]` generates
  `find() -> Option<Self>` from `#[dataref = "sim/..."]`-tagged fields,
  calling each field type's own `find` (works unchanged for both
  `ReadOnly<T>`/`ReadWrite<T>`, since the derive doesn't need to know which).
  `examples/hello-plugin` converted to the attribute-macro flow; `trybuild`
  `.pass(...)` tests confirm both macros expand to code that compiles.

## Phase 7 — Commands, Utilities, Scenery, Instance, Planes (done); rest split into 7b

- `xplm::command::Command`/`CommandHandler` (`XPLMUtilities.h`'s command
  subsystem): `Command` is a thin, `Copy` handle — unlike everything else in
  this crate it isn't `Drop`-owned, since a command isn't owned by any one
  plugin and outlives whichever one created it. `CommandHandler` (from
  `Command::register_handler`) is the RAII + trampoline half — dropping it
  unregisters just that callback, via the exact `(command, fn ptr, before,
  refcon)` tuple `XPLMUnregisterCommandHandler` requires to match.
- `xplm::utilities`: a handful of stateless free functions —
  `system_path`/`prefs_path` (reads the SDK's documented 512-byte buffer
  convention into a `String`), `versions`, `speak_string`, `reload_scenery`.
- `xplm::scenery`: `TerrainProbe` (`XPLMProbeRef`, RAII — the SDK recommends
  reusing one probe for nearby points rather than allocating per-query),
  `Object` (a loaded `.obj`, X-Plane-refcounted, `Drop` calls
  `XPLMUnloadObject`; `load`/`load_async`, the latter a single-shot
  `FnOnce` trampoline since there's no way to cancel an in-flight load),
  `DrawInfo`, and magnetic-variation free functions (`XPLM300`+).
- `xplm::instance::Instance` (`XPLMInstance.h`) builds on `Object`/`DrawInfo`
  — this is why Instance was moved out of Phase 5 to land here instead of
  being split from Scenery artificially. `set_position` takes a `&[f32]`
  matching the dataref list `Instance::new` was given, one value per entry
  in the same order. `Object::new_instance(&self, datarefs)` is sugar for
  `Instance::new(&object, datarefs)` — `Instance::from(Object)` doesn't fit
  since `new` genuinely needs the dataref list too, not just one value to
  convert from.
- Decided (asked, not assumed): `Object::load_async` stays callback-based,
  not `async fn` — X-Plane's plugin runtime has no ambient executor to poll
  a `Future`, so making it `async` in `xplm` itself wouldn't solve the
  "who drives this?" problem, just move it onto every caller. README
  documents the plain oneshot-channel bridge a caller with their own async
  runtime could use instead, type-checked in `readme_examples.rs`.
- README gained "Commands" and "Terrain probing and instanced object
  drawing" sections; `xplm/tests/readme_examples.rs` type-checks both
  (alongside the existing snippets) without executing them — actually
  calling these functions needs a hosted X-Plane process, the same
  limitation as everywhere else real `XPLM*` calls show up in this crate.
- `xplm::aircraft` (`XPLMPlanes.h`, done): free functions for the user's own
  aircraft (`set_users_aircraft`, `place_user_at_airport`/`_location`,
  `aircraft_count`, `nth_aircraft_model`) plus `AircraftAccess`, the RAII
  wrapper for exclusive AI/multiplayer aircraft control — same
  single-global-slot shape as `CameraControl`, but `acquire` at least tells
  you whether you got it, and takes an optional one-shot "available now"
  callback (same `FnOnce`-trampoline shape as `Object::load_async`) for when
  you don't. `XPLMInitFlight`/`XPLMUpdateFlight` (`XPLM430`-gated) are
  intentionally *not* wrapped — this crate caps version support at
  `XPLM420` — rather than merely deferred.
- **Still split out as Phase 7b**: the rest of `XPLMUtilities.h` (directory
  listing, data files, key sniffers, hotkeys), Widgets (`SDK/CHeaders/Widgets`
  — `XPWidgets.h`, `XPStandardWidgets.h`, `XPUIGraphics.h`; same RAII +
  trampoline treatment, and the no-stale-index/no-raw-pointer invariant from
  `CLAUDE.md` applies to any item-tree API in there), and XPMP2
  multiplayer/legacy aircraft (`LegacyAircraft.cs`, `Multiplayer.cs`).
  `SDK/CHeaders/Wrappers` (C++ convenience wrappers) remains reference-only,
  not ported.

## Phase 8 — Parity example

- Port `XPL/Program.cs` (the original C# sample plugin) to Rust using the
  Phase 6 macros, as the acceptance test that the abstraction is ergonomically
  equivalent to the C# original.
