// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_npm::NpmPackageId;
use deno_resolver::workspace::WorkspaceResolver;

use super::InstallReporter;
use super::print_install_report;
use crate::args::AddFlags;
use crate::args::CiFlags;
use crate::args::Flags;
use crate::args::InstallFlagsLocal;
use crate::args::InstallTopLevelFlags;
use crate::args::SyncTypesFlags;
use crate::factory::CliFactory;
use crate::npm::CliNpmResolver;
use crate::sys::CliSys;

pub async fn install_local(
  flags: Arc<Flags>,
  install_flags: InstallFlagsLocal,
) -> Result<(), AnyError> {
  match install_flags {
    InstallFlagsLocal::Add(add_flags) => {
      crate::tools::pm::add(
        flags,
        add_flags,
        crate::tools::pm::AddCommandName::Install,
      )
      .await
    }
    InstallFlagsLocal::Entrypoints(entrypoints) => {
      super::install_from_entrypoints(flags, entrypoints).await
    }
    InstallFlagsLocal::TopLevel(top_level_flags) => {
      install_top_level(flags, top_level_flags).await
    }
  }
}

#[derive(Debug, Default)]
pub struct CategorizedInstalledDeps {
  pub normal_deps: Vec<NpmPackageId>,
  pub dev_deps: Vec<NpmPackageId>,
}

pub fn categorize_installed_npm_deps(
  npm_resolver: &CliNpmResolver,
  workspace: &WorkspaceResolver<CliSys>,
  install_reporter: &InstallReporter,
) -> CategorizedInstalledDeps {
  let Some(managed_resolver) = npm_resolver.as_managed() else {
    return CategorizedInstalledDeps::default();
  };
  // compute the summary info
  let snapshot = managed_resolver.resolution().snapshot();

  let top_level_packages = snapshot.top_level_packages();

  // all this nonsense is to categorize into normal and dev deps
  let mut normal_deps = HashSet::new();
  let mut dev_deps = HashSet::new();

  for package_json in workspace.package_jsons() {
    let deps = package_json.resolve_local_package_json_deps();
    for (k, v) in deps.dependencies.iter() {
      let Ok(s) = v else {
        continue;
      };
      match s {
        deno_package_json::PackageJsonDepValue::File(_)
        | deno_package_json::PackageJsonDepValue::Tarball(_) => {
          // TODO(nathanwhit)
          // TODO(bartlomieju)
        }
        deno_package_json::PackageJsonDepValue::Req(package_req) => {
          normal_deps.insert(package_req.name.to_string());
        }
        deno_package_json::PackageJsonDepValue::Workspace { .. } => {
          // ignore workspace deps
        }
        deno_package_json::PackageJsonDepValue::Catalog(catalog_name) => {
          if workspace
            .resolve_catalog_dep(k.as_str(), catalog_name)
            .is_some()
          {
            normal_deps.insert(k.to_string());
          }
        }
      }
    }

    for (k, v) in deps.dev_dependencies.iter() {
      let Ok(s) = v else {
        continue;
      };
      match s {
        deno_package_json::PackageJsonDepValue::File(_)
        | deno_package_json::PackageJsonDepValue::Tarball(_) => {
          // TODO(nathanwhit)
          // TODO(bartlomieju)
        }
        deno_package_json::PackageJsonDepValue::Req(package_req) => {
          dev_deps.insert(package_req.name.to_string());
        }
        deno_package_json::PackageJsonDepValue::Workspace { .. } => {
          // ignore workspace deps
        }
        deno_package_json::PackageJsonDepValue::Catalog(catalog_name) => {
          if workspace
            .resolve_catalog_dep(k.as_str(), catalog_name)
            .is_some()
          {
            dev_deps.insert(k.to_string());
          }
        }
      }
    }
  }

  let mut installed_normal_deps = Vec::new();
  let mut installed_dev_deps = Vec::new();

  let npm_installed_set = if npm_resolver.root_node_modules_path().is_some() {
    &install_reporter.stats.intialized_npm
  } else {
    &install_reporter.stats.downloaded_npm
  };
  for pkg in top_level_packages {
    if !npm_installed_set.contains(&pkg.nv.to_string()) {
      continue;
    }
    if normal_deps.contains(&pkg.nv.name.to_string()) {
      installed_normal_deps.push(pkg);
    } else if dev_deps.contains(&pkg.nv.name.to_string()) {
      installed_dev_deps.push(pkg);
    } else {
      installed_normal_deps.push(pkg);
    }
  }

  installed_normal_deps.sort_by(|a, b| a.nv.name.cmp(&b.nv.name));
  installed_dev_deps.sort_by(|a, b| a.nv.name.cmp(&b.nv.name));

  CategorizedInstalledDeps {
    normal_deps: installed_normal_deps.into_iter().cloned().collect(),
    dev_deps: installed_dev_deps.into_iter().cloned().collect(),
  }
}

