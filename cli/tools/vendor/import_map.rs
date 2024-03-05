// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::LineAndColumnIndex;
use deno_ast::ModuleSpecifier;
use deno_ast::SourceTextInfo;
use deno_core::error::AnyError;
use deno_graph::source::ResolutionMode;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::Position;
use deno_graph::Range;
use deno_graph::Resolution;
use import_map::ImportMap;
use import_map::SpecifierMap;
use indexmap::IndexMap;
use log::warn;

use crate::args::JsxImportSourceConfig;
use crate::cache::ParsedSourceCache;

use super::mappings::Mappings;
use super::specifiers::is_remote_specifier;
use super::specifiers::is_remote_specifier_text;

struct ImportMapBuilder<'a> {
  base_dir: &'a ModuleSpecifier,
  mappings: &'a Mappings,
  imports: ImportsBuilder<'a>,
  scopes: IndexMap<String, ImportsBuilder<'a>>,
}

impl<'a> ImportMapBuilder<'a> {
  pub fn new(base_dir: &'a ModuleSpecifier, mappings: &'a Mappings) -> Self {
    ImportMapBuilder {
      base_dir,
      mappings,
      imports: ImportsBuilder::new(base_dir, mappings),
      scopes: Default::default(),
    }
  }

  pub fn base_dir(&self) -> &ModuleSpecifier {
    self.base_dir
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
          .relative_specifier_text(self.base_dir, base_specifier),
      )
      .or_insert_with(|| ImportsBuilder::new(self.base_dir, self.mappings))
  }

  pub fn into_import_map(
    self,
    original_import_map: Option<&ImportMap>,
  ) -> ImportMap {
    fn get_local_imports(
      new_relative_path: &str,
      original_imports: &SpecifierMap,
    ) -> Vec<(String, String)> {
      let mut result = Vec::new();
      for entry in original_imports.entries() {
        if let Some(raw_value) = entry.raw_value {
          if raw_value.starts_with("./") || raw_value.starts_with("../") {
            let sub_index = raw_value.find('/').unwrap() + 1;
            result.push((
              entry.raw_key.to_string(),
              format!("{}{}", new_relative_path, &raw_value[sub_index..]),
            ));
          }
        }
      }
      result
    }

    fn add_local_imports<'a>(
      new_relative_path: &str,
      original_imports: &SpecifierMap,
      get_new_imports: impl FnOnce() -> &'a mut SpecifierMap,
    ) {
      let local_imports =
        get_local_imports(new_relative_path, original_imports);
      if !local_imports.is_empty() {
        let new_imports = get_new_imports();
        for (key, value) in local_imports {
          if let Err(warning) = new_imports.append(key, value) {
            warn!("{}", warning);
          }
        }
      }
    }

    let mut import_map = ImportMap::new(self.base_dir.clone());

    if let Some(original_im) = original_import_map {
      let original_base_dir = ModuleSpecifier::from_directory_path(
        original_im
          .base_url()
          .to_file_path()
          .unwrap()
          .parent()
          .unwrap(),
      )
      .unwrap();
      let new_relative_path = self
        .mappings
        .relative_specifier_text(self.base_dir, &original_base_dir);
      // add the imports
      add_local_imports(&new_relative_path, original_im.imports(), || {
        import_map.imports_mut()
      });

      for scope in original_im.scopes() {
        if scope.raw_key.starts_with("./") || scope.raw_key.starts_with("../") {
          let sub_index = scope.raw_key.find('/').unwrap() + 1;
          let new_key =
            format!("{}{}", new_relative_path, &scope.raw_key[sub_index..]);
          add_local_imports(&new_relative_path, scope.imports, || {
            import_map.get_or_append_scope_mut(&new_key).unwrap()
          });
        }
      }
    }

    let imports = import_map.imports_mut();
    for (key, value) in self.imports.imports {
      if !imports.contains(&key) {
        imports.append(key, value).unwrap();
      }
    }

    for (scope_key, scope_value) in self.scopes {
      if !scope_value.imports.is_empty() {
        let imports = import_map.get_or_append_scope_mut(&scope_key).unwrap();
        for (key, value) in scope_value.imports {
          if !imports.contains(&key) {
            imports.append(key, value).unwrap();
          }
        }
      }
    }

    import_map
  }
}

