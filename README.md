# XPLM (Rust)

Idiomatic Rust bindings for the X-Plane plugin SDK, spanning `xplm-sys` (raw FFI),
`xplm` (safe wrappers), and `xplm-macros` (`#[plugin]`, `#[derive(DataRefContainer)]`).

## Getting the SDK

The X-Plane SDK headers/libraries aren't tracked by git in this repo (both are
gitignored, per each crate's own `vendor/` path below) — run the fetch script
once per machine:

```powershell
.\scripts\fetch-xplm-sdk.ps1
```
```bash
./scripts/fetch-xplm-sdk.sh
```

This downloads the SDK zip and places it in both locations this workspace's
build scripts expect:

```
XPLM/
├── xplm-sys/vendor/xplm-sdk/       # full SDK — xplm-sys/build.rs
│   ├── CHeaders/
│   ├── Libraries/
│   └── license.txt
├── xpmp2-sys/vendor/xplm-sdk/      # headers-only subset — xpmp2-sys/build.rs
│   └── CHeaders/XPLM/
├── xpmp2-sys/vendor/XPMP2/         # git submodule, see below
├── xplm-sys/
├── xplm/
└── xplm-macros/
```

(The zip's top-level folder is already named `SDK` — all-caps — once
extracted; no manual renaming needed.)

Both crates also honor an `XPLM_SDK_DIR` env var if you'd rather point at a
different SDK checkout instead of the vendored copy (e.g. a shared CI cache,
or a newer SDK version than this repo currently targets) — set it to a
directory containing `CHeaders/`+`Libraries/` and skip the fetch script.

`xpmp2-sys`/`xpmp2` also need the `XPMP2` git submodule, at
`xpmp2-sys/vendor/XPMP2`:

```
git submodule update --init xpmp2-sys/vendor/XPMP2
```

(or set `XPMP2_SRC_DIR` to point at your own checkout instead).

Because both crates' `Cargo.toml` explicitly `include` their `vendor/`
contents in the published package (despite `.gitignore`), a `cargo publish`
build and a real `cargo add`'d downstream consumer both get the SDK/XPMP2
source baked into the crate with no separate download step — this repo's own
gitignored `vendor/` copies are what a maintainer populates (via the fetch
script) before running `cargo publish`.

### Publishing a release

`cargo publish -p flybywireless-xplm-sys` (and `-xpmp2-sys`) will always
refuse without `--allow-dirty`, even on an otherwise fully-committed working
tree — this isn't a sign something's wrong. `cargo publish`'s dirty-check
walks every file the package is about to include and checks whether *each
one* is tracked by git; `vendor/xplm-sdk`/`vendor/XPMP2` are deliberately
gitignored (see above), so they always show up as "untracked" to that check
regardless of the rest of the repo's state. `--allow-dirty` is the correct,
expected flag for these two crates specifically, every release — not a
one-time workaround.

Publish in dependency order, since crates.io needs a dependency to already
be resolvable before a dependent crate's own `cargo publish` verify-build can
succeed:

```
cargo publish -p flybywireless-xplm-sys --allow-dirty
cargo publish -p flybywireless-xplm-macros
cargo publish -p flybywireless-xplm
cargo publish -p flybywireless-xpmp2-sys --allow-dirty
cargo publish -p flybywireless-xpmp2
```

`cargo publish` itself waits for each crate to become resolvable on
crates.io's index before returning, so no manual delay is needed between
these — just run them in order.

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

Only the versions present in the vendored `xplm-sys/vendor/xplm-sdk/CHeaders` are exposed; there is no
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

### Directory listing and data files

```rust
use xplm::utilities::{directory_entries, load_data_file, save_data_file, DataFileType};

// Lazy, pull-based enumeration (like C#'s `IEnumerable<string>`) — entries
// are fetched a small page at a time as you iterate, not staged into one
// big buffer upfront.
directory_entries("Resources/plugins/MyPlugin/")
    .expect("path had an interior NUL")
    .for_each(|name| xplm::log(&format!("found: {name}\n")));

save_data_file(DataFileType::Situation, "Output/situations/my_plugin_autosave.sit");
load_data_file(DataFileType::Situation, Some("Output/situations/my_plugin_autosave.sit"));
```

