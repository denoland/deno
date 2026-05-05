// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::Path;

use deno_ast::MediaType;

/// Convert a TypeScript path to its JavaScript equivalent.
/// `.ts` → `.js`, `.tsx` → `.js`, `.mts` → `.mjs`, `.cts` → `.cjs`
pub fn ts_to_js_extension(path: &str) -> String {
  let path = path.trim_start_matches("./");
  // Pass through declaration files unchanged
  if path.ends_with(".d.ts")
    || path.ends_with(".d.mts")
    || path.ends_with(".d.cts")
  {
    return path.to_string();
  }
  if let Some(stem) = path.strip_suffix(".tsx") {
    format!("{stem}.js")
  } else if let Some(stem) = path.strip_suffix(".ts") {
    format!("{stem}.js")
  } else if let Some(stem) = path.strip_suffix(".mts") {
    format!("{stem}.mjs")
  } else if let Some(stem) = path.strip_suffix(".cts") {
    format!("{stem}.cjs")
  } else {
    path.to_string()
  }
}

/// Convert a TypeScript path to its declaration file equivalent.
/// `.ts` → `.d.ts`, `.tsx` → `.d.ts`, `.mts` → `.d.mts`, `.cts` → `.d.cts`
pub fn ts_to_dts_extension(path: &str) -> String {
  let path = path.trim_start_matches("./");
  // Already a declaration file - pass through
  if path.ends_with(".d.ts")
    || path.ends_with(".d.mts")
    || path.ends_with(".d.cts")
  {
    return path.to_string();
  }
  if let Some(stem) = path.strip_suffix(".tsx") {
    format!("{stem}.d.ts")
  } else if let Some(stem) = path.strip_suffix(".ts") {
    format!("{stem}.d.ts")
  } else if let Some(stem) = path.strip_suffix(".mts") {
    format!("{stem}.d.mts")
  } else if let Some(stem) = path.strip_suffix(".cts") {
    format!("{stem}.d.cts")
  } else if let Some(stem) = path.strip_suffix(".js") {
    format!("{stem}.d.ts")
  } else if let Some(stem) = path.strip_suffix(".mjs") {
    format!("{stem}.d.mts")
  } else {
    format!("{path}.d.ts")
  }
}

/// Convert a JavaScript path to its declaration file equivalent.
/// `.js` → `.d.ts`, `.mjs` → `.d.mts`, `.cjs` → `.d.cts`
pub fn js_to_dts_extension(path: &str) -> String {
  if let Some(stem) = path.strip_suffix(".mjs") {
    format!("{stem}.d.mts")
  } else if let Some(stem) = path.strip_suffix(".cjs") {
    format!("{stem}.d.cts")
  } else if let Some(stem) = path.strip_suffix(".js") {
    format!("{stem}.d.ts")
  } else {
    format!("{path}.d.ts")
  }
}

/// Compute the output path for a file, replacing its extension. Falls back
/// to string-level extension stripping when the path has no recognizable
/// stem or contains non-UTF8 bytes (rare but possible on Windows).
pub fn compute_output_path(relative_path: &str, new_ext: &str) -> String {
  // Declaration files pass through unchanged -- their extension is
  // meaningful and `file_stem()` would strip only the final `.ts`,
  // turning `foo.d.ts` into `foo.d.js`.
  if relative_path.ends_with(".d.ts")
    || relative_path.ends_with(".d.mts")
    || relative_path.ends_with(".d.cts")
  {
    return relative_path.to_string();
  }

  let path = Path::new(relative_path);
  let parent = path.parent().unwrap_or(Path::new(""));
  let stem = path
    .file_stem()
    .and_then(|s| s.to_str())
    .map(|s| s.to_string())
    .unwrap_or_else(|| {
      // Last-resort: strip after the final '.' in the original string.
      relative_path
        .rsplit_once('.')
        .map(|(s, _)| s.to_string())
        .unwrap_or_else(|| relative_path.to_string())
    });

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
    // Declaration files should pass through unchanged
    assert_eq!(ts_to_js_extension("mod.d.ts"), "mod.d.ts");
    assert_eq!(ts_to_js_extension("mod.d.mts"), "mod.d.mts");
    assert_eq!(ts_to_js_extension("mod.d.cts"), "mod.d.cts");
  }

  #[test]
  fn test_ts_to_dts_extension() {
    assert_eq!(ts_to_dts_extension("mod.ts"), "mod.d.ts");
    assert_eq!(ts_to_dts_extension("./mod.ts"), "mod.d.ts");
    assert_eq!(ts_to_dts_extension("mod.tsx"), "mod.d.ts");
    assert_eq!(ts_to_dts_extension("mod.mts"), "mod.d.mts");
    assert_eq!(ts_to_dts_extension("mod.js"), "mod.d.ts");
    // Already declaration files should pass through
    assert_eq!(ts_to_dts_extension("mod.d.ts"), "mod.d.ts");
    assert_eq!(ts_to_dts_extension("mod.d.mts"), "mod.d.mts");
    assert_eq!(ts_to_dts_extension("mod.d.cts"), "mod.d.cts");
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
    // Declaration files pass through unchanged
    assert_eq!(compute_output_path("mod.d.ts", ".js"), "mod.d.ts");
    assert_eq!(compute_output_path("mod.d.mts", ".mjs"), "mod.d.mts");
    assert_eq!(compute_output_path("mod.d.cts", ".cjs"), "mod.d.cts");
    assert_eq!(
      compute_output_path("src/types.d.ts", ".js"),
      "src/types.d.ts"
    );
  }
}
