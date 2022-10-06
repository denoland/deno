#[test]
fn op_macro() {
  let t = trybuild::TestCases::new();
  t.compile_fail("tests/compile_fail/*.rs");
  t.pass("tests/01_fast_callback_options.rs");
}
