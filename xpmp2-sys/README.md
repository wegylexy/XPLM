# flybywireless-xpmp2-sys

Raw FFI over [XPMP2](https://github.com/TwinFan/XPMP2): `bindgen` for its
flat-C surface, plus a hand-written C++ shim exposing `XPMP2::Aircraft`'s
virtuals as `extern "C"` functions. Compiles XPMP2's sources directly via the
`cc` crate.

Published on crates.io as `flybywireless-xpmp2-sys`. See the
[workspace README](https://github.com/wegylexy/XPLM) for setup.
