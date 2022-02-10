// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::Path;
use std::path::PathBuf;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_graph::ModuleKind;
use deno_runtime::permissions::Permissions;

use crate::flags::VendorFlags;
use crate::lockfile;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;

use self::import_map::build_import_map;
use self::mappings::Mappings;
use self::specifiers::is_remote_specifier;

mod import_map;
mod mappings;
mod specifiers;

pub async fn vendor(ps: ProcState, flags: VendorFlags) -> Result<(), AnyError> {
  // todo: error when someone uses an import map in the vendor folder
  // todo: need to handle rewriting out the current import map to the new location? Though probably not possible.
  // I think people will need to manually update
  let output_dir = resolve_and_validate_output_dir(&flags)?;
  let graph = create_graph(&ps, &flags).await?;
  let all_modules = graph.modules();
  let remote_modules = all_modules
    .iter()
    .filter(|m| is_remote_specifier(&m.specifier))
    .copied()
    .collect::<Vec<_>>();
  let mappings =
    Mappings::from_remote_modules(&graph, &remote_modules, &output_dir)?;

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
    std::fs::create_dir_all(local_path.parent().unwrap())?;
    std::fs::write(local_path, file_text)?;
  }

  // create the import map
  if !mappings.base_specifiers().is_empty() {
    let import_map_text = build_import_map(&all_modules, &mappings);
    std::fs::write(output_dir.join("import_map.json"), import_map_text)?;
  }

  Ok(())
}

fn resolve_and_validate_output_dir(
  flags: &VendorFlags,
) -> Result<PathBuf, AnyError> {
  let output_dir = match &flags.output_path {
    Some(output_path) => output_path.clone(),
    None => std::env::current_dir()?.join("vendor"),
  };
  if !flags.force && !is_dir_empty(&output_dir)? {
    bail!("Directory {} was not empty. Please provide an empty directory or use --force to ignore this error and potentially overwrite its contents.", output_dir.display());
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
    .entry_points
    .iter()
    .map(|p| {
      let url = resolve_url_or_path(p)?;
      Ok((url, deno_graph::ModuleKind::Esm))
    })
    .collect::<Result<Vec<_>, AnyError>>()?;

  // todo(dsherret): there is a lot of copy and paste here from
  // other parts of the codebase
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
