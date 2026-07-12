use xplm::plugin::XPlanePlugin;

#[xplm::plugin(
    name = "Test Plugin",
    signature = "org.xplm-rust.trybuild-test",
    description = "trybuild check that #[plugin(...)] expands and compiles"
)]
struct TestPlugin;

impl XPlanePlugin for TestPlugin {
    fn start() -> Self {
        TestPlugin
    }
}

fn main() {}
