// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_ast::TextChange;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use log::warn;

use crate::args::CliOptions;
use crate::args::ConfigFile;
use crate::args::Flags;
use crate::args::FmtOptionsConfig;
use crate::args::VendorFlags;
use crate::factory::CliFactory;
use crate::graph_util::ModuleGraphBuilder;
use crate::tools::fmt::format_json;
use crate::util::fs::canonicalize_path;
use crate::util::fs::resolve_from_cwd;
use crate::util::path::relative_specifier;
use crate::util::path::specifier_to_file_path;

mod analyze;
mod build;
mod import_map;
mod mappings;
mod specifiers;
#[cfg(test)]
mod test;

pub async fn vendor(
  flags: Flags,
  vendor_flags: VendorFlags,
) -> Result<(), AnyError> {
  let mut cli_options = CliOptions::from_flags(flags)?;
  let raw_output_dir = match &vendor_flags.output_path {
    Some(output_path) => output_path.to_owned(),
    None => PathBuf::from("vendor/"),
  };
  let output_dir = resolve_from_cwd(&raw_output_dir)?;
  validate_output_dir(&output_dir, &vendor_flags)?;
  validate_options(&mut cli_options, &output_dir)?;
  let factory = CliFactory::from_cli_options(Arc::new(cli_options));
  let cli_options = factory.cli_options();
  let graph = create_graph(
    factory.module_graph_builder().await?,
    &vendor_flags,
    cli_options.initial_cwd(),
  )
  .await?;
  let had_npm_packages = !graph.npm_packages.is_empty();
  let vendored_count = build::build(
    graph,
    factory.parsed_source_cache()?,
    &output_dir,
    factory.maybe_import_map().await?.as_deref(),
    factory.maybe_lockfile().clone(),
    &build::RealVendorEnvironment,
  )?;

  log::info!(
    concat!("Vendored {} {} into {} directory.",),
    vendored_count,
    if vendored_count == 1 {
      "module"
    } else {
      "modules"
    },
    raw_output_dir.display(),
  );
  if vendored_count > 0 {
    let import_map_path = raw_output_dir.join("import_map.json");
    let result =
      maybe_update_config_file(&output_dir, cli_options, had_npm_packages);
    if result.updated_import_map {
      log::info!(
        concat!(
          "\nUpdated your local Deno configuration file with a reference to the ",
          "new vendored import map at {}. Invoking Deno subcommands will now ",
          "automatically resolve using the vendored modules. You may override ",
          "this by providing the `--import-map <other-import-map>` flag or by ",
          "manually editing your Deno configuration file.",
        ),
        import_map_path.display(),
      );
    } else {
      log::info!(
        concat!(
          "\nTo use vendored modules, specify the `--import-map {}` flag when ",
          r#"invoking Deno subcommands or add an `"importMap": "<path_to_vendored_import_map>"` "#,
          "entry to a deno.json file.",
        ),
        import_map_path.display(),
      );
    }
  }

  Ok(())
}

fn validate_output_dir(
  output_dir: &Path,
  flags: &VendorFlags,
) -> Result<(), AnyError> {
  if !flags.force && !is_dir_empty(output_dir)? {
    bail!(concat!(
      "Output directory was not empty. Please specify an empty directory or use ",
      "--force to ignore this error and potentially overwrite its contents.",
    ));
  }
  Ok(())
}

fn validate_options(
  options: &mut CliOptions,
  output_dir: &Path,
) -> Result<(), AnyError> {
  // check the import map
  if let Some(import_map_path) = options
    .resolve_import_map_specifier()?
    .and_then(|p| specifier_to_file_path(&p).ok())
    .and_then(|p| canonicalize_path(&p).ok())
  {
    // make the output directory in order to canonicalize it for the check below
    std::fs::create_dir_all(output_dir)?;
    let output_dir = canonicalize_path(output_dir).with_context(|| {
      format!("Failed to canonicalize: {}", output_dir.display())
    })?;

    if import_map_path.starts_with(output_dir) {
      // canonicalize to make the test for this pass on the CI
      let cwd = canonicalize_path(&std::env::current_dir()?)?;
      // We don't allow using the output directory to help generate the
      // new state because this may lead to cryptic error messages.
      log::warn!(
        concat!(
          "Ignoring import map. Specifying an import map file ({}) in the ",
          "deno vendor output directory is not supported. If you wish to use ",
          "an import map while vendoring, please specify one located outside ",
          "this directory."
        ),
        import_map_path
          .strip_prefix(&cwd)
          .unwrap_or(&import_map_path)
          .display()
          .to_string(),
      );

      // don't use an import map in the config
      options.set_import_map_specifier(None);
    }
  }

  Ok(())
}

