// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug)]
struct Pattern {
  prefix: String,
  suffix: String,
}
impl Pattern {
  fn new_with_wildcard(pattern: &str, wildcard_index: usize) -> Self {
    let prefix = pattern[..wildcard_index].to_string();
    let suffix = pattern[wildcard_index + 1..].to_string();
    Self { prefix, suffix }
  }

  fn new_prefix(pattern: String) -> Self {
    Self {
      prefix: pattern,
      suffix: String::new(),
    }
  }
}
#[derive(Debug)]
struct Patterns {
  patterns: Vec<Pattern>,
  exact: HashSet<String>,
}

impl Patterns {
  fn is_match(&self, path: &str) -> bool {
    if self.exact.contains(path) {
      return true;
    }
    for pattern in &self.patterns {
      if path.starts_with(&pattern.prefix) && path.ends_with(&pattern.suffix) {
        return true;
      }
    }
    false
  }
}
#[derive(Debug)]
pub struct ExternalsMatcher {
  pre_resolve: Patterns,
  post_resolve: Patterns,
}

fn is_package_path(path: &str) -> bool {
  !path.starts_with('/')
    && !path.starts_with("./")
    && !path.starts_with("../")
    && path != "."
    && path != ".."
}

fn to_absolute_path(path: &str, cwd: &Path) -> String {
  if path.starts_with('/') {
    path.to_string()
  } else {
    let path = cwd.join(path);
    deno_path_util::normalize_path(Cow::Owned(path))
      .to_string_lossy()
      .into_owned()
  }
}

impl ExternalsMatcher {
  /// A set of patterns indicating files to mark as external.
  ///
  /// For instance given, `--external="*.node" --external="*.wasm"`, the matcher will match
  /// any path that ends with `.node` or `.wasm`.
  pub fn new(externals: &[String], cwd: &Path) -> Self {
    let mut pre_resolve = Patterns {
      patterns: vec![],
      exact: HashSet::new(),
    };
    let mut post_resolve = Patterns {
      patterns: vec![],
      exact: HashSet::new(),
    };
    for external in externals {
      let wildcard = external.find("*");
      if let Some(wildcard_index) = wildcard {
        if external[wildcard_index + 1..].contains('*') {
          log::error!("Externals must not contain multiple wildcards");
          continue;
        }
        pre_resolve
          .patterns
          .push(Pattern::new_with_wildcard(external, wildcard_index));
        if !is_package_path(external) {
          let normalized = to_absolute_path(external, cwd);
          if let Some(index) = normalized.find('*') {
            post_resolve
              .patterns
              .push(Pattern::new_with_wildcard(&normalized, index));
          }
        }
      } else {
        pre_resolve.exact.insert(external.to_string());
        if is_package_path(external) {
          pre_resolve
            .patterns
            .push(Pattern::new_prefix([external, "/"].join("")));
        } else {
          let normalized = to_absolute_path(external, cwd);
          post_resolve.exact.insert(normalized);
        }
      }
    }
    Self {
      pre_resolve,
      post_resolve,
    }
  }

  pub fn is_pre_resolve_match(&self, path: &str) -> bool {
    self.pre_resolve.is_match(path)
  }

  pub fn is_post_resolve_match(&self, path: &str) -> bool {
    self.post_resolve.is_match(path)
  }
}

#[cfg(test)]
mod tests {
  #![allow(clippy::print_stderr)]
  use std::path::Path;

  use super::ExternalsMatcher;

  struct Matches {
    pre_resolve: Vec<String>,
    post_resolve: Vec<String>,
  }

  fn matches_all<'a, S: AsRef<str>>(
    patterns: impl IntoIterator<Item = S>,
    matches: Matches,
    no_match: impl IntoIterator<Item = &'a str>,
  ) -> bool {
    let patterns = patterns
      .into_iter()
      .map(|p| p.as_ref().to_string())
      .collect::<Vec<_>>();
    let cwd = std::env::current_dir().unwrap();
    let matcher = ExternalsMatcher::new(&patterns, &cwd);
    for path in matches.pre_resolve {
      if !matcher.is_pre_resolve_match(&path) {
        eprintln!("failed to match pre resolve: {}", path);
        return false;
      }
    }
    for path in matches.post_resolve {
      if !matcher.is_post_resolve_match(&path) {
        eprintln!("failed to match post resolve: {}", path);
        return false;
      }
    }
    for path in no_match {
      if matcher.is_pre_resolve_match(path) {
        eprintln!("matched pre resolve when it should not: {}", path);
        return false;
      }
      if matcher.is_post_resolve_match(path) {
        eprintln!("matched post resolve when it should not: {}", path);
        return false;
      }
    }
    true
  }

  fn s<S: AsRef<str>>(s: impl IntoIterator<Item = S>) -> Vec<String> {
    s.into_iter().map(|p| p.as_ref().to_string()).collect()
  }

  fn path_str(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().into_owned()
  }

  #[test]
  fn matches_package_path() {
    assert!(matches_all(
      ["chalk"],
      Matches {
        pre_resolve: s(["chalk", "chalk/foo"]),
        post_resolve: vec![],
      },
      ["other/chalk", "./chalk/foo.ts", "./chalk"]
    ));
    assert!(matches_all(
      ["@std/fs"],
      Matches {
        pre_resolve: s(["@std/fs", "@std/fs/foo"]),
        post_resolve: vec![],
      },
      ["other/@std/fs", "./@std/fs/foo.ts", "./@std/fs"]
    ));
  }

  #[test]
  fn matches_path() {
    assert!(matches_all(
      ["/node_modules/fo"],
      Matches {
        pre_resolve: s(["/node_modules/fo"]),
        post_resolve: s(["/node_modules/fo"]),
      },
      ["/node_modules/foo"]
    ));

    let cwd = std::env::current_dir().unwrap();
    assert!(matches_all(
      ["./foo"],
      Matches {
        pre_resolve: s(["./foo"]),
        post_resolve: s([path_str(cwd.join("foo"))]),
      },
      ["other/foo", "./foo.ts", "./foo/bar", "thing/./foo"]
    ));
  }

  #[test]
  fn matches_wildcard() {
    assert!(matches_all(
      ["*.node"],
      Matches {
        pre_resolve: s(["foo.node", "foo/bar.node"]),
        post_resolve: vec![],
      },
      ["foo.ts", "./foo.node.ts", "./foo/bar.node.ts"]
    ));
    assert!(matches_all(
      ["@std/*"],
      Matches {
        pre_resolve: s(["@std/fs", "@std/fs/foo"]),
        post_resolve: vec![],
      },
      ["other/@std/fs", "./@std/fs/foo.ts", "./@std/fs"]
    ));
    let cwd = std::env::current_dir().unwrap();
    assert!(matches_all(
      ["./foo/*"],
      Matches {
        pre_resolve: s(["./foo/bar", "./foo/baz"]),
        post_resolve: vec![
          path_str(cwd.join("foo").join("bar")),
          path_str(cwd.join("foo").join("baz")),
        ],
      },
      ["other/foo/bar", "./bar/foo", "./bar/./foo/bar"]
    ));
  }
}
