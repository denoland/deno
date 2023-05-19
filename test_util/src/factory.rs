use std::collections::HashSet;

// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use glob::glob;

/// Generate a unit test factory verified and backed by a glob.
#[macro_export]
macro_rules! unit_test_factory {
  ($test_fn:ident, $glob:literal, [ $($test:ident),+ $(,)? ]) => {
    #[test]
    fn check_test_glob() {
      $crate::factory::check_test_glob($glob, [$( stringify!($test) ),+].as_slice());
    }

    $(
      #[test]
      fn $test() {
        $test_fn(stringify!($test))
      }
    )+
  }
}

/// Validate that the glob matches the list of tests specified.
pub fn check_test_glob(glob_pattern: &'static str, files: &[&'static str]) {
  let mut found = HashSet::new();
  let mut list = vec![];
  for file in glob(glob_pattern).expect("Failed to read test path") {
    let mut file = file.expect("Invalid file from glob");
    file.set_extension("");
    let file = file.file_name().unwrap().to_string_lossy().into_owned();
    found.insert(file.clone());
    list.push(file);
  }

  let mut error = false;
  for file in files {
    if found.contains(*file) {
      found.remove(*file);
    } else {
      error = true;
    }
  }

  if error || !found.is_empty() {
    panic!(
      "Glob did not match provided list of files. Expected: \n[\n  {}\n]",
      list.join(",\n  ")
    );
  }
}
