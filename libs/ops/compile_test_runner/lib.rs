// Copyright 2018-2025 the Deno authors. MIT license.

#[macro_export]
macro_rules! prelude {
  () => {
    #[allow(unused_imports)]
    use deno_ops::FromV8;
    #[allow(unused_imports)]
    use deno_ops::WebIDL;
    #[allow(unused_imports)]
    use deno_ops::op2;

    pub fn main() {}
  };
}

#[cfg(test)]
mod compile_tests {
  #[test]
  fn op2() {
    let t = trybuild::TestCases::new();
    t.pass("../op2/test_cases/**/*.rs");
    t.pass("../op2/test_cases/compiler_pass/*.rs");
    t.compile_fail("../op2/test_cases_fail/**/*.rs");
  }

  #[test]
  fn webidl() {
    let t = trybuild::TestCases::new();
    t.pass("../webidl/test_cases/*.rs");
    t.compile_fail("../webidl/test_cases_fail/*.rs");
  }

  #[test]
  fn from_v8() {
    let t = trybuild::TestCases::new();
    t.pass("../conversion/from_v8/test_cases/*.rs");
    t.compile_fail("../conversion/from_v8/test_cases_fail/*.rs");
  }
}
