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

## Phase 3 — DataRef subsystem (done)

- Ported the scalar half of `DataAccess.cs` to `xplm::dataref::DataRef<T, Access>`
  (`T` sealed to `i32`/`f32`/`f64`), with `ReadOnly<T>`/`ReadWrite<T>` type
  aliases over sealed `ReadOnlyMarker`/`ReadWriteMarker` phantom types.
  `get()` is defined for both; `set()` only exists under a `Writable` bound
  that only `ReadWriteMarker` implements — a compile error, not a runtime
  writability check.
- `ArrayDataRef<T, Access>` (`T` sealed to `i32`/`f32`) covers
  `XPLMGetDatavi`/`XPLMSetDatavi`/`XPLMGetDatavf`/`XPLMSetDatavf`, with
  `len()`/`get(offset, &mut [T])`/`set(offset, &[T])` (`set` gated the same
  way). Byte-array (`xplmType_Data`) datarefs are out of scope for now.
- `XPLMRegisterDataAccessor` (publishing your own dataref) is deferred — it's
  a distinct RAII+trampoline object (closer to `FlightLoop`'s shape than to
  `find()`), not part of the "read an existing dataref" surface this phase
  covers.
- Tested: `trybuild` compile-fail tests (`xplm/tests/compile_fail.rs`) confirm
  `set()` doesn't exist on `ReadOnly<f32>`/`ReadOnlyArray<f32>`. Runtime
  correctness against real sim datarefs is unverified outside a hosted
  X-Plane process (same limitation as Phase 2 — `XPLMFindDataRef`/`XPLMGetData*`
  assume the sim engine is actually running, not just that the DLL is
  loaded) and is deferred to the Phase 4/5 example plugin's in-sim `/verify`.

## Phase 4 — Plugin lifecycle + trampolines (done)

- Ported `XPluginBase.cs`/`Plugin.cs` to `xplm::plugin::XPlanePlugin`
  (`start/enable/disable/stop/receive_message`, plus `NAME`/`SIGNATURE`/
  `DESCRIPTION` consts) and a `register_plugin!($t)` declarative macro
  generating the five `extern "C"` exports (`XPluginStart/Stop/Enable/
  Disable/ReceiveMessage`), each wrapped in `guard()`. State lives in a
  `static Mutex<Option<$t>>` generated per invocation — concrete-typed, no
  `dyn`/`Box` needed since the macro knows `$t` at expansion time.
  `register_plugin!` is the mechanism; Phase 6's `#[plugin(...)]` attribute
  macro will just be sugar over the same generated code.
- `FlightLoop` needed `unsafe impl Send` to live inside that static Mutex
  (X-Plane only ever calls back from its main thread, so no concurrent
  access occurs in practice — not `Sync`, since nothing here supports
  concurrent reads either).
- Built `examples/hello-plugin`, a `cdylib` implementing `XPlanePlugin` and
  using a `FlightLoop` heartbeat, proving Phases 2-4 together. Verified the
  five required symbols are present in the built DLL. Manual in-sim
  load/unload verification (actually running it inside X-Plane) is still
  outstanding — flagged for whenever there's a live X-Plane session to test
  against, same as the runtime DataRef checks deferred from Phase 3.

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
