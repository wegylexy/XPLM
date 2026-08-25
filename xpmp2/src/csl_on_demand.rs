//! On-demand CSL package fetching, feature-gated behind `csl-on-demand` — a
//! thin wrapper over the published
//! [`flybywireless-csl-client`](https://crates.io/crates/flybywireless-csl-client)
//! crate (`csl_client`), which speaks `csl-on-demand`'s real `/match` +
//! `/manifest/{root}/{id}` HTTP protocol. This crate doesn't reimplement
//! that client: it already exists, is maintained alongside the server it
//! talks to, and has nothing XPLM/XPMP2-specific in it worth forking.
//!
//! This module doesn't route through [`Multiplayer::load_csl_package`](crate::Multiplayer::load_csl_package)
//! at all — that method exists for a plugin that bulk-loads a local CSL
//! library up front (gated behind the separate `csl-offline` feature), a
//! different usage pattern from on-demand's "fetch one model, load just
//! that model's package, the instant it's needed." [`CslCache::request`]
//! loads the fetched package into XPMP2 itself, so on-demand loading works
//! whether or not `csl-offline` is even enabled.
//!
//! # Non-blocking, callback-based — like `Object::load_async`
//!
//! [`CslCache::request`] is modeled on
//! [`xplm::scenery::Object::load_async`]'s shape: fire it, get on with your
//! frame, and a callback runs later once the model is ready — never a
//! blocking call, never something you `.await`. The difference is *why* a
//! callback is needed at all: `XPLMLoadObjectAsync` has X-Plane itself doing
//! the async work and calling back on the main thread when it's done, so
//! `load_async`'s trampoline just forwards that guarantee. XPLM has no
//! equivalent primitive for an arbitrary network fetch, so `CslCache` builds
//! the same guarantee itself — a background worker thread does the actual
//! fetching (and, once done, calls `XPMPLoadCSLPackage`... except that call
//! is only safe from the main thread, so instead the *result* crosses back
//! to an internal flight loop this cache registers, which does the
//! `XPMPLoadCSLPackage` call and then invokes your callback, both
//! guaranteed on the main thread). From the caller's side, though, the
//! contract reads identically to `load_async`: pass a callback, get called
//! back on the main thread with a ready-to-use result.
//!
//! No separate `async fn`/`Future`-based entry point is offered alongside
//! the callback one — `xplm` deliberately has no `async fn` callbacks
//! anywhere else either (see `xplm::aircraft`'s doc comment), and a second
//! API surface here would just be two ways to do the same thing for no
//! caller who isn't already committed to an async runtime of their own.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use anyhow::{Context, Result};
use csl_client::ModelClient;
use xplm::processing::{FlightLoop, FlightLoopPhase};

/// The smallest possible valid PNG: a 1x1 transparent pixel. Good enough for
/// [`write_stub_resource_dir`]'s `MapIcons.png` — XPMP2 itself never decodes
/// this file (see that function's doc), it only ever gets handed to
/// X-Plane's own `XPLMDrawMapIconFromSheet` if a plugin turns on in-sim map
/// icon drawing, which a `csl-on-demand`-only plugin has no reason to do.
const STUB_PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, // 8-bit RGBA
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, // IDAT chunk
    0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4,
    0x00, // IDAT CRC
    0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, // IEND chunk
    0x42, 0x60, 0x82,
];

