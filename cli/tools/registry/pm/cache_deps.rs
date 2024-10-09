// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use crate::factory::CliFactory;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use deno_core::error::AnyError;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::StreamExt;
use deno_semver::package::PackageReq;

pub async fn cache_top_level_deps(
  // todo(dsherret): don't pass the factory into this function. Instead use ctor deps
  factory: &CliFactory,
  jsr_resolver: Option<Arc<crate::jsr::JsrFetchResolver>>,
) -> Result<(), AnyError> {
  let npm_resolver = factory.npm_resolver().await?;
  let cli_options = factory.cli_options()?;
  let root_permissions = factory.root_permissions_container()?;
  if let Some(npm_resolver) = npm_resolver.as_managed() {
    if !npm_resolver.ensure_top_level_package_json_install().await? {
      if let Some(lockfile) = cli_options.maybe_lockfile() {
        lockfile.error_if_changed()?;
      }

      npm_resolver.cache_packages().await?;
    }
  }
  // cache as many entries in the import map as we can
  let resolver = factory.workspace_resolver().await?;
  if let Some(import_map) = resolver.maybe_import_map() {
    let jsr_resolver = if let Some(resolver) = jsr_resolver {
      resolver
    } else {
      Arc::new(crate::jsr::JsrFetchResolver::new(
        factory.file_fetcher()?.clone(),
      ))
    };

    let mut roots = Vec::new();

    let mut info_futures = FuturesUnordered::new();

    let mut seen_reqs = std::collections::HashSet::new();

    for entry in import_map.imports().entries() {
      let Some(specifier) = entry.value else {
        continue;
      };

      match specifier.scheme() {
        "jsr" => {
          let specifier_str = specifier.as_str();
          let specifier_str =
            specifier_str.strip_prefix("jsr:").unwrap_or(specifier_str);
          if let Ok(req) = PackageReq::from_str(specifier_str) {
            if !seen_reqs.insert(req.clone()) {
              continue;
            }
            let jsr_resolver = jsr_resolver.clone();
            info_futures.push(async move {
              if let Some(nv) = jsr_resolver.req_to_nv(&req).await {
                if let Some(info) = jsr_resolver.package_version_info(&nv).await
                {
                  return Some((specifier.clone(), info));
                }
              }
              None
            });
          }
        }
        "npm" => roots.push(specifier.clone()),
        _ => {
          if entry.key.ends_with('/') && specifier.as_str().ends_with('/') {
            continue;
          }
          roots.push(specifier.clone());
        }
      }
    }

    while let Some(info_future) = info_futures.next().await {
      if let Some((specifier, info)) = info_future {
        if info.export(".").is_some() {
          roots.push(specifier.clone());
          continue;
        }
        let exports = info.exports();
        for (k, _) in exports {
          if let Ok(spec) = specifier.join(k) {
            roots.push(spec);
          }
        }
      }
    }
    let mut graph_permit = factory
      .main_module_graph_container()
      .await?
      .acquire_update_permit()
      .await;
    let graph = graph_permit.graph_mut();
    factory
      .module_load_preparer()
      .await?
      .prepare_module_load(
        graph,
        &roots,
        false,
        deno_config::deno_json::TsTypeLib::DenoWorker,
        root_permissions.clone(),
        None,
      )
      .await?;
  }

  Ok(())
}
