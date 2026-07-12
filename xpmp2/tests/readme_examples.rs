//! Type-checks the root README's "Multiplayer (XPMP2)" snippets so they
//! can't silently drift out of sync with the actual API. These functions
//! are never called — calling into real XPMP2/XPLM functions outside a
//! hosted X-Plane process crashes, the same limitation noted throughout
//! this workspace — `cargo test` type-checking this file without running it
//! is exactly what's needed here.

#![allow(dead_code)]

use xpmp2::{Aircraft, Multiplayer, Plane, PlaneHandle};

struct MyPlane {
    lat: f64,
    lon: f64,
    alt_ft: f64,
}

impl Aircraft for MyPlane {
    fn update_position(
        &mut self,
        plane: &PlaneHandle,
        _elapsed_since_last_call: f32,
        _fl_counter: i32,
    ) {
        plane.set_location(self.lat, self.lon, self.alt_ft);
    }
}

fn _readme_multiplayer_init() -> (Multiplayer, Option<Plane>) {
    let multiplayer = Multiplayer::init("My Plugin", "./Resources", Some("A320"), None)
        .expect("XPMPMultiplayerInit failed");

    let plane = Plane::new(
        &multiplayer,
        "A320",
        "",
        "",
        0,
        "",
        MyPlane {
            lat: 0.0,
            lon: 0.0,
            alt_ft: 5000.0,
        },
    );

    (multiplayer, plane)
}

// `CslCache::request`'s callback must be `Send`, but `Multiplayer`/`Plane`
// are deliberately `!Send`/`!Sync` — so the callback below only ever
// forwards a plain `Send`-safe result through a channel, exactly like
// `examples/xpmp2-template` does; a separate flight loop, which alone owns
// `multiplayer`, is what actually calls `Plane::new`. See that example for
// the full, runnable version of this pattern.
#[cfg(feature = "csl-on-demand")]
fn _readme_csl_on_demand(multiplayer: Multiplayer) {
    use std::sync::mpsc;
    use xpmp2::csl_on_demand::{CslCache, FetchedPackage};

    let csl_cache = CslCache::new(&multiplayer, "https://csl.example.com", "./CSLCache");

    let (fetched_tx, fetched_rx) = mpsc::channel::<Result<FetchedPackage, String>>();
    csl_cache.request(Some("A320"), None, None, None, move |result| {
        let _ = fetched_tx.send(result.map_err(|err| err.to_string()));
    });

    // Elsewhere, on the main thread (e.g. a flight loop callback):
    if let Ok(result) = fetched_rx.try_recv() {
        match result {
            Ok(fetched) => {
                let plane = Plane::new(
                    &multiplayer,
                    "A320",
                    "",
                    "",
                    0,
                    &fetched.csl_id,
                    MyPlane {
                        lat: 0.0,
                        lon: 0.0,
                        alt_ft: 5000.0,
                    },
                );
                let _ = plane;
            }
            Err(err) => xplm::log(&format!("CSL fetch failed: {err}\n")),
        }
    }
}