/// Writes minimal stub `related.txt`/`Doc8643.txt`/`MapIcons.png` into `dir`
/// for a plugin that only ever uses `csl-on-demand` (never `csl-offline`) —
/// `Multiplayer::init`'s `resource_dir` requires these three files to
/// *exist* regardless of which CSL-loading feature is enabled
/// (`XPMPValidateResourceFiles` in XPMP2's `XPMPMultiplayer.cpp` runs
/// unconditionally before either loading strategy is reachable — see
/// `../README.md`'s CSL-loading section), but on-demand mode never actually
/// *uses* their content: it always supplies an exact `csl_id`
/// (`FetchedPackage::csl_id`) rather than falling back to XPMP2's local
/// `ChangeModel`/`CSLModelMatching`, which is the only thing that reads
/// `Doc8643.txt`/`related.txt` for real. So a genuinely empty
/// `related.txt`/`Doc8643.txt` (confirmed against `RelatedLoad`/
/// `Doc8643Load` in `RelatedDoc8643.cpp`: both just open the file and loop
/// over zero lines, returning success) and a trivial 1x1 `MapIcons.png`
/// (confirmed XPMP2 itself never decodes this file, only checks it exists —
/// see `STUB_PNG_1X1`'s doc) are enough. This spares a `csl-on-demand`-only
/// plugin from having to vendor XPMP2's real `Resources/` folder at all.
///
/// Call this **before** `Multiplayer::init` (which is what actually
/// validates `dir`), not after — unlike [`CslCache::new`]'s analogous
/// `_blobs`-package registration, there's no already-initialized
/// `Multiplayer` to require here, precisely because this has to run earlier
/// than that. Idempotent: skips any file that's already present, so it's
/// safe to call every plugin start without ever overwriting a real
/// `Resources/` folder a plugin author later drops in on purpose (e.g. to
/// pick up real wake-turbulence categorization for on-demand-matched
/// aircraft, which is otherwise the one thing this stub gives up).
pub fn write_stub_resource_dir(dir: impl AsRef<Path>) -> std::io::Result<()> {
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;
    for (name, content) in [
        ("related.txt", &b""[..]),
        ("Doc8643.txt", &b""[..]),
        ("MapIcons.png", STUB_PNG_1X1),
    ] {
        let path = dir.join(name);
        if !path.exists() {
            std::fs::write(&path, content)?;
        }
    }
    Ok(())
}

use crate::Multiplayer;

/// A fetched-and-loaded CSL package, ready for [`crate::Plane::new`].
#[derive(Clone)]
pub struct FetchedPackage {
    /// The materialized package directory (already handed to
    /// `XPMPLoadCSLPackage` by the time your callback runs).
    pub package_dir: PathBuf,
    /// This model's exact `"{root}/{id}"` XPMP2 CSL identifier — pass it as
    /// [`crate::Plane::new`]'s `csl_id` argument. Doing so makes XPMP2
    /// assign this exact model directly (`Aircraft::AssignModel`, an exact
    /// lookup by this id) instead of re-running its own local ICAO/livery
    /// matching (`ChangeModel`/`CSLModelMatching`), which needs
    /// `Doc8643.txt`/`related.txt` in `Multiplayer::init`'s `resource_dir`
    /// to work well — on-demand mode already got its match server-side (via
    /// `csl-on-demand`'s `/match` endpoint) and doesn't need those files at
    /// all. Passing `""` instead here would silently fall back to that
    /// local matching, which is not what on-demand mode is for.
    pub csl_id: String,
}

/// De-dup key for [`CslCache::request`] — identical `icao`/`airline`/
/// `livery`/`seed` are treated as the same model.
type RequestKey = (Option<String>, Option<String>, Option<String>, Option<u8>);

/// One pending [`CslCache::request`] — sent to the background worker thread.
struct Job {
    key: RequestKey,
    icao: Option<String>,
    airline: Option<String>,
    livery: Option<String>,
    seed: Option<u8>,
}

type Waiters = Arc<Mutex<HashMap<RequestKey, Vec<Box<dyn FnOnce(Result<FetchedPackage>) + Send>>>>>;

