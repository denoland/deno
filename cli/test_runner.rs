// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::fs as deno_fs;
use crate::installer::is_remote_url;
use globset;
use std;
use std::path::PathBuf;
use url::Url;
use walkdir::WalkDir;

fn find_local_test_modules(globs: Vec<String>, root_path: PathBuf) -> Vec<Url> {
  use globset::{Glob, GlobSetBuilder};
  dbg!(globs.clone());
  let mut builder = GlobSetBuilder::new();
  // A GlobBuilder can be used to configure each glob's match semantics
  // independently.
  assert!(root_path.is_absolute());
  assert!(root_path.is_dir());
  let root_path = root_path
    .canonicalize()
    .expect("Can't canonicalize root path");

  // TODO: use errors here
  for glob_string in globs {
    let mut glob_path = PathBuf::from(glob_string);
    if !glob_path.is_absolute() {
      glob_path = root_path.join(glob_path);
    }
    dbg!(&glob_path);
    let g = Glob::new(&glob_path.to_string_lossy()).expect("Bad glob string");
    builder.add(g.clone());
    dbg!(g.glob(), g.regex());
  }
  let glob_set = builder.build().expect("Failed to build glob");

  WalkDir::new(&root_path)
    .into_iter()
    .filter_map(|v| v.ok())
    .filter(|p| {
      let result = glob_set.is_match(p.path());
      dbg!(p.path(), result);
      result
    })
    .map(|p| {
      dbg!("mapping");
      Url::from_file_path(p.path()).unwrap()
    })
    .collect()
}

fn find_test_modules(include: Vec<String>, root_path: PathBuf) -> Vec<Url> {
  dbg!(include.clone());
  let (include_paths, include_urls): (Vec<String>, Vec<String>) =
    include.into_iter().partition(|n| !is_remote_url(n));
  let remote_urls: Vec<Url> = include_urls
    .into_iter()
    .map(|u| Url::parse(&u).unwrap())
    .collect();
  let mut file_urls = find_local_test_modules(include_paths, root_path);
  file_urls.extend_from_slice(&remote_urls);
  file_urls
}

fn render_test_file(
  modules: Vec<Url>,
  fail_fast: bool,
  _quiet: bool,
) -> String {
  let mut test_file = "".to_string();

  for module in modules {
    test_file.push_str(&format!("import \"{}\";\n", module.to_string()));
  }

  let run_tests_cmd =
    format!("Deno.runTests({{ exitOnFail: {} }});\n", fail_fast);
  test_file.push_str(&run_tests_cmd);

  test_file
}

pub fn run_test_modules(
  include: Option<Vec<String>>,
  fail_fast: bool,
  quiet: bool,
) -> Option<PathBuf> {
  let allow_none = false;
  let default_globs = vec!["**/?(*_)test.{js,ts}".to_string()];
  let include = include.unwrap_or_else(|| default_globs);
  let cwd = std::env::current_dir().expect("No current directory");
  let test_modules = find_test_modules(include, cwd.to_owned());

  if test_modules.is_empty() {
    println!("No matching test modules found");

    if !allow_none {
      std::process::exit(1);
    }

    return None;
  }

  // Create temporary test file which contains
  // all matched modules as import statements.
  let test_file = render_test_file(test_modules, fail_fast, quiet);
  let test_file_path = cwd.join(".deno.test.ts");
  deno_fs::write_file(&test_file_path, test_file.as_bytes(), 0o666)
    .expect("Can't write test file");
  Some(test_file_path)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_util;
  #[test]
  fn find_test_modules_dir_1() {
    let test_data_path = test_util::root_path().join("cli/tests/test_runner");
    let g = vec![
      "**/test.js".to_string(),
      "**/test.ts".to_string(),
      "**/*_test.js".to_string(),
      "**/*_test.ts".to_string(),
    ];
    let mut matched_urls = find_test_modules(g, test_data_path.clone());
    let expected_file_paths = vec![
      "bar_test.js",
      "foo_test.ts",
      "subdir/bar_test.js",
      "subdir/foo_test.ts",
      "subdir/test.js",
      "subdir/test.ts",
      "test.js",
      "test.ts",
    ];
    let mut expected_urls: Vec<Url> = expected_file_paths
      .into_iter()
      .map(|p| {
        let full_path = test_data_path.join(p).canonicalize().unwrap();
        Url::from_file_path(full_path).unwrap()
      })
      .collect();
    matched_urls.sort();
    expected_urls.sort();
    assert_eq!(matched_urls, expected_urls);
  }

  // #[test]
  // fn find_test_modules_dir_2() {
  //   const urls = await findTestModulesArray(["subdir"], [], TEST_DATA_PATH);
  // assertEquals(urls.sort(), [
  //   `${TEST_DATA_URL}/subdir/bar_test.js`,
  //   `${TEST_DATA_URL}/subdir/foo_test.ts`,
  //   `${TEST_DATA_URL}/subdir/test.js`,
  //   `${TEST_DATA_URL}/subdir/test.ts`
  // ]);
  // }

  #[test]
  fn find_test_modules_glob() {
    let test_data_path = test_util::root_path().join("cli/tests/test_runner");
    let mut matched_urls = find_test_modules(
      vec!["**/test.{js,ts}".to_string()],
      test_data_path.clone(),
    );
    let expected_file_paths = vec![
      "bar_test.js",
      "foo_test.ts",
      "subdir/bar_test.js",
      "subdir/foo_test.ts",
    ];
    let mut expected_urls: Vec<Url> = expected_file_paths
      .into_iter()
      .map(|p| {
        let full_path = test_data_path.join(p).canonicalize().unwrap();
        Url::from_file_path(full_path).unwrap()
      })
      .collect();
    matched_urls.sort();
    expected_urls.sort();
    assert_eq!(matched_urls, expected_urls);
  }

  // #[test]
  // fn find_test_modules_exclude_dir() {
  //   const urls = await findTestModulesArray(["."], ["subdir"], TEST_DATA_PATH);
  // assertEquals(urls.sort(), [
  //   `${TEST_DATA_URL}/bar_test.js`,
  //   `${TEST_DATA_URL}/foo_test.ts`,
  //   `${TEST_DATA_URL}/test.js`,
  //   `${TEST_DATA_URL}/test.ts`
  // ]);
  // }

  // #[test]
  // fn find_test_modules_exclude_glob() {
  //   const urls = await findTestModulesArray(["."], ["**/foo*"], TEST_DATA_PATH);
  //   assertEquals(urls.sort(), [
  //     `${TEST_DATA_URL}/bar_test.js`,
  //     `${TEST_DATA_URL}/subdir/bar_test.js`,
  //     `${TEST_DATA_URL}/subdir/test.js`,
  //     `${TEST_DATA_URL}/subdir/test.ts`,
  //     `${TEST_DATA_URL}/test.js`,
  //     `${TEST_DATA_URL}/test.ts`
  //   ]);
  // }

  #[test]
  fn find_test_modules_remote() {
    let urls = vec![
      "https://example.com/colors_test.ts".to_string(),
      "http://example.com/printf_test.ts".to_string(),
    ];
    let matches =
      find_test_modules(urls.clone(), std::env::current_dir().unwrap());
    let matched_urls: Vec<String> =
      matches.into_iter().map(|m| m.to_string()).collect();
    assert_eq!(matched_urls, urls);
  }
}
