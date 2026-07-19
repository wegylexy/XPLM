# flybywireless-xplm-reloader

Standalone UDP client for discovering a running X-Plane instance and sending
it SDK commands — no dependency on the X-Plane SDK or `xplm-sys`. Intended
for tooling that runs *outside* a plugin process (build scripts, hot-reload
drivers, CI) and needs to make an already-running X-Plane reload plugins it
already knows about — e.g. after replacing an already-loaded plugin's file.

**Not** a way to make X-Plane pick up a brand-new plugin without a restart:
`sim/operation/reload_plugins` only re-loads plugins X-Plane already found in
its startup scan of `Resources/plugins`; it does not repeat that scan, so a
plugin folder that didn't exist at boot stays invisible until X-Plane itself
is restarted. Confirmed in practice via `examples/hot-reload-xpl`'s install
scripts, which no longer attempt this for first-time installs.

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
