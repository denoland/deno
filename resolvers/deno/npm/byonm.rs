// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use deno_package_json::PackageJson;
use deno_package_json::PackageJsonDepValue;
use deno_package_json::PackageJsonRc;
use deno_path_util::url_to_file_path;
use deno_semver::package::PackageReq;
use deno_semver::StackString;
use deno_semver::Version;
use node_resolver::cache::NodeResolutionSys;
use node_resolver::errors::PackageFolderResolveError;
use node_resolver::errors::PackageFolderResolveIoError;
use node_resolver::errors::PackageJsonLoadError;
use node_resolver::errors::PackageNotFoundError;
use node_resolver::InNpmPackageChecker;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::PackageJsonResolverRc;
use node_resolver::UrlOrPathRef;
use sys_traits::FsCanonicalize;
use sys_traits::FsDirEntry;
use sys_traits::FsMetadata;
use sys_traits::FsRead;
use sys_traits::FsReadDir;
use thiserror::Error;
use url::Url;

use super::local::normalize_pkg_name_for_node_modules_deno_folder;

#[derive(Debug, Error, deno_error::JsError)]
pub enum ByonmResolvePkgFolderFromDenoReqError {
  #[class(generic)]
  #[error("Could not find \"{}\" in a node_modules folder. Deno expects the node_modules/ directory to be up to date. Did you forget to run `deno install`?", .0)]
  MissingAlias(StackString),
  #[class(inherit)]
  #[error(transparent)]
  PackageJson(#[from] PackageJsonLoadError),
  #[class(generic)]
  #[error("Could not find a matching package for 'npm:{}' in the node_modules directory. Ensure you have all your JSR and npm dependencies listed in your deno.json or package.json, then run `deno install`. Alternatively, turn on auto-install by specifying `\"nodeModulesDir\": \"auto\"` in your deno.json file.", .0)]
  UnmatchedReq(PackageReq),
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
}

pub struct ByonmNpmResolverCreateOptions<TSys: FsRead> {
  // todo(dsherret): investigate removing this
  pub root_node_modules_dir: Option<PathBuf>,
  pub sys: NodeResolutionSys<TSys>,
  pub pkg_json_resolver: PackageJsonResolverRc<TSys>,
}

#[allow(clippy::disallowed_types)]
pub type ByonmNpmResolverRc<TSys> =
  crate::sync::MaybeArc<ByonmNpmResolver<TSys>>;

#[derive(Debug)]
pub struct ByonmNpmResolver<
  TSys: FsCanonicalize + FsRead + FsMetadata + FsReadDir,
> {
  sys: NodeResolutionSys<TSys>,
  pkg_json_resolver: PackageJsonResolverRc<TSys>,
  root_node_modules_dir: Option<PathBuf>,
}

impl<TSys: Clone + FsCanonicalize + FsRead + FsMetadata + FsReadDir> Clone
  for ByonmNpmResolver<TSys>
{
  fn clone(&self) -> Self {
    Self {
      sys: self.sys.clone(),
      pkg_json_resolver: self.pkg_json_resolver.clone(),
      root_node_modules_dir: self.root_node_modules_dir.clone(),
    }
  }
}

impl<TSys: FsCanonicalize + FsRead + FsMetadata + FsReadDir>
  ByonmNpmResolver<TSys>
{
  pub fn new(options: ByonmNpmResolverCreateOptions<TSys>) -> Self {
    Self {
      root_node_modules_dir: options.root_node_modules_dir,
      sys: options.sys,
      pkg_json_resolver: options.pkg_json_resolver,
    }
  }

  pub fn root_node_modules_path(&self) -> Option<&Path> {
    self.root_node_modules_dir.as_deref()
  }

  fn load_pkg_json(
    &self,
    path: &Path,
  ) -> Result<Option<PackageJsonRc>, PackageJsonLoadError> {
    self.pkg_json_resolver.load_package_json(path)
  }

  /// Finds the ancestor package.json that contains the specified dependency.
  pub fn find_ancestor_package_json_with_dep(
    &self,
    dep_name: &str,
    referrer: &Url,
  ) -> Option<PackageJsonRc> {
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
    fn node_resolve_dir<TSys: FsCanonicalize + FsMetadata>(
      sys: &NodeResolutionSys<TSys>,
      alias: &str,
      start_dir: &Path,
    ) -> std::io::Result<Option<PathBuf>> {
      for ancestor in start_dir.ancestors() {
        let node_modules_folder = ancestor.join("node_modules");
        let sub_dir = join_package_name(Cow::Owned(node_modules_folder), alias);
        if sys.is_dir(&sub_dir) {
          return Ok(Some(
            deno_path_util::fs::canonicalize_path_maybe_not_exists(
              sys, &sub_dir,
            )?,
          ));
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
          node_resolve_dir(&self.sys, &alias, pkg_json.dir_path())?
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
  ) -> Result<Option<(PackageJsonRc, StackString)>, PackageJsonLoadError> {
    fn resolve_alias_from_pkg_json(
      req: &PackageReq,
      pkg_json: &PackageJson,
    ) -> Option<StackString> {
      let deps = pkg_json.resolve_local_package_json_deps();
      for (key, value) in
        deps.dependencies.iter().chain(deps.dev_dependencies.iter())
      {
        if let Ok(value) = value {
          match value {
            PackageJsonDepValue::File(_) => {
              // skip
            }
            PackageJsonDepValue::Req(dep_req) => {
              if dep_req.name == req.name
                && dep_req.version_req.intersects(&req.version_req)
              {
                return Some(key.clone());
              }
            }
            PackageJsonDepValue::Workspace(_workspace) => {
              if key.as_str() == req.name
                && req.version_req.tag() == Some("workspace")
              {
                return Some(key.clone());
              }
            }
          }
        }
      }
      None
    }

    // attempt to resolve the npm specifier from the referrer's package.json,
    let maybe_referrer_path = url_to_file_path(referrer).ok();
    if let Some(file_path) = maybe_referrer_path {
      for dir_path in file_path.as_path().ancestors().skip(1) {
        let package_json_path = dir_path.join("package.json");
        if let Some(pkg_json) = self.load_pkg_json(&package_json_path)? {
          if let Some(alias) =
            resolve_alias_from_pkg_json(req, pkg_json.as_ref())
          {
            return Ok(Some((pkg_json, alias)));
          }
        }
      }
    }

    // fall fallback to the project's package.json
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

    // now try to resolve based on the closest node_modules directory
    let maybe_referrer_path = url_to_file_path(referrer).ok();
    let search_node_modules = |node_modules: &Path| {
      if req.version_req.tag().is_some() {
        return None;
      }

      let pkg_folder = node_modules.join(&req.name);
      if let Ok(Some(dep_pkg_json)) =
        self.load_pkg_json(&pkg_folder.join("package.json"))
      {
        if dep_pkg_json.name.as_deref() == Some(req.name.as_str()) {
          let matches_req = dep_pkg_json
            .version
            .as_ref()
            .and_then(|v| Version::parse_from_npm(v).ok())
            .map(|version| req.version_req.matches(&version))
            .unwrap_or(true);
          if matches_req {
            return Some((dep_pkg_json, req.name.clone()));
          }
        }
      }
      None
    };
    if let Some(file_path) = &maybe_referrer_path {
      for dir_path in file_path.as_path().ancestors().skip(1) {
        if let Some(result) =
          search_node_modules(&dir_path.join("node_modules"))
        {
          return Ok(Some(result));
        }
      }
    }

    // and finally check the root node_modules directory
    if let Some(root_node_modules_dir) = &self.root_node_modules_dir {
      let already_searched = maybe_referrer_path
        .as_ref()
        .and_then(|referrer_path| {
          root_node_modules_dir
            .parent()
            .map(|root_dir| referrer_path.starts_with(root_dir))
        })
        .unwrap_or(false);
      if !already_searched {
        if let Some(result) = search_node_modules(root_node_modules_dir) {
          return Ok(Some(result));
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
    let Ok(entries) = self.sys.fs_read_dir(&node_modules_deno_dir) else {
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
      let Ok(entry) = entry else {
        continue;
      };
      let Ok(file_type) = entry.file_type() else {
        continue;
      };
      if !file_type.is_dir() {
        continue;
      }
      let entry_name = entry.file_name().to_string_lossy().into_owned();
      let Some(version_and_copy_idx) = entry_name.strip_prefix(&search_prefix)
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
          node_modules_deno_dir.join(&entry_name).join(".initialized");
        let Ok(contents) = self.sys.fs_read_to_string_lossy(&initialized_file)
        else {
          continue;
        };
        let mut tags = contents.split(',').map(str::trim);
        if tags.any(|t| t == tag) {
          if let Some((best_version_version, _)) = &best_version {
            if version > *best_version_version {
              best_version = Some((version, entry_name));
            }
          } else {
            best_version = Some((version, entry_name));
          }
        }
      } else if req.version_req.matches(&version) {
        if let Some((best_version_version, _)) = &best_version {
          if version > *best_version_version {
            best_version = Some((version, entry_name));
          }
        } else {
          best_version = Some((version, entry_name));
        }
      }
    }

    best_version.map(|(_version, entry_name)| {
      join_package_name(
        Cow::Owned(node_modules_deno_dir.join(entry_name).join("node_modules")),
        &req.name,
      )
    })
  }
}

impl<TSys: FsCanonicalize + FsMetadata + FsRead + FsReadDir>
  NpmPackageFolderResolver for ByonmNpmResolver<TSys>
{
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &UrlOrPathRef,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    fn inner<TSys: FsMetadata>(
      sys: &NodeResolutionSys<TSys>,
      name: &str,
      referrer: &UrlOrPathRef,
    ) -> Result<PathBuf, PackageFolderResolveError> {
      let maybe_referrer_file = referrer.path().ok();
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

          let sub_dir = join_package_name(node_modules_folder, name);
          if sys.is_dir(&sub_dir) {
            return Ok(sub_dir);
          }
        }
      }

      Err(
        PackageNotFoundError {
          package_name: name.to_string(),
          referrer: referrer.display(),
          referrer_extra: None,
        }
        .into(),
      )
    }

    let path = inner(&self.sys, name, referrer)?;
    self.sys.fs_canonicalize(&path).map_err(|err| {
      PackageFolderResolveIoError {
        package_name: name.to_string(),
        referrer: referrer.display(),
        source: err,
      }
      .into()
    })
  }
}

#[derive(Debug, Clone)]
pub struct ByonmInNpmPackageChecker;

impl InNpmPackageChecker for ByonmInNpmPackageChecker {
  fn in_npm_package(&self, specifier: &Url) -> bool {
    specifier.scheme() == "file"
      && specifier
        .path()
        .to_ascii_lowercase()
        .contains("/node_modules/")
  }
}

fn join_package_name(mut path: Cow<Path>, package_name: &str) -> PathBuf {
  // ensure backslashes are used on windows
  for part in package_name.split('/') {
    match path {
      Cow::Borrowed(inner) => path = Cow::Owned(inner.join(part)),
      Cow::Owned(ref mut path) => {
        path.push(part);
      }
    }
  }
  path.into_owned()
}
