//! End-to-end proof that a `CslCache`-fetched model is actually *loaded*
//! into XPMP2's model catalog, not just downloaded to disk. Regression
//! guard for the `../_blobs/...` bug: `csl-on-demand`'s server used to emit
//! CSL package references XPMP2 couldn't resolve (`ERR_PKG_UNKNOWN`), so
//! every fetched model silently failed to register even though its files
//! landed on disk fine. The fix (published as `flybywireless-xpmp2` 0.2.0 /
//! `flybywireless-csl-client` 0.2.0): the server now emits `_blobs/...`
//! (no `../`), and `CslCache::new` registers that `_blobs` package with
//! XPMP2 up front, before any fetch. See `xpmp2/CLAUDE.md`/the crate's own
//! README "Multiplayer (XPMP2)" section for the full story.
//!
//! # Why this doesn't drive `CslCache::request`'s own callback
//!
//! `CslCache::request`'s completion is delivered through an
//! `xplm::processing::FlightLoop`, which only ever fires when X-Plane's own
//! frame loop calls back into it — a plain `cargo test` process has no such
//! loop (see `xplm::processing::tests`'s own doc comment: "requires
//! XPLM_64.dll to be loaded by an actual X-Plane host, which a plain `cargo
//! test` process doesn't provide"). So this test exercises the same pieces
//! `CslCache` itself is built from, but drives them directly instead of
//! through that flight loop:
//! - `CslCache::new` — synchronous, no flight loop involved. This is the
//!   actual bug fix: it registers the `_blobs` package with XPMP2 before
//!   any fetch can reference it.
//! - `csl_client::ModelClient::request` — the same async HTTP client
//!   `CslCache` wraps internally, called directly here instead of through
//!   `CslCache`'s background worker thread.
//! - `CslCache::load_local` — the same synchronous `XPMPLoadCSLPackage`
//!   call `CslCache`'s flight loop would have made with the fetch result.
//!
//! Together these are exactly what `CslCache::request`'s callback would have
//! delivered on a real X-Plane frame loop — just invoked without needing one.
//!
//! # `#[ignore]`d under plain `cargo test` — confirmed by running it
//!
//! Sidestepping the flight loop (above) isn't enough on its own:
//! `Multiplayer::init` (`XPMPMultiplayerInit`) itself crashes
//! (`STATUS_ACCESS_VIOLATION`) when called from a plain `cargo test`
//! process with no real X-Plane host behind `XPLM_64.dll` — confirmed by
//! actually running this test with `eprintln!` markers around every call:
//! execution reaches "calling Multiplayer::init" and dies there, every
//! time, before that call can even return. This matches
//! `readme_examples.rs`'s own module doc in this same directory ("calling
//! into real XPMP2/XPLM functions outside a hosted X-Plane process
//! crashes") and `xplm::processing::tests`'s doc comment about
//! `XPLMCreateFlightLoop` needing "an actual X-Plane host, which a plain
//! `cargo test` process doesn't provide" — this crate's own tests already
//! document this exact limitation for simpler calls than
//! `XPMPMultiplayerInit`.
//!
//! So this test is `#[ignore]`d rather than deleted or left to fail CI: it
//! is a real, runnable proof of the fix (server `_blobs` refs +
//! `CslCache::new`'s package registration) when actually loaded as a plugin
//! inside a hosted X-Plane process — e.g. via the
//! `examples/hot-reload-xpl`/`hot-reload-dll` pattern this workspace already
//! has for exactly this class of "needs a real sim host" testing — not when
//! run as an ordinary standalone `cargo test` binary. Run manually with
//! `cargo test -p flybywireless-xpmp2 --features csl-on-demand --test
//! csl_on_demand_e2e -- --ignored` from inside such a host if you need to
//! re-verify it; a plain `cargo test`/CI run will not execute it, and could
//! not pass even if it did.

