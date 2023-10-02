// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod managed;

// todo(#18967): move the cache, registry, and tarball into the managed folder
mod cache;
mod registry;
mod tarball;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::url::Url;
use deno_graph::NpmPackageReqResolution;
use deno_lockfile::Lockfile;
use deno_npm::registry::NpmRegistryApi;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmSystemInfo;
use deno_runtime::deno_node::NpmResolver;
use deno_semver::npm::NpmPackageNvReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;

use crate::args::PackageJsonDepsProvider;
use cache::NpmCache;
use managed::create_npm_fs_resolver;
use managed::NpmResolution;
use managed::PackageJsonDepsInstaller;
use registry::CliNpmRegistryApi;

pub use cache::NpmCacheDir;
pub use managed::ManagedCliNpmResolver;
pub use managed::NpmProcessState;

pub enum CliNpmResolverCreateOptionsSnapshot {
  ResolveFromLockfile(Arc<Mutex<Lockfile>>),
  Provided(Option<ValidSerializedNpmResolutionSnapshot>),
}

pub enum CliNpmResolverCreateOptionsPackageJsonInstaller {
  ConditionalInstall(Arc<PackageJsonDepsProvider>),
  NoInstall,
}

// todo(THIS PR): create managed specific options
pub struct CliNpmResolverCreateOptions {
  pub snapshot: CliNpmResolverCreateOptionsSnapshot,
  pub maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  pub fs: Arc<dyn deno_runtime::deno_fs::FileSystem>,
  pub http_client: Arc<crate::http_util::HttpClient>,
  pub npm_global_cache_dir: PathBuf,
  pub cache_setting: crate::args::CacheSetting,
  pub text_only_progress_bar: crate::util::progress_bar::ProgressBar,
  pub maybe_node_modules_path: Option<PathBuf>,
  pub npm_system_info: NpmSystemInfo,
  pub package_json_installer: CliNpmResolverCreateOptionsPackageJsonInstaller,
}

pub async fn create_cli_npm_resolver_for_lsp(
  options: CliNpmResolverCreateOptions,
) -> Arc<dyn CliNpmResolver> {
  let npm_cache = create_cache(&options);
  let npm_api = create_api(&options, npm_cache.clone());
  let snapshot = match resolve_snapshot(&npm_api, options.snapshot).await {
    Ok(snapshot) => snapshot,
    Err(err) => {
      log::warn!("failed to resolve snapshot: {}", err);
      None
    }
  };
  create_inner(
    npm_cache,
    npm_api,
    snapshot,
    options.maybe_lockfile,
    options.fs,
    options.text_only_progress_bar,
    options.maybe_node_modules_path,
    options.npm_system_info,
    options.package_json_installer,
  )
}

pub async fn create_cli_npm_resolver(
  options: CliNpmResolverCreateOptions,
) -> Result<Arc<dyn CliNpmResolver>, AnyError> {
  let npm_cache = create_cache(&options);
  let npm_api = create_api(&options, npm_cache.clone());
  let snapshot = resolve_snapshot(&npm_api, options.snapshot).await?;
  Ok(create_inner(
    npm_cache,
    npm_api,
    snapshot,
    options.maybe_lockfile,
    options.fs,
    options.text_only_progress_bar,
    options.maybe_node_modules_path,
    options.npm_system_info,
    options.package_json_installer,
  ))
}

#[allow(clippy::too_many_arguments)]
fn create_inner(
  npm_cache: Arc<NpmCache>,
  npm_api: Arc<CliNpmRegistryApi>,
  snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  fs: Arc<dyn deno_runtime::deno_fs::FileSystem>,
  text_only_progress_bar: crate::util::progress_bar::ProgressBar,
  node_modules_dir_path: Option<PathBuf>,
  npm_system_info: NpmSystemInfo,
  package_json_installer: CliNpmResolverCreateOptionsPackageJsonInstaller,
) -> Arc<dyn CliNpmResolver> {
  let resolution = Arc::new(NpmResolution::from_serialized(
    npm_api.clone(),
    snapshot,
    maybe_lockfile.clone(),
  ));
  let npm_fs_resolver = create_npm_fs_resolver(
    fs.clone(),
    npm_cache.clone(),
    &text_only_progress_bar,
    crate::args::npm_registry_default_url().to_owned(),
    resolution.clone(),
    node_modules_dir_path,
    npm_system_info.clone(),
  );
  let package_json_deps_installer = match package_json_installer {
    CliNpmResolverCreateOptionsPackageJsonInstaller::ConditionalInstall(
      provider,
    ) => Arc::new(PackageJsonDepsInstaller::new(
      provider,
      npm_api.clone(),
      resolution.clone(),
    )),
    CliNpmResolverCreateOptionsPackageJsonInstaller::NoInstall => {
      Arc::new(PackageJsonDepsInstaller::no_op())
    }
  };
  Arc::new(ManagedCliNpmResolver::new(
    npm_api,
    fs,
    resolution,
    npm_fs_resolver,
    npm_cache,
    maybe_lockfile,
    package_json_deps_installer,
    text_only_progress_bar,
    npm_system_info,
  ))
}