### Terrain probing and instanced object drawing

`TerrainProbe` finds the physical scenery mesh under a point; `Object`/`Instance`
load a `.obj` and draw it, moving it (and the datarefs it animates with) from a
flight loop rather than a drawing callback:

```rust
use xplm::scenery::{DrawInfo, Object, ProbeOutcome, TerrainProbe};

let probe = TerrainProbe::new();
if let ProbeOutcome::Hit(hit) = probe.probe_terrain(x, y, z) {
    // hit.location is the terrain point directly below (x, y, z).
}

let object = Object::load("Resources/plugins/MyPlugin/my_object.obj")
    .expect("failed to load object");
// Object::new_instance(...) is shorthand for Instance::new(&object, ...).
let instance = object
    .new_instance(&["sim/graphics/animation/sin_wave_2"])
    .expect("failed to create instance");

// From a flight loop or UI callback (never a drawing callback):
instance.set_position(
    DrawInfo { x, y, z, pitch: 0.0, heading: 0.0, roll: 0.0 },
    &[0.5], // one value per dataref passed to Instance::new, same order
);
```

#### Loading objects asynchronously

`Object::load_async` is callback-based, not `async fn` — X-Plane's plugin
runtime has no ambient executor to poll a `Future` for you, so making this
`async` in `xplm` itself would just move the "who drives this?" problem onto
every caller without actually solving it. If you already have your own
async runtime driving other work in your plugin (uncommon, but not unheard
of), bridging the callback into a real `Future` is a plain oneshot-channel
pattern — nothing `xplm`-specific:

```rust
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
```

You'd still need something to actually poll this `Future` to completion —
e.g. a small executor stepped once per `FlightLoop` tick — which is a bigger
piece of infrastructure than `xplm` provides today (it'd need to be a
crate-level addition, not a per-caller pattern, to be worth shipping).

### Aircraft

`AircraftAccess` is exclusive — only one plugin can hold it at a time —
same shape as `CameraControl`, but `acquire` tells you whether you got it,
and takes an optional callback for when you don't:

```rust
use xplm::aircraft::AircraftAccess;

let access = AircraftAccess::acquire(
    None, // don't load any AI aircraft models, just take control
    Some(|| xplm::log("aircraft access is available now\n")),
);

match access {
    Some(access) => {
        access.set_active_aircraft_count(1);
        // access.set_aircraft_model/disable_ai_for_plane, etc.
    }
    None => {
        // Another plugin holds access; the callback above will run once it
        // releases it, but you must call acquire() again then to take it —
        // a notification isn't a grant.
    }
}
```

`access` releases exclusive control (`XPLMReleasePlanes`) when dropped.
`xplm::aircraft` also has free functions for the user's own aircraft —
`set_users_aircraft`, `place_user_at_airport`/`place_user_at_location`,
`aircraft_count`, `nth_aircraft_model` — none of which need `AircraftAccess`.

### Key sniffers and hot keys

Both live in `XPLMDisplay.h` (not `XPLMUtilities.h`), so they're in
`xplm::window` alongside `Window`. A key sniffer sees every keystroke while
it's registered; a hot key fires once per press of a specific combination:

```rust
use xplm::window::{register_hot_key, register_key_sniffer, KeyFlags};

let sniffer = register_key_sniffer(true, |_key, _flags, _virtual_key| {
    true // let the key continue on to the window system
});

let hot_key = register_hot_key(
    'k',
    KeyFlags::default(), // no modifiers
    "Do the thing",
    || xplm::log("hot key pressed\n"),
);
```

Dropping `sniffer`/`hot_key` unregisters them. `xplm::window::hot_key_count`/
`nth_hot_key` enumerate *every* plugin's hot keys (not just your own) — don't
cache the index across calls, since another plugin (un)registering one shifts
the positions after it, the same reindexing hazard as `xplm::menu`.