/// Recovers this package's exact CSL id from its freshly-materialized
/// `xsb_aircraft.txt` — `flybywireless-csl-client`'s `FetchedModel` doesn't
/// expose the matched aircraft's short id directly (only the package
/// directory), but a package fetched via `/match` always contains exactly
/// one `OBJ8_AIRCRAFT` entry (the one matched model — see `csl-on-demand`'s
/// `Obj8Aircraft::pack()`), so reading that one line back out is reliable.
fn read_csl_id(package_dir: &Path) -> Result<String> {
    let root = package_dir
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("{} has no usable file name", package_dir.display()))?;
    let xsb_path = package_dir.join("xsb_aircraft.txt");
    let content = std::fs::read_to_string(&xsb_path)
        .with_context(|| format!("reading {}", xsb_path.display()))?;
    let id = content
        .lines()
        .find_map(|line| line.strip_prefix("OBJ8_AIRCRAFT ").map(str::trim))
        .with_context(|| format!("no OBJ8_AIRCRAFT line in {}", xsb_path.display()))?;
    Ok(format!("{root}/{id}"))
}

/// Non-blocking, callback-based CSL fetching. Construct once per base URL /
/// cache directory and reuse across requests, same as the underlying
/// `ModelClient` recommends, so concurrent requests for liveries of the
/// same model share in-flight downloads.
///
/// # Hybrid local + on-demand
///
/// `csl-on-demand` is the one feature a hybrid plugin needs — it does not
/// also require enabling `csl-offline`. Load whatever real local CSL
/// libraries the user has installed with [`CslCache::load_local`] (same
/// underlying `XPMPLoadCSLPackage` call `Multiplayer::load_csl_package`
/// makes behind `csl-offline`, just reachable without that feature), and use
/// [`CslCache::request`]/[`ModelClient::probe`](csl_client::ModelClient::probe)
/// for everything on-demand. XPMP2 merges every loaded package into one
/// catalog and matches across all of it regardless of source, so this is
/// enough by itself to get local-preferred, on-demand-fallback behavior:
/// probe this cache's server for a candidate's quality, compare it against
/// whatever quality a local-only `ChangeModel`/`CSLModelMatching` pass
/// already found, and only actually `request` (i.e. fetch over the network)
/// when the server's candidate is strictly better. See `csl-on-demand`'s own
/// `CLAUDE.md` ("Hybrid local+on-demand matching") for the full design.
pub struct CslCache {
    jobs_tx: mpsc::Sender<Job>,
    out_dir: PathBuf,
    // Requests currently in flight (fetch started, result not yet
    // delivered), keyed by icao/airline/livery/seed. `request` consults this
    // before sending a new `Job` — a second call for a key already present
    // just appends its callback to the waiter list instead of kicking off a
    // duplicate fetch. `Arc<Mutex<_>>` (not `Rc<RefCell<_>>`) purely to keep
    // `CslCache` itself `Send`, matching every other XPLM handle stored in a
    // `static Mutex<Option<_>>` plugin-state slot — actual access is still
    // only ever from the main thread, so there's no real contention.
    in_flight: Waiters,
    // Keeps the completion-draining flight loop (see `new`) registered for
    // as long as this `CslCache` is alive; unregisters (and stops
    // delivering callbacks) on `Drop`, same as every other XPLM RAII handle
    // in this workspace. The background worker thread exits on its own once
    // `jobs_tx` (dropped alongside this field) closes its channel.
    _poll_loop: FlightLoop,
}

