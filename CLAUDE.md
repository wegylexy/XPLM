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

## Current phase

Phase 2 (panic-boundary `guard()` + `FlightLoop` RAII/trampoline pattern) is done
— see git log for what's landed. Remaining phases (3-8: DataRef subsystem, plugin
lifecycle, menu/processing/instance/camera/display, macros, remaining surfaces,
parity example) are tracked in [PHASES.md](PHASES.md); keep that file updated as
phases complete instead of duplicating the breakdown here.