### Widgets

The widgets UI toolkit (`xplm-sys/vendor/xplm-sdk/CHeaders/Widgets`) is behind the opt-in
`widgets` Cargo feature — it links a second native library
(`XPWidgets_64`), so plugins that don't use it don't pay for that:

```toml
xplm = { version = "...", features = ["widgets"] }
```

```rust
use xplm::widget::{create_widget, DispatchMode, WidgetClass, WidgetMessage};

let button = create_widget(
    10, 90, 110, 70, // left, top, right, bottom
    true,
    "Click me",
    false, // not a root widget — it's placed inside another one
    Some(root.handle()),
    WidgetClass::Button,
).expect("failed to create button widget");

// Standard widget classes' own messages (here, xpMsg_PushButtonPressed)
// aren't named constants yet — reach them via WidgetMessage::Other and
// XPStandardWidgets.h until they are.
let handled = button.handle().send_message(
    WidgetMessage::Other(1300),
    DispatchMode::Direct,
    0, 0,
);
```

Every widget API addresses a widget by its raw `XPWidgetID` directly (not an
index), so — unlike `xplm::menu::MenuItem` — a `Widget`/`WidgetRef` handle
never goes stale on its own; only `WidgetRef::children`'s *enumeration
order* shifts if the tree is mutated mid-iteration, the same caveat as any
collection. Dropping a `Widget` destroys it (and its descendants) natively.
For entirely custom behavior instead of a built-in class, use
`create_custom_widget`, which takes a `FnMut(WidgetMessage, WidgetRef, isize,
isize) -> bool` closure in place of a `WidgetClass`.

### Multiplayer (XPMP2)

