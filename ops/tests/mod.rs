#[test]
fn op_macro() {
  let t = trybuild::TestCases::new();
  t.compile_fail("tests/compile_fail/*.rs");
}
