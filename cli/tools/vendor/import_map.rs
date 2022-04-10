// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::collections::BTreeMap;

use deno_ast::LineAndColumnIndex;
use deno_ast::ModuleSpecifier;
use deno_ast::SourceTextInfo;
use deno_core::serde_json;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::Position;
use deno_graph::Range;
use deno_graph::Resolved;
use serde::Serialize;

use super::mappings::Mappings;
use super::specifiers::is_remote_specifier;
use super::specifiers::is_remote_specifier_text;

#[derive(Serialize)]
struct SerializableImportMap {
  imports: BTreeMap<String, String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  scopes: BTreeMap<String, BTreeMap<String, String>>,
}

struct ImportMapBuilder<'a> {
  mappings: &'a Mappings,
  imports: ImportsBuilder<'a>,
  scopes: BTreeMap<String, ImportsBuilder<'a>>,
}

impl<'a> ImportMapBuilder<'a> {
  pub fn new(mappings: &'a Mappings) -> Self {
    ImportMapBuilder {
      mappings,
      imports: ImportsBuilder::new(mappings),
      scopes: Default::default(),
    }
  }

  pub fn scope(
    &mut self,
    base_specifier: &ModuleSpecifier,
  ) -> &mut ImportsBuilder<'a> {
    self
      .scopes
      .entry(
        self
          .mappings
          .relative_specifier_text(self.mappings.output_dir(), base_specifier),
      )
      .or_insert_with(|| ImportsBuilder::new(self.mappings))
  }

  pub fn into_serializable(self) -> SerializableImportMap {
    SerializableImportMap {
      imports: self.imports.imports,
      scopes: self
        .scopes
        .into_iter()
        .map(|(key, value)| (key, value.imports))
        .collect(),
    }
  }

  pub fn into_file_text(self) -> String {
    let mut text =
      serde_json::to_string_pretty(&self.into_serializable()).unwrap();
    text.push('\n');
    text
  }
}

struct ImportsBuilder<'a> {
  mappings: &'a Mappings,
  imports: BTreeMap<String, String>,
}

impl<'a> ImportsBuilder<'a> {
  pub fn new(mappings: &'a Mappings) -> Self {
    Self {
      mappings,
      imports: Default::default(),
    }
  }

  pub fn add(&mut self, key: String, specifier: &ModuleSpecifier) {
    self.imports.insert(
      key,
      self
        .mappings
        .relative_specifier_text(self.mappings.output_dir(), specifier),
    );
  }
}

pub fn build_import_map(
  graph: &ModuleGraph,
  modules: &[&Module],
  mappings: &Mappings,
) -> String {
  let mut import_map = ImportMapBuilder::new(mappings);
  visit_modules(graph, modules, mappings, &mut import_map);

  for base_specifier in mappings.base_specifiers() {
    import_map
      .imports
      .add(base_specifier.to_string(), base_specifier);
  }

  import_map.into_file_text()
}

fn visit_modules(
  graph: &ModuleGraph,
  modules: &[&Module],
  mappings: &Mappings,
  import_map: &mut ImportMapBuilder,
) {
  for module in modules {
    let text_info = match &module.maybe_parsed_source {
      Some(source) => source.source(),
      None => continue,
    };
    let source_text = match &module.maybe_source {
      Some(source) => source,
      None => continue,
    };

    for dep in module.dependencies.values() {
      visit_maybe_resolved(
        &dep.maybe_code,
        graph,
        import_map,
        &module.specifier,
        mappings,
        text_info,
        source_text,
      );
      visit_maybe_resolved(
        &dep.maybe_type,
        graph,
        import_map,
        &module.specifier,
        mappings,
        text_info,
        source_text,
      );
    }

    if let Some((_, maybe_resolved)) = &module.maybe_types_dependency {
      visit_maybe_resolved(
        maybe_resolved,
        graph,
        import_map,
        &module.specifier,
        mappings,
        text_info,
        source_text,
      );
    }
  }
}

fn visit_maybe_resolved(
  maybe_resolved: &Resolved,
  graph: &ModuleGraph,
  import_map: &mut ImportMapBuilder,
  referrer: &ModuleSpecifier,
  mappings: &Mappings,
  text_info: &SourceTextInfo,
  source_text: &str,
) {
  if let Resolved::Ok {
    specifier, range, ..
  } = maybe_resolved
  {
    let text = text_from_range(text_info, source_text, range);
    // if the text is empty then it's probably an x-TypeScript-types
    if !text.is_empty() {
      handle_dep_specifier(
        text, specifier, graph, import_map, referrer, mappings,
      );
    }
  }
}

fn handle_dep_specifier(
  text: &str,
  unresolved_specifier: &ModuleSpecifier,
  graph: &ModuleGraph,
  import_map: &mut ImportMapBuilder,
  referrer: &ModuleSpecifier,
  mappings: &Mappings,
) {
  let specifier = graph.resolve(unresolved_specifier);
  // do not handle specifiers pointing at local modules
  if !is_remote_specifier(&specifier) {
    return;
  }

  let base_specifier = mappings.base_specifier(&specifier);
  if is_remote_specifier_text(text) {
    if !text.starts_with(base_specifier.as_str()) {
      panic!("Expected {} to start with {}", text, base_specifier);
    }

    let sub_path = &text[base_specifier.as_str().len()..];
    let expected_relative_specifier_text =
      mappings.relative_path(base_specifier, &specifier);
    if expected_relative_specifier_text == sub_path {
      return;
    }

    import_map.imports.add(text.to_string(), &specifier);
  } else {
    let expected_relative_specifier_text =
      mappings.relative_specifier_text(referrer, &specifier);
    if expected_relative_specifier_text == text {
      return;
    }

    let key = if text.starts_with("./") || text.starts_with("../") {
      // resolve relative specifier key
      let mut local_base_specifier = mappings.local_uri(base_specifier);
      local_base_specifier.set_query(unresolved_specifier.query());
      local_base_specifier = local_base_specifier
        .join(&unresolved_specifier.path()[1..])
        .unwrap_or_else(|_| {
          panic!(
            "Error joining {} to {}",
            unresolved_specifier.path(),
            local_base_specifier
          )
        });
      local_base_specifier.set_query(unresolved_specifier.query());
      mappings
        .relative_specifier_text(mappings.output_dir(), &local_base_specifier)
    } else {
      // absolute (`/`) or bare specifier should be left as-is
      text.to_string()
    };
    let imports = import_map.scope(base_specifier);
    imports.add(key, &specifier);
  }
}

fn text_from_range<'a>(
  text_info: &SourceTextInfo,
  text: &'a str,
  range: &Range,
) -> &'a str {
  let result = &text[byte_range(text_info, range)];
  if result.starts_with('"') || result.starts_with('\'') {
    // remove the quotes
    &result[1..result.len() - 1]
  } else {
    result
  }
}

fn byte_range(
  text_info: &SourceTextInfo,
  range: &Range,
) -> std::ops::Range<usize> {
  let start = byte_index(text_info, &range.start);
  let end = byte_index(text_info, &range.end);
  start..end
}

fn byte_index(text_info: &SourceTextInfo, pos: &Position) -> usize {
  // todo(https://github.com/denoland/deno_graph/issues/79): use byte indexes all the way down
  text_info
    .byte_index(LineAndColumnIndex {
      line_index: pos.line,
      column_index: pos.character,
    })
    .0 as usize
}
