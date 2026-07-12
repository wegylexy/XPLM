//! Minimal example plugin proving Phases 2-6 end to end: plugin lifecycle
//! via the `#[xplm::plugin(...)]` attribute macro, the panic-guarded
//! trampoline pattern, a `FlightLoop` RAII wrapper, a `Menu`, a `Window`, and
//! `xplm::graphics` drawing.
//!
//! Build with `cargo build -p hello-plugin`, then load the resulting DLL as
//! an `.xpl` in a running X-Plane to verify manually (see PHASES.md Phase 4).

use xplm::graphics::{self, Font};
use xplm::menu::{Menu, MenuCheckState};
use xplm::plugin::XPlanePlugin;
use xplm::processing::{FlightLoop, FlightLoopPhase};
use xplm::window::Window;

#[xplm::plugin(
    name = "Hello Plugin (Rust)",
    signature = "org.xplm-rust.hello-plugin",
    description = "Phase 4-6 example: plugin lifecycle + flight loop + menu + window + graphics."
)]
struct HelloPlugin {
    // Held only to keep the flight loop/menu/window registered for the
    // plugin's lifetime; dropping them (in `stop`) unregisters the native
    // objects.
    _heartbeat: FlightLoop,
    _menu: Menu,
    _window: Window,
}

impl XPlanePlugin for HelloPlugin {
    // NAME/SIGNATURE/DESCRIPTION come from #[xplm::plugin(...)] above, not
    // overridden here — see register_plugin!'s two forms in xplm::plugin.

    fn start() -> Self {
        xplm::log("hello-plugin: XPluginStart\n");
        let heartbeat = FlightLoop::new(FlightLoopPhase::AfterFlightModel, |_, _, counter| {
            xplm::log(&format!("hello-plugin: heartbeat #{counter}\n"));
            -1.0 // every flight loop cycle
        });
        // XPLMCreateFlightLoop registers the callback unscheduled; opt in.
        heartbeat.schedule(-1.0, true);

        let window = Window::builder(50, 250, 300, 50)
            .on_draw(|window| {
                let (left, top, right, bottom) = window.geometry();
                graphics::draw_translucent_dark_box(left, top, right, bottom);
                graphics::draw_string([1.0, 1.0, 1.0], left + 5, top - 15, "Hello from Rust!", None, Font::Basic);
            })
            .build()
            .expect("failed to create hello-plugin window");
        let window_handle = window.handle();

        let menu = Menu::new_in_plugins_menu("Hello Plugin", move |item_index| {
            xplm::log(&format!("hello-plugin: menu item {item_index} clicked\n"));
            window_handle.set_visible(!window_handle.is_visible());
        })
        .expect("failed to create hello-plugin menu");
        // add_item returns a MenuItem handle with getters/setters directly
        // on it — no need to thread the index back through Menu yourself.
        if let Some(item) = menu.add_item("Toggle window") {
            item.set_checked(MenuCheckState::Unchecked);
        }

        Self {
            _heartbeat: heartbeat,
            _menu: menu,
            _window: window,
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
