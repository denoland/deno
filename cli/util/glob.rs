// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;

pub fn expand_globs(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, AnyError> {
  let mut new_paths = vec![];
  for path in paths {
    let path_str = path.to_string_lossy();
    if is_glob_pattern(&path_str) {
      let globbed_paths = glob(&path_str)?;

      for globbed_path_result in globbed_paths {
        new_paths.push(globbed_path_result?);
      }
    } else {
      new_paths.push(path);
    }
  }

  Ok(new_paths)
}

pub fn glob(pattern: &str) -> Result<glob::Paths, AnyError> {
  glob::glob_with(&escape_brackets(pattern), match_options())
    .with_context(|| format!("Failed to expand glob: \"{}\"", pattern))
}

pub fn is_glob_pattern(path: &str) -> bool {
  path.chars().any(|c| matches!(c, '*' | '?'))
}

fn escape_brackets(pattern: &str) -> String {
  // Escape brackets - we currently don't support them, because with introduction
  // of glob expansion paths like "pages/[id].ts" would suddenly start giving
  // wrong results. We might want to revisit that in the future.
  pattern.replace('[', "[[]").replace(']', "[]]")
}

fn match_options() -> glob::MatchOptions {
  // Matches what `deno_task_shell` does
  glob::MatchOptions {
    // false because it should work the same way on case insensitive file systems
    case_sensitive: false,
    // true because it copies what sh does
    require_literal_separator: true,
    // true because it copies with sh does—these files are considered "hidden"
    require_literal_leading_dot: true,
  }
}
