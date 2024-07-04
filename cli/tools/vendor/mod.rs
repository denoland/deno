// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_ast::TextChange;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_url_or_path;
use deno_graph::GraphKind;
use log::warn;

use crate::args::CliOptions;
use crate::args::ConfigFile;
use crate::args::Flags;
use crate::args::FmtOptionsConfig;
use crate::args::VendorFlags;
use crate::factory::CliFactory;
use crate::tools::fmt::format_json;
use crate::util::fs::canonicalize_path;
use crate::util::fs::resolve_from_cwd;
use crate::util::path::relative_specifier;
use deno_runtime::fs_util::specifier_to_file_path;

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
    Some(output_path) => PathBuf::from(output_path).to_owned(),
    None => PathBuf::from("vendor/"),
  };
  let output_dir = resolve_from_cwd(&raw_output_dir)?;
  validate_output_dir(&output_dir, &vendor_flags)?;
  validate_options(&mut cli_options, &output_dir)?;
  let factory = CliFactory::from_cli_options(Arc::new(cli_options));
  let cli_options = factory.cli_options();
  if cli_options.workspace.config_folders().len() > 1 {
    bail!("deno vendor is not supported in a workspace. Set `\"vendor\": true` in the workspace deno.json file instead");
  }
  let entry_points =
    resolve_entry_points(&vendor_flags, cli_options.initial_cwd())?;
  let jsx_import_source =
    cli_options.workspace.to_maybe_jsx_import_source_config()?;
  let module_graph_creator = factory.module_graph_creator().await?.clone();
  let workspace_resolver = factory.workspace_resolver().await?;
  let root_folder = cli_options.workspace.root_folder().1;
  let maybe_config_file = root_folder.deno_json.as_ref();
  let output = build::build(build::BuildInput {
    entry_points,
    build_graph: move |entry_points| {
      async move {
        module_graph_creator
          .create_graph(GraphKind::All, entry_points)
          .await
      }
      .boxed_local()
    },
    parsed_source_cache: factory.parsed_source_cache(),
    output_dir: &output_dir,
    maybe_original_import_map: workspace_resolver.maybe_import_map(),
    maybe_jsx_import_source: jsx_import_source.as_ref(),
    resolver: factory.resolver().await?.as_graph_resolver(),
    environment: &build::RealVendorEnvironment,
  })
  .await?;

  let vendored_count = output.vendored_count;
  let graph = output.graph;
  let npm_package_count = graph.npm_packages.len();
  let try_add_node_modules_dir = npm_package_count > 0
    && cli_options.node_modules_dir_enablement().unwrap_or(true);

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

  let try_add_import_map = vendored_count > 0;
  let modified_result = maybe_update_config_file(
    &output_dir,
    maybe_config_file,
    try_add_import_map,
    try_add_node_modules_dir,
  );

  // cache the node_modules folder when it's been added to the config file
  if modified_result.added_node_modules_dir {
    let node_modules_path =
      cli_options.node_modules_dir_path().cloned().or_else(|| {
        maybe_config_file
          .as_ref()
          .map(|d| &d.specifier)
          .filter(|c| c.scheme() == "file")
          .and_then(|c| c.to_file_path().ok())
          .map(|config_path| config_path.parent().unwrap().join("node_modules"))
      });
    if let Some(node_modules_path) = node_modules_path {
      let cli_options =
        cli_options.with_node_modules_dir_path(node_modules_path);
      let factory = CliFactory::from_cli_options(Arc::new(cli_options));
      if let Some(managed) = factory.npm_resolver().await?.as_managed() {
        managed.cache_packages().await?;
      }
    }
    log::info!(
      concat!(
        "Vendored {} npm {} into node_modules directory. Set `nodeModulesDir: false` ",
        "in the Deno configuration file to disable vendoring npm packages in the future.",
      ),
      npm_package_count,
      if npm_package_count == 1 {
        "package"
      } else {
        "packages"
      },
    );
  }

  if vendored_count > 0 {
    let import_map_path = raw_output_dir.join("import_map.json");
    if modified_result.updated_import_map {
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
  let import_map_specifier = options
    .resolve_specified_import_map_specifier()?
    .or_else(|| {
      let config_file = options.workspace.root_folder().1.deno_json.as_ref()?;
      config_file
        .to_import_map_specifier()
        .ok()
        .flatten()
        .or_else(|| {
          if config_file.is_an_import_map() {
            Some(config_file.specifier.clone())
          } else {
            None
          }
        })
    });
  // check the import map
  if let Some(import_map_path) = import_map_specifier
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
  maybe_config_file: Option<&Arc<ConfigFile>>,
  try_add_import_map: bool,
  try_add_node_modules_dir: bool,
) -> ModifiedResult {
  assert!(output_dir.is_absolute());
  let config_file = match maybe_config_file {
    Some(config_file) => config_file,
    None => return ModifiedResult::default(),
  };
  if config_file.specifier.scheme() != "file" {
    return ModifiedResult::default();
  }

  let fmt_config_options = config_file
    .to_fmt_config()
    .ok()
    .map(|config| config.options)
    .unwrap_or_default();
  let result = update_config_file(
    config_file,
    &fmt_config_options,
    if try_add_import_map {
      Some(
        ModuleSpecifier::from_file_path(output_dir.join("import_map.json"))
          .unwrap(),
      )
    } else {
      None
    },
    try_add_node_modules_dir,
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
  import_map_specifier: Option<ModuleSpecifier>,
  try_add_node_modules_dir: bool,
) -> Result<ModifiedResult, AnyError> {
  let config_path = specifier_to_file_path(&config_file.specifier)?;
  let config_text = std::fs::read_to_string(&config_path)?;
  let import_map_specifier =
    import_map_specifier.and_then(|import_map_specifier| {
      relative_specifier(&config_file.specifier, &import_map_specifier)
    });
  let modified_result = update_config_text(
    &config_text,
    fmt_options,
    import_map_specifier.as_deref(),
    try_add_node_modules_dir,
  )?;
  if let Some(new_text) = &modified_result.new_text {
    std::fs::write(config_path, new_text)?;
  }
  Ok(modified_result)
}

#[derive(Default)]
struct ModifiedResult {
  updated_import_map: bool,
  added_node_modules_dir: bool,
  new_text: Option<String>,
}

fn update_config_text(
  text: &str,
  fmt_options: &FmtOptionsConfig,
  import_map_specifier: Option<&str>,
  try_add_node_modules_dir: bool,
) -> Result<ModifiedResult, AnyError> {
  use jsonc_parser::ast::ObjectProp;
  use jsonc_parser::ast::Value;
  let text = if text.trim().is_empty() { "{}\n" } else { text };
  let ast =
    jsonc_parser::parse_to_ast(text, &Default::default(), &Default::default())?;
  let obj = match ast.value {
    Some(Value::Object(obj)) => obj,
    _ => bail!("Failed updating config file due to no object."),
  };
  let mut modified_result = ModifiedResult::default();
  let mut text_changes = Vec::new();
  let mut should_format = false;

  if try_add_node_modules_dir {
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
      modified_result.added_node_modules_dir = true;
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
    format_json(&PathBuf::from("deno.json"), &new_text, fmt_options)
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

fn resolve_entry_points(
  flags: &VendorFlags,
  initial_cwd: &Path,
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  flags
    .specifiers
    .iter()
    .map(|p| resolve_url_or_path(p, initial_cwd).map_err(|e| e.into()))
    .collect::<Result<Vec<_>, _>>()
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
    assert!(!result.added_node_modules_dir);
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
    assert!(result.added_node_modules_dir);
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
    assert!(result.added_node_modules_dir);
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
    assert!(!result.added_node_modules_dir);
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
    assert!(!result.added_node_modules_dir);
    assert!(!result.updated_import_map);
    assert_eq!(result.new_text, None);
  }
}
