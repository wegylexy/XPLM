# XPLM (Rust)

Idiomatic Rust bindings for the X-Plane plugin SDK, spanning `xplm-sys` (raw FFI),
`xplm` (safe wrappers), and `xplm-macros` (`#[plugin]`, `#[derive(DataRefContainer)]`).

## Getting the SDK

The X-Plane SDK headers/libraries are not vendored in this repo. Download and unzip
them before building:

1. Download the SDK zip: https://developer.x-plane.com/wp-content/plugins/code-sample-generation/sdk_zip_files/XPSDK430.zip
2. Unzip it at the repository root so you end up with:

   ```
   XPLM/
   ├── SDK/
   │   ├── CHeaders/
   │   ├── Libraries/
   │   ├── license.txt
   │   └── README.txt
   ├── xplm-sys/
   ├── xplm/
   └── xplm-macros/
   ```

   The zip's top-level folder is typically named after its release (e.g. `XPSDK430`) —
   rename it to `SDK` after extracting.

`SDK/` is gitignored; `xplm-sys`'s `build.rs` reads headers/libs from it at build time.

## Supported X-Plane / XPLM versions

Cargo features on `xplm-sys` and `xplm` mirror the `XPLM<version>` macros defined in
the SDK headers, cumulative low-to-high (enabling a higher version feature pulls in
every lower one, matching the SDK's requirement that all applicable version macros be
defined together):

| Feature   | X-Plane version      |
|-----------|-----------------------|
| `XPLM200` | 9.00+                 |
| `XPLM210` | 10.00+ (10.20+ for 64-bit) |
| `XPLM300` | 11.10+                |
| `XPLM301` | 11.20+                |
| `XPLM302` | 11.21+                |
| `XPLM303` | 11.50+                |
| `XPLM400` | 12.04+                |
| `XPLM410` | 12.1.0+               |
| `XPLM420` | 12.3.0+ (default)     |

Only the versions present in the vendored `SDK/CHeaders` are exposed; there is no
`XPLM430` feature even if a newer SDK zip defines it, until this crate is updated.
