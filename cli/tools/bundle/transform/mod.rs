use deno_ast::{
  parse_module, swc::visit::FoldWith, ParseParams, ParsedSource, SourceMap,
};

use super::bundle_graph::{BundleJsModule, BundleModule};

pub fn transform_bundle(module: &BundleModule) {
  //

  match module {
    BundleModule::Js(module) => {
      let parsed_source = parse_module(ParseParams {
        specifier: module.specifier.clone(),
        media_type: module.media_type,
        capture_tokens: false,
        maybe_syntax: None,
        scope_analysis: false,
        text: module.source.clone().into(),
      })
      .unwrap();

      // TODO: optional
      let source_map =
        SourceMap::single(module.specifier.clone(), module.source.clone());

      let program = parsed_source.program();

      // Transpile
    }

    // FIXME
    BundleModule::Json(_json_module) => todo!(),
    BundleModule::Wasm(_wasm_module) => {}
    BundleModule::Node(_) | BundleModule::External(_) => {}
  }
}
