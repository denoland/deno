// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::url::Url;

pub fn find_root(urls: Vec<&Url>) -> Option<Url> {
  if urls.is_empty() {
    return None;
  }

  // Gets the common first part of all the urls.
  let root = urls[0]
    .as_ref()
    .chars()
    .enumerate()
    .take_while(|(i, c)| {
      urls.iter().all(|u| u.as_ref().chars().nth(*i) == Some(*c))
    })
    .map(|(_, c)| c)
    .collect::<String>();

  if let Some(index) = root.rfind('/') {
    // Removes the basename part if exists.
    Url::parse(&root[..index + 1]).ok()
  } else {
    Url::parse(&root).ok()
  }
}

pub fn percent_to_class(percent: f32) -> &'static str {
  match percent {
    x if x < 50.0 => "low",
    x if x < 80.0 => "medium",
    _ => "high",
  }
}

pub fn calc_coverage_display_info(
  hit: usize,
  miss: usize,
) -> (usize, f32, &'static str) {
  let total = hit + miss;
  let percent = if total == 0 {
    100.0
  } else {
    (hit as f32 / total as f32) * 100.0
  };
  let class = percent_to_class(percent);
  (total, percent, class)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_find_root() {
    let urls = vec![
      Url::parse("file:///a/b/c/d/e.ts").unwrap(),
      Url::parse("file:///a/b/c/d/f.ts").unwrap(),
      Url::parse("file:///a/b/c/d/g.ts").unwrap(),
    ];
    let urls = urls.iter().collect();
    assert_eq!(find_root(urls), Url::parse("file:///a/b/c/d/").ok());
  }

  #[test]
  fn test_find_root_empty() {
    let urls = vec![];
    assert_eq!(find_root(urls), None);
  }

  #[test]
  fn test_find_root_with_similar_filenames() {
    let urls = vec![
      Url::parse("file:///a/b/c/d/foo0.ts").unwrap(),
      Url::parse("file:///a/b/c/d/foo1.ts").unwrap(),
      Url::parse("file:///a/b/c/d/foo2.ts").unwrap(),
    ];
    let urls = urls.iter().collect();
    assert_eq!(find_root(urls), Url::parse("file:///a/b/c/d/").ok());
  }

  #[test]
  fn test_find_root_with_similar_dirnames() {
    let urls = vec![
      Url::parse("file:///a/b/c/foo0/mod.ts").unwrap(),
      Url::parse("file:///a/b/c/foo1/mod.ts").unwrap(),
      Url::parse("file:///a/b/c/foo2/mod.ts").unwrap(),
    ];
    let urls = urls.iter().collect();
    assert_eq!(find_root(urls), Url::parse("file:///a/b/c/").ok());
  }

  #[test]
  fn test_percent_to_class() {
    assert_eq!(percent_to_class(0.0), "low");
    assert_eq!(percent_to_class(49.9), "low");
    assert_eq!(percent_to_class(50.0), "medium");
    assert_eq!(percent_to_class(79.9), "medium");
    assert_eq!(percent_to_class(80.0), "high");
    assert_eq!(percent_to_class(100.0), "high");
  }
}
