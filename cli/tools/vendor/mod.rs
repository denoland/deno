// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_runtime::permissions::Permissions;

use crate::flags::VendorFlags;
use crate::fs_util::resolve_from_cwd;
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
  let output_dir = resolve_and_validate_output_dir(&flags, &ps)?;
  let graph = create_graph(&ps, &flags).await?;

  build::build(&graph, &output_dir, &build::RealVendorEnvironment)
}

fn resolve_and_validate_output_dir(
  flags: &VendorFlags,
  ps: &ProcState,
) -> Result<PathBuf, AnyError> {
  let output_dir = resolve_from_cwd(&match &flags.output_path {
    Some(output_path) => output_path.to_owned(),
    None => PathBuf::from("vendor"),
  })?;
  if !flags.force && !is_dir_empty(&output_dir)? {
    bail!(
      concat!(
        "Output directory {} was not empty. Please provide an empty directory or use ",
        "--force to ignore this error and potentially overwrite its contents.",
      ),
      output_dir.display(),
    );
  }

  // check the import map
  if let Some(import_map_path) = ps
    .maybe_import_map
    .as_ref()
    .and_then(|m| m.base_url().to_file_path().ok())
  {
    if import_map_path.starts_with(&output_dir) {
      // We don't allow using the output directory to help generate the new state
      // of itself because supporting this scenario adds a lot of complexity.
      bail!(
        "Using an import map found in the output directory is not supported."
      );
    }
  }

  Ok(output_dir)
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
  let maybe_import_map_resolver =
    ps.maybe_import_map.clone().map(ImportMapResolver::new);
  let maybe_jsx_resolver = ps
    .maybe_config_file
    .as_ref()
    .map(|cf| {
      cf.to_maybe_jsx_import_source_module()
        .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
    })
    .flatten();
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
