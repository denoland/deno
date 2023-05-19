use std::{collections::HashSet, path::PathBuf};

// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use glob::glob;

/// Generate a unit test factory verified and backed by a glob.
#[macro_export]
macro_rules! unit_test_factory {
  ($test_fn:ident, $base:literal, $glob:literal, [ $( $test:ident ),+ $(,)? ]) => {
    #[test]
    fn check_test_glob() {
      $crate::factory::check_test_glob($base, $glob, [$( stringify!( $test ) ),+].as_slice());
    }

    $(
      #[allow(non_snake_case)]
      #[test]
      fn $test() {
        $test_fn(stringify!($test).replace("_DIR_", "/"))
      }
    )+
  }
}

/// Validate that the glob matches the list of tests specified.
pub fn check_test_glob(
  base: &'static str,
  glob_pattern: &'static str,
  files: &[&'static str],
) {
  let base_dir = PathBuf::from(base)
    .canonicalize()
    .unwrap()
    .to_string_lossy()
    .into_owned();
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
    let file = file.to_string_lossy().into_owned();
    let file = file
      .strip_prefix(&base_dir)
      .expect("File {file} did not start with {base_dir} prefix");
    let file = file.strip_prefix('/').unwrap().to_owned();
    list.push(file.replace('/', "_DIR_"));
    found.insert(file.replace('/', "_DIR_"));
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
