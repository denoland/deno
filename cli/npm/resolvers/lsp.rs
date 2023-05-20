// todo: DELETE THIS -- NOT USED

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_fs::FileSystem;

use crate::npm::NpmCache;
use crate::npm::NpmResolution;
use crate::util::progress_bar::ProgressBar;

use super::global::GlobalNpmPackageResolver;
use super::local::LocalNpmPackageResolver;
use super::NpmPackageFsResolver;

struct LspNpmPackageFsResolver {
  local: LocalNpmPackageResolver,
  global: GlobalNpmPackageResolver,
}

#[async_trait]
impl NpmPackageFsResolver for LspNpmPackageFsResolver {
  fn root_dir_url(&self) -> &Url {
    self.local.root_dir_url()
  }

  fn node_modules_path(&self) -> Option<PathBuf> {
    self.local.node_modules_path()
  }

  fn package_folder(
    &self,
    package_id: &deno_npm::NpmPackageId,
  ) -> Result<PathBuf, deno_core::error::AnyError> {
    if let Ok(folder) = self.local.package_folder(package_id) {
      if folder.exists() {
        return Ok(folder);
      }
    }
    self.global.package_folder(package_id)
  }

  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    referrer: &deno_ast::ModuleSpecifier,
    mode: deno_runtime::deno_node::NodeResolutionMode,
  ) -> Result<PathBuf, deno_core::error::AnyError> {
    if let Ok(folder) = self
      .local
      .resolve_package_folder_from_package(name, referrer, mode)
    {
      if folder.exists() {
        return Ok(folder);
      }
    }
    self
      .global
      .resolve_package_folder_from_package(name, referrer, mode)
  }

  fn resolve_package_folder_from_specifier(
    &self,
    specifier: &deno_ast::ModuleSpecifier,
  ) -> Result<PathBuf, deno_core::error::AnyError> {
    if let Ok(folder) =
      self.local.resolve_package_folder_from_specifier(specifier)
    {
      if folder.exists() {
        return Ok(folder);
      }
    }
    self.global.resolve_package_folder_from_specifier(specifier)
  }

  async fn cache_packages(&self) -> Result<(), AnyError> {
    self.global.cache_packages().await
  }

  fn ensure_read_permission(
    &self,
    _permissions: &dyn deno_runtime::deno_node::NodePermissions,
    _path: &std::path::Path,
  ) -> Result<(), deno_core::error::AnyError> {
    Ok(()) // lsp, so this is ok
  }
}

pub fn create_lsp_npm_fs_resolver(
  fs: Arc<dyn FileSystem>,
  cache: Arc<NpmCache>,
  progress_bar: &ProgressBar,
  registry_url: Url,
  resolution: Arc<NpmResolution>,
  maybe_node_modules_path: Option<PathBuf>,
  system_info: NpmSystemInfo,
) -> Arc<dyn NpmPackageFsResolver> {
  let maybe_local = maybe_node_modules_path.map(|node_modules_folder| {
    LocalNpmPackageResolver::new(
      fs.clone(),
      cache.clone(),
      progress_bar.clone(),
      registry_url.clone(),
      node_modules_folder,
      resolution.clone(),
      system_info.clone(),
    )
  });
  let global = GlobalNpmPackageResolver::new(
    fs,
    cache,
    registry_url,
    resolution,
    system_info,
  );
  match maybe_local {
    Some(local) => Arc::new(LspNpmPackageFsResolver { local, global }),
    None => Arc::new(global),
  }
}
