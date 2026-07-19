//! CLI front-end for `flybywireless_xplm_reloader::reload_plugins` — invoked from
//! `examples/hot-reload-xpl/scripts/install-loader.{ps1,sh}` so a fresh
//! install can tell an already-running X-Plane to reload plugins over UDP
//! instead of requiring the user to restart it.
//!
//! Usage: `reload-plugins [timeout_secs]` (default 5). Exits non-zero (with
//! a message on stderr) if no X-Plane beacon is seen in time, or the send
//! fails — callers should treat that as "reload didn't happen, restart
//! X-Plane manually" rather than a hard error.

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
