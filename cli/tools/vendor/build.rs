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
    let file_text = match module.kind {
      ModuleKind::Esm => {
        /*let text_changes =
          collect_remote_module_text_changes(&mappings, module);
        apply_text_changes(source, text_changes)*/
        source.to_string()
      }
      ModuleKind::Asserted => source.to_string(),
      _ => {
        log::warn!(
          "Unsupported module kind {:?} for {}",
          module.kind,
          module.specifier
        );
        continue;
      }
    };
    environment.create_dir_all(local_path.parent().unwrap())?;
    environment.write_file(&local_path, &file_text)?;
  }

  // create the import map
  if !mappings.base_specifiers().is_empty() {
    let import_map_text = build_import_map(&all_modules, &mappings);
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
  async fn should_handle_remote_files() {
    let mut builder = VendorTestBuilder::with_default_setup();
    let output = builder
      .with_loader(|loader| {
        loader
          .add_local_file("/mod.ts", r#"import "https://localhost/mod.ts";"#)
          .add_remote_file("https://localhost/mod.ts", "export class Test {}");
      })
      .build()
      .await
      .unwrap();

    assert_eq!(
      output.import_map,
      Some(json!({
        "imports": {
          "https://localhost/": "./localhost"
        }
      }))
    )
  }
}