async fn install_top_level(
  flags: Arc<Flags>,
  top_level_flags: InstallTopLevelFlags,
) -> Result<(), AnyError> {
  let start_instant = std::time::Instant::now();
  let factory = CliFactory::from_flags(flags.clone());
  // surface any errors in the package.json
  factory
    .npm_installer()
    .await?
    .ensure_no_pkg_json_dep_errors()?;
  let npm_installer = factory.npm_installer().await?;
  npm_installer.ensure_no_pkg_json_dep_errors()?;

  // Tarball URL deps in package.json are remote code. Download each one
  // (gated behind --allow-import, just like any other remote import) into the
  // per-project tarball cache so the install can extract it like a local
  // tarball dep, leaving the URL in package.json untouched.
  let mut tarball_lockfile_entries = Vec::new();
  let tarball_pkgs = npm_installer.tarball_pkgs();
  if !tarball_pkgs.is_empty() {
    let permissions = factory.root_permissions_container()?;
    let http_client = factory.http_client_provider().clone();
    for tarball in tarball_pkgs {
      let pkg_json_dir = deno_path_util::url_to_file_path(&tarball.location)
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
      let entry = crate::tools::pm::download_tarball_url_dep(
        &tarball.url,
        &pkg_json_dir,
        http_client.clone(),
        permissions,
      )
      .await
      .with_context(|| {
        format!(
          "Cannot install tarball \"{}\" from package.json",
          tarball.url
        )
      })?;
      tarball_lockfile_entries.push(entry);
    }
  }

  // Recreate the factory so the freshly downloaded tarballs are discovered in
  // the cache and installed as local tarball deps.
  let factory = if tarball_lockfile_entries.is_empty() {
    factory
  } else {
    CliFactory::from_flags(flags)
  };

  // the actual work
  crate::tools::pm::cache_top_level_deps(
    &factory,
    None,
    crate::tools::pm::CacheTopLevelDepsOptions {
      lockfile_only: top_level_flags.lockfile_only,
    },
  )
  .await?;

  // Record the downloaded tarballs (integrity + resolved transitive dep ids)
  // in the lockfile now that npm install has resolved their dependencies.
  crate::tools::pm::write_tarball_entries_to_lockfile(
    &factory,
    &tarball_lockfile_entries,
  )
  .await?;

  if let Some(lockfile) = factory.maybe_lockfile().await? {
    lockfile.write_if_changed()?;
  }

  let install_reporter = factory.install_reporter()?.unwrap().clone();
  let workspace = factory.workspace_resolver().await?;
  let npm_resolver = factory.npm_resolver().await?;
  print_install_report(
    &factory.sys(),
    start_instant.elapsed(),
    &install_reporter,
    workspace,
    npm_resolver,
  );

  Ok(())
}

