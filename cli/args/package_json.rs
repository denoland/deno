// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_config::workspace::Workspace;
use deno_package_json::PackageJsonDepValue;
use deno_semver::package::PackageReq;

#[derive(Debug)]
pub struct InstallNpmRemotePkg {
  pub alias: String,
  // todo(24419): use this when setting up the node_modules dir
  #[allow(dead_code)]
  pub base_dir: PathBuf,
  pub req: PackageReq,
}

#[derive(Debug)]
pub struct InstallNpmWorkspacePkg {
  pub alias: String,
  // todo(24419): use this when setting up the node_modules dir
  #[allow(dead_code)]
  pub base_dir: PathBuf,
  pub target_dir: PathBuf,
}

#[derive(Debug, Default)]
pub struct PackageJsonInstallDepsProvider {
  remote_pkgs: Vec<InstallNpmRemotePkg>,
  workspace_pkgs: Vec<InstallNpmWorkspacePkg>,
}

impl PackageJsonInstallDepsProvider {
  pub fn empty() -> Self {
    Self::default()
  }

  pub fn from_workspace(workspace: &Arc<Workspace>) -> Self {
    let mut workspace_pkgs = Vec::new();
    let mut remote_pkgs = Vec::new();
    let workspace_npm_pkgs = workspace.npm_packages();
    for pkg_json in workspace.package_jsons() {
      let deps = pkg_json.resolve_local_package_json_deps();
      let mut pkg_pkgs = Vec::with_capacity(deps.len());
      for (alias, dep) in deps {
        let Ok(dep) = dep else {
          continue;
        };
        match dep {
          PackageJsonDepValue::Req(pkg_req) => {
            let workspace_pkg = workspace_npm_pkgs.iter().find(|pkg| {
              pkg.matches_req(&pkg_req)
              // do not resolve to the current package
              && pkg.pkg_json.path != pkg_json.path
            });

            if let Some(pkg) = workspace_pkg {
              workspace_pkgs.push(InstallNpmWorkspacePkg {
                alias,
                base_dir: pkg_json.dir_path().to_path_buf(),
                target_dir: pkg.pkg_json.dir_path().to_path_buf(),
              });
            } else {
              pkg_pkgs.push(InstallNpmRemotePkg {
                alias,
                base_dir: pkg_json.dir_path().to_path_buf(),
                req: pkg_req,
              });
            }
          }
          PackageJsonDepValue::Workspace(version_req) => {
            if let Some(pkg) = workspace_npm_pkgs.iter().find(|pkg| {
              pkg.matches_name_and_version_req(&alias, &version_req)
            }) {
              workspace_pkgs.push(InstallNpmWorkspacePkg {
                alias,
                base_dir: pkg_json.dir_path().to_path_buf(),
                target_dir: pkg.pkg_json.dir_path().to_path_buf(),
              });
            }
          }
        }
      }
      // sort within each package
      pkg_pkgs.sort_by(|a, b| a.alias.cmp(&b.alias));

      remote_pkgs.extend(pkg_pkgs);
    }
    remote_pkgs.shrink_to_fit();
    workspace_pkgs.shrink_to_fit();
    Self {
      remote_pkgs,
      workspace_pkgs,
    }
  }

  pub fn remote_pkgs(&self) -> &Vec<InstallNpmRemotePkg> {
    &self.remote_pkgs
  }

  pub fn workspace_pkgs(&self) -> &Vec<InstallNpmWorkspacePkg> {
    &self.workspace_pkgs
  }
}
