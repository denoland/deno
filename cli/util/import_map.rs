// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_ast::ParsedSource;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_graph::DefaultModuleAnalyzer;
use deno_graph::DependencyDescriptor;
use deno_graph::DynamicTemplatePart;
use deno_graph::MediaType;
use deno_graph::TypeScriptReference;
use import_map::ImportMap;

use crate::graph_util::format_range_with_colors;

pub struct ImportMapUnfurler<'a> {
  import_map: &'a ImportMap,
}

impl<'a> ImportMapUnfurler<'a> {
  pub fn new(import_map: &'a ImportMap) -> Self {
    Self { import_map }
  }

  pub fn unfurl(
    &self,
    url: &ModuleSpecifier,
    data: Vec<u8>,
  ) -> Result<(Vec<u8>, Vec<String>), AnyError> {
    let mut diagnostics = vec![];
    let media_type = MediaType::from_specifier(url);

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
        return Ok((data, diagnostics));
      }
    }

    let text = String::from_utf8(data)?;
    let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: url.to_string(),
      text_info: deno_ast::SourceTextInfo::from_string(text),
      media_type,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })?;
    let mut text_changes = Vec::new();
    let module_info = DefaultModuleAnalyzer::module_info(&parsed_source);
    let analyze_specifier =
      |specifier: &str,
       range: &deno_graph::PositionRange,
       text_changes: &mut Vec<deno_ast::TextChange>| {
        let resolved = self.import_map.resolve(specifier, url);
        if let Ok(resolved) = resolved {
          text_changes.push(deno_ast::TextChange {
            range: to_range(&parsed_source, range),
            new_text: make_relative_to(url, &resolved),
          });
        }
      };
    for dep in &module_info.dependencies {
      match dep {
        DependencyDescriptor::Static(dep) => {
          analyze_specifier(
            &dep.specifier,
            &dep.specifier_range,
            &mut text_changes,
          );
        }
        DependencyDescriptor::Dynamic(dep) => {
          let success = try_unfurl_dynamic_dep(
            self.import_map,
            url,
            &parsed_source,
            dep,
            &mut text_changes,
          );

          if !success {
            diagnostics.push(
              format!("Dynamic import was not analyzable and won't use the import map once published.\n    at {}",
                format_range_with_colors(&deno_graph::Range {
                  specifier: url.clone(),
                  start: dep.range.start.clone(),
                  end: dep.range.end.clone(),
                })
              )
            );
          }
        }
      }
    }
    for ts_ref in &module_info.ts_references {
      let specifier_with_range = match ts_ref {
        TypeScriptReference::Path(range) => range,
        TypeScriptReference::Types(range) => range,
      };
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        &mut text_changes,
      );
    }
    for specifier_with_range in &module_info.jsdoc_imports {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        &mut text_changes,
      );
    }
    if let Some(specifier_with_range) = &module_info.jsx_import_source {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
        &mut text_changes,
      );
    }
    Ok((
      deno_ast::apply_text_changes(
        parsed_source.text_info().text_str(),
        text_changes,
      )
      .into_bytes(),
      diagnostics,
    ))
  }

  #[cfg(test)]
  fn unfurl_to_string(
    &self,
    url: &ModuleSpecifier,
    data: Vec<u8>,
  ) -> Result<(String, Vec<String>), AnyError> {
    let (data, diagnostics) = self.unfurl(url, data)?;
    let content = String::from_utf8(data)?;
    Ok((content, diagnostics))
  }
}

fn make_relative_to(from: &ModuleSpecifier, to: &ModuleSpecifier) -> String {
  if to.scheme() == "file" {
    format!("./{}", from.make_relative(to).unwrap())
  } else {
    to.to_string()
  }
}

