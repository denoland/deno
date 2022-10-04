// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;

use crate::fs_util;
use crate::fs_util::path_with_stem_suffix;

/// Partitions the provided specifiers by the non-path and non-query parts of a specifier.
pub fn partition_by_root_specifiers<'a>(
  specifiers: impl Iterator<Item = &'a ModuleSpecifier>,
) -> BTreeMap<ModuleSpecifier, Vec<ModuleSpecifier>> {
  let mut root_specifiers: BTreeMap<ModuleSpecifier, Vec<ModuleSpecifier>> =
    Default::default();
  for remote_specifier in specifiers {
    let mut root_specifier = remote_specifier.clone();
    root_specifier.set_query(None);
    root_specifier.set_path("/");

    let specifiers = root_specifiers.entry(root_specifier).or_default();
    specifiers.push(remote_specifier.clone());
  }
  root_specifiers
}

/// Gets the directory name to use for the provided root.
pub fn dir_name_for_root(root: &ModuleSpecifier) -> PathBuf {
  fs_util::root_url_to_safe_local_dirname(root)
}

/// Gets a unique file path given the provided file path
/// and the set of existing file paths. Inserts to the
/// set when finding a unique path.
pub fn get_unique_path(
  mut path: PathBuf,
  unique_set: &mut HashSet<String>,
) -> PathBuf {
  let original_path = path.clone();
  let mut count = 2;
  // case insensitive comparison so the output works on case insensitive file systems
  while !unique_set.insert(path.to_string_lossy().to_lowercase()) {
    path = path_with_stem_suffix(&original_path, &format!("_{}", count));
    count += 1;
  }
  path
}

pub fn make_url_relative(
  root: &ModuleSpecifier,
  url: &ModuleSpecifier,
) -> Result<String, AnyError> {
  root.make_relative(url).ok_or_else(|| {
    anyhow!(
      "Error making url ({}) relative to root: {}",
      url.to_string(),
      root.to_string()
    )
  })
}

pub fn is_remote_specifier(specifier: &ModuleSpecifier) -> bool {
  specifier.scheme().to_lowercase().starts_with("http")
}

pub fn is_remote_specifier_text(text: &str) -> bool {
  text.trim_start().to_lowercase().starts_with("http")
}

pub fn sanitize_filepath(text: &str) -> String {
  text
    .chars()
    .map(|c| {
      if fs_util::is_banned_path_char(c) {
        '_'
      } else {
        c
      }
    })
    .collect()
}

#[cfg(test)]
mod test {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn partition_by_root_specifiers_same_sub_folder() {
    run_partition_by_root_specifiers_test(
      vec![
        "https://deno.land/x/mod/A.ts",
        "https://deno.land/x/mod/other/A.ts",
      ],
      vec![(
        "https://deno.land/",
        vec![
          "https://deno.land/x/mod/A.ts",
          "https://deno.land/x/mod/other/A.ts",
        ],
      )],
    );
  }

  #[test]
  fn partition_by_root_specifiers_different_sub_folder() {
    run_partition_by_root_specifiers_test(
      vec![
        "https://deno.land/x/mod/A.ts",
        "https://deno.land/x/other/A.ts",
      ],
      vec![(
        "https://deno.land/",
        vec![
          "https://deno.land/x/mod/A.ts",
          "https://deno.land/x/other/A.ts",
        ],
      )],
    );
  }

  #[test]
  fn partition_by_root_specifiers_different_hosts() {
    run_partition_by_root_specifiers_test(
      vec![
        "https://deno.land/mod/A.ts",
        "http://deno.land/B.ts",
        "https://deno.land:8080/C.ts",
        "https://localhost/mod/A.ts",
        "https://other/A.ts",
      ],
      vec![
        ("http://deno.land/", vec!["http://deno.land/B.ts"]),
        ("https://deno.land/", vec!["https://deno.land/mod/A.ts"]),
        (
          "https://deno.land:8080/",
          vec!["https://deno.land:8080/C.ts"],
        ),
        ("https://localhost/", vec!["https://localhost/mod/A.ts"]),
        ("https://other/", vec!["https://other/A.ts"]),
      ],
    );
  }

  fn run_partition_by_root_specifiers_test(
    input: Vec<&str>,
    expected: Vec<(&str, Vec<&str>)>,
  ) {
    let input = input
      .iter()
      .map(|s| ModuleSpecifier::parse(s).unwrap())
      .collect::<Vec<_>>();
    let output = partition_by_root_specifiers(input.iter());
    // the assertion is much easier to compare when everything is strings
    let output = output
      .into_iter()
      .map(|(s, vec)| {
        (
          s.to_string(),
          vec.into_iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
      })
      .collect::<Vec<_>>();
    let expected = expected
      .into_iter()
      .map(|(s, vec)| {
        (
          s.to_string(),
          vec.into_iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
      })
      .collect::<Vec<_>>();
    assert_eq!(output, expected);
  }

  #[test]
  fn test_unique_path() {
    let mut paths = HashSet::new();
    assert_eq!(
      get_unique_path(PathBuf::from("/test"), &mut paths),
      PathBuf::from("/test")
    );
    assert_eq!(
      get_unique_path(PathBuf::from("/test"), &mut paths),
      PathBuf::from("/test_2")
    );
    assert_eq!(
      get_unique_path(PathBuf::from("/test"), &mut paths),
      PathBuf::from("/test_3")
    );
    assert_eq!(
      get_unique_path(PathBuf::from("/TEST"), &mut paths),
      PathBuf::from("/TEST_4")
    );
    assert_eq!(
      get_unique_path(PathBuf::from("/test.txt"), &mut paths),
      PathBuf::from("/test.txt")
    );
    assert_eq!(
      get_unique_path(PathBuf::from("/test.txt"), &mut paths),
      PathBuf::from("/test_2.txt")
    );
    assert_eq!(
      get_unique_path(PathBuf::from("/TEST.TXT"), &mut paths),
      PathBuf::from("/TEST_3.TXT")
    );
  }
}
