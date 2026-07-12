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

## Phase 7 — Commands + core Utilities (done); rest split into 7b

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
- **Split out as Phase 7b**: Planes, Scenery, Instance (`Planes.cs`,
  `Scenery.cs`, `Instance.cs`/`XPLMInstance.h` — Instance depends on
  `XPLMDrawInfo_t`/`XPLMObjectRef` from Scenery, so they land together), the
  rest of `XPLMUtilities.h` (directory listing, data files, key sniffers,
  hotkeys), Widgets (`SDK/CHeaders/Widgets` — `XPWidgets.h`,
  `XPStandardWidgets.h`, `XPUIGraphics.h`; same RAII + trampoline treatment,
  and the no-stale-index/no-raw-pointer invariant from `CLAUDE.md` applies to
  any item-tree API in there), and XPMP2 multiplayer/legacy aircraft
  (`LegacyAircraft.cs`, `Multiplayer.cs`). `SDK/CHeaders/Wrappers` (C++
  convenience wrappers) remains reference-only, not ported.
- README updated with a "Commands" usage section; `xplm/tests/readme_examples.rs`
  extended to type-check it alongside the existing snippets.

## Phase 8 — Parity example

- Port `XPL/Program.cs` (the original C# sample plugin) to Rust using the
  Phase 6 macros, as the acceptance test that the abstraction is ergonomically
  equivalent to the C# original.
