// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_package_json::PackageJson;
use deno_package_json::PackageJsonDepValue;
use deno_path_util::url_to_file_path;
use deno_semver::package::PackageReq;
use deno_semver::Version;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageJsonLoadError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::load_pkg_json;
use node_resolver::NpmResolver;
use thiserror::Error;
use url::Url;

use crate::fs::DenoPkgJsonFsAdapter;
use crate::fs::DenoResolverFs;

use super::local::normalize_pkg_name_for_node_modules_deno_folder;

#[derive(Debug, Error)]
pub enum ByonmResolvePkgFolderFromDenoReqError {
  #[error("Could not find \"{}\" in a node_modules folder. Deno expects the node_modules/ directory to be up to date. Did you forget to run `deno install`?", .0)]
  MissingAlias(String),
  #[error(transparent)]
  PackageJson(#[from] PackageJsonLoadError),
  #[error("Could not find a matching package for 'npm:{}' in the node_modules directory. Ensure you have all your JSR and npm dependencies listed in your deno.json or package.json, then run `deno install`. Alternatively, turn on auto-install by specifying `\"nodeModulesDir\": \"auto\"` in your deno.json file.", .0)]
  UnmatchedReq(PackageReq),
  #[error(transparent)]
  Io(#[from] std::io::Error),
}

pub struct ByonmNpmResolverCreateOptions<Fs: DenoResolverFs> {
  pub fs: Fs,
  // todo(dsherret): investigate removing this
  pub root_node_modules_dir: Option<PathBuf>,
}

#[derive(Debug)]
pub struct ByonmNpmResolver<Fs: DenoResolverFs> {
  fs: Fs,
  root_node_modules_dir: Option<PathBuf>,
}

impl<Fs: DenoResolverFs + Clone> Clone for ByonmNpmResolver<Fs> {
  fn clone(&self) -> Self {
    Self {
      fs: self.fs.clone(),
      root_node_modules_dir: self.root_node_modules_dir.clone(),
    }
  }
}

impl<Fs: DenoResolverFs> ByonmNpmResolver<Fs> {
  pub fn new(options: ByonmNpmResolverCreateOptions<Fs>) -> Self {
    Self {
      fs: options.fs,
      root_node_modules_dir: options.root_node_modules_dir,
    }
  }

  pub fn root_node_modules_dir(&self) -> Option<&Path> {
    self.root_node_modules_dir.as_deref()
  }

  fn load_pkg_json(
    &self,
    path: &Path,
  ) -> Result<Option<Arc<PackageJson>>, PackageJsonLoadError> {
    load_pkg_json(&DenoPkgJsonFsAdapter(&self.fs), path)
  }

  /// Finds the ancestor package.json that contains the specified dependency.
  pub fn find_ancestor_package_json_with_dep(
    &self,
    dep_name: &str,
    referrer: &Url,
  ) -> Option<Arc<PackageJson>> {
    let referrer_path = url_to_file_path(referrer).ok()?;
    let mut current_folder = referrer_path.parent()?;
    loop {
      let pkg_json_path = current_folder.join("package.json");
      if let Ok(Some(pkg_json)) = self.load_pkg_json(&pkg_json_path) {
        if let Some(deps) = &pkg_json.dependencies {
          if deps.contains_key(dep_name) {
            return Some(pkg_json);
          }
        }
        if let Some(deps) = &pkg_json.dev_dependencies {
          if deps.contains_key(dep_name) {
            return Some(pkg_json);
          }
        }
      }

      if let Some(parent) = current_folder.parent() {
        current_folder = parent;
      } else {
        return None;
      }
    }
  }

  pub fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &Url,
  ) -> Result<PathBuf, ByonmResolvePkgFolderFromDenoReqError> {
    fn node_resolve_dir<Fs: DenoResolverFs>(
      fs: &Fs,
      alias: &str,
      start_dir: &Path,
    ) -> std::io::Result<Option<PathBuf>> {
      for ancestor in start_dir.ancestors() {
        let node_modules_folder = ancestor.join("node_modules");
        let sub_dir = join_package_name(&node_modules_folder, alias);
        if fs.is_dir_sync(&sub_dir) {
          return Ok(Some(deno_path_util::canonicalize_path_maybe_not_exists(
            &sub_dir,
            &|path| fs.realpath_sync(path),
          )?));
        }
      }
      Ok(None)
    }

    // now attempt to resolve if it's found in any package.json
    let maybe_pkg_json_and_alias =
      self.resolve_pkg_json_and_alias_for_req(req, referrer)?;
    match maybe_pkg_json_and_alias {
      Some((pkg_json, alias)) => {
        // now try node resolution
        if let Some(resolved) =
          node_resolve_dir(&self.fs, &alias, pkg_json.dir_path())?
        {
          return Ok(resolved);
        }

        Err(ByonmResolvePkgFolderFromDenoReqError::MissingAlias(alias))
      }
      None => {
        // now check if node_modules/.deno/ matches this constraint
        if let Some(folder) = self.resolve_folder_in_root_node_modules(req) {
          return Ok(folder);
        }

        Err(ByonmResolvePkgFolderFromDenoReqError::UnmatchedReq(
          req.clone(),
        ))
      }
    }
  }

  fn resolve_pkg_json_and_alias_for_req(
    &self,
    req: &PackageReq,
    referrer: &Url,
  ) -> Result<Option<(Arc<PackageJson>, String)>, PackageJsonLoadError> {
    fn resolve_alias_from_pkg_json(
      req: &PackageReq,
      pkg_json: &PackageJson,
    ) -> Option<String> {
      let deps = pkg_json.resolve_local_package_json_deps();
      for (key, value) in deps {
        if let Ok(value) = value {
          match value {
            PackageJsonDepValue::Req(dep_req) => {
              if dep_req.name == req.name
                && dep_req.version_req.intersects(&req.version_req)
              {
                return Some(key);
              }
            }
            PackageJsonDepValue::Workspace(_workspace) => {
              if key == req.name && req.version_req.tag() == Some("workspace") {
                return Some(key);
              }
            }
          }
        }
      }
      None
    }

    // attempt to resolve the npm specifier from the referrer's package.json,
    if let Ok(file_path) = url_to_file_path(referrer) {
      let mut current_path = file_path.as_path();
      while let Some(dir_path) = current_path.parent() {
        let package_json_path = dir_path.join("package.json");
        if let Some(pkg_json) = self.load_pkg_json(&package_json_path)? {
          if let Some(alias) =
            resolve_alias_from_pkg_json(req, pkg_json.as_ref())
          {
            return Ok(Some((pkg_json, alias)));
          }
        }
        current_path = dir_path;
      }
    }

    // otherwise, fall fallback to the project's package.json
    if let Some(root_node_modules_dir) = &self.root_node_modules_dir {
      let root_pkg_json_path =
        root_node_modules_dir.parent().unwrap().join("package.json");
      if let Some(pkg_json) = self.load_pkg_json(&root_pkg_json_path)? {
        if let Some(alias) = resolve_alias_from_pkg_json(req, pkg_json.as_ref())
        {
          return Ok(Some((pkg_json, alias)));
        }
      }
    }

    Ok(None)
  }

  fn resolve_folder_in_root_node_modules(
    &self,
    req: &PackageReq,
  ) -> Option<PathBuf> {
    // now check if node_modules/.deno/ matches this constraint
    let root_node_modules_dir = self.root_node_modules_dir.as_ref()?;
    let node_modules_deno_dir = root_node_modules_dir.join(".deno");
    let Ok(entries) = self.fs.read_dir_sync(&node_modules_deno_dir) else {
      return None;
    };
    let search_prefix = format!(
      "{}@",
      normalize_pkg_name_for_node_modules_deno_folder(&req.name)
    );
    let mut best_version = None;

    // example entries:
    // - @denotest+add@1.0.0
    // - @denotest+add@1.0.0_1
    for entry in entries {
      if !entry.is_directory {
        continue;
      }
      let Some(version_and_copy_idx) = entry.name.strip_prefix(&search_prefix)
      else {
        continue;
      };
      let version = version_and_copy_idx
        .rsplit_once('_')
        .map(|(v, _)| v)
        .unwrap_or(version_and_copy_idx);
      let Ok(version) = Version::parse_from_npm(version) else {
        continue;
      };
      if let Some(tag) = req.version_req.tag() {
        let initialized_file =
          node_modules_deno_dir.join(&entry.name).join(".initialized");
        let Ok(contents) = self.fs.read_to_string_lossy(&initialized_file)
        else {
          continue;
        };
        let mut tags = contents.split(',').map(str::trim);
        if tags.any(|t| t == tag) {
          if let Some((best_version_version, _)) = &best_version {
            if version > *best_version_version {
              best_version = Some((version, entry.name));
            }
          } else {
            best_version = Some((version, entry.name));
          }
        }
      } else if req.version_req.matches(&version) {
        if let Some((best_version_version, _)) = &best_version {
          if version > *best_version_version {
            best_version = Some((version, entry.name));
          }
        } else {
          best_version = Some((version, entry.name));
        }
      }
    }

    best_version.map(|(_version, entry_name)| {
      join_package_name(
        &node_modules_deno_dir.join(entry_name).join("node_modules"),
        &req.name,
      )
    })
  }
}

impl<Fs: DenoResolverFs + Send + Sync + std::fmt::Debug> NpmResolver
  for ByonmNpmResolver<Fs>
{
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &Url,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    fn inner<Fs: DenoResolverFs>(
      fs: &Fs,
      name: &str,
      referrer: &Url,
    ) -> Result<PathBuf, PackageFolderResolveError> {
      let maybe_referrer_file = url_to_file_path(referrer).ok();
      let maybe_start_folder =
        maybe_referrer_file.as_ref().and_then(|f| f.parent());
      if let Some(start_folder) = maybe_start_folder {
        for current_folder in start_folder.ancestors() {
          let node_modules_folder = if current_folder.ends_with("node_modules")
          {
            Cow::Borrowed(current_folder)
          } else {
            Cow::Owned(current_folder.join("node_modules"))
          };

          let sub_dir = join_package_name(&node_modules_folder, name);
          if fs.is_dir_sync(&sub_dir) {
            return Ok(sub_dir);
          }
        }
      }

      Err(
        PackageNotFoundError {
          package_name: name.to_string(),
          referrer: referrer.clone(),
          referrer_extra: None,
        }
        .into(),
      )
    }

    let path = inner(&self.fs, name, referrer)?;
    self.fs.realpath_sync(&path).map_err(|err| {
      PackageFolderResolveIoError {
        package_name: name.to_string(),
        referrer: referrer.clone(),
        source: err,
      }
      .into()
    })
  }

  fn in_npm_package(&self, specifier: &Url) -> bool {
    specifier.scheme() == "file"
      && specifier
        .path()
        .to_ascii_lowercase()
        .contains("/node_modules/")
  }
}

fn join_package_name(path: &Path, package_name: &str) -> PathBuf {
  let mut path = path.to_path_buf();
  // ensure backslashes are used on windows
  for part in package_name.split('/') {
    path = path.join(part);
  }
  path
}
