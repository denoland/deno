// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::sync::Arc;

use crate::factory::CliFactory;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::graph_util::CreateGraphOptions;
use deno_core::error::AnyError;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::StreamExt;
use deno_semver::jsr::JsrPackageReqReference;

pub async fn cache_top_level_deps(
  // todo(dsherret): don't pass the factory into this function. Instead use ctor deps
  factory: &CliFactory,
  jsr_resolver: Option<Arc<crate::jsr::JsrFetchResolver>>,
) -> Result<(), AnyError> {
  let npm_resolver = factory.npm_resolver().await?;
  let cli_options = factory.cli_options()?;
  if let Some(npm_resolver) = npm_resolver.as_managed() {
    npm_resolver.ensure_top_level_package_json_install().await?;
    if let Some(lockfile) = cli_options.maybe_lockfile() {
      lockfile.error_if_changed()?;
    }
  }
  // cache as many entries in the import map as we can
  let resolver = factory.workspace_resolver().await?;

  let mut maybe_graph_error = Ok(());
  if let Some(import_map) = resolver.maybe_import_map() {
    let jsr_resolver = if let Some(resolver) = jsr_resolver {
      resolver
    } else {
      Arc::new(crate::jsr::JsrFetchResolver::new(
        factory.file_fetcher()?.clone(),
      ))
    };
    let mut graph_permit = factory
      .main_module_graph_container()
      .await?
      .acquire_update_permit()
      .await;
    let graph = graph_permit.graph_mut();
    if let Some(lockfile) = cli_options.maybe_lockfile() {
      let lockfile = lockfile.lock();
      crate::graph_util::fill_graph_from_lockfile(graph, &lockfile);
    }

    let mut roots = Vec::new();

    let mut info_futures = FuturesUnordered::new();

    let mut seen_reqs = std::collections::HashSet::new();

    for entry in import_map.imports().entries().chain(
      import_map
        .scopes()
        .flat_map(|scope| scope.imports.entries()),
    ) {
      let Some(specifier) = entry.value else {
        continue;
      };

      match specifier.scheme() {
        "jsr" => {
          let specifier_str = specifier.as_str();
          if let Ok(req) = JsrPackageReqReference::from_str(specifier_str) {
            if let Some(sub_path) = req.sub_path() {
              if sub_path.ends_with('/') {
                continue;
              }
              roots.push(specifier.clone());
              continue;
            }
            if !seen_reqs.insert(req.req().clone()) {
              continue;
            }
            let resolved_req = graph.packages.mappings().get(req.req());
            let jsr_resolver = jsr_resolver.clone();
            info_futures.push(async move {
              let nv = if let Some(req) = resolved_req {
                Cow::Borrowed(req)
              } else {
                Cow::Owned(jsr_resolver.req_to_nv(req.req()).await?)
              };
              if let Some(info) = jsr_resolver.package_version_info(&nv).await {
                return Some((specifier.clone(), info));
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
          if specifier.scheme() == "file" {
            if let Ok(path) = specifier.to_file_path() {
              if !path.is_file() {
                continue;
              }
            }
          }
          roots.push(specifier.clone());
        }
      }
    }

    while let Some(info_future) = info_futures.next().await {
      if let Some((specifier, info)) = info_future {
        let exports = info.exports();
        for (k, _) in exports {
          if let Ok(spec) = specifier.join(k) {
            roots.push(spec);
          }
        }
      }
    }
    drop(info_futures);

    let graph_builder = factory.module_graph_builder().await?;
    graph_builder
      .build_graph_with_npm_resolution(
        graph,
        CreateGraphOptions {
          loader: None,
          graph_kind: graph.graph_kind(),
          is_dynamic: false,
          roots: roots.clone(),
          npm_caching: crate::graph_util::NpmCachingStrategy::Manual,
        },
      )
      .await?;
    maybe_graph_error = graph_builder.graph_roots_valid(graph, &roots);
  }

  if let Some(npm_resolver) = npm_resolver.as_managed() {
    npm_resolver
      .cache_packages(crate::npm::PackageCaching::All)
      .await?;
  }

  maybe_graph_error?;

  Ok(())
}
