// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_package_json::PackageJsonDepValue;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_node::DenoPkgJsonFsAdapter;
use deno_runtime::deno_node::NodePermissions;
use deno_runtime::deno_node::NodeRequireResolver;
use deno_runtime::deno_node::NpmProcessStateProvider;
use deno_runtime::deno_node::PackageJson;
use deno_runtime::fs_util::specifier_to_file_path;
use deno_semver::package::PackageReq;
use deno_semver::Version;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageJsonLoadError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::load_pkg_json;
use node_resolver::NpmResolver;

use crate::args::NpmProcessState;
use crate::args::NpmProcessStateKind;
use crate::util::fs::canonicalize_path_maybe_not_exists_with_fs;

use super::managed::normalize_pkg_name_for_node_modules_deno_folder;
use super::CliNpmResolver;
use super::InnerCliNpmResolverRef;

pub struct CliNpmResolverByonmCreateOptions {
  pub fs: Arc<dyn FileSystem>,
  // todo(dsherret): investigate removing this
  pub root_node_modules_dir: Option<PathBuf>,
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
  root_node_modules_dir: Option<PathBuf>,
}

impl ByonmCliNpmResolver {
  fn load_pkg_json(
    &self,
    path: &Path,
  ) -> Result<Option<Arc<PackageJson>>, PackageJsonLoadError> {
    load_pkg_json(&DenoPkgJsonFsAdapter(self.fs.as_ref()), path)
  }

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

  fn resolve_pkg_json_and_alias_for_req(
    &self,
    req: &PackageReq,
    referrer: &ModuleSpecifier,
  ) -> Result<Option<(Arc<PackageJson>, String)>, AnyError> {
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
    if let Ok(file_path) = specifier_to_file_path(referrer) {
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
      if req.version_req.matches(&version) {
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

impl NpmResolver for ByonmCliNpmResolver {
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    fn inner(
      fs: &dyn FileSystem,
      name: &str,
      referrer: &ModuleSpecifier,
    ) -> Result<PathBuf, PackageFolderResolveError> {
      let maybe_referrer_file = specifier_to_file_path(referrer).ok();
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

    let path = inner(&*self.fs, name, referrer)?;
    self.fs.realpath_sync(&path).map_err(|err| {
      PackageFolderResolveIoError {
        package_name: name.to_string(),
        referrer: referrer.clone(),
        source: err.into_io_error(),
      }
      .into()
    })
  }

  fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
    specifier.scheme() == "file"
      && specifier
        .path()
        .to_ascii_lowercase()
        .contains("/node_modules/")
  }
}

impl NodeRequireResolver for ByonmCliNpmResolver {
  fn ensure_read_permission(
    &self,
    permissions: &mut dyn NodePermissions,
    path: &Path,
  ) -> Result<(), AnyError> {
    if !path
      .components()
      .any(|c| c.as_os_str().to_ascii_lowercase() == "node_modules")
    {
      _ = permissions.check_read_path(path)?;
    }
    Ok(())
  }
}

impl NpmProcessStateProvider for ByonmCliNpmResolver {
  fn get_npm_process_state(&self) -> String {
    serde_json::to_string(&NpmProcessState {
      kind: NpmProcessStateKind::Byonm,
      local_node_modules_path: self
        .root_node_modules_dir
        .as_ref()
        .map(|p| p.to_string_lossy().to_string()),
    })
    .unwrap()
  }
}

impl CliNpmResolver for ByonmCliNpmResolver {
  fn into_npm_resolver(self: Arc<Self>) -> Arc<dyn NpmResolver> {
    self
  }

  fn into_require_resolver(self: Arc<Self>) -> Arc<dyn NodeRequireResolver> {
    self
  }

  fn into_process_state_provider(
    self: Arc<Self>,
  ) -> Arc<dyn NpmProcessStateProvider> {
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
    self.root_node_modules_dir.as_ref()
  }

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
    referrer: &ModuleSpecifier,
  ) -> Result<PathBuf, AnyError> {
    fn node_resolve_dir(
      fs: &dyn FileSystem,
      alias: &str,
      start_dir: &Path,
    ) -> Result<Option<PathBuf>, AnyError> {
      for ancestor in start_dir.ancestors() {
        let node_modules_folder = ancestor.join("node_modules");
        let sub_dir = join_package_name(&node_modules_folder, alias);
        if fs.is_dir_sync(&sub_dir) {
          return Ok(Some(canonicalize_path_maybe_not_exists_with_fs(
            &sub_dir, fs,
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
          node_resolve_dir(self.fs.as_ref(), &alias, pkg_json.dir_path())?
        {
          return Ok(resolved);
        }

        bail!(
          concat!(
            "Could not find \"{}\" in a node_modules folder. ",
            "Deno expects the node_modules/ directory to be up to date. ",
            "Did you forget to run `deno install`?"
          ),
          alias,
        );
      }
      None => {
        // now check if node_modules/.deno/ matches this constraint
        if let Some(folder) = self.resolve_folder_in_root_node_modules(req) {
          return Ok(folder);
        }

        bail!(
          concat!(
            "Could not find a matching package for 'npm:{}' in the node_modules ",
            "directory. Ensure you have all your JSR and npm dependencies listed ",
            "in your deno.json or package.json, then run `deno install`. Alternatively, ",
            r#"turn on auto-install by specifying `"nodeModulesDir": "auto"` in your "#,
            "deno.json file."
          ),
          req,
        );
      }
    }
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
