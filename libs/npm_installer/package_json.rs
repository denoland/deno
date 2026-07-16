// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_config::workspace::Workspace;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonDepValueParseError;
use deno_package_json::PackageJsonDepWorkspaceReq;
use deno_resolver::npm::InvalidPackageNamePathError;
use deno_resolver::npm::package_name_for_node_modules_path_parts;
use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageName;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use serde_json;
use thiserror::Error;
use url::Url;

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

#[derive(Debug)]
pub enum InstallWorkspacePkgDep {
  Remote { alias: StackString, req: PackageReq },
  Workspace { alias: StackString, nv: PackageNv },
}

impl InstallWorkspacePkgDep {
  /// The name the dependency is linked under in the member's `node_modules`.
  pub fn alias(&self) -> &StackString {
    match self {
      InstallWorkspacePkgDep::Remote { alias, .. } => alias,
      InstallWorkspacePkgDep::Workspace { alias, .. } => alias,
    }
  }
}

#[derive(Debug)]
pub struct InstallWorkspacePkg {
  pub nv: PackageNv,
  pub target_dir: PathBuf,
  /// Whether this package is the workspace root. The root's `node_modules`
  /// is set up separately, so the per-member linking must skip it.
  pub is_root: bool,
  pub scripts: std::collections::HashMap<SmallStackString, String>,
  pub deps: Vec<InstallWorkspacePkgDep>,
}

#[derive(Debug, Error, Clone)]
#[error("Failed to install '{}'\n    at {}", alias, location)]
pub struct PackageJsonDepValueParseWithLocationError {
  pub location: Url,
  pub alias: StackString,
  #[source]
  pub source: PackageJsonDepValueParseError,
}

/// A `workspace:<version>` dependency referenced a local workspace member by
/// name, but the member's version did not satisfy the requested constraint.
/// Like pnpm, this is a hard error rather than silently linking the member or
/// falling back to the registry. The message mirrors the equivalent
/// `deno run` resolver error (`VersionNotSatisfied`).
#[derive(Debug, Error, Clone)]
#[error(
  "Failed to install '{alias}'{}: found package.json in workspace, but version '{version}' didn't satisfy constraint '{version_req}'\n    at {location}",
  self.member_suffix()
)]
pub struct WorkspaceMemberVersionNotSatisfiedError {
  pub location: Url,
  /// The dependency key the member is imported under.
  pub alias: StackString,
  /// The resolved workspace member name. Differs from `alias` for pnpm-style
  /// `workspace:<name>@<range>` aliases.
  pub member_name: StackString,
  pub version_req: VersionReq,
  pub version: Version,
}

impl WorkspaceMemberVersionNotSatisfiedError {
  /// When the member is imported under a different name (an alias), append the
  /// resolved member name so the offending entry is easy to locate.
  fn member_suffix(&self) -> String {
    if self.member_name == self.alias {
      String::new()
    } else {
      format!(" (workspace member '{}')", self.member_name)
    }
  }
}

#[derive(Debug, Error, Clone)]
#[error("Invalid package alias '{}'\n    at {}", alias, location)]
pub struct InvalidPackageAliasError {
  pub location: Url,
  pub alias: StackString,
  #[source]
  pub source: InvalidPackageNamePathError,
}

