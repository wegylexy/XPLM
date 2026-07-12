//! Rust port of the original C# template plugin (`XPL/Program.cs`, from
//! before this crate's Rust rewrite — recovered from git history) — the
//! acceptance test that this crate's ergonomics hold up against the C#
//! original, not just against the SDK's raw C API. Same behavior: override
//! TCAS while enabled, and (on a flight loop stub) write bearing/distance/
//! altitude datarefs for a fixed number of intruder slots.

use std::ffi::c_void;

use xplm::dataref::{ReadWrite, ReadWriteArray};
use xplm::plugin::{XPLMPluginID, XPlanePlugin};
use xplm::processing::{FlightLoop, FlightLoopPhase};
use xplm::utilities::versions;

/// How many TCAS intruder slots this example writes into — a placeholder
/// for wherever a real plugin's own traffic model would come from.
const INTRUDER_COUNT: usize = 2;

#[xplm::plugin(
    name = "Fly by Wireless",
    signature = "tw.timmywong.flybywireless",
    description = "X-Plane plugin library template."
)]
struct XplTemplate {
    override_tcas: ReadWrite<i32>,
    // `_flight_loop` is never read again after `start()` — it's kept alive
    // so `Drop` unregisters it when the plugin unloads, matching the C#
    // original's explicit `_myLoop.Dispose()` in `Dispose(bool)`.
    _flight_loop: FlightLoop,
}

impl XPlanePlugin for XplTemplate {
    fn start() -> Self {
        // Example: check for API support (the C# original throws here;
        // `start()` has no failure signal of its own, so panicking is the
        // equivalent — `guard()` catches it, logs it, and the plugin simply
        // ends up with no live instance).
        let (_xplane_version, xplm_version, _host_id) = versions();
        assert!(xplm_version >= 303, "TCAS override not supported.");

        // Example: finds datarefs.
        let override_tcas = ReadWrite::<i32>::find("sim/operation/override/override_TCAS")
            .expect("sim/operation/override/override_TCAS not found");
        let bearing =
            ReadWriteArray::<f32>::find("sim/cockpit2/tcas/indicators/relative_bearing_degs")
                .expect("relative_bearing_degs not found");
        let distance =
            ReadWriteArray::<f32>::find("sim/cockpit2/tcas/indicators/relative_distance_mtrs")
                .expect("relative_distance_mtrs not found");
        let altitude =
            ReadWriteArray::<f32>::find("sim/cockpit2/tcas/indicators/relative_altitude_mtrs")
                .expect("relative_altitude_mtrs not found");

        // Example: registers my flight loop.
        let _flight_loop = FlightLoop::new(FlightLoopPhase::AfterFlightModel, move |_, _, _| {
            // TODO: fill in real bearings/distances/altitudes per intruder.
            let values = [0.0f32; INTRUDER_COUNT];
            bearing.set(0, &values);
            distance.set(0, &values);
            altitude.set(0, &values);
            // Schedules for one second later.
            1.0
        });

        Self {
            override_tcas,
            _flight_loop,
        }
    }

    fn enable(&mut self) -> bool {
        // Example: overrides TCAS.
        self.override_tcas.set(1);

        // Example: starts my flight loop one cycle after registration, i.e.
        // immediately.
        self._flight_loop.schedule(-1.0, false);

        true
    }

    fn disable(&mut self) {
        // Example: stops my flight loop.
        self._flight_loop.schedule(0.0, false);

        // Example: clears TCAS override.
        self.override_tcas.set(0);
    }

    fn receive_message(&mut self, from_plugin_id: XPLMPluginID, message: i32, param: *mut c_void) {
        // TODO: handle message from another plugin.
        let _ = (from_plugin_id, message, param);
    }
}
