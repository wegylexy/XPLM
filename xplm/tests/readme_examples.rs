//! Type-checks the README's usage snippets so they can't silently drift out
//! of sync with the actual API. These functions are never called — calling
//! into real `XPLM*` functions (e.g. `XPLMAppendMenuItem`) outside a hosted
//! X-Plane process crashes, the same limitation noted throughout this crate
//! (see `xplm::processing`'s module docs) — `cargo test` type-checking this
//! file without running it is exactly what's needed here.

#![allow(dead_code)]

use xplm::aircraft::AircraftAccess;
use xplm::command::{Command, CommandHandler, CommandPhase};
use xplm::dataref::{ReadOnly, ReadWrite};
use xplm::instance::Instance;
use xplm::menu::Menu;
use xplm::processing::{FlightLoop, FlightLoopPhase};
use xplm::scenery::{DrawInfo, Object, ProbeOutcome, TerrainProbe};
use xplm::utilities::{directory_entries, load_data_file, save_data_file, DataFileType};
#[cfg(feature = "widgets")]
use xplm::widget::{create_widget, DispatchMode, Widget, WidgetClass, WidgetMessage};
use xplm::window::{register_hot_key, register_key_sniffer, HotKey, KeyFlags, KeySniffer};

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
        telemetry
            .override_joystick
            .set(if lat > 0.0 { 1 } else { 0 });
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

// README "Directory listing and data files" snippet.
fn _readme_directory_and_data_files() {
    directory_entries("Resources/plugins/MyPlugin/")
        .expect("path had an interior NUL")
        .for_each(|name| xplm::log(&format!("found: {name}\n")));

    save_data_file(
        DataFileType::Situation,
        "Output/situations/my_plugin_autosave.sit",
    );
    load_data_file(
        DataFileType::Situation,
        Some("Output/situations/my_plugin_autosave.sit"),
    );
}

// README "Terrain probing and instanced object drawing" snippet.
fn _readme_scenery_instance(x: f32, y: f32, z: f32) -> Instance {
    let probe = TerrainProbe::new();
    if let ProbeOutcome::Hit(hit) = probe.probe_terrain(x, y, z) {
        let _ = hit.location;
    }

    let object =
        Object::load("Resources/plugins/MyPlugin/my_object.obj").expect("failed to load object");
    let instance = object
        .new_instance(&["sim/graphics/animation/sin_wave_2"])
        .expect("failed to create instance");

    instance.set_position(
        DrawInfo {
            x,
            y,
            z,
            pitch: 0.0,
            heading: 0.0,
            roll: 0.0,
        },
        &[0.5],
    );

    instance
}

// README "Loading objects asynchronously" snippet.
mod readme_async_bridge {
    use std::cell::RefCell;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    use xplm::scenery::Object;

    #[derive(Default)]
    struct LoadState {
        result: Option<Option<Object>>,
        waker: Option<Waker>,
    }

    struct LoadObject(Rc<RefCell<LoadState>>);

    impl Future for LoadObject {
        type Output = Option<Object>;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let mut state = self.0.borrow_mut();
            match state.result.take() {
                Some(object) => Poll::Ready(object),
                None => {
                    state.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
    }

    fn load_object(path: &str) -> LoadObject {
        let state = Rc::new(RefCell::new(LoadState::default()));
        let state_for_callback = state.clone();
        Object::load_async(path, move |object| {
            let mut state = state_for_callback.borrow_mut();
            state.result = Some(object);
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        });
        LoadObject(state)
    }

    #[allow(dead_code)]
    fn _typecheck() -> LoadObject {
        load_object("Resources/plugins/MyPlugin/my_object.obj")
    }
}

// README "Aircraft" snippet.
fn _readme_aircraft_access() -> Option<AircraftAccess> {
    let access = AircraftAccess::acquire(
        None,
        Some(|| xplm::log("aircraft access is available now\n")),
    );

    match access {
        Some(access) => {
            access.set_active_aircraft_count(1);
            Some(access)
        }
        None => None,
    }
}

// README "Key sniffers and hot keys" snippet.
fn _readme_key_sniffer_and_hot_key() -> (KeySniffer, Option<HotKey>) {
    let sniffer = register_key_sniffer(true, |_key, _flags, _virtual_key| true)
        .expect("failed to register key sniffer");

    let hot_key = register_hot_key('k', KeyFlags::default(), "Do the thing", || {
        xplm::log("hot key pressed\n")
    });

    (sniffer, hot_key)
}

// README "Widgets" snippet.
#[cfg(feature = "widgets")]
fn _readme_widget(root: Widget) -> Widget {
    let button = create_widget(
        10,
        90,
        110,
        70,
        true,
        "Click me",
        false,
        Some(root.handle()),
        WidgetClass::Button,
    )
    .expect("failed to create button widget");

    let handled = button.handle().send_message(
        WidgetMessage::Other(1300), // xpMsg_PushButtonPressed
        DispatchMode::Direct,
        0,
        0,
    );
    let _ = handled;

    button
}
