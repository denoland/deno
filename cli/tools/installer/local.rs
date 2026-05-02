// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_npm::NpmPackageId;
use deno_resolver::workspace::WorkspaceResolver;

use super::InstallReporter;
use super::print_install_report;
use crate::args::AddFlags;
use crate::args::Flags;
use crate::args::InstallFlagsLocal;
use crate::args::InstallTopLevelFlags;
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
        deno_package_json::PackageJsonDepValue::File(_) => {
          // TODO(nathanwhit)
          // TODO(bartlomieju)
        }
        deno_package_json::PackageJsonDepValue::Req(package_req) => {
          normal_deps.insert(package_req.name.to_string());
        }
        deno_package_json::PackageJsonDepValue::Workspace(_) => {
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
        deno_package_json::PackageJsonDepValue::File(_) => {
          // TODO(nathanwhit)
          // TODO(bartlomieju)
        }
        deno_package_json::PackageJsonDepValue::Req(package_req) => {
          dev_deps.insert(package_req.name.to_string());
        }
        deno_package_json::PackageJsonDepValue::Workspace(_) => {
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
  let factory = CliFactory::from_flags(flags);
  // surface any errors in the package.json
  factory
    .npm_installer()
    .await?
    .ensure_no_pkg_json_dep_errors()?;
  let npm_installer = factory.npm_installer().await?;
  npm_installer.ensure_no_pkg_json_dep_errors()?;

  // the actual work
  crate::tools::pm::cache_top_level_deps(
    &factory,
    None,
    crate::tools::pm::CacheTopLevelDepsOptions {
      lockfile_only: top_level_flags.lockfile_only,
    },
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
    bail!(
      "Failed to install \"{}\" specifier. If you are trying to install {} globally, run again with `-g` flag:\n  deno install -g {}",
      url.scheme(),
      url.as_str(),
      url.as_str()
    );
  }
  Ok(())
}
