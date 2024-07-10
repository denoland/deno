// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_config::package_json::PackageJsonDepValue;
use deno_config::workspace::Workspace;
use deno_semver::package::PackageReq;

#[derive(Debug)]
pub struct InstallNpmWorkspacePkg {
  pub alias: String,
  pub pkg_dir: PathBuf,
}

// todo(#24419): this is not correct, but it's good enough for now.
// We need deno_npm to be able to understand workspace packages and
// then have a way to properly lay them out on the file system
#[derive(Debug, Default)]
pub struct PackageJsonInstallDepsProvider {
  remote_pkg_reqs: Vec<PackageReq>,
  workspace_pkgs: Vec<InstallNpmWorkspacePkg>,
}

impl PackageJsonInstallDepsProvider {
  pub fn empty() -> Self {
    Self::default()
  }

  pub fn from_workspace(workspace: &Arc<Workspace>) -> Self {
    let mut workspace_pkgs = Vec::new();
    let mut remote_pkg_reqs = Vec::new();
    let workspace_npm_pkgs = workspace.npm_packages();
    for pkg_json in workspace.package_jsons() {
      let deps = pkg_json.resolve_local_package_json_deps();
      let mut pkg_reqs = Vec::with_capacity(deps.len());
      for (alias, dep) in deps {
        let Ok(dep) = dep else {
          continue;
        };
        match dep {
          PackageJsonDepValue::Req(pkg_req) => {
            if let Some(pkg) = workspace_npm_pkgs
              .iter()
              .find(|pkg| pkg.matches_req(&pkg_req))
            {
              workspace_pkgs.push(InstallNpmWorkspacePkg {
                alias,
                pkg_dir: pkg.pkg_json.dir_path().to_path_buf(),
              });
            } else {
              pkg_reqs.push(pkg_req)
            }
          }
          PackageJsonDepValue::Workspace(version_req) => {
            if let Some(pkg) = workspace_npm_pkgs.iter().find(|pkg| {
              pkg.matches_name_and_version_req(&alias, &version_req)
            }) {
              workspace_pkgs.push(InstallNpmWorkspacePkg {
                alias,
                pkg_dir: pkg.pkg_json.dir_path().to_path_buf(),
              });
            }
          }
        }
      }
      // sort within each package
      pkg_reqs.sort();

      remote_pkg_reqs.extend(pkg_reqs);
    }
    remote_pkg_reqs.shrink_to_fit();
    workspace_pkgs.shrink_to_fit();
    Self {
      remote_pkg_reqs,
      workspace_pkgs,
    }
  }

  pub fn remote_pkg_reqs(&self) -> &Vec<PackageReq> {
    &self.remote_pkg_reqs
  }

  pub fn workspace_pkgs(&self) -> &Vec<InstallNpmWorkspacePkg> {
    &self.workspace_pkgs
  }
}