struct ImportsBuilder<'a> {
  base_dir: &'a ModuleSpecifier,
  mappings: &'a Mappings,
  imports: IndexMap<String, String>,
}

impl<'a> ImportsBuilder<'a> {
  pub fn new(base_dir: &'a ModuleSpecifier, mappings: &'a Mappings) -> Self {
    Self {
      base_dir,
      mappings,
      imports: Default::default(),
    }
  }

  pub fn add(&mut self, key: String, specifier: &ModuleSpecifier) {
    let value = self
      .mappings
      .relative_specifier_text(self.base_dir, specifier);

    // skip creating identity entries
    if key != value {
      self.imports.insert(key, value);
    }
  }
}

pub struct BuildImportMapInput<'a> {
  pub base_dir: &'a ModuleSpecifier,
  pub modules: &'a [&'a Module],
  pub graph: &'a ModuleGraph,
  pub mappings: &'a Mappings,
  pub original_import_map: Option<&'a ImportMap>,
  pub jsx_import_source: Option<&'a JsxImportSourceConfig>,
  pub resolver: &'a dyn deno_graph::source::Resolver,
  pub parsed_source_cache: &'a ParsedSourceCache,
}

pub fn build_import_map(
  input: BuildImportMapInput<'_>,
) -> Result<String, AnyError> {
  let BuildImportMapInput {
    base_dir,
    modules,
    graph,
    mappings,
    original_import_map,
    jsx_import_source,
    resolver,
    parsed_source_cache,
  } = input;
  let mut builder = ImportMapBuilder::new(base_dir, mappings);
  visit_modules(graph, modules, mappings, &mut builder, parsed_source_cache)?;

  for base_specifier in mappings.base_specifiers() {
    builder
      .imports
      .add(base_specifier.to_string(), base_specifier);
  }

  // add the jsx import source to the destination import map, if mapped in the original import map
  if let Some(jsx_import_source) = jsx_import_source {
    if let Some(specifier_text) = jsx_import_source.maybe_specifier_text() {
      if let Ok(resolved_url) = resolver.resolve(
        &specifier_text,
        &deno_graph::Range {
          specifier: jsx_import_source.base_url.clone(),
          start: deno_graph::Position::zeroed(),
          end: deno_graph::Position::zeroed(),
        },
        ResolutionMode::Execution,
      ) {
        builder.imports.add(specifier_text, &resolved_url);
      }
    }
  }

  Ok(builder.into_import_map(original_import_map).to_json())
}

fn visit_modules(
  graph: &ModuleGraph,
  modules: &[&Module],
  mappings: &Mappings,
  import_map: &mut ImportMapBuilder,
  parsed_source_cache: &ParsedSourceCache,
) -> Result<(), AnyError> {
  for module in modules {
    let module = match module {
      Module::Js(module) => module,
      // skip visiting Json modules as they are leaves
      Module::Json(_)
      | Module::Npm(_)
      | Module::Node(_)
      | Module::External(_) => continue,
    };

    let parsed_source =
      parsed_source_cache.get_parsed_source_from_js_module(module)?;
    let text_info = parsed_source.text_info().clone();

    for dep in module.dependencies.values() {
      visit_resolution(
        &dep.maybe_code,
        graph,
        import_map,
        &module.specifier,
        mappings,
        &text_info,
        &module.source,
      );
      visit_resolution(
        &dep.maybe_type,
        graph,
        import_map,
        &module.specifier,
        mappings,
        &text_info,
        &module.source,
      );
    }

    if let Some(types_dep) = &module.maybe_types_dependency {
      visit_resolution(
        &types_dep.dependency,
        graph,
        import_map,
        &module.specifier,
        mappings,
        &text_info,
        &module.source,
      );
    }
  }

  Ok(())
}

