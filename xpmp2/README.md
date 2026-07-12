# flybywireless-xpmp2

Safe wrappers over [`flybywireless-xpmp2-sys`](https://crates.io/crates/flybywireless-xpmp2-sys):
`Multiplayer` (RAII init/cleanup), an `Aircraft` trait + RAII `Plane`/
`PlaneHandle`, and (behind the `csl-on-demand` feature) `CslCache` for
fetching CSL packages on demand.

Published on crates.io as `flybywireless-xpmp2`. See the
[workspace README](https://github.com/wegylexy/XPLM)'s "Multiplayer
(XPMP2)" section for usage and the `csl-offline`/`csl-on-demand` feature
split.
