use deno_core::op;
use deno_core::v8;

pub const V8_ES_SHIM: &str = include_str!("00_v8.js");

#[op]
fn op_v8_cached_data_version_tag() -> u32 {
  v8::script_compiler::cached_data_version_tag()
}

#[op]
fn op_v8_set_flags_from_string(flags: String) {
  v8::V8::set_flags_from_string(&flags);
}

pub fn ops() -> Vec<deno_core::OpDecl> {
  vec![
    op_v8_cached_data_version_tag::decl(),
    op_v8_set_flags_from_string::decl(),
  ]
}
