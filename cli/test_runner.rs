// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::installer::is_remote_url;
use deno_core::ErrBox;
use std;
use std::path::PathBuf;
use url::Url;

pub fn prepare_test_modules_urls(
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

pub fn render_test_file(modules: Vec<Url>, fail_fast: bool) -> String {
  let mut test_file = "".to_string();

  for module in modules {
    test_file.push_str(&format!("import \"{}\";\n", module.to_string()));
  }

  let run_tests_cmd =
    format!("Deno.runTests({{ exitOnFail: {} }});\n", fail_fast);
  test_file.push_str(&run_tests_cmd);

  test_file
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