#![cfg(feature = "csl-on-demand")]

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// NOTE: this crate's package name is `flybywireless-xpmp2`, so its lib
// target name (and thus what an integration test must import) is
// `flybywireless_xpmp2`, not `xpmp2` — `readme_examples.rs` in this same
// directory uses `xpmp2::...` and does not currently compile as a result
// (confirmed while writing this test); flagged separately, not fixed here
// per this task's "don't touch xpmp2 source" scope.
use flybywireless_xpmp2::csl_on_demand::CslCache;
use flybywireless_xpmp2::Multiplayer;

const BIND_ADDR: &str = "127.0.0.1:18091";

/// Kills the `csl-service` child process and removes the temp working
/// directory on drop — runs on both the pass and the panic/fail path, since
/// a `Drop` impl runs during unwinding too.
struct Guard {
    child: Child,
    tmp: PathBuf,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.tmp);
    }
}

/// `csl-on-demand` is expected checked out as a sibling of this (`XPLM`)
/// repo, matching how this whole workspace was set up for this test.
fn csl_on_demand_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent() // xpmp2/ -> XPLM/
        .expect("xpmp2's parent directory")
        .parent() // XPLM/ -> repos/
        .expect("XPLM's parent directory")
        .join("csl-on-demand")
}

/// Minimal CSL fixture: one package ("TestPkg") exporting one aircraft
/// ("TestA320") via a single untextured OBJ8 model — just enough for
/// `csl-ingest`/`csl-service` to serve a real `/match` + `/manifest` round
/// trip without needing a real CSL library on disk.
fn write_fixture(source_dir: &Path) {
    let pkg_dir = source_dir.join("CSL").join("TestPkg");
    std::fs::create_dir_all(&pkg_dir).expect("create fixture package dir");
    std::fs::write(
        pkg_dir.join("xsb_aircraft.txt"),
        "EXPORT_NAME TestPkg\n\
         OBJ8_AIRCRAFT TestA320\n\
         OBJ8 SOLID YES TestPkg/test.obj\n\
         ICAO A320\n",
    )
    .expect("write xsb_aircraft.txt");
    // No TEXTURE lines needed — the manifest/template-splice path works
    // (and is exercised) the same whether or not an OBJ8 model references
    // any textures; this fixture only needs to prove registration, not
    // render anything.
    std::fs::write(
        pkg_dir.join("test.obj"),
        "I\n800\nOBJ8\n\nPOINT_COUNTS 0 0 0 0\n",
    )
    .expect("write test.obj");
}

