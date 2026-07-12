//! Acceptance test for `xpmp2`'s on-demand CSL flow, end to end: some other
//! part of a real plugin (a network feed, an ADS-B parser, whatever detects
//! new traffic) reports a newly-spotted aircraft over a plain `mpsc`
//! channel; a flight loop drains that channel and kicks off a
//! `CslCache::request` per aircraft; once that request's callback reports
//! the model fetched and loaded, the same flight loop creates a `Plane`
//! from it. Two independent hops (spotted → requested, fetched → placed),
//! both driven by polling a channel from a flight loop rather than by
//! blocking anywhere on the main thread.
//!
//! This example stands in for the "some other part of a real plugin" piece
//! by sending one demo `Spotted` event immediately in `start()` — a real
//! plugin would instead have its traffic source hold onto `spotted_tx`
//! (or a clone of it) and send into it whenever it detects new traffic.
//!
//! See `Xpmp2Template::start`'s `TODO`s: the resource directory and
//! `csl-on-demand` server URL below are placeholders, not real paths/hosts.

use std::ffi::c_void;
use std::sync::mpsc;

use xplm::plugin::{XPLMPluginID, XPlanePlugin};
use xplm::processing::{FlightLoop, FlightLoopPhase};
use xpmp2::csl_on_demand::{CslCache, FetchedPackage};
use xpmp2::{Aircraft, Multiplayer, Plane, PlaneHandle};

/// A single stationary demo aircraft — real position/attitude would come
/// from wherever a real plugin's own traffic model lives (network, replay,
/// etc.); this just proves `update_position` round-trips through the shim.
struct DemoAircraft {
    lat: f64,
    lon: f64,
    alt_ft: f64,
}

impl Aircraft for DemoAircraft {
    fn update_position(
        &mut self,
        plane: &PlaneHandle,
        _elapsed_since_last_call: f32,
        _fl_counter: i32,
    ) {
        plane.set_location(self.lat, self.lon, self.alt_ft);
        plane.set_heading(0.0);
        plane.set_on_ground(false);
    }
}

/// A newly-detected piece of traffic, exactly as a real plugin's traffic
/// source (network feed, ADS-B parser, ...) would report one — enough to
/// both request a CSL match and, once it arrives, create the `Plane`.
struct Spotted {
    icao_type: String,
    icao_airline: String,
    livery: String,
    mode_s_id: u32,
}

#[xplm::plugin(
    name = "Fly by Wireless XPMP2 Template",
    signature = "tw.timmywong.flybywireless.xpmp2template",
    description = "xpmp2 on-demand CSL fetching example."
)]
struct Xpmp2Template {
    // `Multiplayer`, `CslCache`, and every `Plane` created from them live
    // entirely inside this flight loop's captured closure, not as separate
    // `Self` fields — sharing them via `Rc<RefCell<_>>` instead wouldn't
    // compile: `Rc<T>: Send` requires `T: Sync`, and `Multiplayer`/`Plane`
    // are deliberately `!Sync` (see their doc comments), so an `Rc` around
    // either could never make this plugin's state `Send`, which
    // `register_plugin!`'s static storage requires. Owning everything from
    // this one closure sidesteps the question entirely; nothing outside
    // this plugin needs to reach any of it.
    _traffic_loop: FlightLoop,
    // A real plugin's traffic source would hold (a clone of) this and send
    // into it whenever new traffic is detected; this example just sends one
    // demo event in `start()` to exercise the same path deterministically,
    // so nothing else in this example reads it back out again — kept as a
    // field only to show where a real traffic source would plug in.
    #[allow(dead_code)]
    spotted_tx: mpsc::Sender<Spotted>,
}