fn create_cache(options: &CliNpmResolverCreateOptions) -> Arc<NpmCache> {
  Arc::new(NpmCache::new(
    NpmCacheDir::new(options.npm_global_cache_dir.clone()),
    options.cache_setting.clone(),
    options.fs.clone(),
    options.http_client.clone(),
    options.text_only_progress_bar.clone(),
  ))
}

fn create_api(
  options: &CliNpmResolverCreateOptions,
  npm_cache: Arc<NpmCache>,
) -> Arc<CliNpmRegistryApi> {
  Arc::new(CliNpmRegistryApi::new(
    crate::args::npm_registry_default_url().to_owned(),
    npm_cache.clone(),
    options.http_client.clone(),
    options.text_only_progress_bar.clone(),
  ))
}

async fn resolve_snapshot(
  api: &CliNpmRegistryApi,
  snapshot: CliNpmResolverCreateOptionsSnapshot,
) -> Result<Option<ValidSerializedNpmResolutionSnapshot>, AnyError> {
  match snapshot {
    CliNpmResolverCreateOptionsSnapshot::ResolveFromLockfile(lockfile) => {
      if !lockfile.lock().overwrite {
        let snapshot = snapshot_from_lockfile(lockfile.clone(), api)
          .await
          .with_context(|| {
            format!(
              "failed reading lockfile '{}'",
              lockfile.lock().filename.display()
            )
          })?;
        // clear the memory cache to reduce memory usage
        api.clear_memory_cache();
        Ok(Some(snapshot))
      } else {
        Ok(None)
      }
    }
    CliNpmResolverCreateOptionsSnapshot::Provided(snapshot) => Ok(snapshot),
  }
}

async fn snapshot_from_lockfile(
  lockfile: Arc<Mutex<Lockfile>>,
  api: &dyn NpmRegistryApi,
) -> Result<ValidSerializedNpmResolutionSnapshot, AnyError> {
  let incomplete_snapshot = {
    let lock = lockfile.lock();
    deno_npm::resolution::incomplete_snapshot_from_lockfile(&lock)?
  };
  let snapshot =
    deno_npm::resolution::snapshot_from_lockfile(incomplete_snapshot, api)
      .await?;
  Ok(snapshot)
}

pub enum InnerCliNpmResolverRef<'a> {
  Managed(&'a ManagedCliNpmResolver),
  #[allow(dead_code)]
  Byonm(&'a ByonmCliNpmResolver),
}

pub trait CliNpmResolver: NpmResolver {
  fn into_npm_resolver(self: Arc<Self>) -> Arc<dyn NpmResolver>;

  fn clone_snapshotted(&self) -> Arc<dyn CliNpmResolver>;

  fn root_dir_url(&self) -> &Url;

  fn as_inner(&self) -> InnerCliNpmResolverRef;

  fn as_managed(&self) -> Option<&ManagedCliNpmResolver> {
    match self.as_inner() {
      InnerCliNpmResolverRef::Managed(inner) => Some(inner),
      InnerCliNpmResolverRef::Byonm(_) => None,
    }
  }

  fn node_modules_path(&self) -> Option<PathBuf>;

  /// Checks if the provided package req's folder is cached.
  fn is_pkg_req_folder_cached(&self, req: &PackageReq) -> bool;

  /// Resolves a package requirement for deno graph. This should only be
  /// called by deno_graph's NpmResolver or for resolving packages in
  /// a package.json
  fn resolve_npm_for_deno_graph(
    &self,
    pkg_req: &PackageReq,
  ) -> NpmPackageReqResolution;

  fn resolve_pkg_nv_ref_from_pkg_req_ref(
    &self,
    req_ref: &NpmPackageReqReference,
  ) -> Result<NpmPackageNvReference, PackageReqNotFoundError>;

  /// Resolve the root folder of the package the provided specifier is in.
  ///
  /// This will error when the provided specifier is not in an npm package.
  fn resolve_pkg_folder_from_specifier(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Option<PathBuf>, AnyError>;

  fn resolve_pkg_folder_from_deno_module_req(
    &self,
    req: &PackageReq,
  ) -> Result<PathBuf, AnyError>;

  fn resolve_pkg_folder_from_deno_module(
    &self,
    nv: &PackageNv,
  ) -> Result<PathBuf, AnyError>;

  /// Gets the state of npm for the process.
  fn get_npm_process_state(&self) -> String;

  // todo(#18967): should instead return a hash state of the resolver
  // or perhaps this could be non-BYONM only and byonm always runs deno check
  fn package_reqs(&self) -> HashMap<PackageReq, PackageNv>;
}

// todo(#18967): implement this
pub struct ByonmCliNpmResolver;