fn wait_for_port(addr: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect(addr).is_ok() {
            return;
        }
        if Instant::now() >= deadline {
            panic!("csl-service never started listening on {addr} within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore = "XPMPMultiplayerInit crashes (STATUS_ACCESS_VIOLATION) outside a real, hosted \
            X-Plane process — see this file's module doc. Run manually inside such a host."]
fn fetched_model_is_actually_loaded_into_xpmp2() {
    let csl_root = csl_on_demand_root();
    assert!(
        csl_root.join("Cargo.toml").exists(),
        "expected a csl-on-demand checkout at {} (a sibling of this XPLM repo)",
        csl_root.display()
    );

    let tmp = std::env::temp_dir().join(format!(
        "csl_on_demand_e2e_{}_{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    let source_dir = tmp.join("source");
    let resources_dir = tmp.join("csl-resources");
    let cache_dir = tmp.join("CSLCache");
    let xpmp_resources = tmp.join("xpmp-resources");
    std::fs::create_dir_all(&xpmp_resources).expect("create xpmp resource dir");
    write_fixture(&source_dir);

    // Build csl-service + csl-ingest (debug, same as this test's own
    // profile) before running either.
    let status = Command::new("cargo")
        .args(["build", "-p", "csl-service"])
        .current_dir(&csl_root)
        .status()
        .expect("failed to run `cargo build -p csl-service` — is cargo on PATH?");
    assert!(status.success(), "cargo build -p csl-service failed");

    let target_dir = csl_root.join("target").join("debug");
    let exe = |name: &str| target_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let ingest_bin = exe("csl-ingest");
    let service_bin = exe("csl-service");
    assert!(
        ingest_bin.exists(),
        "csl-ingest binary not found at {}",
        ingest_bin.display()
    );
    assert!(
        service_bin.exists(),
        "csl-service binary not found at {}",
        service_bin.display()
    );

    let status = Command::new(&ingest_bin)
        .arg(&resources_dir)
        .arg(&source_dir)
        .status()
        .expect("failed to run csl-ingest");
    assert!(status.success(), "csl-ingest failed");
    assert!(
        resources_dir.join("manifests").is_dir(),
        "csl-ingest produced no manifests/ directory under {}",
        resources_dir.display()
    );

    let child = Command::new(&service_bin)
        .env("CSL_RESOURCES", &resources_dir)
        .env("CSL_BIND", BIND_ADDR)
        .env("CSL_PATH_PREFIX", "/csl")
        .env("CSL_BLOBS_PACKAGE", "_blobs")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn csl-service");

    let guard = Guard {
        child,
        tmp: tmp.clone(),
    };
    wait_for_port(BIND_ADDR, Duration::from_secs(15));

    eprintln!("[e2e] calling skip_resource_validation");
    flybywireless_xpmp2::skip_resource_validation(true);
    eprintln!("[e2e] calling Multiplayer::init");
    let multiplayer = Multiplayer::init(
        "csl_on_demand_e2e",
        xpmp_resources
            .to_str()
            .expect("xpmp resource dir path is valid UTF-8"),
        Some("A320"),
        None,
    )
    .expect("XPMPMultiplayerInit failed");
    eprintln!("[e2e] Multiplayer::init returned Ok");

    // Baseline: nothing loaded yet.
    eprintln!("[e2e] calling XPMPGetNumberOfInstalledModels (before)");
    let before = unsafe { xpmp2_sys::XPMPGetNumberOfInstalledModels() };
    eprintln!("[e2e] before = {before}");
    assert_eq!(
        before, 0,
        "expected zero installed models before any package was loaded"
    );

    // The actual fix under test: constructing `CslCache` registers the
    // `_blobs` package with XPMP2 immediately (synchronously) — before this
    // existed, every OBJ8/xsb_aircraft.txt reference to `_blobs/...` (or,
    // pre-fix, `../_blobs/...`) failed to resolve.
    let csl_cache = CslCache::new(
        &multiplayer,
        format!("http://{BIND_ADDR}/csl"),
        &cache_dir,
        "_blobs",
    )
    .expect("CslCache::new failed");
    eprintln!("[e2e] CslCache::new returned Ok");

    // Fetch through the same `csl_client::ModelClient` `CslCache::request`
    // uses internally (see this file's module doc for why we call it
    // directly instead of going through `CslCache::request`'s flight-loop
    // delivery).
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let client = csl_client::ModelClient::new(
        reqwest::Client::new(),
        format!("http://{BIND_ADDR}/csl"),
        csl_cache.out_dir().to_path_buf(),
    );
    let fetched = runtime
        .block_on(client.request(Some("A320"), None, None, None))
        .expect("ModelClient::request failed — the fixture model failed to fetch");
    eprintln!(
        "[e2e] ModelClient::request returned Ok, package_dir={}",
        fetched.package_dir.display()
    );

    // The other half of what `CslCache::request`'s flight loop would have
    // done: load the fetched package into XPMP2 (`XPMPLoadCSLPackage`).
    csl_cache
        .load_local(&fetched.package_dir.to_string_lossy())
        .expect("CslCache::load_local failed to load the fetched package");
    eprintln!("[e2e] CslCache::load_local returned Ok");

    let after = unsafe { xpmp2_sys::XPMPGetNumberOfInstalledModels() };
    eprintln!("[e2e] after = {after}");

    assert!(
        after > before,
        "expected XPMPGetNumberOfInstalledModels() to increase once the fetched package was \
         loaded (before={before}, after={after}) — a stuck-at-zero count here is exactly the \
         ERR_PKG_UNKNOWN regression this test guards against: the model's files can download \
         fine while still never registering with XPMP2 if the `_blobs` package reference can't \
         resolve"
    );

    drop(multiplayer);
    drop(guard);
}
