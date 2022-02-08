// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;

/// Partitions the provided specifiers by specifiers that do not have a
/// parent specifier.
pub fn partition_by_root_specifiers<'a>(
  specifiers: impl Iterator<Item = &'a ModuleSpecifier>,
) -> Vec<(ModuleSpecifier, Vec<ModuleSpecifier>)> {
  let mut root_specifiers: Vec<(ModuleSpecifier, Vec<ModuleSpecifier>)> =
    Vec::new();
  for remote_specifier in specifiers {
    let mut found = false;
    for (root_specifier, specifiers) in root_specifiers.iter_mut() {
      if let Some(relative_url) = root_specifier.make_relative(remote_specifier)
      {
        // found a new root
        if relative_url.starts_with("../") {
          let end_ancestor_index =
            relative_url.len() - relative_url.trim_start_matches("../").len();
          *root_specifier = root_specifier
            .join(&relative_url[..end_ancestor_index])
            .unwrap();
        }

        specifiers.push(remote_specifier.clone());
        found = true;
        break;
      }
    }
    if !found {
      // get the specifier without the directory
      let root_specifier = remote_specifier
        .join("./")
        .unwrap_or_else(|_| remote_specifier.clone());
      root_specifiers.push((root_specifier, vec![remote_specifier.clone()]));
    }
  }
  root_specifiers
}

/// Gets the flattened directory name to use for the provided root
/// specifier and its descendant specifiers. We use the descendant
/// specifiers to estimate the maximum directory path length in
/// order to truncate the root directory name if necessary due to
/// the 260 character max path length on Windows.
pub fn dir_name_for_root(
  root: &ModuleSpecifier,
  specifiers: &[ModuleSpecifier],
) -> PathBuf {
  // all the provided specifiers should be descendants of the root
  debug_assert!(specifiers
    .iter()
    .all(|s| s.as_str().starts_with(root.as_str())));

  let mut result = String::new();
  if let Some(domain) = root.domain() {
    result.push_str(&sanitize_segment(domain));
  }
  if let Some(port) = root.port() {
    if !result.is_empty() {
      result.push('_');
    }
    result.push_str(&port.to_string());
  }
  if let Some(segments) = root.path_segments() {
    for segment in segments.filter(|s| !s.is_empty()) {
      if !result.is_empty() {
        result.push('/');
      }
      result.push_str(&sanitize_segment(segment));
    }
  }

  PathBuf::from(if result.is_empty() {
    "unknown".to_string()
  } else {
    // Limit the size of the directory to reduce the chance of max path
    // errors on Windows. This uses bytes instead of chars because it's
    // faster, but the approximation should be good enough.
    let root_len = root.as_str().len();
    let max_specifier_len = specifiers
      .iter()
      .map(|s| s.as_str().len())
      .max()
      .unwrap_or(root_len);
    let sub_path_len = max_specifier_len - root_len;
    let max_win_path = 260;
    // This is the approximate length that a path might be before the root directory.
    // It should be a hardcoded number and not calculated based on the system as the
    // produced code will most likely not stay only on the system or directory that
    // produced it.
    let approx_path_prefix_len = 80;
    let truncate_len = std::cmp::max(
      10,
      max_win_path as isize
        - approx_path_prefix_len as isize
        - sub_path_len as isize,
    ) as usize;

    // if the final text should be truncated, then truncate and
    // flatten it to a single folder name
    let text = match result.char_indices().nth(truncate_len) {
      Some((i, _)) => (&result[..i]).replace('/', "_"),
      None => result,
    };
    text.trim_end_matches('_').trim_end_matches('.').to_string()
  })
}

/// Gets a path with the specified file stem suffix.
pub fn path_with_stem_suffix(path: &Path, suffix: &str) -> PathBuf {
  if let Some(file_name) = path.file_name().map(|f| f.to_string_lossy()) {
    if let Some(file_stem) = path.file_stem().map(|f| f.to_string_lossy()) {
      if let Some(ext) = path.extension().map(|f| f.to_string_lossy()) {
        return if file_stem.to_lowercase().ends_with(".d") {
          path.with_file_name(format!(
            "{}_{}.d.{}",
            &file_stem[..file_stem.len() - ".d".len()],
            suffix,
            ext
          ))
        } else {
          path.with_file_name(format!("{}_{}.{}", file_stem, suffix, ext))
        };
      }
    }

    path.with_file_name(format!("{}_{}", file_name, suffix))
  } else {
    path.with_file_name(suffix)
  }
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
  // case insensitive comparison for case insensitive file systems
  while !unique_set.insert(path.to_string_lossy().to_lowercase()) {
    path = path_with_stem_suffix(&original_path, &count.to_string());
    count += 1;
  }
  path
}

