// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_config::workspace::Workspace;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseError;
use deno_package_json::PackageJsonDepWorkspaceReq;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageName;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::VersionReq;
use thiserror::Error;

#[derive(Debug)]
pub struct InstallNpmRemotePkg {
  pub alias: Option<StackString>,
  pub base_dir: PathBuf,
  pub req: PackageReq,
}

#[derive(Debug)]
pub struct InstallLocalPkg {
  pub alias: Option<StackString>,
  pub target_dir: PathBuf,
}

#[derive(Debug)]
pub struct InstallPatchPkg {
  pub nv: PackageNv,
  pub target_dir: PathBuf,
}

#[derive(Debug, Error, Clone)]
#[error("Failed to install '{}'\n    at {}", alias, location)]
pub struct PackageJsonDepValueParseWithLocationError {
  pub location: Url,
  pub alias: StackString,
  #[source]
  pub source: PackageJsonDepValueParseError,
}

#[derive(Debug, Default)]
pub struct NpmInstallDepsProvider {
  remote_pkgs: Vec<InstallNpmRemotePkg>,
  local_pkgs: Vec<InstallLocalPkg>,
  patch_pkgs: Vec<InstallPatchPkg>,
  pkg_json_dep_errors: Vec<PackageJsonDepValueParseWithLocationError>,
}

impl NpmInstallDepsProvider {
  pub fn empty() -> Self {
    Self::default()
  }

  pub fn from_workspace(workspace: &Arc<Workspace>) -> Self {
    // todo(dsherret): estimate capacity?
    let mut local_pkgs = Vec::new();
    let mut remote_pkgs = Vec::new();
    let mut patch_pkgs = Vec::new();
    let mut pkg_json_dep_errors = Vec::new();
    let workspace_npm_pkgs = workspace.npm_packages();

    for (_, folder) in workspace.config_folders() {
      // deal with the deno.json first because it takes precedence during resolution
      if let Some(deno_json) = &folder.deno_json {
        // don't bother with externally referenced import maps as users
        // should inline their import map to get this behaviour
        if let Some(serde_json::Value::Object(obj)) = &deno_json.json.imports {
          let mut pkg_pkgs = Vec::with_capacity(obj.len());
          for (_alias, value) in obj {
            let serde_json::Value::String(specifier) = value else {
              continue;
            };
            let Ok(npm_req_ref) = NpmPackageReqReference::from_str(specifier)
            else {
              continue;
            };
            let pkg_req = npm_req_ref.into_inner().req;
            let workspace_pkg = workspace_npm_pkgs
              .iter()
              .find(|pkg| pkg.matches_req(&pkg_req));

            if let Some(pkg) = workspace_pkg {
              local_pkgs.push(InstallLocalPkg {
                alias: None,
                target_dir: pkg.pkg_json.dir_path().to_path_buf(),
              });
            } else {
              pkg_pkgs.push(InstallNpmRemotePkg {
                alias: None,
                base_dir: deno_json.dir_path(),
                req: pkg_req,
              });
            }
          }

          // sort within each package (more like npm resolution)
          pkg_pkgs.sort_by(|a, b| a.req.cmp(&b.req));
          remote_pkgs.extend(pkg_pkgs);
        }
      }

      if let Some(pkg_json) = &folder.pkg_json {
        let deps = pkg_json.resolve_local_package_json_deps();
        let mut pkg_pkgs = Vec::with_capacity(
          deps.dependencies.len() + deps.dev_dependencies.len(),
        );
        for (alias, dep) in
          deps.dependencies.iter().chain(deps.dev_dependencies.iter())
        {
          let dep = match dep {
            Ok(dep) => dep,
            Err(err) => {
              pkg_json_dep_errors.push(
                PackageJsonDepValueParseWithLocationError {
                  location: pkg_json.specifier(),
                  alias: alias.clone(),
                  source: err.clone(),
                },
              );
              continue;
            }
          };
          match dep {
            PackageJsonDepValue::File(specifier) => {
              local_pkgs.push(InstallLocalPkg {
                alias: Some(alias.clone()),
                target_dir: pkg_json.dir_path().join(specifier),
              })
            }
            PackageJsonDepValue::Req(pkg_req) => {
              let workspace_pkg = workspace_npm_pkgs.iter().find(|pkg| {
                pkg.matches_req(pkg_req)
                // do not resolve to the current package
                && pkg.pkg_json.path != pkg_json.path
              });

              if let Some(pkg) = workspace_pkg {
                local_pkgs.push(InstallLocalPkg {
                  alias: Some(alias.clone()),
                  target_dir: pkg.pkg_json.dir_path().to_path_buf(),
                });
              } else {
                pkg_pkgs.push(InstallNpmRemotePkg {
                  alias: Some(alias.clone()),
                  base_dir: pkg_json.dir_path().to_path_buf(),
                  req: pkg_req.clone(),
                });
              }
            }
            PackageJsonDepValue::Workspace(workspace_version_req) => {
              let version_req = match workspace_version_req {
                PackageJsonDepWorkspaceReq::VersionReq(version_req) => {
                  version_req.clone()
                }
                PackageJsonDepWorkspaceReq::Tilde
                | PackageJsonDepWorkspaceReq::Caret => {
                  VersionReq::parse_from_npm("*").unwrap()
                }
              };
              if let Some(pkg) = workspace_npm_pkgs.iter().find(|pkg| {
                pkg.matches_name_and_version_req(alias, &version_req)
              }) {
                local_pkgs.push(InstallLocalPkg {
                  alias: Some(alias.clone()),
                  target_dir: pkg.pkg_json.dir_path().to_path_buf(),
                });
              }
            }
          }
        }

        // sort within each package as npm does
        pkg_pkgs.sort_by(|a, b| a.alias.cmp(&b.alias));
        remote_pkgs.extend(pkg_pkgs);
      }
    }

    if workspace.has_unstable("npm-patch") {
      for pkg in workspace.patch_pkg_jsons() {
        let Some(name) = pkg.name.as_ref() else {
          continue;
        };
        let Some(version) = pkg
          .version
          .as_ref()
          .and_then(|v| Version::parse_from_npm(v).ok())
        else {
          continue;
        };
        patch_pkgs.push(InstallPatchPkg {
          nv: PackageNv {
            name: PackageName::from_str(name),
            version,
          },
          target_dir: pkg.dir_path().to_path_buf(),
        })
      }
    }

    remote_pkgs.shrink_to_fit();
    local_pkgs.shrink_to_fit();
    patch_pkgs.shrink_to_fit();
    Self {
      remote_pkgs,
      local_pkgs,
      patch_pkgs,
      pkg_json_dep_errors,
    }
  }

  pub fn remote_pkgs(&self) -> &[InstallNpmRemotePkg] {
    &self.remote_pkgs
  }

  pub fn local_pkgs(&self) -> &[InstallLocalPkg] {
    &self.local_pkgs
  }

  pub fn patch_pkgs(&self) -> &[InstallPatchPkg] {
    &self.patch_pkgs
  }

  pub fn pkg_json_dep_errors(
    &self,
  ) -> &[PackageJsonDepValueParseWithLocationError] {
    &self.pkg_json_dep_errors
  }
}
