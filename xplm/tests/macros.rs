//! Proves `#[xplm::plugin(...)]` and `#[derive(xplm::DataRefContainer)]`
//! expand to code that actually compiles.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/plugin_attribute.rs");
    t.pass("tests/ui/dataref_container.rs");
}
