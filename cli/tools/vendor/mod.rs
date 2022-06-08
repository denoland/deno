// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use ::import_map::ImportMap;
use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_runtime::permissions::Permissions;

use crate::flags::VendorFlags;
use crate::fs_util;
use crate::lockfile;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;

mod analyze;
mod build;
mod import_map;
mod mappings;
mod specifiers;
#[cfg(test)]
mod test;

pub async fn vendor(ps: ProcState, flags: VendorFlags) -> Result<(), AnyError> {
  let raw_output_dir = match &flags.output_path {
    Some(output_path) => output_path.to_owned(),
    None => PathBuf::from("vendor/"),
  };
  let output_dir = fs_util::resolve_from_cwd(&raw_output_dir)?;
  validate_output_dir(&output_dir, &flags)?;
  let maybe_import_map = ps.maybe_import_map.as_ref().map(|m| {
    m.with_folder_removed(
      &ModuleSpecifier::from_directory_path(&output_dir).unwrap(),
    )
  });
  let graph = create_graph(&ps, &flags, &maybe_import_map).await?;
  let build_result = build::build(
    &graph,
    &output_dir,
    maybe_import_map,
    &build::RealVendorEnvironment,
  )?;

  eprintln!(
    concat!("Vendored {} {} into {} directory.",),
    build_result.vendored_count,
    if build_result.vendored_count == 1 {
      "module"
    } else {
      "modules"
    },
    raw_output_dir.display(),
  );
  if let Some(import_map_path) = build_result.import_map_path {
    let cwd = std::env::current_dir().unwrap();
    let import_map_path_str = import_map_path.display().to_string();
    let relative_cwd_path = import_map_path_str
      .trim_start_matches(&cwd.display().to_string())
      .trim_start_matches('\\')
      .trim_start_matches('/');
    eprintln!(
      concat!(
        "\nTo use vendored modules, specify the `--import-map {}` flag when ",
        r#"invoking deno subcommands or add an `"importMap": "<file_path>"` "#,
        "entry to your deno.json file.",
      ),
      relative_cwd_path,
    );
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

fn is_dir_empty(dir_path: &Path) -> Result<bool, AnyError> {
  match std::fs::read_dir(&dir_path) {
    Ok(mut dir) => Ok(dir.next().is_none()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(true),
    Err(err) => {
      bail!("Error reading directory {}: {}", dir_path.display(), err)
    }
  }
}

async fn create_graph(
  ps: &ProcState,
  flags: &VendorFlags,
  maybe_import_map: &Option<ImportMap>,
) -> Result<deno_graph::ModuleGraph, AnyError> {
  let entry_points = flags
    .specifiers
    .iter()
    .map(|p| {
      let url = resolve_url_or_path(p)?;
      Ok((url, deno_graph::ModuleKind::Esm))
    })
    .collect::<Result<Vec<_>, AnyError>>()?;

  // todo(dsherret): there is a lot of copy and paste here from
  // other parts of the codebase. We should consolidate this.
  let mut cache = crate::cache::FetchCacher::new(
    ps.dir.gen_cache.clone(),
    ps.file_fetcher.clone(),
    Permissions::allow_all(),
    Permissions::allow_all(),
  );
  let maybe_locker = lockfile::as_maybe_locker(ps.lockfile.clone());
  let maybe_imports = if let Some(config_file) = &ps.maybe_config_file {
    config_file.to_maybe_imports()?
  } else {
    None
  };
  let maybe_import_map_resolver = maybe_import_map
    .clone()
    .map(|m| ImportMapResolver::new(Arc::new(m)));
  let maybe_jsx_resolver = ps.maybe_config_file.as_ref().and_then(|cf| {
    cf.to_maybe_jsx_import_source_module()
      .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
  });
  let maybe_resolver = if maybe_jsx_resolver.is_some() {
    maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
  } else {
    maybe_import_map_resolver
      .as_ref()
      .map(|im| im.as_resolver())
  };

  let graph = deno_graph::create_graph(
    entry_points,
    false,
    maybe_imports,
    &mut cache,
    maybe_resolver,
    maybe_locker,
    None,
    None,
  )
  .await;

  graph.lock()?;
  graph.valid()?;

  Ok(graph)
}