fn maybe_update_config_file(
  output_dir: &Path,
  options: &CliOptions,
  had_npm_packages: bool,
) -> ModifiedResult {
  assert!(output_dir.is_absolute());
  let config_file = match options.maybe_config_file() {
    Some(config_file) => config_file,
    None => return ModifiedResult::default(),
  };
  if config_file.specifier.scheme() != "file" {
    return ModifiedResult::default();
  }

  let fmt_config = config_file
    .to_fmt_config()
    .ok()
    .unwrap_or_default()
    .unwrap_or_default();
  let result = update_config_file(
    config_file,
    &fmt_config.options,
    &ModuleSpecifier::from_file_path(output_dir.join("import_map.json"))
      .unwrap(),
    had_npm_packages,
  );
  match result {
    Ok(modified_result) => modified_result,
    Err(err) => {
      warn!("Error updating config file. {:#}", err);
      ModifiedResult::default()
    }
  }
}

fn update_config_file(
  config_file: &ConfigFile,
  fmt_options: &FmtOptionsConfig,
  import_map_specifier: &ModuleSpecifier,
  had_npm_packages: bool,
) -> Result<ModifiedResult, AnyError> {
  let config_path = specifier_to_file_path(&config_file.specifier)?;
  let config_text = std::fs::read_to_string(&config_path)?;
  let import_map_specifier =
    relative_specifier(&config_file.specifier, import_map_specifier);
  let modified_result = update_config_text(
    &config_text,
    fmt_options,
    import_map_specifier.as_deref(),
    had_npm_packages,
  )?;
  if let Some(new_text) = &modified_result.new_text {
    std::fs::write(config_path, new_text)?;
  }
  Ok(modified_result)
}

#[derive(Default)]
struct ModifiedResult {
  updated_import_map: bool,
  updated_node_modules_dir: bool,
  new_text: Option<String>,
}

