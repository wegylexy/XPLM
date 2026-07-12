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

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result};
use csl_client::ModelClient;
use xplm::processing::{FlightLoop, FlightLoopPhase};

use crate::Multiplayer;

/// A fetched-and-loaded CSL package, ready for [`crate::Plane::new`].
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

/// One pending [`CslCache::request`] — sent to the background worker thread.
struct Job {
    icao: Option<String>,
    airline: Option<String>,
    livery: Option<String>,
    seed: Option<u8>,
    callback: Box<dyn FnOnce(Result<FetchedPackage>) + Send>,
}

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
pub struct CslCache {
    jobs_tx: mpsc::Sender<Job>,
    out_dir: PathBuf,
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
    /// across sim restarts. `_multiplayer` is only proof a [`Multiplayer`]
    /// is live; it isn't stored (nothing here touches XPMP2 until a
    /// `request`'s fetch actually completes).
    pub fn new(
        _multiplayer: &Multiplayer,
        base_url: impl Into<String>,
        out_dir: impl Into<PathBuf>,
    ) -> Self {
        let out_dir = out_dir.into();
        let (jobs_tx, jobs_rx) = mpsc::channel::<Job>();
        let (completed_tx, completed_rx) = mpsc::channel::<Box<dyn FnOnce() + Send>>();

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
                // Both the CSL-id recovery (reads the just-written
                // xsb_aircraft.txt off disk) and the eventual
                // XPMPLoadCSLPackage call are safe to do off the main
                // thread — file I/O isn't main-thread-bound, only the
                // XPMP2 call itself is, which is why it's deferred into the
                // thunk below rather than done here.
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
                    });
                // The XPMPLoadCSLPackage call happens inside the thunk, run
                // by the flight loop below, not here — it's only safe from
                // the X-Plane main thread, and this closure runs on the
                // background worker thread.
                let callback = job.callback;
                let thunk: Box<dyn FnOnce() + Send> = Box::new(move || {
                    let result = result.and_then(|fetched| {
                        crate::load_csl_package_raw(&fetched.package_dir.to_string_lossy())
                            .map_err(|message| anyhow::anyhow!(message))?;
                        Ok(fetched)
                    });
                    callback(result);
                });
                if completed_tx.send(thunk).is_err() {
                    break; // CslCache was dropped; nothing left to deliver to.
                }
            }
        });

        let poll_loop = FlightLoop::new(FlightLoopPhase::AfterFlightModel, move |_, _, _| {
            for thunk in completed_rx.try_iter() {
                thunk();
            }
            -1.0 // check again next frame — completions should surface promptly.
        });
        poll_loop.schedule(-1.0, true);

        Self {
            jobs_tx,
            out_dir,
            _poll_loop: poll_loop,
        }
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
    pub fn request(
        &self,
        icao: Option<&str>,
        airline: Option<&str>,
        livery: Option<&str>,
        seed: Option<u8>,
        callback: impl FnOnce(Result<FetchedPackage>) + Send + 'static,
    ) {
        // An error here means the worker thread already exited (e.g. it
        // failed to build its Tokio runtime) — the callback is simply never
        // called, same as `Object::load_async` returning `false` and never
        // invoking its callback on a bad path.
        let _ = self.jobs_tx.send(Job {
            icao: icao.map(str::to_string),
            airline: airline.map(str::to_string),
            livery: livery.map(str::to_string),
            seed,
            callback: Box::new(callback),
        });
    }

    /// Directory this cache fetches/reconstructs files into (as passed to
    /// [`CslCache::new`]).
    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }
}
