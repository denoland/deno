// Copyright 2018-2026 the Deno authors. MIT license.

pub fn rewrite(specifier: &str) -> Option<String> {
  let path_end = specifier.find(['?', '#']).unwrap_or(specifier.len());
  let (path, suffix) = specifier.split_at(path_end);
  if path.ends_with(".d.ts")
    || path.ends_with(".d.mts")
    || path.ends_with(".d.cts")
  {
    None
  } else if let Some(path) = path.strip_suffix(".tsx") {
    Some(format!("{path}.js{suffix}"))
  } else if let Some(path) = path.strip_suffix(".ts") {
    Some(format!("{path}.js{suffix}"))
  } else if let Some(path) = path.strip_suffix(".mts") {
    Some(format!("{path}.mjs{suffix}"))
  } else {
    path
      .strip_suffix(".cts")
      .map(|path| format!("{path}.cjs{suffix}"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rewrites_typescript_extensions() {
    assert_eq!(rewrite("./mod.ts").as_deref(), Some("./mod.js"));
    assert_eq!(rewrite("../mod.tsx").as_deref(), Some("../mod.js"));
    assert_eq!(rewrite("./mod.mts").as_deref(), Some("./mod.mjs"));
    assert_eq!(rewrite("./mod.cts").as_deref(), Some("./mod.cjs"));
  }

  #[test]
  fn preserves_suffixes_and_non_typescript_extensions() {
    assert_eq!(
      rewrite("./mod.ts?raw#fragment").as_deref(),
      Some("./mod.js?raw#fragment")
    );
    assert_eq!(rewrite("./mod.d.ts"), None);
    assert_eq!(rewrite("./mod.js"), None);
    assert_eq!(rewrite("./literal.ts.txt"), None);
  }
}
