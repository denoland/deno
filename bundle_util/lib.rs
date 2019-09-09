use deno_typescript::ImportMap;
use std::path::PathBuf;

pub static DENO_BUNDLE_UTIL_MAIN_PATH: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/main.ts");

pub fn get_import_map() -> ImportMap {
  vec![(
    "deno_util".to_string(),
    PathBuf::from(DENO_BUNDLE_UTIL_MAIN_PATH),
  )]
}
