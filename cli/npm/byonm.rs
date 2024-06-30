// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::load_pkg_json;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NpmResolver;
use deno_runtime::deno_node::PackageJson;
use deno_semver::package::PackageReq;

use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::util::fs::canonicalize_path_maybe_not_exists_with_fs;
use deno_runtime::fs_util::specifier_to_file_path;

use super::CliNpmResolver;
use super::InnerCliNpmResolverRef;

pub struct CliNpmResolverByonmCreateOptions {
  pub fs: Arc<dyn FileSystem>,
  pub root_node_modules_dir: PathBuf,
}

pub fn create_byonm_npm_resolver(
  options: CliNpmResolverByonmCreateOptions,
) -> Arc<dyn CliNpmResolver> {
  Arc::new(ByonmCliNpmResolver {
    fs: options.fs,
    root_node_modules_dir: options.root_node_modules_dir,
  })
}

#[derive(Debug)]
pub struct ByonmCliNpmResolver {
  fs: Arc<dyn FileSystem>,
  root_node_modules_dir: PathBuf,
}

impl ByonmCliNpmResolver {
  /// Finds the ancestor package.json that contains the specified dependency.
  pub fn find_ancestor_package_json_with_dep(
    &self,
    dep_name: &str,
    referrer: &ModuleSpecifier,
  ) -> Option<Arc<PackageJson>> {
    let referrer_path = referrer.to_file_path().ok()?;
    let mut current_folder = referrer_path.parent()?;
    loop {
      let pkg_json_path = current_folder.join("package.json");
      if let Ok(Some(pkg_json)) =
        load_pkg_json(self.fs.as_ref(), &pkg_json_path)
      {
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

  fn resolve_pkg_json_and_alias_for_req(
    &self,
    req: &PackageReq,
    referrer: &ModuleSpecifier,
  ) -> Result<(Arc<PackageJson>, String), AnyError> {
    fn resolve_alias_from_pkg_json(
      req: &PackageReq,
      pkg_json: &PackageJson,
    ) -> Option<String> {
      let deps = pkg_json.resolve_local_package_json_version_reqs();
      for (key, value) in deps {
        if let Ok(value) = value {
          if value.name == req.name
            && value.version_req.intersects(&req.version_req)
          {
            return Some(key);
          }
        }
      }
      None
    }

    // attempt to resolve the npm specifier from the referrer's package.json,
    if let Ok(file_path) = specifier_to_file_path(referrer) {
      let mut current_path = file_path.as_path();
      while let Some(dir_path) = current_path.parent() {
        let package_json_path = dir_path.join("package.json");
        if let Some(pkg_json) =
          load_pkg_json(self.fs.as_ref(), &package_json_path)?
        {
          if let Some(alias) =
            resolve_alias_from_pkg_json(req, pkg_json.as_ref())
          {
            return Ok((pkg_json, alias));
          }
        }
        current_path = dir_path;
      }
    }

    // otherwise, fall fallback to the project's package.json
    let root_pkg_json_path = self
      .root_node_modules_dir
      .parent()
      .unwrap()
      .join("package.json");
    if let Some(pkg_json) =
      load_pkg_json(self.fs.as_ref(), &root_pkg_json_path)?
    {
      if let Some(alias) = resolve_alias_from_pkg_json(req, pkg_json.as_ref()) {
        return Ok((pkg_json, alias));
      }
    }

    bail!(
      concat!(
        "Could not find a matching package for 'npm:{}' in a package.json file. ",
        "You must specify this as a package.json dependency when the ",
        "node_modules folder is not managed by Deno.",
      ),
      req,
    );
  }
}

impl NpmResolver for ByonmCliNpmResolver {
  fn get_npm_process_state(&self) -> String {
    serde_json::to_string(&NpmProcessState {
      kind: NpmProcessStateKind::Byonm,
      local_node_modules_path: Some(
        self.root_node_modules_dir.to_string_lossy().to_string(),
      ),
    })
    .unwrap()
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    fn inner(
      fs: &dyn FileSystem,
      name: &str,
      referrer: &ModuleSpecifier,
    ) -> Result<PathBuf, AnyError> {
      let referrer_file = specifier_to_file_path(referrer)?;
      let mut current_folder = referrer_file.parent().unwrap();
      loop {
        let node_modules_folder = if current_folder.ends_with("node_modules") {
          Cow::Borrowed(current_folder)
        } else {
          Cow::Owned(current_folder.join("node_modules"))
        };

        let sub_dir = join_package_name(&node_modules_folder, name);
        if fs.is_dir_sync(&sub_dir) {
          return Ok(sub_dir);
        }

        if let Some(parent) = current_folder.parent() {
          current_folder = parent;
        } else {
          break;
        }
      }

      bail!(
        "could not find package '{}' from referrer '{}'.",
        name,
        referrer
      );
    }

    let path = inner(&*self.fs, name, referrer)?;
    Ok(self.fs.realpath_sync(&path)?)
  }

  fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    specifier.scheme() == "file"
      && specifier
        .path()
        .to_ascii_lowercase()
        .contains("/node_modules/")
  }

  fn ensure_read_permission(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError> {
    if !path
      .components()
      .any(|c| c.as_os_str().to_ascii_lowercase() == "node_modules")
    {
      permissions.check_read(path)?;
    }
    Ok(())
  }
}

impl CliNpmResolver for ByonmCliNpmResolver {
  fn into_npm_resolver(self: Arc<Self>) -> Arc<dyn NpmResolver> {
    self
  }

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver> {
    Arc::new(Self {
      fs: self.fs.clone(),
      root_node_modules_dir: self.root_node_modules_dir.clone(),
    })
  }

  fn as_inner(&self) -> InnerCliNpmResolverRef {
    InnerCliNpmResolverRef::Byonm(self)
  }

  fn root_node_modules_path(&self) -> Option<&PathBuf> {
    Some(&self.root_node_modules_dir)
  }

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    // resolve the pkg json and alias
    let (pkg_json, alias) =
      self.resolve_pkg_json_and_alias_for_req(req, referrer)?;
    // now try node resolution
    for ancestor in pkg_json.path.parent().unwrap().ancestors() {
      let node_modules_folder = ancestor.join("node_modules");
      let sub_dir = join_package_name(&node_modules_folder, &alias);
      if self.fs.is_dir_sync(&sub_dir) {
        return Ok(canonicalize_path_maybe_not_exists_with_fs(
          &sub_dir,
          self.fs.as_ref(),
        )?);
      }
    }

    bail!(
      concat!(
        "Could not find \"{}\" in a node_modules folder. ",
        "Deno expects the node_modules/ directory to be up to date. ",
        "Did you forget to run `npm install`?"
      ),
      alias,
    );
  }

  fn check_state_hash(&self) -> Option<u64> {
    // it is very difficult to determine the check state hash for byonm
    // so we just return None to signify check caching is not supported
    None
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
