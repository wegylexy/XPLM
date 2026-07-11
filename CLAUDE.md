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

## Panic safety

Every `extern "C"` trampoline that X-Plane calls back into MUST go through
`xplm::guard()` (`catch_unwind`). A panic unwinding into the native host is UB —
treat a missing guard on a new trampoline as a correctness bug, not a style nit.

## Current phase

Phase 0/1 (workspace scaffold + versioned `xplm-sys` FFI) per the plan agreed with
the user. See git log / PR description for phase-by-phase breakdown:
0 scaffold, 1 xplm-sys, 2 panic/RAII primitives, 3 DataRef subsystem, 4 plugin
lifecycle + trampolines, 5 menu/processing/instance/camera/display, 6 macros,
7 remaining surfaces (planes/scenery/utilities/widgets/XPMP2), 8 example port parity
with the original C# `XPL/Program.cs` sample.
