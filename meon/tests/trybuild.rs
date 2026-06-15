//! UI tests for the `define_parser!` proc-macro.
//!
//! Each `tests/ui/fail/*.rs` feeds a malformed grammar and is paired with a
//! `.stderr` snapshot of the expected, span-located `compile_error!`.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/*.rs");
    t.compile_fail("tests/ui/fail/*.rs");
}