/// An error surfaced while reconciling a package.json's dependencies against
/// the workspace before installing.
#[derive(Debug, Error, Clone)]
pub enum EnsurePackageJsonDepsError {
  #[error(transparent)]
  DepValueParse(#[from] Box<PackageJsonDepValueParseWithLocationError>),
  #[error(transparent)]
  InvalidPackageAlias(#[from] Box<InvalidPackageAliasError>),
  #[error(transparent)]
  WorkspaceMemberVersionNotSatisfied(
    #[from] Box<WorkspaceMemberVersionNotSatisfiedError>,
  ),
}

#[derive(Debug, Default)]
pub struct NpmInstallDepsProvider {
  remote_pkgs: Vec<InstallNpmRemotePkg>,
  local_pkgs: Vec<InstallLocalPkg>,
  patch_pkgs: Vec<InstallPatchPkg>,
  workspace_pkgs: Vec<InstallWorkspacePkg>,
  pkg_json_dep_errors: Vec<PackageJsonDepValueParseWithLocationError>,
  invalid_package_alias_errors: Vec<InvalidPackageAliasError>,
  workspace_member_version_errors: Vec<WorkspaceMemberVersionNotSatisfiedError>,
}

fn package_json_to_lifecycle_nv(
  pkg_json: &deno_package_json::PackageJson,
) -> PackageNv {
  let name = pkg_json
    .name
    .as_deref()
    .map(PackageName::from_str)
    .unwrap_or_else(|| {
      PackageName::from_str(
        pkg_json
          .dir_path()
          .file_name()
          .and_then(|name| name.to_str())
          .unwrap_or("workspace"),
      )
    });
  let version = pkg_json
    .version
    .as_deref()
    .and_then(|version| Version::parse_from_npm(version).ok())
    .unwrap_or_else(|| Version::parse_from_npm("0.0.0").unwrap());
  PackageNv { name, version }
}

impl NpmInstallDepsProvider {
  pub fn empty() -> Self {
    Self::default()
  }

  pub fn from_workspace(
    workspace: &Arc<Workspace>,
    production: bool,
    skip_types: bool,
  ) -> Self {
    // todo(dsherret): estimate capacity?
    let mut local_pkgs = Vec::new();
    let mut remote_pkgs = Vec::new();
    let mut patch_pkgs = Vec::new();
    let mut workspace_pkgs = Vec::new();
    let mut pkg_json_dep_errors = Vec::new();
    let mut invalid_package_alias_errors = Vec::new();
    let mut workspace_member_version_errors = Vec::new();
    let workspace_npm_pkgs = workspace.npm_packages();

    for (folder_url, folder) in workspace.config_folders() {
      let is_root = folder_url == workspace.root_dir_url();
      // deal with the deno.json first because it takes precedence during resolution
      if let Some(deno_json) = &folder.deno_json {
        // don't bother with externally referenced import maps as users
        // should inline their import map to get this behaviour
        if let Some(serde_json::Value::Object(obj)) = &deno_json.json.imports {
          let mut pkg_pkgs = Vec::with_capacity(obj.len());
          for (alias, value) in obj {
            let serde_json::Value::String(specifier) = value else {
              continue;
            };
            let pkg_req = if let Some(catalog_name) =
              specifier.strip_prefix("catalog:")
            {
              // `catalog:`/`catalog:<name>` entries resolve to the version
              // requirement defined in the workspace root's catalog
              let catalog_name = if catalog_name.is_empty() {
                "default"
              } else {
                catalog_name
              };
              let name = alias.strip_suffix('/').unwrap_or(alias);
              let Some(version_req_str) = workspace
                .catalogs()
                .get(catalog_name)
                .and_then(|catalog| catalog.get(name))
              else {
                continue;
              };
              let Ok(version_req) = VersionReq::parse_from_npm(version_req_str)
              else {
                continue;
              };
              PackageReq {
                name: PackageName::from_str(name),
                version_req,
              }
            } else {
              let Ok(npm_req_ref) = NpmPackageReqReference::from_str(specifier)
              else {
                continue;
              };
              npm_req_ref.into_inner().req
            };

            if skip_types && pkg_req.name.starts_with("@types/") {
              continue;
            }

            let workspace_pkg = workspace_npm_pkgs
              .iter()
              .find(|pkg| pkg.matches_req_including_pre(&pkg_req));

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
        let empty = Default::default();
        let dev_deps = if production {
          &empty
        } else {
          &deps.dev_dependencies
        };
        let mut workspace_pkg_deps =
          Vec::with_capacity(deps.dependencies.len() + dev_deps.len());
        for (alias, dep) in deps.dependencies.iter().chain(dev_deps.iter()) {
          if let Err(err) = package_name_for_node_modules_path_parts(alias) {
            invalid_package_alias_errors.push(InvalidPackageAliasError {
              location: pkg_json.specifier(),
              alias: alias.clone(),
              source: err,
            });
            continue;
          }
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
              if skip_types && pkg_req.name.starts_with("@types/") {
                continue;
              }
              let workspace_pkg = workspace_npm_pkgs.iter().find(|pkg| {
                pkg.matches_req_including_pre(pkg_req)
                        // do not resolve to the current package
                        && pkg.pkg_json.path != pkg_json.path
              });

              if let Some(pkg) = workspace_pkg {
                workspace_pkg_deps.push(InstallWorkspacePkgDep::Workspace {
                  alias: alias.clone(),
                  nv: pkg.nv.clone(),
                });
                local_pkgs.push(InstallLocalPkg {
                  alias: Some(alias.clone()),
                  target_dir: pkg.pkg_json.dir_path().to_path_buf(),
                });
              } else {
                workspace_pkg_deps.push(InstallWorkspacePkgDep::Remote {
                  alias: alias.clone(),
                  req: pkg_req.clone(),
                });
                pkg_pkgs.push(InstallNpmRemotePkg {
                  alias: Some(alias.clone()),
                  base_dir: pkg_json.dir_path().to_path_buf(),
                  req: pkg_req.clone(),
                });
              }
            }
            PackageJsonDepValue::Workspace { name, version_req } => {
              // A `workspace:` dependency resolves to the local workspace
              // member with a matching name. The member is looked up by its
              // own package name, which for pnpm-style aliases
              // (`workspace:<name>@<range>`) differs from the dependency key
              // that's used as the import alias. `workspace:*`, `workspace:~`
              // and `workspace:^` are placeholders that match the member
              // regardless of its version (the range only affects what gets
              // written when publishing). An explicit `workspace:<range>` must
              // be satisfied by the member's version though; like pnpm, a
              // mismatch is a hard error rather than silently linking the
              // member or falling back to the registry. Prerelease versions
              // within the range bounds match too, since the member is provided
              // explicitly (#30155).
              let target_name = name.as_deref().unwrap_or(alias);
              if let Some(pkg) = workspace_npm_pkgs
                .iter()
                .find(|pkg| pkg.matches_name(target_name))
              {
                let satisfied = match version_req {
                  PackageJsonDepWorkspaceReq::Tilde
                  | PackageJsonDepWorkspaceReq::Caret => true,
                  PackageJsonDepWorkspaceReq::VersionReq(version_req) => pkg
                    .matches_name_and_version_req_including_pre(
                      target_name,
                      version_req,
                    ),
                };
                if satisfied {
                  workspace_pkg_deps.push(InstallWorkspacePkgDep::Workspace {
                    alias: alias.clone(),
                    nv: pkg.nv.clone(),
                  });
                  local_pkgs.push(InstallLocalPkg {
                    alias: Some(alias.clone()),
                    target_dir: pkg.pkg_json.dir_path().to_path_buf(),
                  });
                } else if let PackageJsonDepWorkspaceReq::VersionReq(
                  version_req,
                ) = version_req
                {
                  workspace_member_version_errors.push(
                    WorkspaceMemberVersionNotSatisfiedError {
                      location: pkg_json.specifier(),
                      alias: alias.clone(),
                      member_name: target_name.into(),
                      version_req: version_req.clone(),
                      version: pkg.nv.version.clone(),
                    },
                  );
                }
              }
            }
            PackageJsonDepValue::Catalog(catalog_name) => {
              let catalogs = workspace.catalogs();
              if let Some(catalog) = catalogs.get(catalog_name.as_str())
                && let Some(version_req_str) = catalog.get(alias.as_str())
                && let Ok(version_req) =
                  VersionReq::parse_from_npm(version_req_str)
              {
                let pkg_req = PackageReq {
                  name: alias.clone(),
                  version_req,
                };
                let workspace_pkg = workspace_npm_pkgs.iter().find(|pkg| {
                  pkg.matches_req_including_pre(&pkg_req)
                    && pkg.pkg_json.path != pkg_json.path
                });

                if let Some(pkg) = workspace_pkg {
                  workspace_pkg_deps.push(InstallWorkspacePkgDep::Workspace {
                    alias: alias.clone(),
                    nv: pkg.nv.clone(),
                  });
                  local_pkgs.push(InstallLocalPkg {
                    alias: Some(alias.clone()),
                    target_dir: pkg.pkg_json.dir_path().to_path_buf(),
                  });
                } else {
                  workspace_pkg_deps.push(InstallWorkspacePkgDep::Remote {
                    alias: alias.clone(),
                    req: pkg_req.clone(),
                  });
                  pkg_pkgs.push(InstallNpmRemotePkg {
                    alias: Some(alias.clone()),
                    base_dir: pkg_json.dir_path().to_path_buf(),
                    req: pkg_req,
                  });
                }
              }
            }
          }
        }

        // sort within each package as npm does
        pkg_pkgs.sort_by(|a, b| a.alias.cmp(&b.alias));
        remote_pkgs.extend(pkg_pkgs);
        workspace_pkgs.push(InstallWorkspacePkg {
          nv: package_json_to_lifecycle_nv(pkg_json),
          target_dir: pkg_json.dir_path().to_path_buf(),
          is_root,
          scripts: pkg_json
            .scripts
            .as_ref()
            .map(|scripts| {
              scripts
                .iter()
                .map(|(key, value)| {
                  (SmallStackString::from_str(key), value.clone())
                })
                .collect()
            })
            .unwrap_or_default(),
          deps: workspace_pkg_deps,
        });

        // Also symlink each non-root workspace member that is itself an npm
        // package (has a `name`) into the root `node_modules` under its real
        // package name. This mirrors npm/pnpm: a member referenced only by bare
        // specifier (not declared as a dependency, not via an `npm:` import map
        // value) would otherwise land only in `workspace_pkgs` and never be
        // linked into the root `node_modules`, so external Node tooling that
        // resolves through `node_modules` fails with MODULE_NOT_FOUND (#35359).
        // Keyed on the member's real package.json `name` (never an import-map
        // alias) so arbitrary `"foo": "npm:..."` aliases stay unlinked (#25542,
        // #25538). A non-empty `local_pkgs` also defeats the installers'
        // `has_no_packages` early-return so `node_modules` is created.
        if !is_root && let Some(name) = pkg_json.name.as_ref() {
          let alias = StackString::from_str(name);
          if let Err(err) = package_name_for_node_modules_path_parts(&alias) {
            invalid_package_alias_errors.push(InvalidPackageAliasError {
              location: pkg_json.specifier(),
              alias,
              source: err,
            });
          } else {
            local_pkgs.push(InstallLocalPkg {
              alias: Some(alias),
              target_dir: pkg_json.dir_path().to_path_buf(),
            });
          }
        }
      }
    }

    for pkg in workspace.link_pkg_jsons() {
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

    remote_pkgs.shrink_to_fit();
    local_pkgs.shrink_to_fit();
    patch_pkgs.shrink_to_fit();
    workspace_pkgs.shrink_to_fit();
    Self {
      remote_pkgs,
      local_pkgs,
      patch_pkgs,
      workspace_pkgs,
      pkg_json_dep_errors,
      invalid_package_alias_errors,
      workspace_member_version_errors,
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

  pub fn workspace_pkgs(&self) -> &[InstallWorkspacePkg] {
    &self.workspace_pkgs
  }

  pub fn pkg_json_dep_errors(
    &self,
  ) -> &[PackageJsonDepValueParseWithLocationError] {
    &self.pkg_json_dep_errors
  }

  pub fn invalid_package_alias_errors(&self) -> &[InvalidPackageAliasError] {
    &self.invalid_package_alias_errors
  }

  pub fn workspace_member_version_errors(
    &self,
  ) -> &[WorkspaceMemberVersionNotSatisfiedError] {
    &self.workspace_member_version_errors
  }
}
