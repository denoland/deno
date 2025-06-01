// Copyright 2018-2025 the Deno authors. MIT license.

#[cfg(not(feature = "include_js_files_for_snapshotting"))]
pub static SOURCE_CODE_FOR_99_MAIN_JS: &str = include_str!("js/99_main.js");

#[cfg(feature = "include_js_files_for_snapshotting")]
pub static PATH_FOR_99_MAIN_JS: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/js/99_main.js");