pub fn make_url_relative(
  root: &ModuleSpecifier,
  url: &ModuleSpecifier,
) -> Result<String, AnyError> {
  let mut url = url.clone();
  url.set_query(None);
  root.make_relative(&url).ok_or_else(|| {
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
    .map(|c| if is_banned_path_char(c) { '_' } else { c })
    .collect()
}

fn is_banned_path_char(c: char) -> bool {
  matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
}

fn sanitize_segment(text: &str) -> String {
  text
    .chars()
    .map(|c| if is_banned_segment_char(c) { '_' } else { c })
    .collect()
}

fn is_banned_segment_char(c: char) -> bool {
  matches!(c, '/' | '\\') || is_banned_path_char(c)
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
        "https://deno.land/x/mod/",
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
        "https://deno.land/x/",
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
        "https://localhost/mod/A.ts",
        "https://other/A.ts",
      ],
      vec![
        ("https://deno.land/mod/", vec!["https://deno.land/mod/A.ts"]),
        ("https://localhost/mod/", vec!["https://localhost/mod/A.ts"]),
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
  fn should_get_dir_name_root() {
    run_test(
      "http://deno.land/x/test",
      &["http://deno.land/x/test/mod.ts"],
      "deno.land/x/test",
    );
    run_test(
      "http://localhost",
      &["http://localhost/test.mod"],
      "localhost",
    );
    run_test(
      "http://localhost/test%20test",
      &["http://localhost/test%20test/asdf"],
      "localhost/test%20test",
    );
    // will flatten and truncate
    run_test(
      // length of 45
      "http://localhost/testtestestingtestingtesting",
      // length of 210
      &["http://localhost/testtestestingtestingtesting/testingthisoutwithaverlongspecifiertestingtasfasdfasdfasdfadsfasdfasfasdfasfasdfasdfasfasdfasfdasdfasdfasdfasdfasdfsdafasdfasdasdfasdfasdfasdfasdfasdfaasdfasdfas.ts"],
      // Max(10, 260 - 80 - (210 - 45)) = 15 chars
      "localhost_testt",
    );
    // will truncate
    run_test(
      // length of 45
      "http://localhost/testtestestingtestingtesting",
      // length of 220
      &["http://localhost/testtestestingtestingtesting/testingthisoutwithaverlongspecifiertestingtasfasdfasdfasdfadsfasdfasfasdfasfasdfasdfasfasdfasfdasdfasdfasdfasdfasdfsdafasdfasdasdfasdfasdfasdfasdfasdfaasdfasdfasteststttts.ts"],
      // Max(10, 260 - 80 - (210 - 45)) = 10 and trim the trailing underscore
      "localhost",
    );

    fn run_test(specifier: &str, specifiers: &[&str], expected: &str) {
      assert_eq!(
        dir_name_for_root(
          &ModuleSpecifier::parse(specifier).unwrap(),
          &specifiers
            .iter()
            .map(|s| ModuleSpecifier::parse(s).unwrap())
            .collect::<Vec<_>>(),
        ),
        PathBuf::from(expected)
      );
    }
  }

  #[test]
  fn test_path_with_stem_suffix() {
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/"), "2"),
      PathBuf::from("/2")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test"), "2"),
      PathBuf::from("/test_2")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.txt"), "2"),
      PathBuf::from("/test_2.txt")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test/subdir"), "2"),
      PathBuf::from("/test/subdir_2")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test/subdir.other.txt"), "2"),
      PathBuf::from("/test/subdir.other_2.txt")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.d.ts"), "2"),
      PathBuf::from("/test_2.d.ts")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.D.TS"), "2"),
      // good enough
      PathBuf::from("/test_2.d.TS")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.d.mts"), "2"),
      PathBuf::from("/test_2.d.mts")
    );
    assert_eq!(
      path_with_stem_suffix(&PathBuf::from("/test.d.cts"), "2"),
      PathBuf::from("/test_2.d.cts")
    );
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
