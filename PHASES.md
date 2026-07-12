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

## Phase 5 — Menu, Camera (done); Instance/Display/Graphics rescoped

- `xplm::menu::Menu` (`XPLMMenus.h`): same boxed-closure-behind-a-refcon
  trampoline shape as `FlightLoop`, plus an internal item list mirroring the
  C# original's `List<Item>`. `Menu::add_item` returns a `MenuItem` handle
  (matching the C# original's `Item`-returning `AppendItem`) with
  `set_checked`/`checked`/`set_enabled`/`set_name`/`remove` on it — but
  crucially `MenuItem` does **not** cache its own index or expose the raw
  item pointer. Each item is an `Rc<ItemToken>` identity marker; the `Menu`'s
  internal `Vec<Option<Rc<ItemToken>>>` (one slot per XPLM index, `None` for
  separators so positions stay aligned) is the source of truth, and
  `MenuItem::index()` finds its current position by identity (`Rc::ptr_eq`)
  each time it's called — the same reasoning as the C# original's
  `Items.IndexOf(this)`, so it's automatically correct after an earlier item
  is removed and X-Plane reindexes everything below it, rather than going
  stale. The item's XPLM refcon is that same identity pointer; the
  trampoline finds the clicked item's current index the same way.
  `Menu::new_in_plugins_menu` handles the two-step `XPLMAppendMenuItem` +
  `XPLMCreateMenu` dance needed to anchor a plugin's top-level menu under
  X-Plane's Plugins menu.
- `xplm::camera::CameraControl` (`XPLMCamera.h`): same shape again. Notably
  the SDK has exactly one global camera-control slot with no handle
  returned by `XPLMControlCamera` — documented as a caveat on `Drop` (a
  second `CameraControl` silently orphans a first still-alive one; dropping
  the first afterward releases control the second thinks it holds). This is
  inherent to the SDK's shape, not something the wrapper can fix.
- **Rescoped**: `Instance` (`XPLMInstance.h`) depends on `XPLMDrawInfo_t` and
  `XPLMObjectRef` from `XPLMScenery.h` — pulling it in here would mean
  half-implementing Scenery anyway, so it moves to Phase 7 alongside
  Scenery instead of being split across two phases artificially.
- **Rescoped**: `Display`/`Graphics` (`XPLMDisplay.h` is 2100+ lines, with
  ~8 window callbacks — draw/click/key/cursor/wheel/right-click/etc.) is too
  large a surface for this pass without rushing a shallow implementation.
  Split out as **Phase 5b** (windows + drawing) to be scoped and tackled on
  its own.
- Tested: extended `examples/hello-plugin` with a `Menu` item alongside the
  existing `FlightLoop` heartbeat; confirmed the five required plugin
  exports are still present in the built DLL. `CameraControl` isn't
  exercised in the example (nothing to sensibly demo without a live camera
  to observe) — covered by its unit-level type conversions only.

## Phase 5b — Windows (done); Graphics still open

- `xplm::window::Window`/`WindowBuilder` (`XPLMCreateWindowEx`): the SDK
  requires all five core callbacks (draw/mouse-click/key/cursor/mouse-wheel)
  to be non-null, so `WindowBuilder` defaults each to a no-op/pass-through
  closure — you only set `.on_draw(...)`/`.on_mouse_click(...)`/etc. for the
  ones you actually need, rather than a five-or-six-closure constructor.
  `.on_right_click(...)` (`XPLM300`+) follows the same optional pattern.
  `WindowRef` is a `Copy`, non-owning handle (geometry/visibility/title/
  focus/front-ness getters+setters) passed into every callback and returned
  by `Window::handle()`, separate from the owning `Window` so callbacks and
  outside code (e.g. a menu handler toggling window visibility, as
  `examples/hello-plugin` now does) can both hold a reference without
  fighting over ownership. Unlike `Menu`, a window has no child-index-space
  that goes stale, so the identity-lookup pattern doesn't apply here — this
  is a plain boxed-closures-behind-a-refcon trampoline set, `FlightLoop`'s
  shape.
- Hit and fixed a real `AssertUnwindSafe` + disjoint-closure-capture pitfall
  while wiring the mouse-click trampoline: capturing `&RefCell<_>` (even
  wrapped in `AssertUnwindSafe`) across `guard()`'s boundary fails, because
  edition-2021 disjoint capture grabs the wrapper's inner field directly,
  bypassing the blanket `UnwindSafe` impl the wrapper exists to provide. Fix
  was to capture only the raw `refcon` pointer (matching every other
  trampoline in this crate) and dereference it inside the closure body,
  never capturing a typed `&RefCell` from the enclosing scope.
- Building this surfaced two version-gating bugs, now fixed:
  - `xplm::processing` (`FlightLoop`, Phase 2) uses `XPLMCreateFlightLoop`/
    `XPLMDestroyFlightLoop`/`XPLMScheduleFlightLoop`, all `#if defined(XPLM210)`
    in the header, but the module itself was never `cfg`-gated — invisible
    under default features (which always include `XPLM210`), but a hard
    build failure under `--features XPLM200` alone. Now
    `#[cfg(feature = "XPLM210")]` on `pub mod processing;` in `lib.rs`.
  - `window.rs` initially over-gated `WindowRef::bring_to_front`/`is_in_front`
    behind `XPLM300`, but `XPLMBringWindowToFront`/`XPLMIsWindowInFront` are
    actually ungated in the header (available since the legacy API) — fixed
    by removing the incorrect `cfg`.
  - Caught by actually building `--no-default-features --features XPLM200`
    as a check, not just the default feature set — worth doing for every
    future module, not just this one.
- **Still open**: `XPLMGraphics.h` (drawing primitives, coordinate
  conversion) — mostly stateless free functions, lower effort than the
  window surface. Not done in this pass; `examples/hello-plugin`'s window
  draw callback is a no-op as a result (there's nothing to draw with yet).

## Phase 6 — `xplm-macros`

- `#[plugin(name = ..., signature = ..., description = ...)]` attribute macro
  generating the Phase 4 boilerplate.
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
