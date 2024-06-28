// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use deno_config::package_json::PackageJsonDepValue;
use deno_config::workspace::Workspace;
use deno_semver::package::PackageReq;

// todo(dsherret): this is not correct, but it's good enough for now.
// We need deno_npm to be able to understand workspace packages and
// then have a way to properly lay them out on the file system
#[derive(Debug, Default)]
pub struct PackageJsonInstallDepsProvider(Vec<PackageReq>);

impl PackageJsonInstallDepsProvider {
  pub fn empty() -> Self {
    Self(Vec::new())
  }

  pub fn from_workspace(workspace: &Arc<Workspace>) -> Self {
    fn pkg_json_reqs(
      pkg_json: &deno_config::package_json::PackageJson,
    ) -> Vec<PackageReq> {
      let mut reqs = pkg_json
        .resolve_local_package_json_deps()
        .into_values()
        .filter_map(|v| v.ok())
        .filter_map(|dep| match dep {
          PackageJsonDepValue::Req(req) => Some(req),
          PackageJsonDepValue::Workspace(_) => None,
        })
        .collect::<Vec<_>>();
      // sort within each package
      reqs.sort();
      reqs
    }

    let reqs = {
      let (root_folder_url, root_folder) = workspace.root_folder();
      let workspace_npm_pkgs = workspace.npm_packages();
      root_folder
        .pkg_json
        .as_ref()
        .map(|r| pkg_json_reqs(r))
        .into_iter()
        .flatten()
        .chain(
          workspace
            .config_folders()
            .iter()
            .filter(|(folder_url, _)| *folder_url != root_folder_url)
            .filter_map(|(_, folder)| folder.pkg_json.as_ref())
            .flat_map(|p| pkg_json_reqs(p).into_iter()),
        )
        .filter(|req| {
          !workspace_npm_pkgs.iter().any(|pkg| pkg.matches_req(req))
        })
        .collect::<Vec<_>>()
    };
    Self(reqs)
  }

  pub fn reqs(&self) -> &Vec<PackageReq> {
    &self.0
  }
}
