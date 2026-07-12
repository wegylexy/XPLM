# Remaining phases

Phases 0 through 5b are done — see git log for full rationale (each commit
message carries the detail that used to live here). Summary below; only
Phases 6-8 are described in full, since those are what's actually left. Each
phase should land as its own commit(s) and be testable before moving to the
next.

## Done (0-5b)

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

## Phase 6 — `xplm-macros`

- `#[plugin(name = ..., signature = ..., description = ...)]` attribute macro
  generating the Phase 4 boilerplate (sugar over `register_plugin!`).
- `#[derive(DataRefContainer)]` generating Phase 3 lookups from
  `#[dataref = "sim/..."]` field attributes, `find()` returning `Option<Self>`.
- Testable: `trybuild` UI tests for macro-expansion correctness; a
  snapshot/expand test comparing macro output against the hand-written
  equivalent.

## Phase 7 — Remaining surfaces

- Planes, Scenery, Utilities (`Planes.cs`, `Scenery.cs`, `Utilities.cs`), plus
  Instance (`Instance.cs`/`XPLMInstance.h`), moved here from Phase 5 since it
  depends on `XPLMDrawInfo_t`/`XPLMObjectRef` from Scenery.
- Widgets, off `SDK/CHeaders/Widgets` (`XPWidgets.h`, `XPStandardWidgets.h`,
  `XPUIGraphics.h`) — same RAII + trampoline treatment as everything else,
  and the same no-stale-index/no-raw-pointer invariant from `CLAUDE.md`
  applies to any item-tree-style API in here.
  `SDK/CHeaders/Wrappers` (the C++ convenience wrappers) is reference-only and
  is *not* ported — Rust's RAII already supersedes what those exist for in
  C++.
- XPMP2 multiplayer/legacy aircraft (`LegacyAircraft.cs`, `Multiplayer.cs`).
- Lower priority than Phases 2-6: additive rather than core-architecture-
  defining.

## Phase 8 — Parity example

- Port `XPL/Program.cs` (the original C# sample plugin) to Rust using the
  Phase 6 macros, as the acceptance test that the abstraction is ergonomically
  equivalent to the C# original.