Multiplayer traffic (other planes drawn via the [XPMP2](https://github.com/TwinFan/XPMP2)
library, as used by e.g. LiveTraffic) is a separate crate, `xpmp2`, since it
depends on a whole second native library rather than XPLM itself:

```toml
xpmp2 = { version = "..." }
```

Initialize once (typically in `XPlanePlugin::start`), and implement `Aircraft`
for your own per-plane state:

```rust
use xpmp2::{Aircraft, Multiplayer, Plane, PlaneHandle};

struct MyPlane { lat: f64, lon: f64, alt_ft: f64 }

impl Aircraft for MyPlane {
    fn update_position(&mut self, plane: &PlaneHandle, _elapsed_since_last_call: f32, _fl_counter: i32) {
        plane.set_location(self.lat, self.lon, self.alt_ft);
    }
}

let multiplayer = Multiplayer::init(
    "My Plugin",
    "./Resources", // XPMP2's Doc8643.txt/MapIcons.png/related.txt — see below
    Some("A320"),  // fallback ICAO type if none can be deduced
    None,
).expect("XPMPMultiplayerInit failed");

let plane = Plane::new(&multiplayer, "A320", "", "", 0, "", MyPlane { lat: 0.0, lon: 0.0, alt_ft: 5000.0 })
    .expect("XPMP2 rejected the plane");
```

`Multiplayer`/`Plane` are `!Sync` (XPMP2, like the rest of XPLM, is only
safe to call from X-Plane's main thread) but are `Send`, so they're fine to
hold in plugin state; `Plane::new` takes `&Multiplayer` only as proof one is
live, not as a stored borrow, specifically so a plugin can own both a
`Multiplayer` and a growing `Vec<Plane>` without a self-referential struct.

CSL packages (the actual 3D models XPMP2 draws) get into XPMP2 one of two
mutually-exclusive ways, each its own opt-in feature:

- **`csl-offline`** — a plugin that ships/installs its whole CSL library
  locally, loaded up front via `Multiplayer::load_csl_package`. This is the
  path that needs `Doc8643.txt`/`related.txt`/`MapIcons.png` in the
  `resource_dir` passed to `Multiplayer::init`, since XPMP2 does its own
  ICAO/livery-based matching (`ChangeModel`) against them.
- **`csl-on-demand`** — fetch and load exactly one model's package the
  instant it's needed, via the published
  [`flybywireless-csl-client`](https://crates.io/crates/flybywireless-csl-client)
  crate speaking [csl-on-demand](https://github.com/wegylexy/csl-on-demand)'s
  `/match` + `/manifest` protocol:

  ```toml
  xpmp2 = { version = "...", default-features = false, features = ["XPLM420", "csl-on-demand"] }
  ```

  ```rust
  use std::sync::mpsc;
  use xpmp2::csl_on_demand::{CslCache, FetchedPackage};

  let csl_cache = CslCache::new(&multiplayer, "https://csl.example.com", "./CSLCache");

  // Non-blocking — modeled on xplm::scenery::Object::load_async's shape.
  // The callback itself must be Send, but Multiplayer/Plane are
  // deliberately !Send/!Sync, so it only ever forwards a plain result
  // through a channel; a flight loop (which alone owns `multiplayer`)
  // drains that channel and is what actually calls Plane::new — see
  // examples/xpmp2-template for the full, runnable version.
  let (fetched_tx, fetched_rx) = mpsc::channel::<Result<FetchedPackage, String>>();
  csl_cache.request(Some("A320"), None, None, None, move |result| {
      let _ = fetched_tx.send(result.map_err(|err| err.to_string()));
  });

  // ...later, from a flight loop callback on the main thread:
  if let Ok(result) = fetched_rx.try_recv() {
      match result {
          Ok(fetched) => {
              // fetched.csl_id is this model's exact XPMP2 CSL identifier
              // ("{root}/{id}") — pass it to Plane::new so XPMP2 assigns
              // this exact, already server-matched model directly, instead
              // of falling back to local Doc8643/ICAO-based matching.
              let plane = Plane::new(
                  &multiplayer, "A320", "", "", 0, &fetched.csl_id,
                  MyPlane { lat: 0.0, lon: 0.0, alt_ft: 5000.0 },
              );
              let _ = plane;
          }
          Err(err) => xplm::log(&format!("CSL fetch failed: {err}\n")),
      }
  }
  ```

  Because on-demand mode always supplies an exact `csl_id`, it never needs
  `Doc8643.txt`/`related.txt`/`MapIcons.png` — `resource_dir` can point at
  an otherwise-empty directory. Built with `csl-on-demand`, `Plane::new`
  panics on an empty `csl_id`, since that would silently (and, before any
  package is loaded, unsuccessfully) fall back to the local matching path
  this mode is specifically for avoiding.

A full worked example (simulated traffic-spotted event → fetch → load →
`Plane::new`, all via `mpsc` channels polled from flight loops, never
blocking the main thread) lives in
[`examples/xpmp2-template`](examples/xpmp2-template).

## Hot-reloading a plugin under development

A DLL can't unload itself, and X-Plane only ever loads whatever's registered
in `Resources/plugins/` at startup — so the usual edit/rebuild/test loop means
stopping the sim. [`examples/hot-reload-xpl`](examples/hot-reload-xpl) +
[`examples/hot-reload-dll`](examples/hot-reload-dll) demonstrate a loader/
payload split that avoids that:

- **`hot-reload-xpl`** is a thin loader plugin. It's the only one that
  actually gets copied into `Resources/plugins/` (as `64/win.xpl`, `64/mac.xpl`,
  or `64/lin.xpl`), and it never needs rebuilding — it just forwards every SDK
  callback to whatever payload library is currently named in a watch file,
  polling once a second for a new build and swapping it in
  (`libloading::Library` load/drop — `LoadLibraryW`/`FreeLibrary` on Windows,
  `dlopen`/`dlclose` on macOS/Linux) without touching the sim. Nothing in the
  loader's own forwarding/swap logic is platform-specific — `libloading`
  already abstracts that, and `xplm-sys` already builds for all three target
  OSes — only *where the watch file lives* differs (see below).
- **`hot-reload-dll`** is the payload — an ordinary plugin crate, rebuilt on
  every edit. This example's payload tunes COM1 to a hard-coded frequency
  once (`FlightLoop` + `xplm::dataref::ReadWrite`), so you can change the
  frequency, rebuild, and watch it get retuned in-sim without a restart.

The watch file lives at `dirs::data_local_dir()/xplm-hotreload/hot-reload-example.json`
— `%LocalAppData%\xplm-hotreload\...` on Windows, `~/Library/Application
Support/xplm-hotreload/...` on macOS, `${XDG_DATA_HOME:-~/.local/share}/xplm-hotreload/...`
on Linux. The loader and the build task must agree on this exact path; the
fixed filename is fine for this one example, but a reusable version of this
pattern should derive it from a hash of the payload crate's manifest directory
instead, so multiple hot-reloaded projects on one machine don't collide.

The install-location file lookup (`examples/hot-reload-xpl/scripts/find-xplane-root.{ps1,sh}`,
used by both the "ensure X-Plane running" and "install loader" tasks) follows
X-Plane's own documented convention: `%LocalAppData%\x-plane_install_12.txt`
(Windows), `~/Library/Preferences/x-plane_install_12.txt` (macOS),
`~/.x-plane/x-plane_install_12.txt` (Linux) — falling back to each `_11.txt`.
Per that same documentation, the file can list multiple install locations,
including stale/moved ones, so the scripts scan every line and use the first
one that actually has a `Resources` subfolder, rather than trusting line 1.

> **Prerequisites for macOS/Linux:** the `hot-reload: build payload` task
> needs `jq` installed to parse `cargo`'s JSON build output, and the
> mac/Linux `launch.json` attach config needs the CodeLLDB VS Code
> extension.

Because cargo's own artifact-name hash is based on package/feature/profile
metadata, not source content, two builds of unchanged config still produce the
same output filename even after editing source — which Windows won't let you
overwrite while the old one is loaded. The build task instead passes a fresh
`-C extra-filename` (a timestamp) to `cargo rustc` on every build, so each
build gets a genuinely unique DLL name and the lock never applies.

Also worth knowing about attaching a debugger: Windows debuggers attach to
the whole `X-Plane.exe` process, not to an individual loaded module — there's
no way around that. In practice this still feels scoped to just your plugin,
since X-Plane's own code carries no debug symbols and you only ever set
breakpoints in your payload's source.

### F5 in VS Code

Open `examples/hot-reload-xpl` as the VS Code workspace root (its `.vscode/`
holds the config). Hitting F5 on a completely clean checkout, with X-Plane not
even running, does everything automatically:

1. **`hot-reload: ensure X-Plane running`** — starts X-Plane (found via the
   install-location file described above) if it isn't already running, and
   waits for the process to appear. A fully cold X-Plane launch takes a while
   to reach its main loop, so the very first F5 on a machine may need longer
   than subsequent ones before attach succeeds — that's the sim's own boot
   time, not something this task can speed up.
2. **`hot-reload: install loader`** — builds `hot-reload-xpl` and copies it
   into `<X-Plane>/Resources/plugins/hot-reload-xpl/64/{win,mac,lin}.xpl` (per
   OS), unconditionally, every run (so a previously-copied loader can never go
   stale unnoticed).
3. **`hot-reload: build payload`** — builds `hot-reload-dll` with a unique
   `-C extra-filename`, then writes the watch file.
4. VS Code then attaches its debugger to X-Plane (`cppvsdbg` on Windows,
   CodeLLDB on macOS/Linux — pick the matching launch config for your OS).
   Breakpoints set in `hot-reload-dll`'s source resolve once the loader's next
   1Hz tick picks up the new build.

From then on: edit `COM1_FREQ_833` in `examples/hot-reload-dll/src/lib.rs`,
hit F5 again, and the new frequency gets tuned within about a second — no
restart, no manual copying.

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
