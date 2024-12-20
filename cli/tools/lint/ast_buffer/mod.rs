use deno_ast::ParsedSource;
use swc::serialize_swc_to_buffer;

mod buffer;
mod swc;
mod ts_estree;

pub fn serialize_ast_to_buffer(parsed_source: &ParsedSource) -> Vec<u8> {
  // TODO: We could support multiple languages here
  eprintln!("parsed source {:#?}", parsed_source);
  serialize_swc_to_buffer(parsed_source)
}
