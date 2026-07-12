use std::{env, path::PathBuf};

/// Escapes regex metacharacters — same helper as `xplm-sys/build.rs`, kept
/// in sync manually rather than shared, since these are two independent
/// crates.
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

/// `XPMP2`'s own `SOURCE_FILES` list (`CMakeLists.txt`), minus
/// `SoundFMOD.cpp` — that one's gated behind `INCLUDE_FMOD_SOUND`, off by
/// default upstream too, and pulls in a separate FMOD SDK this crate
/// doesn't vendor.
const SOURCES: &[&str] = &[
    "2D.cpp",
    "AIMultiplayer.cpp",
    "Aircraft.cpp",
    "CSLCopy.cpp",
    "CSLModels.cpp",
    "Contrail.cpp",
    "Map.cpp",
    "Network.cpp",
    "RelatedDoc8643.cpp",
    "Remote.cpp",
    "Sound.cpp",
    "Utilities.cpp",
    "XPMPMultiplayer.cpp",
];

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let xpmp2_dir = manifest_dir.join("../external/XPMP2");
    let xpmp2_inc = xpmp2_dir.join("inc");
    let xpmp2_src = xpmp2_dir.join("src");
    let xplm_headers = manifest_dir.join("../SDK/CHeaders/XPLM");

    if !xpmp2_inc.join("XPMPMultiplayer.h").is_file() {
        panic!(
            "external/XPMP2 submodule not found/initialized (expected {}). \
             Run `git submodule update --init external/XPMP2`.",
            xpmp2_inc.display()
        );
    }
    if !xplm_headers.join("XPLMDefs.h").is_file() {
        panic!(
            "SDK/CHeaders/XPLM not found (expected {}) — see README.md for \
             the SDK download step; xpmp2-sys needs it for the same reason \
             xplm-sys does (XPMP2's own headers include XPLM's).",
            xplm_headers.display()
        );
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    // --- Compile XPMP2 itself as a static library -------------------------
    //
    // Upstream's CMakeLists.txt always builds XPMP2 as a STATIC lib unless
    // XPMP2_BUILD_SHARED_LIBS is explicitly turned on (Windows-only, and
    // only useful for sharing one XPMP2 instance across multiple plugins in
    // the same process — not needed here); static matches how XPMP2 is
    // meant to be linked directly into a single plugin binary on every
    // platform, so that's what this build does too, uniformly.
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(&xpmp2_inc)
        .include(&xpmp2_src)
        .include(&xplm_headers)
        // Matches CMakeLists.txt's `target_compile_definitions(XPMP2 PUBLIC
        // XPLM200=1 XPLM210=1 XPLM300=1 XPLM301=1 XPLM303=1 XPLM400=1)` —
        // XPMP2 itself only conditions on these, never anything above
        // XPLM400, so there's no reason to enable more here.
        .define("XPLM200", "1")
        .define("XPLM210", "1")
        .define("XPLM300", "1")
        .define("XPLM301", "1")
        .define("XPLM303", "1")
        .define("XPLM400", "1")
        // Pinned to the vendored submodule's tagged version
        // (external/XPMP2 is pinned to v3.6.1) — update alongside the
        // submodule if it's ever bumped.
        .define("XPMP2_VERSION", "\"3.6.1\"")
        .define("XPMP2_VER_MAJOR", "3")
        .define("XPMP2_VER_MINOR", "6")
        .define("XPMP2_VER_PATCH", "1");

    match target_os.as_str() {
        "windows" => {
            build
                .define("IBM", "1")
                .define("APL", "0")
                .define("LIN", "0")
                .define("_WIN32_WINNT", "0x0601")
                .define("_CRT_SECURE_NO_WARNINGS", None)
                .flag_if_supported("/EHsc");
        }
        "linux" => {
            build
                .define("LIN", "1")
                .define("IBM", "0")
                .define("APL", "0")
                .flag_if_supported("-fexceptions")
                .flag_if_supported("-Wno-deprecated-declarations");
        }
        "macos" => {
            build
                .define("APL", "1")
                .define("IBM", "0")
                .define("LIN", "0")
                .flag_if_supported("-fexceptions")
                .flag_if_supported("-Wno-deprecated-declarations");
        }
        other => panic!("unsupported target_os for XPMP2: {other}"),
    }

    for source in SOURCES {
        build.file(xpmp2_src.join(source));
    }
    build.compile("xpmp2");

    // Sources built above call into Winsock (XPMP2::Network for its
    // "Remote" shared-traffic client) — the static lib itself compiles
    // without them, but whatever finally links this crate in needs them
    // resolvable.
    if target_os == "windows" {
        println!("cargo:rustc-link-lib=dylib=Ws2_32");
        println!("cargo:rustc-link-lib=dylib=Iphlpapi");
    }

    // --- bindgen the flat-C surface only -----------------------------------
    //
    // Only `XPMPMultiplayer.h` (init/cleanup/sound/CSL-loading/contrail —
    // see PHASES.md's "What XPMP2 actually exposes" note) is handed to
    // bindgen here. `XPMPAircraft.h`'s `XPMP2::Aircraft` is a C++ class
    // bindgen can't wrap directly — that needs the hand-written C++ shim
    // described in PHASES.md's Phase 1, not yet written, exposed to this
    // crate as its own small `extern "C"` header once it exists.
    let mut builder = bindgen::Builder::default()
        .header(xpmp2_inc.join("XPMPMultiplayer.h").to_str().unwrap())
        .clang_arg("-x")
        .clang_arg("c++")
        .clang_arg("-std=c++17")
        .clang_arg(format!("-I{}", xpmp2_inc.display()))
        .clang_arg(format!("-I{}", xplm_headers.display()))
        .clang_arg("-DXPLM200=1")
        .clang_arg("-DXPLM210=1")
        .clang_arg("-DXPLM300=1")
        .clang_arg("-DXPLM301=1")
        .clang_arg("-DXPLM303=1")
        .clang_arg("-DXPLM400=1")
        .allowlist_file(format!(
            "{}.*",
            regex_escape(&xpmp2_inc.join("XPMPMultiplayer.h").to_string_lossy())
        ))
        // These few functions take/return `std::string`/`XPMP2::Aircraft*`
        // instead of the plain C types (`const char *`, `bool`, ...) the
        // rest of this header uses — bindgen can't safely lay out MSVC's
        // `std::string` ABI, and `XPMP2::Aircraft` is the C++ class this
        // crate doesn't bind yet (see the module doc). Every one of these
        // has a plain-C-typed equivalent already in this same header
        // (`XPMPGetModelInfo` for `XPMPGetModelInfo2`, etc.) except
        // `XPMPAddModelDataRef`/`XPMPSoundGetActiveAudioDevice`'s
        // `std::string*` overload, which just aren't exposed until/unless
        // something needs them enough to justify a small C-string shim.
        .blocklist_function("XPMPSoundGetAudioDeviceName")
        .blocklist_function("XPMPSoundSetAudioDeviceName")
        .blocklist_function("XPMPGetModelInfo2")
        .blocklist_function("XPMPAddModelDataRef")
        .blocklist_function("XPMPGetAircraft")
        .blocklist_function("XPMPSoundGetActiveAudioDevice")
        .blocklist_type("std::.*")
        .opaque_type("std::.*")
        .layout_tests(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    builder = match target_os.as_str() {
        "windows" => {
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
        _ => unreachable!("checked above"),
    };

    let bindings = builder
        .generate()
        .expect("unable to generate xpmp2-sys bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write bindings.rs");

    println!("cargo:rerun-if-changed={}", xpmp2_inc.display());
    println!("cargo:rerun-if-changed={}", xpmp2_src.display());
}