impl CslCache {
    /// `base_url` is the `csl-on-demand` server's root (e.g.
    /// `https://csl.example.com`); `out_dir` is a local directory this cache
    /// is free to fill with fetched/reconstructed CSL files — content-
    /// addressed, so it's safe to share across plugins/models and to persist
    /// across sim restarts. `blobs_pkg` must match that server's own
    /// `CSL_BLOBS_PACKAGE` (default `"_blobs"`) — every reconstructed `.obj`
    /// and every generated `xsb_aircraft.txt` OBJ8 line this cache ever
    /// writes embeds a `{blobs_pkg}/...` reference, and XPMP2 fails
    /// `ERR_PKG_UNKNOWN` on any such reference until that exact
    /// `EXPORT_NAME` has been registered — which is what this constructor
    /// does immediately, before any fetch can run, by writing a one-line
    /// `xsb_aircraft.txt` for `{out_dir}/{blobs_pkg}` and loading it like any
    /// other package. See `csl-on-demand`'s `CLAUDE.md` ("`_blobs`
    /// references must be `EXPORT_NAME`-relative") for why this has to exist
    /// at all. `_multiplayer` is only proof a [`Multiplayer`] is live; it
    /// isn't stored (nothing else here touches XPMP2 until a `request`'s
    /// fetch actually completes).
    pub fn new(
        _multiplayer: &Multiplayer,
        base_url: impl Into<String>,
        out_dir: impl Into<PathBuf>,
        blobs_pkg: impl Into<String>,
    ) -> Result<Self, String> {
        let out_dir = out_dir.into();
        let blobs_pkg = blobs_pkg.into();

        let blobs_dir = out_dir.join(&blobs_pkg);
        std::fs::create_dir_all(&blobs_dir)
            .map_err(|e| format!("creating {}: {e}", blobs_dir.display()))?;
        let xsb_path = blobs_dir.join("xsb_aircraft.txt");
        // Idempotent and content-stable (the whole point of `blobs_pkg`
        // being a fixed name), so only write it the first time — every
        // later `new` on the same `out_dir` just re-registers the package
        // XPMP2-side without touching the file.
        if !xsb_path.exists() {
            std::fs::write(&xsb_path, format!("EXPORT_NAME {blobs_pkg}\n"))
                .map_err(|e| format!("writing {}: {e}", xsb_path.display()))?;
        }
        crate::load_csl_package_raw(&blobs_dir.to_string_lossy())?;

        let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
        // Worker thread reports the raw fetch result (not yet loaded into
        // XPMP2, and not yet dispatched to any waiter) keyed by request —
        // the flight loop below does the main-thread-only load and the
        // fan-out to every waiting callback for that key.
        let (completed_tx, completed_rx) =
            mpsc::channel::<(RequestKey, Result<FetchedPackage, String>)>();

        let worker_base_url = base_url.into();
        let worker_out_dir = out_dir.clone();
        thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return; // Nothing to recover into; pending/future jobs just never complete.
            };
            let client = ModelClient::new(reqwest::Client::new(), worker_base_url, worker_out_dir);
            for job in jobs_rx {
                // The CSL-id recovery (reads the just-written
                // xsb_aircraft.txt off disk) is safe to do off the main
                // thread — file I/O isn't main-thread-bound. The eventual
                // XPMPLoadCSLPackage call is, so it's deferred to the flight
                // loop below rather than done here.
                let result = runtime
                    .block_on(client.request(
                        job.icao.as_deref(),
                        job.airline.as_deref(),
                        job.livery.as_deref(),
                        job.seed,
                    ))
                    .and_then(|model| {
                        let csl_id = read_csl_id(&model.package_dir)?;
                        Ok(FetchedPackage {
                            package_dir: model.package_dir,
                            csl_id,
                        })
                    })
                    .map_err(|err| err.to_string());
                if completed_tx.send((job.key, result)).is_err() {
                    break; // CslCache was dropped; nothing left to deliver to.
                }
            }
        });

        let in_flight: Waiters = Arc::new(Mutex::new(HashMap::new()));
        let in_flight_for_loop = Arc::clone(&in_flight);
        let poll_loop = FlightLoop::new(FlightLoopPhase::AfterFlightModel, move |_, _, _| {
            for (key, result) in completed_rx.try_iter() {
                let waiters = in_flight_for_loop
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .remove(&key)
                    .unwrap_or_default();
                if waiters.is_empty() {
                    continue;
                }
                let loaded = result.and_then(|fetched| {
                    crate::load_csl_package_raw(&fetched.package_dir.to_string_lossy())?;
                    Ok(fetched)
                });
                for callback in waiters {
                    callback(loaded.clone().map_err(|message| anyhow::anyhow!(message)));
                }
            }
            -1.0 // check again next frame — completions should surface promptly.
        });
        poll_loop.schedule(-1.0, true);

        Ok(Self {
            jobs_tx,
            out_dir,
            in_flight,
            _poll_loop: poll_loop,
        })
    }

    /// Requests the best-matching model for `icao`/`airline`/`livery`.
    /// Returns immediately — never blocks. Fetches whatever's missing and
    /// loads the resulting package into XPMP2 (`XPMPLoadCSLPackage`) on a
    /// background thread plus this cache's own flight loop, then calls
    /// `callback` on the X-Plane main thread with a [`FetchedPackage`]
    /// (already loaded — pass its `csl_id` straight to `Plane::new`) or the
    /// error that prevented that.
    ///
    /// Safe to call repeatedly for the same model — an already-cached,
    /// already-loaded package resolves with no network I/O beyond the
    /// `/match` lookup and a harmless repeat `XPMPLoadCSLPackage` call.
    ///
    /// Also safe to call again for the same `icao`/`airline`/`livery`/`seed`
    /// while an earlier call for it is still in flight: this de-dups by that
    /// key rather than firing a second fetch — every callback registered for
    /// the same key while a fetch is pending is delivered the same result
    /// once it completes.
    pub fn request(
        &self,
        icao: Option<&str>,
        airline: Option<&str>,
        livery: Option<&str>,
        seed: Option<u8>,
        callback: impl FnOnce(Result<FetchedPackage>) + Send + 'static,
    ) {
        let key = (
            icao.map(str::to_string),
            airline.map(str::to_string),
            livery.map(str::to_string),
            seed,
        );
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(waiters) = in_flight.get_mut(&key) {
            waiters.push(Box::new(callback));
            return;
        }
        in_flight.insert(key.clone(), vec![Box::new(callback)]);
        drop(in_flight);

        // An error here means the worker thread already exited (e.g. it
        // failed to build its Tokio runtime) — the callback is simply never
        // called, same as `Object::load_async` returning `false` and never
        // invoking its callback on a bad path. The now-stranded `in_flight`
        // entry is harmless: a later `request` for the same key would just
        // queue behind it, also never firing, which matches "nothing here
        // works anymore" already being true once the worker thread is gone.
        let _ = self.jobs_tx.send(Job {
            key,
            icao: icao.map(str::to_string),
            airline: airline.map(str::to_string),
            livery: livery.map(str::to_string),
            seed,
        });
    }

    /// Directory this cache fetches/reconstructs files into (as passed to
    /// [`CslCache::new`]).
    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    /// Loads a real, already-installed local CSL package directory —
    /// the hybrid entry point (see the struct doc's "Hybrid local +
    /// on-demand" section). Identical to `Multiplayer::load_csl_package`
    /// (behind the separate `csl-offline` feature): both are a thin call to
    /// the same `XPMPLoadCSLPackage`, exposed here too so a plugin that only
    /// wants `csl-on-demand` doesn't have to also enable `csl-offline` (and
    /// couldn't — the two features are mutually exclusive, see
    /// `../CLAUDE.md`'s "CSL package loading") purely to load the user's
    /// existing libraries alongside on-demand fetching.
    ///
    /// Call this for every local CSL folder the user has configured, once,
    /// before matching against them — safe to call more than once overall
    /// (each additional package's models are added to what's already
    /// loaded), and models from every source (local or fetched) join the
    /// same XPMP2 catalog, so `Plane::new`'s own local matching already sees
    /// all of it with no further wiring needed. Main-thread only, like every
    /// other `XPMPLoadCSLPackage` call in this crate.
    pub fn load_local(&self, csl_folder: &str) -> Result<(), String> {
        crate::load_csl_package_raw(csl_folder)
    }
}