pub async fn ci_command(
  flags: Arc<Flags>,
  ci_flags: CiFlags,
) -> Result<(), AnyError> {
  let factory = CliFactory::from_flags(flags.clone());
  if factory.maybe_lockfile().await?.is_none() {
    bail!(
      "deno ci requires a lockfile, but none was found.\n  hint: run `deno install` to create one."
    );
  }
  if let Some(node_modules_dir) = factory.node_modules_dir_path()?
    && node_modules_dir.exists()
  {
    log::info!(
      "{} {}",
      deno_terminal::colors::gray("Removing"),
      node_modules_dir.display()
    );
    std::fs::remove_dir_all(node_modules_dir).map_err(|err| {
      deno_core::anyhow::anyhow!(
        "failed to remove {}: {err}",
        node_modules_dir.display()
      )
    })?;
  }
  drop(factory);
  install_top_level(
    flags,
    InstallTopLevelFlags {
      lockfile_only: false,
      production: ci_flags.production,
      skip_types: ci_flags.skip_types,
    },
  )
  .await
}

/// `deno sync-types`: generate a tsconfig + type mappings so stock TypeScript
/// tooling can type-check the project. Assumes dependencies are already
/// installed (run `deno install` first); materializes `jsr:`/`http(s):` types
/// and writes `.deno/tsconfig.json`.
pub async fn sync_types_command(
  flags: Arc<Flags>,
  sync_types_flags: SyncTypesFlags,
) -> Result<(), AnyError> {
  use crate::graph_container::CollectSpecifiersOptions;

  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let file_fetcher = factory.file_fetcher()?.clone();
  let http_client = factory.http_client_provider().get_or_create()?;
  let permissions = factory.root_permissions_container()?.clone();

  // Generate at the workspace/config root, not the current working directory,
  // so running `deno sync-types` from a subdirectory still writes the config
  // next to deno.json and picks up the root import map.
  let project_root = cli_options
    .workspace()
    .root_dir_url()
    .to_file_path()
    .map_err(|_| {
      deno_core::anyhow::anyhow!("workspace root is not a local directory")
    })?;

  let has_explicit_roots = !sync_types_flags.roots.is_empty();
  let root_patterns = if has_explicit_roots {
    sync_types_flags.roots
  } else {
    vec![".".to_string()]
  };

  // Build the module graph over either the requested roots or the whole
  // project to discover every external (npm:/jsr:/http(s):) specifier the code
  // actually uses — including specifiers written directly in source and across
  // workspace members, not just those declared in the root import map.
  let graph_specifiers = {
    let graph_container = factory.main_module_graph_container().await?;
    let roots = graph_container.collect_specifiers(
      &root_patterns,
      CollectSpecifiersOptions {
        include_ignored_specified: has_explicit_roots,
      },
    )?;
    // `collect_specifiers` honors deno.json excludes and the vendor dir, but not
    // `node_modules`, our generated `.deno/`, or the DENO_DIR cache. On a real
    // project those trees hold tens of thousands of files; feeding them as graph
    // roots makes the build choke. Keep only the project's own source so we
    // discover the external specifiers it actually imports.
    //
    // The `node_modules`/`.deno` check is on the path RELATIVE to the project
    // root, so a project that happens to live beneath an ancestor directory
    // named `node_modules` (e.g. developed in place) isn't filtered away
    // entirely. DENO_DIR is filtered by prefix when resolvable; it's normally
    // outside the project so its files aren't collected as roots regardless.
    let deno_dir_root = factory.deno_dir().ok().map(|d| d.root.clone());
    let roots: Vec<_> = roots
      .into_iter()
      .filter(|u| {
        let Ok(path) = u.to_file_path() else {
          return true;
        };
        if let Some(root) = &deno_dir_root
          && path.starts_with(root)
        {
          return false;
        }
        let rel = path.strip_prefix(&project_root).unwrap_or(&path);
        !rel.components().any(|c| {
          matches!(c.as_os_str().to_str(), Some("node_modules") | Some(".deno"))
        })
      })
      .collect();
    if has_explicit_roots && roots.is_empty() {
      bail!("No matching module graph roots found.");
    }

    // Build the graph error-tolerantly: `create_graph_with_options` populates
    // the graph and records unresolved modules as error entries without failing
    // the whole build (unlike `check_specifiers`, which validates and discards
    // everything on the first missing module). A real project always has some
    // broken file (dead example scripts, etc.); we want every specifier that
    // *did* resolve regardless.
    let graph_creator = factory.module_graph_creator().await?;
    // Silence warn-level diagnostics during this discovery build. It's an
    // internal step (not the user's `deno check`), and the graph the user
    // installed was already validated by `deno install` — re-emitting e.g.
    // "workspace member ... was not used" warnings here is just noise.
    let prev_log_level = log::max_level();
    log::set_max_level(log::LevelFilter::Error);
    let graph_result = graph_creator
      .create_graph_with_options(crate::graph_util::CreateGraphOptions {
        graph_kind: deno_graph::GraphKind::All,
        roots: roots.clone(),
        imports: vec![],
        is_dynamic: false,
        loader: None,
        npm_caching: cli_options.default_npm_caching_strategy(),
      })
      .await;
    log::set_max_level(prev_log_level);
    let graph = match graph_result {
      Ok(graph) => graph,
      Err(e) => {
        log::debug!("sync-types: graph build failed (continuing): {e}");
        deno_graph::ModuleGraph::new(deno_graph::GraphKind::All)
      }
    };

    let mut specifiers = std::collections::BTreeSet::new();
    // Remote graph roots do not appear as a dependency edge, but still need to
    // be mirrored for stock TypeScript and included in its project.
    for root in &roots {
      if matches!(root.scheme(), "http" | "https") {
        specifiers.insert(root.to_string());
      }
    }
    for module in graph.modules() {
      for (raw, _dep) in module.dependencies() {
        // Collect scheme specifiers (npm:/jsr:/http:) and bare specifiers
        // (import-map aliases like `@std/fmt/colors`, `fresh/runtime`). Skip
        // relative imports and node: builtins. setup_npm_compat resolves the
        // bare ones against the import map.
        if raw.starts_with('.')
          || raw.starts_with('/')
          || raw.starts_with("node:")
        {
          continue;
        }
        specifiers.insert(raw.clone());
      }
    }
    specifiers.into_iter().collect::<Vec<_>>()
  };

  // The stock TypeScript compatibility config needs the managed npm
  // resolution snapshot in global-cache mode. It uses that snapshot to give
  // each resolved package copy its own referenced tsconfig and dependency
  // paths, preserving the contextual resolution that a node_modules tree
  // would normally provide.
  let npm_resolver = factory.npm_resolver().await?.clone();

  let installed = super::npm_compat::setup_npm_compat(
    &project_root,
    &file_fetcher,
    &http_client,
    &permissions,
    &graph_specifiers,
    &npm_resolver,
  )
  .await?;

  log::info!(
    "{} tsconfig for stock TypeScript at {}",
    deno_terminal::colors::green("Synced"),
    project_root.join("tsconfig.json").display(),
  );
  for pkg in &installed {
    log::debug!("  installed jsr package {}@{}", pkg.name, pkg.version);
  }
  if !installed.is_empty() {
    log::info!("  installed {} jsr package(s)", installed.len());
  }
  Ok(())
}

pub fn check_if_installs_a_single_package_globally(
  maybe_add_flags: Option<&AddFlags>,
) -> Result<(), AnyError> {
  let Some(add_flags) = maybe_add_flags else {
    return Ok(());
  };
  if add_flags.packages.len() != 1 {
    return Ok(());
  }
  let Ok(url) = Url::parse(&add_flags.packages[0]) else {
    return Ok(());
  };
  if matches!(url.scheme(), "http" | "https") {
    // Allow tarball URLs to be installed locally (matching npm behavior).
    // All http(s) URLs that aren't git+ URLs are treated as tarballs
    // by npm, pnpm, and bun.
    let path = url.path();
    let is_tarball_url = path.ends_with(".tgz")
      || path.ends_with(".tar.gz")
      || path.contains("/tarball/");
    if !is_tarball_url {
      bail!(
        "Failed to install \"{}\" specifier. If you are trying to install {} globally, run again with `-g` flag:\n  deno install -g {}",
        url.scheme(),
        url.as_str(),
        url.as_str()
      );
    }
  }
  Ok(())
}
