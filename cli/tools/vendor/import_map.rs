use std::collections::BTreeMap;

use deno_ast::ModuleSpecifier;
use deno_core::serde_json;
use deno_graph::Module;
use deno_graph::Resolved;
use serde::Serialize;

use super::mappings::Mappings;
use super::specifiers::is_absolute_specifier_text;
use super::specifiers::is_remote_specifier;

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
    serde_json::to_string_pretty(&self.into_serializable()).unwrap()
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

pub fn build_import_map(modules: &[&Module], mappings: &Mappings) -> String {
  let mut import_map = ImportMapBuilder::new(mappings);
  fill_scopes(modules, mappings, &mut import_map);

  for base_specifier in mappings.base_specifiers() {
    import_map
      .imports
      .add(base_specifier.to_string(), base_specifier);
  }

  import_map.into_file_text()
}

fn fill_scopes(
  modules: &[&Module],
  mappings: &Mappings,
  import_map: &mut ImportMapBuilder,
) {
  for module in modules {
    for (text, dep) in &module.dependencies {
      if let Some(specifier) = dep.get_code() {
        handle_dep_specifier(
          import_map,
          &module.specifier,
          text,
          specifier,
          mappings,
        );
      }
      if let Some(specifier) = dep.get_type() {
        handle_dep_specifier(
          import_map,
          &module.specifier,
          text,
          specifier,
          mappings,
        );
      }
    }
    if let Some((text, Resolved::Ok { specifier, .. })) =
      &module.maybe_types_dependency
    {
      handle_dep_specifier(
        import_map,
        &module.specifier,
        text,
        specifier,
        mappings,
      );
    }
  }
}

fn handle_dep_specifier(
  import_map: &mut ImportMapBuilder,
  referrer: &ModuleSpecifier,
  text: &str,
  specifier: &ModuleSpecifier,
  mappings: &Mappings,
) {
  // do not handle specifiers pointing at local modules
  if !is_remote_specifier(specifier) {
    return;
  }

  let base_specifier = mappings.base_specifier(specifier);
  if is_absolute_specifier_text(text) {
    if !text.starts_with(base_specifier.as_str()) {
      panic!("Expected {} to start with {}", text, base_specifier);
    }

    let sub_path = &text[base_specifier.as_str().len()..];
    let expected_relative_specifier_text =
      mappings.relative_path(base_specifier, specifier);
    if expected_relative_specifier_text == sub_path {
      return;
    }
    println!(
      "File system text: {} || Sub path: {}",
      expected_relative_specifier_text, sub_path
    );

    if !is_remote_specifier(referrer) {
      import_map.imports.add(text.to_string(), specifier);
    } else {
      let imports = import_map.scope(base_specifier);
      imports.add(sub_path.to_string(), specifier);
    }
  } else {
    let expected_relative_specifier_text =
      mappings.relative_specifier_text(referrer, specifier);
    if expected_relative_specifier_text == text {
      return;
    }
    // println!("File system text: {} || Actual: {}", file_system_specifier_text, text);

    let imports = import_map.scope(base_specifier);
    // todo: wrong
    imports.add(text.to_string(), specifier);
  }
}
