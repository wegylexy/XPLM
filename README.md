# XPLM (Rust)

Idiomatic Rust bindings for the X-Plane plugin SDK, spanning `xplm-sys` (raw FFI),
`xplm` (safe wrappers), and `xplm-macros` (`#[plugin]`, `#[derive(DataRefContainer)]`).

## Getting the SDK

The X-Plane SDK headers/libraries are not vendored in this repo. Download and unzip
them before building:

1. Download the SDK zip: https://developer.x-plane.com/wp-content/plugins/code-sample-generation/sdk_zip_files/XPSDK430.zip
2. Unzip it at the repository root so you end up with:

   ```
   XPLM/
   ├── SDK/
   │   ├── CHeaders/
   │   ├── Libraries/
   │   ├── license.txt
   │   └── README.txt
   ├── xplm-sys/
   ├── xplm/
   └── xplm-macros/
   ```

   The zip's top-level folder is typically named after its release (e.g. `XPSDK430`) —
   rename it to `SDK` after extracting.

`SDK/` is gitignored; `xplm-sys`'s `build.rs` reads headers/libs from it at build time.

## Supported X-Plane / XPLM versions

Cargo features on `xplm-sys` and `xplm` mirror the `XPLM<version>` macros defined in
the SDK headers, cumulative low-to-high (enabling a higher version feature pulls in
every lower one, matching the SDK's requirement that all applicable version macros be
defined together):

| Feature   | X-Plane version      |
|-----------|-----------------------|
| `XPLM200` | 9.00+                 |
| `XPLM210` | 10.00+ (10.20+ for 64-bit) |
| `XPLM300` | 11.10+                |
| `XPLM301` | 11.20+                |
| `XPLM302` | 11.21+                |
| `XPLM303` | 11.50+                |
| `XPLM400` | 12.04+                |
| `XPLM410` | 12.1.0+               |
| `XPLM420` | 12.3.0+ (default)     |

Only the versions present in the vendored `SDK/CHeaders` are exposed; there is no
`XPLM430` feature even if a newer SDK zip defines it, until this crate is updated.

## Usage

A full worked example lives in [`examples/hello-plugin`](examples/hello-plugin) —
it combines everything below plus a `FlightLoop` heartbeat, a `Window`, and
`xplm::graphics` drawing. The snippets here isolate each piece.

### A plugin

`#[xplm::plugin(...)]` supplies the plugin's name/signature/description and wires
up the five `extern "C"` exports X-Plane requires; you implement
`xplm::plugin::XPlanePlugin` for the plugin's lifecycle. This example wires up
everything in one place: a menu item click handler, and a `FlightLoop` that
reads and writes datarefs every cycle via `#[derive(DataRefContainer)]`:

```rust
use xplm::dataref::{ReadOnly, ReadWrite};
use xplm::menu::Menu;
use xplm::plugin::XPlanePlugin;
use xplm::processing::{FlightLoop, FlightLoopPhase};

#[derive(xplm::DataRefContainer)]
struct Telemetry {
    #[dataref = "sim/flightmodel/position/latitude"]
    latitude: ReadOnly<f64>,
    #[dataref = "sim/operation/override/override_joystick"]
    override_joystick: ReadWrite<i32>,
}

#[xplm::plugin(
    name = "My Plugin",
    signature = "com.example.my-plugin",
    description = "Does a thing"
)]
struct MyPlugin {
    // Held only to keep the menu/flight loop registered for the plugin's
    // lifetime; dropping them (in `stop`, or when `MyPlugin` is dropped)
    // unregisters the native objects.
    _menu: Menu,
    _flight_loop: FlightLoop,
}

impl XPlanePlugin for MyPlugin {
    // NAME/SIGNATURE/DESCRIPTION come from #[xplm::plugin(...)] above —
    // don't override them here.

    fn start() -> Self {
        let menu = Menu::new_in_plugins_menu("My Plugin", |item_index| {
            // Click handling: item_index tells you which item, looked up
            // fresh each time (see "Menus, including nested submenus" below
            // for why that matters once items can be removed).
            xplm::log(&format!("clicked item {item_index}\n"));
        })
        .expect("failed to create menu");
        menu.add_item("Do the thing");

        let telemetry = Telemetry::find().expect("dataref(s) not found");
        let flight_loop = FlightLoop::new(FlightLoopPhase::AfterFlightModel, move |_, _, _| {
            // Dataref read/write every flight loop cycle: read latitude,
            // write a derived value back. `telemetry` is moved into the
            // closure, so it (and the datarefs it holds) live exactly as
            // long as the flight loop does.
            let lat = telemetry.latitude.get();
            telemetry.override_joystick.set(if lat > 0.0 { 1 } else { 0 });
            -1.0 // run again next cycle
        });
        flight_loop.schedule(-1.0, true); // XPLMCreateFlightLoop starts unscheduled

        Self {
            _menu: menu,
            _flight_loop: flight_loop,
        }
    }

    // enable/disable/stop/receive_message all have no-op defaults; override
    // whichever ones you need.
}
```

