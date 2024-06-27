// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::Arc;

use deno_config::workspace::Workspace;
use deno_semver::package::PackageReq;

// todo(dsherret): this is not correct, but it's good enough for now.
// We need deno_npm to be able to understand workspace packages and
// then have a way to properly lay them out on the file system
#[derive(Debug, Default)]
pub struct PackageJsonDepsProvider(Vec<PackageReq>);

impl PackageJsonDepsProvider {
  pub fn empty() -> Self {
    Self(Vec::new())
  }

  pub fn from_workspace(workspace: &Arc<Workspace>) -> Self {
    let reqs = {
      let (root_folder_url, root_folder) = workspace.root_folder();
      let workspace_npm_pkgs = workspace.npm_packages();
      root_folder
        .pkg_json
        .as_ref()
        .map(|p| {
          // sort within each package
          let mut reqs = p
            .resolve_local_package_json_version_reqs()
            .into_values()
            .filter_map(|v| v.ok())
            .collect::<Vec<_>>();
          reqs.sort();
          reqs.into_iter()
        })
        .into_iter()
        .flatten()
        .chain(
          workspace
            .config_folders()
            .iter()
            .filter(|(folder_url, _)| *folder_url != root_folder_url)
            .filter_map(|(_, folder)| folder.pkg_json.as_ref())
            .flat_map(|p| {
              let mut reqs = p
                .resolve_local_package_json_version_reqs()
                .into_values()
                .filter_map(|v| v.ok())
                .collect::<Vec<_>>();
              reqs.sort();
              reqs.into_iter()
            }),
        )
        .filter(|req| {
          !workspace_npm_pkgs.iter().any(|pkg| {
            req.name == pkg.package_nv.name
              && match req.version_req.inner() {
                deno_semver::RangeSetOrTag::RangeSet(set) => {
                  set.satisfies(&pkg.package_nv.version)
                }
                deno_semver::RangeSetOrTag::Tag(_) => false,
              }
          })
        })
        .collect::<Vec<_>>()
    };
    Self(reqs)
  }

  pub fn reqs(&self) -> &Vec<PackageReq> {
    &self.0
  }
}
