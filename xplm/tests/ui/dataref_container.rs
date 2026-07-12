use xplm::dataref::{ReadOnly, ReadWrite};

#[derive(xplm::DataRefContainer)]
struct Telemetry {
    #[dataref = "sim/flightmodel/position/latitude"]
    latitude: ReadOnly<f64>,
    #[dataref = "sim/cockpit2/engine/actuators/throttle_ratio_all"]
    throttle: ReadWrite<f32>,
}

fn main() {
    // Not called (would need a hosted X-Plane process to succeed) — this
    // only proves the derive expands to a `find()` with the right shape.
    let _: fn() -> Option<Telemetry> = Telemetry::find;
}
