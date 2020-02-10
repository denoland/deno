// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::deno_error::print_err_and_exit;
use crate::fs as deno_fs;
use crate::installer::is_remote_url;
use deno_core::ErrBox;
use std;
use std::path::PathBuf;
use url::Url;

fn prepare_test_modules_urls(
  include: Vec<String>,
  root_path: PathBuf,
) -> Result<Vec<Url>, ErrBox> {
  let (include_paths, include_urls): (Vec<String>, Vec<String>) =
    include.into_iter().partition(|n| !is_remote_url(n));

  let mut prepared = vec![];

  for path in include_paths {
    let p = root_path.join(path).canonicalize()?;
    let url = Url::from_file_path(p).unwrap();
    prepared.push(url);
  }

  for remote_url in include_urls {
    let url = Url::parse(&remote_url)?;
    prepared.push(url);
  }

  Ok(prepared)
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
  let include = include.unwrap_or_else(|| vec![]);
  let cwd = std::env::current_dir().expect("No current directory");
  let res_test_modules = prepare_test_modules_urls(include, cwd.to_owned());

  if let Err(e) = res_test_modules {
    print_err_and_exit(e);
    return None;
  }

  let test_modules = res_test_modules.unwrap();
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
  fn test_prepare_test_modules_urls() {
    let test_data_path = test_util::root_path().join("cli/tests/subdir");
    let mut matched_urls = prepare_test_modules_urls(
      vec![
        "https://example.com/colors_test.ts".to_string(),
        "./mod1.ts".to_string(),
        "./mod3.js".to_string(),
        "subdir2/mod2.ts".to_string(),
        "http://example.com/printf_test.ts".to_string(),
      ],
      test_data_path.clone(),
    )
    .unwrap();
    let test_data_url =
      Url::from_file_path(test_data_path).unwrap().to_string();

    let expected: Vec<Url> = vec![
      format!("{}/mod1.ts", test_data_url),
      format!("{}/mod3.js", test_data_url),
      format!("{}/subdir2/mod2.ts", test_data_url),
      "http://example.com/printf_test.ts".to_string(),
      "https://example.com/colors_test.ts".to_string(),
    ]
    .into_iter()
    .map(|f| Url::parse(&f).unwrap())
    .collect();
    matched_urls.sort();
    assert_eq!(matched_urls, expected);
  }
}
