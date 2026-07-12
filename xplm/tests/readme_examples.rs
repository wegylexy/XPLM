//! Type-checks the README's usage snippets so they can't silently drift out
//! of sync with the actual API. These functions are never called — calling
//! into real `XPLM*` functions (e.g. `XPLMAppendMenuItem`) outside a hosted
//! X-Plane process crashes, the same limitation noted throughout this crate
//! (see `xplm::processing`'s module docs) — `cargo test` type-checking this
//! file without running it is exactly what's needed here.

#![allow(dead_code)]

use xplm::command::{Command, CommandHandler, CommandPhase};
use xplm::dataref::{ReadOnly, ReadWrite};
use xplm::menu::Menu;
use xplm::processing::{FlightLoop, FlightLoopPhase};

// README "Menus, including nested submenus" snippet.
fn _readme_nested_menu(menu: &Menu) {
    let settings_item = menu.add_item("Settings").expect("failed to add item");

    let settings_menu = Menu::new_submenu(menu, &settings_item, "Settings", |item_index| {
        xplm::log(&format!("settings item {item_index} clicked\n"));
    })
    .expect("failed to create submenu");

    settings_menu.add_item("Option A");
    settings_menu.add_item("Option B");
}

// README "A plugin" snippet's Telemetry type + MyPlugin::start() body.
#[derive(xplm::DataRefContainer)]
struct Telemetry {
    #[dataref = "sim/flightmodel/position/latitude"]
    latitude: ReadOnly<f64>,
    #[dataref = "sim/operation/override/override_joystick"]
    override_joystick: ReadWrite<i32>,
}

fn _readme_plugin_start() -> (Menu, FlightLoop) {
    let menu = Menu::new_in_plugins_menu("My Plugin", |item_index| {
        xplm::log(&format!("clicked item {item_index}\n"));
    })
    .expect("failed to create menu");
    menu.add_item("Do the thing");

    let telemetry = Telemetry::find().expect("dataref(s) not found");
    let flight_loop = FlightLoop::new(FlightLoopPhase::AfterFlightModel, move |_, _, _| {
        let lat = telemetry.latitude.get();
        telemetry.override_joystick.set(if lat > 0.0 { 1 } else { 0 });
        -1.0
    });
    flight_loop.schedule(-1.0, true);

    (menu, flight_loop)
}

// README "Commands" snippet.
fn _readme_command() -> (Command, CommandHandler) {
    let cmd = Command::find("sim/autopilot/hold_altitude")
        .or_else(|| Command::create("my_plugin/do_the_thing", "Does the thing"))
        .expect("failed to find/create command");

    let handler = cmd.register_handler(true, |phase| {
        if phase == CommandPhase::Begin {
            xplm::log("do_the_thing: pressed\n");
        }
        true
    });

    cmd.once();

    (cmd, handler)
}
