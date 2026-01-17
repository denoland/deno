// Copyright 2018-2026 the Deno authors. MIT license.

#[macro_export]
// https://stackoverflow.com/questions/38088067/equivalent-of-func-or-function-in-rust
macro_rules! function {
  () => {{
    fn f() {}
    fn type_name_of<T>(_: T) -> &'static str {
      ::std::any::type_name::<T>()
    }
    let name = type_name_of(f);
    let name = name.strip_suffix("::f").unwrap_or(name);
    let name = name.strip_suffix("::{{closure}}").unwrap_or(name);
    name
  }};
}

/// Detect a test timeout and panic with a message that includes the test name.
/// By default, the test timeout is 300 seconds (5 minutes), but any value may
/// be specified as an argument to this function.
#[macro_export]
macro_rules! timeout {
  ( $($timeout:literal)? ) => {
    let _test_timeout_holder = {
      let function = $crate::function!();
      let timeout: &[u64] = &[$($timeout)?];
      let timeout = *timeout.get(0).unwrap_or(&300);
      $crate::test_runner::with_timeout(
        function.to_string(),
        ::std::time::Duration::from_secs(timeout)
      )
    };
  };
}
