// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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
    struct TestTimeoutHolder(::std::sync::mpsc::Sender<()>);

    let _test_timeout_holder = {
      let function = $crate::function!();
      let (tx, rx) = ::std::sync::mpsc::channel::<()>();
      let timeout: &[u64] = &[$($timeout)?];
      let timeout = *timeout.get(0).unwrap_or(&300);
      ::std::thread::spawn(move || {
        if rx.recv_timeout(::std::time::Duration::from_secs(timeout)) == Err(::std::sync::mpsc::RecvTimeoutError::Timeout) {
          use std::io::Write;
          eprintln!("Test {function} timed out after {timeout} seconds, aborting");
          _ = std::io::stderr().flush();
          ::std::process::exit(1);
        }
      });
      TestTimeoutHolder(tx)
    };
  };
}

#[macro_export]
macro_rules! itest(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[test]
  fn $name() {
    $crate::timeout!();
    let test = $crate::CheckOutputIntegrationTest {
      $(
        $key: $value,
       )*
      .. Default::default()
    };
    let output = test.output();
    output.assert_exit_code(test.exit_code);
    if !test.output.is_empty() {
      assert!(test.output_str.is_none());
      output.assert_matches_file(test.output);
    } else {
      output.assert_matches_text(test.output_str.unwrap_or(""));
    }
  }
}
);

#[macro_export]
macro_rules! context(
({$( $key:ident: $value:expr,)*})  => {
  $crate::TestContext::create($crate::TestContextOptions {
    $(
      $key: $value,
      )*
    .. Default::default()
  })
}
);

#[macro_export]
macro_rules! command_step(
({$( $key:ident: $value:expr,)*})  => {
  $crate::CheckOutputIntegrationTestCommandStep {
    $(
      $key: $value,
      )*
    .. Default::default()
  }
}
);
