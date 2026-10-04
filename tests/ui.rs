//! Compile-fail tests for the API's type-level guarantees.

#[test]
#[cfg_attr(miri, ignore = "runs the compiler")]
fn ui() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
