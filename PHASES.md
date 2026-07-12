# Remaining phases

Phase 0/1 (orphan tree + versioned `xplm-sys` FFI) are done — see git log. This
tracks what's left, in order. Each phase should land as its own commit(s) and be
testable before moving to the next.

## Phase 2 — Core safety primitives in `xplm`

- Panic boundary: `xplm::guard()` (already stubbed in `xplm/src/lib.rs`) wraps
  `catch_unwind`, logs via `XPLMDebugString` on panic. Every `extern "C"`
  trampoline must go through it — a panic unwinding into the X-Plane host is UB.
- Prove the trampoline + closure-registry pattern end to end on the simplest
  native handle first: `FlightLoop` (from `Processing.cs`). A safe wrapper that
  registers a `Box<dyn FnMut(...) -> f32>` in a registry, with a static
  `extern "C"` trampoline that looks it up, and `Drop` that unregisters the
  native callback.
- Testable: unit tests for the registry (insert/remove/panic-recovery) under
  plain `cargo test`; an example `cdylib` plugin to manually verify in-sim.

## Phase 3 — DataRef subsystem

- Port `DataAccess.cs` (largest surface, 558 lines in the C# original) to
  `DataRef<T, Access>` with `ReadOnly`/`ReadWrite` phantom marker traits.
  `get()` on both, `set()` only when `Access = ReadWrite` — compile-time
  mutability gating instead of runtime checks.
- Array-dataref variants (`XPLMGetDatavi`/`XPLMGetDatavf`/etc.) as their own
  typed wrappers.
- Testable: `trybuild` compile-fail tests asserting `set()` doesn't exist on
  `DataRef<T, ReadOnly>`; runtime tests against known sim datarefs where a
  harness is available.

## Phase 4 — Plugin lifecycle + trampolines

- Port `XPluginBase.cs`/`Plugin.cs`: `XPlanePlugin` trait
  (`start/enable/disable/stop`), `extern "C"` trampolines for
  `XPluginStart/Enable/Disable/Stop/ReceiveMessage`, global
  `OnceLock<Mutex<Box<dyn XPlanePlugin>>>` registry.
- Testable: build a minimal example plugin crate, produce a `.xpl`, verify
  load/unload manually in X-Plane. First point where `/verify`-style in-sim
  testing applies.

## Phase 5 — Menu, Processing, Instance, Camera, Display/Graphics

- One module per C# file (`Menu.cs`, `Processing.cs`, `Instance.cs`,
  `Camera.cs`, `Display.cs`, `Graphics.cs`), following the RAII + trampoline
  pattern established in Phases 2/4. Each gets its own `Drop`-based unregister
  and its own closure registry.
- Testable: extend the example plugin incrementally — add a menu item, a
  flight loop callback, a camera hook — verify each in-sim.

## Phase 6 — `xplm-macros`

- `#[plugin(name = ..., signature = ..., description = ...)]` attribute macro
  generating the Phase 4 boilerplate.
- `#[derive(DataRefContainer)]` generating Phase 3 lookups from
  `#[dataref = "sim/..."]` field attributes, `find()` returning `Option<Self>`.
- Testable: `trybuild` UI tests for macro-expansion correctness; a
  snapshot/expand test comparing macro output against the hand-written
  equivalent.

## Phase 7 — Remaining surfaces

- Planes, Scenery, Utilities (`Planes.cs`, `Scenery.cs`, `Utilities.cs`).
- Widgets, off `SDK/CHeaders/Widgets` (`XPWidgets.h`, `XPStandardWidgets.h`,
  `XPUIGraphics.h`) — same RAII + trampoline treatment as everything else.
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
