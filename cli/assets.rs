static DENO_RUNTIME: &str = include_str!("../js/lib.deno_runtime.d.ts");

pub fn get_source_code(name: &str) -> Option<&'static str> {
  match name {
    "lib.deno_runtime.d.ts" => Some(DENO_RUNTIME),
    _ => deno_typescript::get_asset(name),
  }
}
