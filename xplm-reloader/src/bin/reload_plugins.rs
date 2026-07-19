//! CLI front-end for `flybywireless_xplm_reloader::reload_plugins`, for
//! telling an already-running X-Plane to reload plugins it already knows
//! about (e.g. after replacing an already-loaded plugin's file) without
//! restarting the sim. NOT usable to make X-Plane discover a brand-new
//! plugin — `examples/hot-reload-xpl/scripts/install-loader.{ps1,sh}` used to
//! call this for that case and no longer does, since it doesn't work: a
//! plugin folder that didn't exist at X-Plane's startup scan stays invisible
//! until an actual restart.
//!
//! Usage: `reload-plugins [timeout_secs]` (default 5). Exits non-zero (with
//! a message on stderr) if no X-Plane beacon is seen in time, or the send
//! fails.

use std::time::Duration;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let timeout_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    match flybywireless_xplm_reloader::reload_plugins(Duration::from_secs(timeout_secs)).await {
        Ok(()) => {
            println!("Sent sim/operation/reload_plugins to X-Plane.");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("reload-plugins: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