If you'd rather not use the attribute macro, `xplm::register_plugin!(MyPlugin)`
does the same thing reading `NAME`/`SIGNATURE`/`DESCRIPTION` off your own
`impl XPlanePlugin` block instead (see `xplm::plugin`'s docs for both forms).

### Menus, including nested submenus

`Menu::add_item` returns a `MenuItem` handle; pass one to `Menu::new_submenu` to
anchor a submenu at that item, nesting as deep as you like:

```rust
use xplm::menu::Menu;

// Top-level menu, under X-Plane's Plugins menu.
let menu = Menu::new_in_plugins_menu("My Plugin", |item_index| {
    xplm::log(&format!("clicked item {item_index}\n"));
})
.expect("failed to create menu");

let settings_item = menu.add_item("Settings").expect("failed to add item");

// A submenu anchored at that item — clicking items on it runs its own
// handler, independent of the parent menu's.
let settings_menu = Menu::new_submenu(&menu, &settings_item, "Settings", |item_index| {
    xplm::log(&format!("settings item {item_index} clicked\n"));
})
.expect("failed to create submenu");

settings_menu.add_item("Option A");
settings_menu.add_item("Option B");
```

`menu`/`settings_menu` must be kept alive for as long as you want the menus to
exist — dropping either destroys it (and, for `menu`, everything nested under
it).

### Reading datarefs with `#[derive(DataRefContainer)]`

Tag each field with the dataref path it should be found by; `find()` looks all
of them up at once, failing if any single one isn't currently registered:

```rust
use xplm::dataref::{ReadOnly, ReadWrite};

#[derive(xplm::DataRefContainer)]
struct AircraftTelemetry {
    #[dataref = "sim/flightmodel/position/latitude"]
    latitude: ReadOnly<f64>,
    #[dataref = "sim/flightmodel/position/longitude"]
    longitude: ReadOnly<f64>,
    #[dataref = "sim/cockpit2/engine/actuators/throttle_ratio_all"]
    throttle: ReadWrite<f32>,
}

let telemetry = AircraftTelemetry::find().expect("dataref(s) not found");
let lat = telemetry.latitude.get();
telemetry.throttle.set(0.75); // only compiles because it's ReadWrite<f32>
```

### Commands

`Command`s aren't owned by any one plugin — `find`/`create` return a handle
that stays valid even after your plugin unloads. `register_handler` returns a
`CommandHandler`; dropping *that* unregisters your callback (the command
itself is unaffected):

```rust
use xplm::command::{Command, CommandPhase};

let cmd = Command::find("sim/autopilot/hold_altitude")
    .or_else(|| Command::create("my_plugin/do_the_thing", "Does the thing"))
    .expect("failed to find/create command");

let handler = cmd.register_handler(true, |phase| {
    if phase == CommandPhase::Begin {
        xplm::log("do_the_thing: pressed\n");
    }
    true // let processing continue (to X-Plane and other plugins)
});

cmd.once(); // or begin()/end() for a held-down command
```

## Running tests (Windows)

`xplm-sys` delay-loads `XPLM_64.dll` (it only exists inside a running X-Plane
process, so tests must be able to start without it), but any test that actually
calls into an `XPLM*` function still needs the real DLL on `PATH` to resolve at
that point. Point `PATH` at a local X-Plane install's `Resources/plugins/`
directory before running `cargo test`. X-Plane records its own install location
in `%LocalAppData%\x-plane_install_12.txt` (or `_11.txt`) — read that instead of
hardcoding a path:

```powershell
$xpRoot = (Get-Content "$env:LocalAppData\x-plane_install_12.txt" -TotalCount 1).Trim()
$env:PATH = "$xpRoot\Resources\plugins;$env:PATH"
cargo test
```

Tests that don't call into XPLM at all (most unit tests — see
`xplm::processing::tests`) pass without this; it's only needed once a test
exercises a real `XPLMCreateFlightLoop`/etc. call.
