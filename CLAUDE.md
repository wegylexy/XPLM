# CLAUDE.md

Rust port of the wegylexy/xplm C# X-Plane SDK wrapper, targeting the same design
goals with Rust idioms: no leaky panics across FFI, RAII-based lifecycle instead of
manual unregister calls, and compile-time-gated dataref mutability instead of runtime
checks.

## Workspace layout

- `xplm-sys` — raw `bindgen` FFI over `SDK/CHeaders`. No safety, no ergonomics.
- `xplm` — safe wrappers: panic-boundary `guard()`, DataRef access-gating, plugin
  lifecycle, RAII wrappers for windows/menus/instances/commands, callback trampolines
  backed by a closure registry.
- `xplm-macros` — `#[plugin(...)]` attribute macro and `#[derive(DataRefContainer)]`,
  re-exported through `xplm`.
- `external/XPMP2` — git submodule of https://github.com/TwinFan/XPMP2.git
  (pinned to `master`/`v3.6.1`, currently the same commit). Its public
  plane-creation API is a C++ class (`XPMP2::Aircraft`, subclass + override
  virtuals), not flat C, despite the SDK's own naming suggesting otherwise
  (`XPCAircraft.h` is itself a deprecated *C++* wrapper class, not a C API).
- `xpmp2-sys` — raw FFI over XPMP2: `bindgen` for its flat-C surface
  (`XPMPMultiplayer.h`), plus a hand-written C++ shim
  (`xpmp2-sys/shim/shim.h`+`shim.cpp`) exposing `XPMP2::Aircraft`'s virtuals
  as `extern "C"` functions, since `bindgen` can't synthesize a vtable or let
  Rust override a C++ virtual method on its own. Compiles XPMP2's own `.cpp`
  sources directly via the `cc` crate rather than vendoring prebuilt libs.
- `xpmp2` — safe wrappers over `xpmp2-sys`: `Multiplayer` (RAII init/
  cleanup), an `Aircraft` trait + RAII `Plane`/`PlaneHandle` over the shim,
  and (behind the `csl-on-demand` feature) `CslCache`, a synchronous wrapper
  around the published `flybywireless-csl-client` crate for fetching CSL
  packages on demand from a `csl-on-demand` server instead of shipping them
  all locally.

## SDK headers

`SDK/` is gitignored — not committed. See [README.md](README.md) for the download
link and expected layout. Never assume `SDK/` exists in a fresh checkout; if a build
fails because `SDK/CHeaders` is missing, that's the download step, not a bug.

## Version features

Cargo features `XPLM200` through `XPLM420` mirror the `XPLM<version>` macros in the
vendored headers exactly — same names, same set (currently: 200, 210, 300, 301, 302,
303, 400, 410, 420; explicitly **not** 430, even though newer SDK zips define it,
until the headers here are updated and the user asks for it). They cascade low-to-high
in both `xplm-sys/Cargo.toml` and `xplm/Cargo.toml` because the SDK does not imply
lower versions when a higher one is defined — enabling `XPLM420` must pass every
`-D` flag for the versions below it too. Don't add a new version feature without
checking it's actually present as an `#if defined(XPLM...)` guard in `SDK/CHeaders`
first (`grep -rohE "XPLM[0-9]{3}" SDK/CHeaders/*/*.h | sort -u`).

## Optional native libraries

`widgets` (`xplm-sys` → `xplm`, off by default, same shape as `deprecated`)
bindgens and links `SDK/CHeaders/Widgets` + `XPWidgets_64` — a second native
DLL alongside `XPLM_64`. It's still a flat C API `bindgen` handles the same
way as the rest of the SDK; it's feature-gated purely so a plugin that never
touches the widgets toolkit doesn't link a DLL it never calls into. Don't
assume `xplm::widget` is compiled in when checking a build — build/test with
`--features widgets` explicitly, same as any `XPLM2xx`-`XPLM4xx` combination.

## CSL package loading (`xpmp2`)

