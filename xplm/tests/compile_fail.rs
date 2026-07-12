//! Proves the DataRef access gating is enforced at compile time: `set()`
//! must not exist on `ReadOnly<T>`/`ReadOnlyArray<T>`.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/set_on_readonly.rs");
    t.compile_fail("tests/ui/set_on_readonly_array.rs");
}
