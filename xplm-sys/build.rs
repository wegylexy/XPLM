use std::{env, path::PathBuf};

/// Cumulative XPLM version defines, in the order the SDK expects them to be
/// passed together (lower first). Cargo's feature dependency chain in
/// Cargo.toml guarantees that enabling e.g. `XPLM420` also enables every
/// entry below it, so we just emit whichever of these cfg-features are on.
const XPLM_VERSIONS: &[&str] = &[
    "XPLM200", "XPLM210", "XPLM300", "XPLM301", "XPLM302", "XPLM303", "XPLM400", "XPLM410",
    "XPLM420",
];

/// Escapes regex metacharacters in a filesystem path so it can be used
/// literally in `bindgen::Builder::allowlist_file`, which takes a regex.
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Resolves the vendored X-Plane SDK: `XPLM_SDK_DIR` env var override (for
/// anyone pointing at a different SDK checkout) if set, else this crate's
/// own `vendor/xplm-sdk` — the latter is what a published crate tarball
/// actually contains (see `Cargo.toml`'s `include`), so a downstream
/// `cargo add`'d consumer builds with zero extra steps by default.
fn sdk_dir(manifest_dir: &std::path::Path) -> PathBuf {
    if let Some(dir) = env::var_os("XPLM_SDK_DIR") {
        return PathBuf::from(dir);
    }
    manifest_dir.join("vendor/xplm-sdk")
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sdk_dir = sdk_dir(&manifest_dir);
    let headers_dir = sdk_dir.join("CHeaders");

    let defines: Vec<&str> = XPLM_VERSIONS
        .iter()
        .copied()
        .filter(|v| env::var(format!("CARGO_FEATURE_{v}")).is_ok())
        .collect();

    let widgets = env::var("CARGO_FEATURE_WIDGETS").is_ok();

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
        // Only emit bindings for the XPLM SDK itself — `<windows.h>` (pulled
        // in transitively on Windows) drags in the entire Win32/COM surface
        // otherwise, which bloats the crate and ships auto-generated layout
        // tests for types (VARIANT, CREATESTRUCTA, ...) we don't own and
        // don't care to verify.
        .allowlist_file(format!(
            "{}.*",
            regex_escape(&headers_dir.join("XPLM").display().to_string())
        ))
        .layout_tests(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    if widgets {
        builder = builder
            .header(headers_dir.join("Widgets/XPWidgetDefs.h").to_str().unwrap())
            .header(headers_dir.join("Widgets/XPWidgets.h").to_str().unwrap())
            .header(
                headers_dir
                    .join("Widgets/XPStandardWidgets.h")
                    .to_str()
                    .unwrap(),
            )
            .header(headers_dir.join("Widgets/XPUIGraphics.h").to_str().unwrap())
            .clang_arg(format!("-I{}", headers_dir.join("Widgets").display()))
            .allowlist_file(format!(
                "{}.*",
                regex_escape(&headers_dir.join("Widgets").display().to_string())
            ));
    }

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

    if env::var("CARGO_FEATURE_DEPRECATED").is_ok() {
        builder = builder.clang_arg("-DXPLM_DEPRECATED");
    }

    if widgets {
        println!("cargo:rustc-cfg=feature=\"widgets\"");
    }

    let bindings = builder
        .generate()
        .expect("unable to generate xplm-sys bindings");

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
    match target_os.as_str() {
        "windows" => {
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=dylib=XPLM_64");
            // XPLM_64.dll only exists inside a running X-Plane process.
            // Delay-load it so `cargo test`/standalone tools can start (and
            // run everything that doesn't actually call into the sim)
            // without it on PATH — it's only resolved lazily, on first real
            // call into an XPLM_* function.
            println!("cargo:rustc-link-arg=/DELAYLOAD:XPLM_64.dll");
            if widgets {
                println!("cargo:rustc-link-lib=dylib=XPWidgets_64");
                println!("cargo:rustc-link-arg=/DELAYLOAD:XPWidgets_64.dll");
            }
            println!("cargo:rustc-link-lib=dylib=delayimp");
        }
        "linux" => {
            // SDK/Libraries/Lin ships plain XPLM_64.so/XPWidgets_64.so stubs
            // (unlike Windows, there's no PE-style delay-load equivalent for
            // ELF — a `cargo test` that actually calls an `XPLM*` function on
            // Linux needs the real lib resolvable via `LD_LIBRARY_PATH` at
            // process start; untested on a real Linux machine in this repo's
            // dev environment, so treat this as convention-based, not
            // confirmed).
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=dylib=XPLM_64");
            if widgets {
                println!("cargo:rustc-link-lib=dylib=XPWidgets_64");
            }
        }
        "macos" => {
            // SDK/Libraries/Mac ships XPLM.framework/XPWidgets.framework
            // bundles, not plain dylibs — link as frameworks, standard
            // X-Plane plugin convention for this platform. Also untested on
            // a real Mac in this repo's dev environment.
            println!("cargo:rustc-link-search=framework={}", lib_dir.display());
            println!("cargo:rustc-link-lib=framework=XPLM");
            if widgets {
                println!("cargo:rustc-link-lib=framework=XPWidgets");
            }
        }
        other => panic!("unsupported target_os for XPLM SDK linking: {other}"),
    }

    println!("cargo:rerun-if-changed={}", headers_dir.display());
}
