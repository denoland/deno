use deno_typescript::ImportMap;
use std::path::PathBuf;

pub static DENO_OPS_FS_MAIN_PATH: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/main.ts");

pub fn get_import_map() -> ImportMap {
  let mut import_maps: Vec<ImportMap> = vec![vec![(
    "deno_ops_fs".to_string(),
    PathBuf::from(DENO_OPS_FS_MAIN_PATH),
  )]];
  import_maps.push(deno_dispatch_json_bundle::get_import_map());
  import_maps.push(deno_bundle_util::get_import_map());
  deno_typescript::merge_import_maps(import_maps)
}