fn update_config_text(
  text: &str,
  fmt_options: &FmtOptionsConfig,
  import_map_specifier: Option<&str>,
  had_npm_packages: bool,
) -> Result<ModifiedResult, AnyError> {
  use jsonc_parser::ast::ObjectProp;
  use jsonc_parser::ast::Value;
  let ast =
    jsonc_parser::parse_to_ast(text, &Default::default(), &Default::default())?;
  let obj = match ast.value {
    Some(Value::Object(obj)) => obj,
    _ => bail!("Failed updating config file due to no object."),
  };
  let mut modified_result = ModifiedResult::default();
  let mut text_changes = Vec::new();
  let mut should_format = false;

  if had_npm_packages {
    // Only modify the nodeModulesDir property if it's not set
    // as this allows people to opt-out of this when vendoring
    // by specifying `nodeModulesDir: false`
    if obj.get("nodeModulesDir").is_none() {
      let insert_position = obj.range.end - 1;
      text_changes.push(TextChange {
        range: insert_position..insert_position,
        new_text: r#""nodeModulesDir": true"#.to_string(),
      });
      should_format = true;
      modified_result.updated_node_modules_dir = true;
    }
  }

  if let Some(import_map_specifier) = import_map_specifier {
    let import_map_specifier = import_map_specifier.replace('\"', "\\\"");
    match obj.get("importMap") {
      Some(ObjectProp {
        value: Value::StringLit(lit),
        ..
      }) => {
        text_changes.push(TextChange {
          range: lit.range.start..lit.range.end,
          new_text: format!("\"{}\"", import_map_specifier),
        });
        modified_result.updated_import_map = true;
      }
      None => {
        // insert it crudely at a position that won't cause any issues
        // with comments and format after to make it look nice
        let insert_position = obj.range.end - 1;
        text_changes.push(TextChange {
          range: insert_position..insert_position,
          new_text: format!(r#""importMap": "{}""#, import_map_specifier),
        });
        should_format = true;
        modified_result.updated_import_map = true;
      }
      // shouldn't happen
      Some(_) => {
        bail!("Failed updating importMap in config file due to invalid type.")
      }
    }
  }

  if text_changes.is_empty() {
    return Ok(modified_result);
  }

  let new_text = deno_ast::apply_text_changes(text, text_changes);
  modified_result.new_text = if should_format {
    format_json(&new_text, fmt_options)
      .ok()
      .map(|formatted_text| formatted_text.unwrap_or(new_text))
  } else {
    Some(new_text)
  };
  Ok(modified_result)
}

fn is_dir_empty(dir_path: &Path) -> Result<bool, AnyError> {
  match std::fs::read_dir(dir_path) {
    Ok(mut dir) => Ok(dir.next().is_none()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(true),
    Err(err) => {
      bail!("Error reading directory {}: {}", dir_path.display(), err)
    }
  }
}

async fn create_graph(
  module_graph_builder: &ModuleGraphBuilder,
  flags: &VendorFlags,
  initial_cwd: &Path,
) -> Result<deno_graph::ModuleGraph, AnyError> {
  let entry_points = flags
    .specifiers
    .iter()
    .map(|p| resolve_url_or_path(p, initial_cwd))
    .collect::<Result<Vec<_>, _>>()?;

  module_graph_builder.create_graph(entry_points).await
}

#[cfg(test)]
mod internal_test {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn update_config_text_no_existing_props_add_prop() {
    let result = update_config_text(
      "{\n}",
      &Default::default(),
      Some("./vendor/import_map.json"),
      false,
    )
    .unwrap();
    assert!(result.updated_import_map);
    assert!(!result.updated_node_modules_dir);
    assert_eq!(
      result.new_text.unwrap(),
      r#"{
  "importMap": "./vendor/import_map.json"
}
"#
    );

    let result = update_config_text(
      "{\n}",
      &Default::default(),
      Some("./vendor/import_map.json"),
      true,
    )
    .unwrap();
    assert!(result.updated_import_map);
    assert!(result.updated_node_modules_dir);
    assert_eq!(
      result.new_text.unwrap(),
      r#"{
  "nodeModulesDir": true,
  "importMap": "./vendor/import_map.json"
}
"#
    );

    let result =
      update_config_text("{\n}", &Default::default(), None, true).unwrap();
    assert!(!result.updated_import_map);
    assert!(result.updated_node_modules_dir);
    assert_eq!(
      result.new_text.unwrap(),
      r#"{
  "nodeModulesDir": true
}
"#
    );
  }

  #[test]
  fn update_config_text_existing_props_add_prop() {
    let result = update_config_text(
      r#"{
  "tasks": {
    "task1": "other"
  }
}
"#,
      &Default::default(),
      Some("./vendor/import_map.json"),
      false,
    )
    .unwrap();
    assert_eq!(
      result.new_text.unwrap(),
      r#"{
  "tasks": {
    "task1": "other"
  },
  "importMap": "./vendor/import_map.json"
}
"#
    );

    // trailing comma
    let result = update_config_text(
      r#"{
  "tasks": {
    "task1": "other"
  },
}
"#,
      &Default::default(),
      Some("./vendor/import_map.json"),
      false,
    )
    .unwrap();
    assert_eq!(
      result.new_text.unwrap(),
      r#"{
  "tasks": {
    "task1": "other"
  },
  "importMap": "./vendor/import_map.json"
}
"#
    );
  }

  #[test]
  fn update_config_text_update_prop() {
    let result = update_config_text(
      r#"{
  "importMap": "./local.json"
}
"#,
      &Default::default(),
      Some("./vendor/import_map.json"),
      false,
    )
    .unwrap();
    assert_eq!(
      result.new_text.unwrap(),
      r#"{
  "importMap": "./vendor/import_map.json"
}
"#
    );
  }

  #[test]
  fn no_update_node_modules_dir() {
    // will not update if this is already set (even if it's false)
    let result = update_config_text(
      r#"{
  "nodeModulesDir": false
}
"#,
      &Default::default(),
      None,
      true,
    )
    .unwrap();
    assert!(!result.updated_node_modules_dir);
    assert!(!result.updated_import_map);
    assert_eq!(result.new_text, None);

    let result = update_config_text(
      r#"{
  "nodeModulesDir": true
}
"#,
      &Default::default(),
      None,
      true,
    )
    .unwrap();
    assert!(!result.updated_node_modules_dir);
    assert!(!result.updated_import_map);
    assert_eq!(result.new_text, None);
  }
}
