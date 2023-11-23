// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_ast::ParsedSource;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::DefaultModuleAnalyzer;
use deno_graph::MediaType;
use deno_graph::TypeScriptReference;
use import_map::ImportMap;

pub struct ImportMapUnfurler {
  import_map: ImportMap,
}

impl ImportMapUnfurler {
  pub fn new(import_map: ImportMap) -> Self {
    Self { import_map }
  }

  pub fn unfurl(
    &self,
    specifier: String,
    data: Vec<u8>,
  ) -> Result<Vec<u8>, AnyError> {
    let url = Url::parse(&specifier)?;
    let media_type = MediaType::from_specifier(&url);

    match media_type {
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx => {
        // continue
      }
      MediaType::SourceMap
      | MediaType::Unknown
      | MediaType::Json
      | MediaType::Wasm
      | MediaType::TsBuildInfo => {
        // not unfurlable data
        return Ok(data);
      }
    }

    let text = String::from_utf8(data)?;
    let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
      specifier,
      text_info: deno_ast::SourceTextInfo::from_string(text),
      media_type,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })?;
    let mut text_changes = Vec::new();
    let module_info = DefaultModuleAnalyzer::module_info(&parsed_source);
    let mut analyze_specifier =
      |specifier: &str, range: &deno_graph::PositionRange| {
        let resolved = self.import_map.resolve(specifier, &url);
        if let Ok(resolved) = resolved {
          let new_text = if resolved.scheme() == "file" {
            format!("./{}", url.make_relative(&resolved).unwrap())
          } else {
            resolved.to_string()
          };
          text_changes.push(deno_ast::TextChange {
            range: to_range(&parsed_source, range),
            new_text,
          });
        }
      };
    for dep in &module_info.dependencies {
      analyze_specifier(&dep.specifier, &dep.specifier_range);
    }
    for ts_ref in &module_info.ts_references {
      let specifier_with_range = match ts_ref {
        TypeScriptReference::Path(range) => range,
        TypeScriptReference::Types(range) => range,
      };
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
      );
    }
    for specifier_with_range in &module_info.jsdoc_imports {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
      );
    }
    if let Some(specifier_with_range) = &module_info.jsx_import_source {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
      );
    }
    Ok(
      deno_ast::apply_text_changes(
        parsed_source.text_info().text_str(),
        text_changes,
      )
      .into_bytes(),
    )
  }
}

fn to_range(
  parsed_source: &ParsedSource,
  range: &deno_graph::PositionRange,
) -> std::ops::Range<usize> {
  let mut range = range
    .as_source_range(parsed_source.text_info())
    .as_byte_range(parsed_source.text_info().range().start);
  let text = &parsed_source.text_info().text_str()[range.clone()];
  if text.starts_with('"') || text.starts_with('\'') {
    range.start += 1;
  }
  if text.ends_with('"') || text.ends_with('\'') {
    range.end -= 1;
  }
  range
}
