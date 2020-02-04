// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::fs as deno_fs;
use crate::installer::is_remote_url;
use globset;
use std;
use std::path::PathBuf;
use url::Url;
use walkdir::WalkDir;

fn find_local_test_modules(globs: Vec<String>) -> Vec<Url> {
  use globset::{Glob, GlobSetBuilder};
  dbg!(globs.clone());
  let mut builder = GlobSetBuilder::new();
  // A GlobBuilder can be used to configure each glob's match semantics
  // independently.
  let cwd = std::env::current_dir().unwrap();

  for glob_string in globs {
    let glob_path = cwd.join(glob_string);
    let g = Glob::new(&glob_path.to_string_lossy()).expect("Bad glob string");
    builder.add(g.clone());
    dbg!(g.glob(), g.regex());
  }
  let glob_set = builder.build().expect("Failed to build glob");

  WalkDir::new(".")
    .into_iter()
    .filter_map(|v| v.ok())
    .map(|e| cwd.join(e.path()).canonicalize())
    .filter_map(|v| v.ok())
    .filter(|p| {
      let result = glob_set.is_match(p);
      dbg!(p, result.clone());
      result
    })
    .map(|p| {
      dbg!("mapping");
      Url::from_file_path(p).unwrap()
    })
    .collect()
}

fn find_test_modules(include: Vec<String>) -> Vec<Url> {
  dbg!(include.clone());
  let (include_paths, include_urls): (Vec<String>, Vec<String>) =
    include.into_iter().partition(|n| !is_remote_url(n));
  let remote_urls: Vec<Url> = include_urls
    .into_iter()
    .map(|u| Url::parse(&u).unwrap())
    .collect();
  let mut file_urls = find_local_test_modules(include_paths);
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

  let run_tests_cmd = format!(
    "Deno.runTests({{
    exitOnFail: {}
  }})",
    fail_fast
  );
  test_file.push_str(&run_tests_cmd);

  test_file.to_string()
}

pub fn run_test_modules(
  include: Option<Vec<String>>,
  fail_fast: bool,
  quiet: bool,
) -> Option<PathBuf> {
  let allow_none = false;
  let include =
    include.unwrap_or_else(|| vec!["**/?(*_)test.{js,ts}".to_string()]);
  let test_modules = find_test_modules(include);

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

  let cwd = std::env::current_dir().expect("No current directory");
  let test_file_path = cwd.join(".deno.test.ts");
  deno_fs::write_file(&test_file_path, test_file.as_bytes(), 0o666)
    .expect("Can't write test file");
  Some(test_file_path)
}