impl XPlanePlugin for Xpmp2Template {
    fn start() -> Self {
        // TODO: point at this plugin's own bundled copy of XPMP2's
        // Resources folder (Doc8643.txt, MapIcons.png, related.txt) — see
        // external/XPMP2/Resources for what needs to ship alongside a real
        // plugin binary.
        let multiplayer = Multiplayer::init(
            "Fly by Wireless XPMP2 Template",
            "./Resources",
            Some("A320"),
            None,
        )
        .expect("XPMPMultiplayerInit failed");

        // TODO: point at a real csl-on-demand server.
        let csl_cache = CslCache::new(&multiplayer, "https://csl.example.com", "./CSLCache");

        let (spotted_tx, spotted_rx) = mpsc::channel::<Spotted>();
        // `CslCache::request`'s callback must be `Send` (it crosses to a
        // background worker thread and back — see csl_on_demand's module
        // doc), but `Plane`/`Spotted` aren't. So the callback only ever
        // forwards a plain `Send`-safe result through this channel; the
        // flight loop below (no `Send` bound on its own closure) is what
        // actually turns a fetched package into a `Plane`.
        let (fetched_tx, fetched_rx) = mpsc::channel::<(Spotted, Result<FetchedPackage, String>)>();

        let mut planes: Vec<Plane> = Vec::new();
        let _traffic_loop = FlightLoop::new(FlightLoopPhase::AfterFlightModel, move |_, _, _| {
            // Hop 1: newly-spotted traffic → kick off a fetch for it.
            while let Ok(spotted) = spotted_rx.try_recv() {
                let fetched_tx = fetched_tx.clone();
                let (icao_type, icao_airline, livery) = (
                    spotted.icao_type.clone(),
                    spotted.icao_airline.clone(),
                    spotted.livery.clone(),
                );
                csl_cache.request(
                    Some(&icao_type),
                    Some(&icao_airline),
                    Some(&livery),
                    None,
                    move |result| {
                        let _ = fetched_tx.send((spotted, result.map_err(|err| err.to_string())));
                    },
                );
            }

            // Hop 2: a fetch completed → place the plane.
            while let Ok((spotted, result)) = fetched_rx.try_recv() {
                match result {
                    Ok(fetched) => {
                        xplm::log(&format!(
                            "xpmp2-template: loaded CSL package {} at {}\n",
                            fetched.csl_id,
                            fetched.package_dir.display()
                        ));
                        let aircraft = DemoAircraft {
                            lat: 0.0,
                            lon: 0.0,
                            alt_ft: 5000.0,
                        };
                        // Passing `fetched.csl_id` (rather than `""`) makes
                        // XPMP2 assign this exact, already server-matched
                        // model directly — see `FetchedPackage::csl_id`'s
                        // doc comment for why on-demand mode needs this
                        // instead of falling back to XPMP2's own local
                        // Doc8643/ICAO-based matching.
                        match Plane::new(
                            &multiplayer,
                            &spotted.icao_type,
                            &spotted.icao_airline,
                            &spotted.livery,
                            spotted.mode_s_id,
                            &fetched.csl_id,
                            aircraft,
                        ) {
                            Some(plane) => planes.push(plane),
                            None => xplm::log("xpmp2-template: XPMP2 rejected the plane\n"),
                        }
                    }
                    Err(err) => xplm::log(&format!("xpmp2-template: CSL fetch failed: {err}\n")),
                }
            }

            -1.0 // check again next frame
        });
        _traffic_loop.schedule(-1.0, true);

        // Demo stand-in for a real traffic source reporting new traffic —
        // see the struct doc.
        let _ = spotted_tx.send(Spotted {
            icao_type: "A320".to_string(),
            icao_airline: String::new(),
            livery: String::new(),
            mode_s_id: 0,
        });

        Self {
            _traffic_loop,
            spotted_tx,
        }
    }

    fn enable(&mut self) -> bool {
        true
    }

    fn disable(&mut self) {}

    fn receive_message(&mut self, from_plugin_id: XPLMPluginID, message: i32, param: *mut c_void) {
        let _ = (from_plugin_id, message, param);
    }
}
