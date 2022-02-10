use std::path::Path;

use deno_core::error::AnyError;
use deno_graph::ModuleGraph;
use deno_graph::ModuleKind;

use super::import_map::build_import_map;
use super::mappings::Mappings;
use super::specifiers::is_remote_specifier;

pub trait VendorEnvironment {
  fn create_dir_all(&self, dir_path: &Path) -> Result<(), AnyError>;
  fn write_file(&self, file_path: &Path, text: &str) -> Result<(), AnyError>;
}

pub struct RealVendorEnvironment;

impl VendorEnvironment for RealVendorEnvironment {
  fn create_dir_all(&self, dir_path: &Path) -> Result<(), AnyError> {
    Ok(std::fs::create_dir_all(dir_path)?)
  }

  fn write_file(&self, file_path: &Path, text: &str) -> Result<(), AnyError> {
    Ok(std::fs::write(file_path, text)?)
  }
}

pub fn build(
  graph: &ModuleGraph,
  output_dir: &Path,
  environment: &impl VendorEnvironment,
) -> Result<(), AnyError> {
  let all_modules = graph.modules();
  let remote_modules = all_modules
    .iter()
    .filter(|m| is_remote_specifier(&m.specifier))
    .copied()
    .collect::<Vec<_>>();
  let mappings =
    Mappings::from_remote_modules(graph, &remote_modules, output_dir)?;

  environment.create_dir_all(output_dir)?;

  // collect and write out all the text changes
  for module in &remote_modules {
    let source = match &module.maybe_source {
      Some(source) => source,
      None => continue,
    };
    let local_path = mappings.local_path(&module.specifier);
    if !matches!(module.kind, ModuleKind::Esm | ModuleKind::Asserted) {
      log::warn!(
        "Unsupported module kind {:?} for {}",
        module.kind,
        module.specifier
      );
      continue;
    }
    environment.create_dir_all(local_path.parent().unwrap())?;
    environment.write_file(&local_path, source)?;
  }

  // create the import map
  if !mappings.base_specifiers().is_empty() {
    let import_map_text = build_import_map(graph, &all_modules, &mappings);
    environment
      .write_file(&output_dir.join("import_map.json"), &import_map_text)?;
  }

  Ok(())
}

#[cfg(test)]
mod test {
  use crate::tools::vendor::test::VendorTestBuilder;
  use deno_core::serde_json::json;
  use pretty_assertions::assert_eq;

  #[tokio::test]
  async fn local_specifiers_to_remote() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add_local_file(
            "/mod.ts",
            concat!(
              r#"import "https://localhost/mod.ts";"#,
              r#"import "https://localhost/other.ts?test";"#,
              r#"import "https://localhost/redirect.ts";"#,
            ),
          )
          .add_remote_file("https://localhost/mod.ts", "export class Mod {}")
          .add_remote_file(
            "https://localhost/other.ts?test",
            "export class Other {}",
          )
          .add_redirect(
            "https://localhost/redirect.ts",
            "https://localhost/mod.ts",
          );
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost",
          "https://localhost/other.ts?test": "./localhost/other.ts",
          "https://localhost/redirect.ts": "./localhost/mod.ts",
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        ("/vendor/localhost/mod.ts", "export class Mod {}"),
        ("/vendor/localhost/other.ts", "export class Other {}"),
      ]),
    );
  }

  #[tokio::test]
  async fn remote_specifiers() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add_local_file(
            "/mod.ts",
            concat!(
              r#"import "https://localhost/mod.ts";"#,
              r#"import "https://other/mod.ts";"#,
            ),
          )
          .add_remote_file(
            "https://localhost/mod.ts",
            concat!(
              "export * from './other.ts';",
              "export * from './redirect.ts';",
              "export * from '/absolute.ts';",
            ),
          )
          .add_remote_file(
            "https://localhost/other.ts",
            "export class Other {}",
          )
          .add_redirect(
            "https://localhost/redirect.ts",
            "https://localhost/other.ts",
          )
          .add_remote_file(
            "https://localhost/absolute.ts",
            "export class Absolute {}",
          )
          .add_remote_file(
            "https://other/mod.ts",
            "export * from './sub/mod.ts';",
          )
          .add_remote_file(
            "https://other/sub/mod.ts",
            concat!(
              "export * from '../sub2/mod.ts';",
              "export * from '../sub2/other?asdf';",
            ),
          )
          .add_remote_file("https://other/sub2/mod.ts", "export class Mod {}")
          .add_remote_file_with_headers(
            "https://other/sub2/other?asdf",
            "export class Other {}",
            &[("content-type", "application/javascript")],
          );
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost",
          "https://other/": "./other"
        },
        "scopes": {
          "./localhost": {
            "/absolute.ts": "./localhost/absolute.ts",
            "./localhost/redirect.ts": "./localhost/other.ts",
          },
          "./other": {
            "./other/sub2/other?asdf": "./other/sub2/other.js"
          }
        }
      }))
    );
    assert_eq!(
      output.files,
      to_file_vec(&[
        ("/vendor/localhost/absolute.ts", "export class Absolute {}"),
        (
          "/vendor/localhost/mod.ts",
          concat!(
            "export * from './other.ts';",
            "export * from './redirect.ts';",
            "export * from '/absolute.ts';",
          )
        ),
        ("/vendor/localhost/other.ts", "export class Other {}"),
        ("/vendor/other/mod.ts", "export * from './sub/mod.ts';"),
        (
          "/vendor/other/sub/mod.ts",
          concat!(
            "export * from '../sub2/mod.ts';",
            "export * from '../sub2/other?asdf';",
          )
        ),
        ("/vendor/other/sub2/mod.ts", "export class Mod {}"),
        ("/vendor/other/sub2/other.js", "export class Other {}"),
      ]),
    );
  }

  fn to_file_vec(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
      .iter()
      .map(|(f, t)| (f.to_string(), t.to_string()))
      .collect()
  }
}
