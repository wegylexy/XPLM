//! Minimal example plugin proving Phases 2-4 end to end: plugin lifecycle
//! (this crate's `XPlanePlugin` impl + `register_plugin!`), the panic-guarded
//! trampoline pattern, and a `FlightLoop` RAII wrapper.
//!
//! Build with `cargo build -p hello-plugin`, then load the resulting DLL as
//! an `.xpl` in a running X-Plane to verify manually (see PHASES.md Phase 4).

use xplm::plugin::XPlanePlugin;
use xplm::processing::{FlightLoop, FlightLoopPhase};

struct HelloPlugin {
    // Held only to keep the flight loop registered for the plugin's
    // lifetime; dropping it (in `stop`) unregisters the callback.
    _heartbeat: FlightLoop,
}

impl XPlanePlugin for HelloPlugin {
    const NAME: &'static str = "Hello Plugin (Rust)";
    const SIGNATURE: &'static str = "org.xplm-rust.hello-plugin";
    const DESCRIPTION: &'static str = "Phase 4 example: plugin lifecycle + flight loop.";

    fn start() -> Self {
        xplm::log("hello-plugin: XPluginStart\n");
        let heartbeat = FlightLoop::new(FlightLoopPhase::AfterFlightModel, |_, _, counter| {
            xplm::log(&format!("hello-plugin: heartbeat #{counter}\n"));
            -1.0 // every flight loop cycle
        });
        // XPLMCreateFlightLoop registers the callback unscheduled; opt in.
        heartbeat.schedule(-1.0, true);
        Self {
            _heartbeat: heartbeat,
        }
    }

    fn enable(&mut self) -> bool {
        xplm::log("hello-plugin: XPluginEnable\n");
        true
    }

    fn disable(&mut self) {
        xplm::log("hello-plugin: XPluginDisable\n");
    }

    fn stop(self) {
        xplm::log("hello-plugin: XPluginStop\n");
    }
}

xplm::register_plugin!(HelloPlugin);