fn visit_resolution(
  resolution: &Resolution,
  graph: &ModuleGraph,
  import_map: &mut ImportMapBuilder,
  referrer: &ModuleSpecifier,
  mappings: &Mappings,
  text_info: &SourceTextInfo,
  source_text: &str,
) {
  if let Some(resolved) = resolution.ok() {
    let text = text_from_range(text_info, source_text, &resolved.range);
    // if the text is empty then it's probably an x-TypeScript-types
    if !text.is_empty() {
      handle_dep_specifier(
        text,
        &resolved.specifier,
        graph,
        import_map,
        referrer,
        mappings,
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
  let specifier = match graph.get(unresolved_specifier) {
    Some(module) => module.specifier().clone(),
    // Ignore when None. The graph was previous validated so this is a
    // dynamic import that was missing and is ignored for vendoring
    None => return,
  };
  // check if it's referencing a remote module
  if is_remote_specifier(&specifier) {
    handle_remote_dep_specifier(
      text,
      unresolved_specifier,
      &specifier,
      import_map,
      referrer,
      mappings,
    )
  } else if specifier.scheme() == "file" {
    handle_local_dep_specifier(
      text,
      unresolved_specifier,
      &specifier,
      import_map,
      referrer,
      mappings,
    );
  }
}

fn handle_remote_dep_specifier(
  text: &str,
  unresolved_specifier: &ModuleSpecifier,
  specifier: &ModuleSpecifier,
  import_map: &mut ImportMapBuilder,
  referrer: &ModuleSpecifier,
  mappings: &Mappings,
) {
  if is_remote_specifier_text(text) {
    let base_specifier = mappings.base_specifier(specifier);
    if text.starts_with(base_specifier.as_str()) {
      let sub_path = &text[base_specifier.as_str().len()..];
      let relative_text =
        mappings.relative_specifier_text(base_specifier, specifier);
      let expected_sub_path = relative_text.trim_start_matches("./");
      if expected_sub_path != sub_path {
        import_map.imports.add(text.to_string(), specifier);
      }
    } else {
      // it's probably a redirect. Add it explicitly to the import map
      import_map.imports.add(text.to_string(), specifier);
    }
  } else {
    let expected_relative_specifier_text =
      mappings.relative_specifier_text(referrer, specifier);
    if expected_relative_specifier_text == text {
      return;
    }

    if !is_remote_specifier(referrer) {
      // local module referencing a remote module using
      // non-remote specifier text means it was something in
      // the original import map, so add a mapping to it
      import_map.imports.add(text.to_string(), specifier);
      return;
    }

    let base_referrer = mappings.base_specifier(referrer);
    let base_dir = import_map.base_dir().clone();
    let imports = import_map.scope(base_referrer);
    if text.starts_with("./") || text.starts_with("../") {
      // resolve relative specifier key
      let mut local_base_specifier = mappings.local_uri(base_referrer);
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
        mappings.relative_specifier_text(&base_dir, &local_base_specifier),
        specifier,
      );

      // add a mapping that uses the local directory name and the remote
      // filename in order to support files importing this relatively
      imports.add(
        {
          let local_path = mappings.local_path(specifier);
          let mut value =
            ModuleSpecifier::from_directory_path(local_path.parent().unwrap())
              .unwrap();
          value.set_query(specifier.query());
          value.set_path(&format!(
            "{}{}",
            value.path(),
            specifier.path_segments().unwrap().last().unwrap(),
          ));
          mappings.relative_specifier_text(&base_dir, &value)
        },
        specifier,
      );
    } else {
      // absolute (`/`) or bare specifier should be left as-is
      imports.add(text.to_string(), specifier);
    }
  }
}

fn handle_local_dep_specifier(
  text: &str,
  unresolved_specifier: &ModuleSpecifier,
  specifier: &ModuleSpecifier,
  import_map: &mut ImportMapBuilder,
  referrer: &ModuleSpecifier,
  mappings: &Mappings,
) {
  if !is_remote_specifier(referrer) {
    // do not handle local modules referencing local modules
    return;
  }

  // The remote module is referencing a local file. This could occur via an
  // existing import map. In this case, we'll have to add an import map
  // entry in order to map the path back to the local path once vendored.
  let base_dir = import_map.base_dir().clone();
  let base_specifier = mappings.base_specifier(referrer);
  let imports = import_map.scope(base_specifier);

  if text.starts_with("./") || text.starts_with("../") {
    let referrer_local_uri = mappings.local_uri(referrer);
    let mut specifier_local_uri =
      referrer_local_uri.join(text).unwrap_or_else(|_| {
        panic!(
          "Error joining {} to {}",
          unresolved_specifier.path(),
          referrer_local_uri
        )
      });
    specifier_local_uri.set_query(unresolved_specifier.query());

    imports.add(
      mappings.relative_specifier_text(&base_dir, &specifier_local_uri),
      specifier,
    );
  } else {
    imports.add(text.to_string(), specifier);
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
