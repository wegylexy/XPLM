//! Hot-reload demo payload: tunes COM1 to a hard-coded frequency once.
//!
//! This is a perfectly ordinary plugin crate — nothing here is aware of
//! hot-reloading. It's `examples/hot-reload-xpl` (the loader) that makes this
//! swappable at runtime; this crate just needs to be a plain `cdylib`.
//!
//! Edit `COM1_FREQ_833` below, then rebuild and let the loader pick it up —
//! see `examples/hot-reload-xpl`'s README section for the F5 dev loop.

use xplm::dataref::ReadWrite;
use xplm::plugin::XPlanePlugin;
use xplm::processing::{FlightLoop, FlightLoopPhase};

/// The value a dev edits between reloads. 122_800 = 122.800 MHz encoded per
/// `com1_frequency_hz_833`'s 8.33 kHz channel-spacing convention (value is
/// the frequency in Hz, sans trailing zero, e.g. 118.000 MHz -> 11800).
const COM1_FREQ_833: i32 = 122_800;

#[xplm::plugin(
    name = "Hot Reload Demo Payload",
    signature = "org.xplm-rust.hot-reload-dll",
    description = "Tunes COM1 to a hard-coded frequency once, to demonstrate hot-reloading."
)]
struct HotReloadPayload {
    // Kept alive only so `Drop` unregisters the flight loop on `XPluginStop`;
    // the callback unschedules itself after one run (see `start()`).
    _tune_once: FlightLoop,
}

impl XPlanePlugin for HotReloadPayload {
    fn start() -> Self {
        xplm::log("hot-reload-dll: XPluginStart\n");

        // Deferred to the first flight loop tick rather than done here
        // directly: some datarefs aren't guaranteed valid yet at
        // XPluginStart time, so waiting one tick is the realistic pattern,
        // not just a demo contrivance.
        let tune_once = FlightLoop::new(FlightLoopPhase::AfterFlightModel, |_, _, _| {
            match ReadWrite::<i32>::find("sim/cockpit2/radios/actuators/com1_frequency_hz_833") {
                Some(com1_freq) => {
                    xplm::log(&format!(
                        "hot-reload-dll: com1_frequency_hz_833 was {}, tuning to {COM1_FREQ_833}\n",
                        com1_freq.get()
                    ));
                    com1_freq.set(COM1_FREQ_833);
                }
                None => xplm::log("hot-reload-dll: com1_frequency_hz_833 dataref not found\n"),
            }
            0.0 // one-shot: don't reschedule
        });
        tune_once.schedule(-1.0, true); // fire on the very next flight loop cycle

        Self {
            _tune_once: tune_once,
        }
    }

    fn enable(&mut self) -> bool {
        xplm::log("hot-reload-dll: XPluginEnable\n");
        true
    }

    fn disable(&mut self) {
        xplm::log("hot-reload-dll: XPluginDisable\n");
    }

    fn stop(self) {
        xplm::log("hot-reload-dll: XPluginStop\n");
    }
}
