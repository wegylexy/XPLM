# flybywireless-xplm-reloader

Standalone UDP client for discovering a running X-Plane instance and sending
it SDK commands — no dependency on the X-Plane SDK or `xplm-sys`. Intended
for tooling that runs *outside* a plugin process (build scripts, hot-reload
drivers, CI) and needs to make an already-running X-Plane pick up a
freshly-installed plugin without a manual restart.

```rust
use std::time::Duration;

// Discovers X-Plane via its UDP beacon and sends sim/operation/reload_plugins.
flybywireless_xplm_reloader::reload_plugins(Duration::from_secs(5)).await?;
```

Lower-level `discover`/`send_command` are also available if a caller needs
to pick among multiple X-Plane instances on the network, or send a different
command.

Also ships a `reload-plugins` CLI binary (`cargo run -p flybywireless-xplm-reloader --bin reload-plugins -- [timeout_secs]`)
for use from non-Rust tooling, e.g. `examples/hot-reload-xpl`'s install script.