/// Attempts to unfurl the dynamic dependency returning `true` on success
/// or `false` when the import was not analyzable.
fn try_unfurl_dynamic_dep(
  import_map: &ImportMap,
  module_url: &lsp_types::Url,
  parsed_source: &ParsedSource,
  dep: &deno_graph::DynamicDependencyDescriptor,
  text_changes: &mut Vec<deno_ast::TextChange>,
) -> bool {
  match &dep.argument {
    deno_graph::DynamicArgument::String(value) => {
      let range = to_range(parsed_source, &dep.argument_range);
      let maybe_relative_index =
        parsed_source.text_info().text_str()[range.start..].find(value);
      let Some(relative_index) = maybe_relative_index else {
        return false;
      };
      let resolved = import_map.resolve(value, module_url);
      let Ok(resolved) = resolved else {
        return false;
      };
      let start = range.start + relative_index;
      text_changes.push(deno_ast::TextChange {
        range: start..start + value.len(),
        new_text: make_relative_to(module_url, &resolved),
      });
      true
    }
    deno_graph::DynamicArgument::Template(parts) => match parts.first() {
      Some(DynamicTemplatePart::String { value }) => {
        // relative doesn't need to be modified
        let is_relative = value.starts_with("./") || value.starts_with("../");
        if is_relative {
          return true;
        }
        if !value.ends_with('/') {
          return false;
        }
        let Ok(resolved) = import_map.resolve(value, module_url) else {
          return false;
        };
        let range = to_range(parsed_source, &dep.argument_range);
        let maybe_relative_index =
          parsed_source.text_info().text_str()[range.start..].find(value);
        let Some(relative_index) = maybe_relative_index else {
          return false;
        };
        let start = range.start + relative_index;
        text_changes.push(deno_ast::TextChange {
          range: start..start + value.len(),
          new_text: make_relative_to(module_url, &resolved),
        });
        true
      }
      Some(DynamicTemplatePart::Expr) => {
        false // failed analyzing
      }
      None => {
        true // ignore
      }
    },
    deno_graph::DynamicArgument::Expr => {
      false // failed analyzing
    }
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

#[cfg(test)]
mod tests {
  use super::*;
  use deno_ast::ModuleSpecifier;
  use deno_core::serde_json::json;
  use import_map::ImportMapWithDiagnostics;
  use pretty_assertions::assert_eq;

  #[test]
  fn test_unfurling() {
    let deno_json_url =
      ModuleSpecifier::parse("file:///dev/deno.json").unwrap();
    let value = json!({
      "imports": {
        "express": "npm:express@5",
        "lib/": "./lib/",
        "fizz": "./fizz/mod.ts"
      }
    });
    let ImportMapWithDiagnostics { import_map, .. } =
      import_map::parse_from_value(&deno_json_url, value).unwrap();
    let unfurler = ImportMapUnfurler::new(&import_map);

    // Unfurling TS file should apply changes.
    {
      let source_code = r#"import express from "express";"
import foo from "lib/foo.ts";
import bar from "lib/bar.ts";
import fizz from "fizz";

const test1 = await import("lib/foo.ts");
const test2 = await import(`lib/foo.ts`);
const test3 = await import(`lib/${expr}`);
const test4 = await import(`./lib/${expr}`);
// will warn
const test5 = await import(`lib${expr}`);
const test6 = await import(`${expr}`);
"#;
      let specifier = ModuleSpecifier::parse("file:///dev/mod.ts").unwrap();
      let (unfurled_source, d) = unfurler
        .unfurl_to_string(&specifier, source_code.as_bytes().to_vec())
        .unwrap();
      assert_eq!(d.len(), 2);
      assert!(d[0].starts_with("Dynamic import was not analyzable and won't use the import map once published."));
      assert!(d[1].starts_with("Dynamic import was not analyzable and won't use the import map once published."));
      let expected_source = r#"import express from "npm:express@5";"
import foo from "./lib/foo.ts";
import bar from "./lib/bar.ts";
import fizz from "./fizz/mod.ts";

const test1 = await import("./lib/foo.ts");
const test2 = await import(`./lib/foo.ts`);
const test3 = await import(`./lib/${expr}`);
const test4 = await import(`./lib/${expr}`);
// will warn
const test5 = await import(`lib${expr}`);
const test6 = await import(`${expr}`);
"#;
      assert_eq!(unfurled_source, expected_source);
    }

    // Unfurling file with "unknown" media type should leave it as is
    {
      let source_code = r#"import express from "express";"
import foo from "lib/foo.ts";
import bar from "lib/bar.ts";
import fizz from "fizz";
"#;
      let specifier = ModuleSpecifier::parse("file:///dev/mod").unwrap();
      let (unfurled_source, d) = unfurler
        .unfurl_to_string(&specifier, source_code.as_bytes().to_vec())
        .unwrap();
      assert!(d.is_empty());
      assert_eq!(unfurled_source, source_code);
    }
  }
}
