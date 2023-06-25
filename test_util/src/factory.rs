// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use glob::glob;
use std::collections::HashSet;
use std::path::PathBuf;

/// Generate a unit test factory verified and backed by a glob.
#[macro_export]
macro_rules! unit_test_factory {
  ($test_fn:ident, $base:literal, $glob:literal, [ $( $test:ident $(= $($path:ident)/+)? ),+ $(,)? ]) => {
    #[test]
    fn check_test_glob() {
      $crate::factory::check_test_glob($base, $glob, [ $( ( stringify!($test), stringify!( $( $($path)/+ )? ) ) ),+ ].as_slice());
    }

    $(
      #[allow(non_snake_case)]
      #[test]
      fn $test() {
        $test_fn($crate::factory::get_path(stringify!($test), stringify!( $( $($path)/+ )?)))
      }
    )+
  };
  (__test__ $($prefix:ident)* $test:ident) => {
    #[allow(non_snake_case)]
    #[test]
    fn $test() {
      $test_fn(stringify!($($prefix)/+ $test))
    }
  };
}

pub fn get_path(test: &'static str, path: &'static str) -> String {
  if path.is_empty() {
    test.to_owned()
  } else {
    path.replace(' ', "")
  }
}

/// Validate that the glob matches the list of tests specified.
pub fn check_test_glob(
  base: &'static str,
  glob_pattern: &'static str,
  files: &[(&'static str, &'static str)],
) {
  let base_dir = PathBuf::from(base)
    .canonicalize()
    .unwrap()
    .to_string_lossy()
    // Strip Windows slashes
    .replace('\\', "/");
  let mut found = HashSet::new();
  let mut list = vec![];
  for file in glob(&format!("{}/{}", base, glob_pattern))
    .expect("Failed to read test path")
  {
    let mut file = file
      .expect("Invalid file from glob")
      .canonicalize()
      .unwrap();
    file.set_extension("");
    let name = file.file_name().unwrap().to_string_lossy();
    // Strip windows slashes
    let file = file.to_string_lossy().replace('\\', "/");
    let file = file
      .strip_prefix(&base_dir)
      .expect("File {file} did not start with {base_dir} prefix");
    let file = file.strip_prefix('/').unwrap().to_owned();
    if file.contains('/') {
      list.push(format!("{}={}", name, file))
    } else {
      list.push(file.clone());
    }
    found.insert(file);
  }

  let mut error = false;
  for (test, path) in files {
    // Remove spaces from the macro
    let path = if path.is_empty() {
      (*test).to_owned()
    } else {
      path.replace(' ', "")
    };
    if found.contains(&path) {
      found.remove(&path);
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
