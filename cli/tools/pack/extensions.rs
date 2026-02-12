// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_ast::MediaType;

/// Convert a TypeScript path to its JavaScript equivalent.
/// `.ts` → `.js`, `.tsx` → `.js`, `.mts` → `.mjs`, `.cts` → `.cjs`
pub fn ts_to_js_extension(path: &str) -> String {
  let path = path.trim_start_matches("./");
  if path.ends_with(".tsx") {
    format!("{}.js", &path[..path.len() - 4])
  } else if path.ends_with(".ts") {
    format!("{}.js", &path[..path.len() - 3])
  } else if path.ends_with(".mts") {
    format!("{}.mjs", &path[..path.len() - 4])
  } else if path.ends_with(".cts") {
    format!("{}.cjs", &path[..path.len() - 4])
  } else {
    path.to_string()
  }
}

/// Convert a TypeScript path to its declaration file equivalent.
/// `.ts` → `.d.ts`, `.tsx` → `.d.ts`, `.mts` → `.d.mts`, `.cts` → `.d.cts`
pub fn ts_to_dts_extension(path: &str) -> String {
  let path = path.trim_start_matches("./");
  if path.ends_with(".tsx") {
    format!("{}.d.ts", &path[..path.len() - 4])
  } else if path.ends_with(".ts") {
    format!("{}.d.ts", &path[..path.len() - 3])
  } else if path.ends_with(".mts") {
    format!("{}.d.mts", &path[..path.len() - 4])
  } else if path.ends_with(".cts") {
    format!("{}.d.cts", &path[..path.len() - 4])
  } else if path.ends_with(".js") {
    format!("{}.d.ts", &path[..path.len() - 3])
  } else if path.ends_with(".mjs") {
    format!("{}.d.mts", &path[..path.len() - 4])
  } else {
    format!("{}.d.ts", path)
  }
}

/// Convert a JavaScript path to its declaration file equivalent.
/// `.js` → `.d.ts`, `.mjs` → `.d.mts`, `.cjs` → `.d.cts`
pub fn js_to_dts_extension(path: &str) -> String {
  if path.ends_with(".mjs") {
    format!("{}.d.mts", &path[..path.len() - 4])
  } else if path.ends_with(".cjs") {
    format!("{}.d.cts", &path[..path.len() - 4])
  } else if path.ends_with(".js") {
    format!("{}.d.ts", &path[..path.len() - 3])
  } else {
    format!("{}.d.ts", path)
  }
}

/// Compute the output path for a file, replacing its extension.
pub fn compute_output_path(relative_path: &str, new_ext: &str) -> String {
  let path = Path::new(relative_path);
  let stem = path.file_stem().unwrap().to_str().unwrap();
  let parent = path.parent().unwrap_or(Path::new(""));

  if parent == Path::new("") {
    format!("{}{}", stem, new_ext)
  } else {
    format!("{}/{}{}", parent.display(), stem, new_ext)
  }
}

/// Get the output extension for a given media type.
pub fn media_type_extension(media_type: MediaType) -> &'static str {
  match media_type {
    MediaType::JavaScript => ".js",
    MediaType::Jsx => ".jsx",
    MediaType::Mjs => ".mjs",
    MediaType::Cjs => ".cjs",
    MediaType::Json => ".json",
    _ => ".js",
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_ts_to_js_extension() {
    assert_eq!(ts_to_js_extension("mod.ts"), "mod.js");
    assert_eq!(ts_to_js_extension("./mod.ts"), "mod.js");
    assert_eq!(ts_to_js_extension("mod.tsx"), "mod.js");
    assert_eq!(ts_to_js_extension("mod.mts"), "mod.mjs");
    assert_eq!(ts_to_js_extension("mod.cts"), "mod.cjs");
    assert_eq!(ts_to_js_extension("mod.js"), "mod.js");
  }

  #[test]
  fn test_ts_to_dts_extension() {
    assert_eq!(ts_to_dts_extension("mod.ts"), "mod.d.ts");
    assert_eq!(ts_to_dts_extension("./mod.ts"), "mod.d.ts");
    assert_eq!(ts_to_dts_extension("mod.tsx"), "mod.d.ts");
    assert_eq!(ts_to_dts_extension("mod.mts"), "mod.d.mts");
    assert_eq!(ts_to_dts_extension("mod.js"), "mod.d.ts");
  }

  #[test]
  fn test_js_to_dts_extension() {
    assert_eq!(js_to_dts_extension("mod.js"), "mod.d.ts");
    assert_eq!(js_to_dts_extension("mod.mjs"), "mod.d.mts");
    assert_eq!(js_to_dts_extension("mod.cjs"), "mod.d.cts");
  }

  #[test]
  fn test_compute_output_path() {
    assert_eq!(compute_output_path("mod.ts", ".js"), "mod.js");
    assert_eq!(compute_output_path("src/mod.ts", ".js"), "src/mod.js");
    assert_eq!(compute_output_path("mod.ts", ".mjs"), "mod.mjs");
  }
}
