// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_ast::LineAndColumnIndex;
use deno_ast::ModuleSpecifier;
use deno_ast::SourceTextInfo;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::Position;
use deno_graph::Range;
use deno_graph::Resolved;
use import_map::ImportMap;
use indexmap::IndexMap;

use super::mappings::Mappings;
use super::specifiers::is_remote_specifier;
use super::specifiers::is_remote_specifier_text;

struct ImportMapBuilder<'a> {
  base: &'a ModuleSpecifier,
  mappings: &'a Mappings,
  imports: ImportsBuilder<'a>,
  scopes: IndexMap<String, ImportsBuilder<'a>>,
}

impl<'a> ImportMapBuilder<'a> {
  pub fn new(base: &'a ModuleSpecifier, mappings: &'a Mappings) -> Self {
    ImportMapBuilder {
      base,
      mappings,
      imports: ImportsBuilder::new(base, mappings),
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
          .relative_specifier_text(self.base, base_specifier),
      )
      .or_insert_with(|| ImportsBuilder::new(self.base, self.mappings))
  }

  pub fn into_import_map(
    self,
    original_import_map: Option<ImportMap>,
  ) -> ImportMap {
    let mut import_map =
      original_import_map.unwrap_or_else(|| ImportMap::new(self.base.clone()));

    let imports = import_map.imports_mut();
    for (key, value) in self.imports.imports {
      if !imports.contains(&key) {
        imports.append(key, value).unwrap();
      }
    }

    for (scope_key, scope_value) in self.scopes {
      let imports = import_map.get_or_append_scope_mut(&scope_key).unwrap();
      for (key, value) in scope_value.imports {
        if !imports.contains(&key) {
          imports.append(key, value).unwrap();
        }
      }
    }

    import_map
  }
}

struct ImportsBuilder<'a> {
  base: &'a ModuleSpecifier,
  mappings: &'a Mappings,
  imports: IndexMap<String, String>,
}

impl<'a> ImportsBuilder<'a> {
  pub fn new(base: &'a ModuleSpecifier, mappings: &'a Mappings) -> Self {
    Self {
      base,
      mappings,
      imports: Default::default(),
    }
  }

  pub fn add(&mut self, key: String, specifier: &ModuleSpecifier) {
    let value = self.mappings.relative_specifier_text(self.base, specifier);

    // skip creating identity entries
    if key != value {
      self.imports.insert(key, value);
    }
  }
}

pub fn build_import_map(
  base: &ModuleSpecifier,
  graph: &ModuleGraph,
  modules: &[&Module],
  mappings: &Mappings,
  original_import_map: Option<ImportMap>,
) -> String {
  let mut builder = ImportMapBuilder::new(base, mappings);
  visit_modules(graph, modules, mappings, &mut builder);

  builder.into_import_map(original_import_map).to_json()
}

fn visit_modules(
  graph: &ModuleGraph,
  modules: &[&Module],
  mappings: &Mappings,
  import_map: &mut ImportMapBuilder,
) {
  for module in modules {
    let text_info = match &module.maybe_parsed_source {
      Some(source) => source.text_info(),
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

  // add an entry for every local module referrencing a remote
  if !is_remote_specifier(&referrer) {
    import_map.imports.add(text.to_string(), &specifier);
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

    let imports = import_map.scope(base_specifier);
    imports.add(text.to_string(), &specifier);
  } else {
    let expected_relative_specifier_text =
      mappings.relative_specifier_text(referrer, &specifier);
    if expected_relative_specifier_text == text {
      return;
    }

    let imports = import_map.scope(base_specifier);
    if text.starts_with("./") || text.starts_with("../") {
      // resolve relative specifier key
      let mut local_base_specifier = mappings.local_uri(base_specifier);
      local_base_specifier.set_query(unresolved_specifier.query());
      local_base_specifier = local_base_specifier
        // path includes "/" so make it relative
        .join(&format!(".{}", unresolved_specifier.path()))
        .unwrap_or_else(|_| {
          panic!(
            "Error joining {} to {}",
            unresolved_specifier.path(),
            local_base_specifier
          )
        });
      local_base_specifier.set_query(unresolved_specifier.query());

      imports.add(
        mappings.relative_specifier_text(
          mappings.output_dir(),
          &local_base_specifier,
        ),
        &specifier,
      );

      // add a mapping that uses the local directory name and the remote
      // filename in order to support files importing this relatively
      imports.add(
        {
          let local_path = mappings.local_path(&specifier);
          let mut value =
            ModuleSpecifier::from_directory_path(local_path.parent().unwrap())
              .unwrap();
          value.set_query(specifier.query());
          value.set_path(&format!(
            "{}{}",
            value.path(),
            specifier.path_segments().unwrap().last().unwrap(),
          ));
          mappings.relative_specifier_text(mappings.output_dir(), &value)
        },
        &specifier,
      );
    } else {
      // absolute (`/`) or bare specifier should be left as-is
      imports.add(text.to_string(), &specifier);
    }
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
  text_info.loc_to_source_pos(LineAndColumnIndex {
    line_index: pos.line,
    column_index: pos.character,
  }) - text_info.range().start
}
