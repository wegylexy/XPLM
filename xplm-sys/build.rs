use std::{env, path::PathBuf};

/// Cumulative XPLM version defines, in the order the SDK expects them to be
/// passed together (lower first). Cargo's feature dependency chain in
/// Cargo.toml guarantees that enabling e.g. `XPLM420` also enables every
/// entry below it, so we just emit whichever of these cfg-features are on.
const XPLM_VERSIONS: &[&str] = &[
    "XPLM200", "XPLM210", "XPLM300", "XPLM301", "XPLM302", "XPLM303", "XPLM400", "XPLM410",
    "XPLM420",
];

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sdk_dir = manifest_dir.join("../SDK");
    let headers_dir = sdk_dir.join("CHeaders");

    let defines: Vec<&str> = XPLM_VERSIONS
        .iter()
        .copied()
        .filter(|v| env::var(format!("CARGO_FEATURE_{v}")).is_ok())
        .collect();

    let mut builder = bindgen::Builder::default()
        .header(headers_dir.join("XPLM/XPLMDefs.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMUtilities.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMDataAccess.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMPlugin.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMProcessing.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMDisplay.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMGraphics.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMCamera.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMInstance.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMMenus.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMMap.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMPlanes.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMScenery.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMNavigation.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMSound.h").to_str().unwrap())
        .header(headers_dir.join("XPLM/XPLMWeather.h").to_str().unwrap())
        .clang_arg(format!("-I{}", headers_dir.join("XPLM").display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    builder = match target_os.as_str() {
        "windows" => {
            // libclang doesn't inherit an MSVC dev-shell environment on its own;
            // pull the same INCLUDE paths cc-rs resolves via vswhere so
            // `#include <windows.h>` (pulled in by XPLMDefs.h's `#if IBM`) resolves.
            let tool = cc::Build::new().get_compiler();
            for (key, val) in tool.env() {
                if key == "INCLUDE" {
                    for path in env::split_paths(val).filter(|p| !p.as_os_str().is_empty()) {
                        builder = builder.clang_arg(format!("-I{}", path.display()));
                    }
                }
            }
            builder
                .clang_arg("-DIBM=1")
                .clang_arg("-DAPL=0")
                .clang_arg("-DLIN=0")
        }
        "linux" => builder
            .clang_arg("-DLIN=1")
            .clang_arg("-DIBM=0")
            .clang_arg("-DAPL=0"),
        "macos" => builder
            .clang_arg("-DAPL=1")
            .clang_arg("-DIBM=0")
            .clang_arg("-DLIN=0"),
        other => panic!("unsupported target_os for XPLM SDK: {other}"),
    };

    for define in &defines {
        builder = builder.clang_arg(format!("-D{define}"));
        println!("cargo:rustc-cfg=feature=\"{define}\"");
    }

    let bindings = builder.generate().expect("unable to generate xplm-sys bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write bindings.rs");

    let lib_dir = match target_os.as_str() {
        "windows" => sdk_dir.join("Libraries/Win"),
        "linux" => sdk_dir.join("Libraries/Lin"),
        "macos" => sdk_dir.join("Libraries/Mac"),
        other => panic!("unsupported target_os for XPLM SDK linking: {other}"),
    };
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    if target_os == "windows" {
        println!("cargo:rustc-link-lib=dylib=XPLM_64");
    }
    // TODO Linux/macOS link flags (bundle-relative rpath, framework search paths) — Phase 1 follow-up.

    println!("cargo:rerun-if-changed={}", headers_dir.display());
}