`csl-offline` (bulk-load a local CSL library via `Multiplayer::load_csl_package`)
and `csl-on-demand` (fetch + load exactly one model's package the instant it's
needed, via the published `flybywireless-csl-client` crate) are two independent
loading strategies for the same underlying `XPMPLoadCSLPackage` call — not a
dependency chain, and deliberately mutually exclusive (a `compile_error!` in
`xpmp2/src/lib.rs` enforces it). Both are opt-in (neither is in `xpmp2`'s
`default` features): if either were on by default, `cargo build --workspace`
would hit a Cargo feature-unification conflict the moment any workspace member
depends on `xpmp2` with the other feature explicitly enabled (Cargo unifies
features across every selected member in one invocation, including `xpmp2`
itself built with its own defaults) — see `examples/xpmp2-template`, which
needs `csl-on-demand`.

`csl-on-demand`'s `CslCache::request` must pass `Plane::new` the *exact*
`"{root}/{id}"` CSL identifier `csl-on-demand`'s `/match` endpoint already
resolved server-side (recovered from the fetched package's `xsb_aircraft.txt`,
since `flybywireless-csl-client`'s `FetchedModel` doesn't expose it directly) —
never an empty `csl_id`. An empty `csl_id` makes XPMP2 fall back to its own
local `ChangeModel`/`CSLModelMatching`, which needs `Doc8643.txt`/`related.txt`
(on-demand mode has no reason to bundle those) and outright fails, via a
thrown `XPMP2Error`, the first time ever, before any package has been loaded
(`external/XPMP2/src/CSLModels.cpp`'s `CSLModelMatching` bails out immediately
if `glob.mapCSLModels` is empty). `Plane::new` asserts against this (panics on
an empty `csl_id`) when built with `csl-on-demand`.

## Panic safety

Every `extern "C"` trampoline that X-Plane calls back into MUST go through
`xplm::guard()` (`catch_unwind`). A panic unwinding into the native host is UB —
treat a missing guard on a new trampoline as a correctness bug, not a style nit.

## No stale indices, no public raw pointers/indices

Any wrapper over an SDK API where items live in an X-Plane-managed, index-addressed
list (menu items today; anything with similar remove-and-reindex semantics in later
phases — e.g. Widgets' item trees) MUST NOT cache a numeric index or expose a raw
pointer/index as a `pub` field on the per-item handle. Caching goes stale silently
the moment an earlier sibling is removed and X-Plane reindexes everything below it.

The pattern used in `xplm::menu` (`Menu`/`MenuItem`) is the template: the parent
holds the authoritative ordered list (`Vec<Option<Rc<ItemToken>>>`, `None` for
slots that don't carry a Rust-side handle but still consume an index, e.g.
separators), each child handle holds an `Rc<Token>` identity marker (not an index,
not a raw pointer), and the current index is computed by identity lookup
(`Rc::ptr_eq`) on every access — mirroring the C# original's `Items.IndexOf(this)`
rather than a cached field. Apply this same shape to Instance/Widgets/Scenery
wrappers in later phases, not just Menu; this was raised as a real bug (not a style
nit) once already and shouldn't need raising twice.

## Running tests

`XPLM_64.dll` only exists inside a running X-Plane process; `xplm-sys` delay-loads
it so `cargo test` can start, but any test that actually calls an `XPLM*` function
still needs it resolvable on `PATH`. Read the real install location from
`%LocalAppData%\x-plane_install_12.txt` (or `_11.txt`) rather than hardcoding a
path — see [README.md](README.md#running-tests-windows) for the one-liner.

## Current status

The pure-XPLM SDK surface (`xplm-sys`/`xplm`/`xplm-macros`, including
Widgets) is fully ported, with `examples/hello-plugin` and
`examples/xpl-template` as acceptance tests. `xpmp2-sys`/`xpmp2` (XPMP2
multiplayer support, including on-demand CSL package fetching) are also
built out, with `examples/xpmp2-template` as its acceptance test. See
[README.md](README.md)'s "Multiplayer (XPMP2)" section for usage; git log
covers how it was built.
