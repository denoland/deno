// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::url::Url;
use deno_graph::JsrPackageReqNotFoundError;
use deno_graph::packages::JsrPackageVersionInfo;
use deno_npm_installer::PackageCaching;
use deno_npm_installer::graph::NpmCachingStrategy;
use deno_semver::Version;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;

use crate::factory::CliFactory;
use crate::graph_container::ModuleGraphContainer;
use crate::graph_container::ModuleGraphUpdatePermit;
use crate::graph_util::BuildGraphRequest;
use crate::graph_util::BuildGraphWithNpmOptions;

pub struct CacheTopLevelDepsOptions {
  pub lockfile_only: bool,
}

pub async fn cache_top_level_deps(
  // todo(dsherret): don't pass the factory into this function. Instead use ctor deps
  factory: &CliFactory,
  jsr_resolver: Option<Arc<crate::jsr::JsrFetchResolver>>,
  options: CacheTopLevelDepsOptions,
) -> Result<(), AnyError> {
  let _clear_guard = factory
    .text_only_progress_bar()
    .deferred_keep_initialize_alive();
  let npm_installer = factory.npm_installer().await?;
  npm_installer
    .ensure_top_level_package_json_install()
    .await?;
  if let Some(lockfile) = factory.maybe_lockfile().await? {
    lockfile.error_if_changed()?;
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
        factory.jsr_version_resolver()?.clone(),
      ))
    };
    let mut graph_permit = factory
      .main_module_graph_container()
      .await?
      .acquire_update_permit()
      .await;
    let graph = graph_permit.graph_mut();
    if let Some(lockfile) = factory.maybe_lockfile().await? {
      lockfile.fill_graph(graph);
    }

    let mut roots = Vec::new();

    let mut info_futures = FuturesUnordered::new();

    let mut seen_reqs = HashSet::new();

    let workspace_npm_packages = resolver
      .package_jsons()
      .filter_map(|pkg_json| {
        pkg_json
          .name
          .as_deref()
          .and_then(|name| Some((name, pkg_json.version.as_deref()?)))
      })
      .collect::<HashMap<_, _>>();
    let workspace_jsr_packages = resolver.jsr_packages();

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
          let Ok(req_ref) = JsrPackageReqReference::from_str(specifier_str)
          else {
            continue;
          };
          if workspace_jsr_packages
            .iter()
            .any(|pkg| pkg.matches_req(req_ref.req()))
          {
            // do not install a workspace jsr package
            continue;
          }
          if let Some(sub_path) = req_ref.sub_path() {
            if sub_path.ends_with('/') {
              continue;
            }
            roots.push(specifier.clone());
            continue;
          }
          if !seen_reqs.insert(req_ref.req().clone()) {
            continue;
          }
          let resolved_req = graph.packages.mappings().get(req_ref.req());
          let resolved_req = resolved_req.and_then(|nv| {
            // the version might end up being upgraded to a newer version that's already in
            // the graph (due to a reverted change), in which case our exports could end up
            // being wrong. to avoid that, see if there's a newer version that matches the version
            // req.
            let versions =
              graph.packages.versions_by_name(&req_ref.req().name)?;
            let mut best = nv;
            for version in versions {
              if version.version > best.version
                && req_ref.req().version_req.matches(&version.version)
              {
                best = version;
              }
            }
            Some(best)
          });

          let jsr_resolver = jsr_resolver.clone();
          info_futures.push(async move {
            let nv = if let Some(nv) = resolved_req {
              Cow::Borrowed(nv)
            } else if let Some(nv) =
              jsr_resolver.req_to_nv(req_ref.req()).await?
            {
              Cow::Owned(nv)
            } else {
              return Result::<
                Option<(Url, Arc<JsrPackageVersionInfo>)>,
                JsrPackageReqNotFoundError,
              >::Ok(None);
            };
            if let Some(info) = jsr_resolver.package_version_info(&nv).await {
              return Ok(Some((specifier.clone(), info)));
            }
            Ok(None)
          });
        }
        "npm" => {
          let Ok(req_ref) =
            NpmPackageReqReference::from_str(specifier.as_str())
          else {
            continue;
          };
          let version = workspace_npm_packages.get(&*req_ref.req().name);
          if let Some(version) = version {
            let Ok(version) = Version::parse_from_npm(version) else {
              continue;
            };
            let version_req = &req_ref.req().version_req;
            if version_req.tag().is_none() && version_req.matches(&version) {
              // if version req matches the workspace package's version, use that
              // (so it doesn't need to be installed)
              continue;
            }
          }

          roots.push(specifier.clone())
        }
        _ => {
          if entry.key.ends_with('/') && specifier.as_str().ends_with('/') {
            continue;
          }
          if specifier.scheme() == "file"
            && let Ok(path) = specifier.to_file_path()
            && !path.is_file()
          {
            continue;
          }
          roots.push(specifier.clone());
        }
      }
    }

    while let Some(info_future) = info_futures.next().await {
      if let Some((specifier, info)) = info_future? {
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
        BuildGraphWithNpmOptions {
          request: BuildGraphRequest::Roots(roots.clone()),
          loader: None,
          is_dynamic: false,
          npm_caching: NpmCachingStrategy::Manual,
        },
      )
      .await?;
    maybe_graph_error =
      graph_builder.graph_roots_valid(graph, &roots, true, true);
  }

  if options.lockfile_only {
    // do a resolution install if the npm snapshot is in a
    // pending state due to a config file change
    npm_installer.install_resolution_if_pending().await?;
  } else {
    npm_installer.cache_packages(PackageCaching::All).await?;
  }

  maybe_graph_error?;

  Ok(())
}
