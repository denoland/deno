// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#[macro_export]
macro_rules! timeout {
  () => {
    let _drop_detect = ::std::sync::Arc::new(());
    {
      let clone = _drop_detect.clone();
      ::std::thread::spawn(move || {
        ::std::thread::sleep(::std::time::Duration::from_secs(120));
        if ::std::sync::Arc::strong_count(&clone) > 1 {
          panic!("Test timed out after 120 seconds");
        }
      });
    }
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
macro_rules! itest_flaky(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[flaky_test::flaky_test]
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
macro_rules! itest_steps(
($name:ident {$( $key:ident: $value:expr,)*})  => {
  #[test]
  fn $name() {
    ($crate::CheckOutputIntegrationTestSteps {
      $(
        $key: $value,
       )*
      .. Default::default()
    }).run()
  }
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
