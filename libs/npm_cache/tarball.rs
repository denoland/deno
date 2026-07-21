// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashMap;
use std::sync::Arc;

use deno_error::JsErrorBox;
use deno_npm::registry::NpmPackageVersionDistInfo;
use deno_npmrc::ResolvedNpmRc;
use deno_semver::package::PackageNv;
use futures::FutureExt;
use futures::future::LocalBoxFuture;
use parking_lot::Mutex;
use url::Url;

use crate::NpmCache;
use crate::NpmCacheHttpClient;
use crate::NpmCacheHttpClientResponse;
use crate::NpmCacheSetting;
use crate::NpmCacheSys;
use crate::remote::maybe_auth_header_value_for_npm_registry;
use crate::rt::MultiRuntimeAsyncValueCreator;
use crate::rt::spawn_blocking;
use crate::tarball_extract::TarballExtractionMode;
use crate::tarball_extract::verify_and_decompress_tarball;
use crate::tarball_extract::write_extracted_tarball;

type LoadResult = Result<(), Arc<JsErrorBox>>;
type LoadFuture = LocalBoxFuture<'static, LoadResult>;

#[derive(Debug, Clone)]
enum MemoryCacheItem {
  /// The cache item hasn't finished yet.
  Pending(Arc<MultiRuntimeAsyncValueCreator<LoadResult>>),
  /// The result errored.
  Errored(Arc<JsErrorBox>),
  /// This package has already been cached.
  Cached,
}

/// Maximum number of concurrent filesystem write operations for tarball extraction.
/// Filesystem operations (open/write/close per file) dominate extraction time,
/// and on macOS APFS has an internal mutex that makes highly parallel filesystem
/// operations contend heavily. Limiting concurrency reduces this contention.
/// Decompression (CPU-bound) is not gated by this limit.
#[cfg(not(target_arch = "wasm32"))]
const MAX_CONCURRENT_FS_WRITES: usize =
  if cfg!(target_os = "macos") { 4 } else { 128 };

/// Coordinates caching of tarballs being loaded from
/// the npm registry.
///
/// This is shared amongst all the workers.
#[derive(Debug)]
pub struct TarballCache<THttpClient: NpmCacheHttpClient, TSys: NpmCacheSys> {
  cache: Arc<NpmCache<TSys>>,
  http_client: Arc<THttpClient>,
  sys: TSys,
  npmrc: Arc<ResolvedNpmRc>,
  memory_cache: Mutex<HashMap<PackageNv, MemoryCacheItem>>,
  #[cfg(not(target_arch = "wasm32"))]
  fs_write_semaphore: tokio::sync::Semaphore,

  reporter: Option<Arc<dyn TarballCacheReporter>>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("Failed caching npm package '{package_nv}'")]
pub struct EnsurePackageError {
  package_nv: Box<PackageNv>,
  #[source]
  source: Arc<JsErrorBox>,
}

pub trait TarballCacheReporter: std::fmt::Debug + Send + Sync {
  fn download_started(&self, _nv: &PackageNv) {}
  fn downloaded(&self, _nv: &PackageNv) {}
  fn reused_cache(&self, _nv: &PackageNv) {}
}

impl<THttpClient: NpmCacheHttpClient, TSys: NpmCacheSys>
  TarballCache<THttpClient, TSys>
{
  pub fn new(
    cache: Arc<NpmCache<TSys>>,
    http_client: Arc<THttpClient>,
    sys: TSys,
    npmrc: Arc<ResolvedNpmRc>,
    reporter: Option<Arc<dyn TarballCacheReporter>>,
  ) -> Self {
    Self {
      cache,
      http_client,
      sys,
      npmrc,
      memory_cache: Default::default(),
      #[cfg(not(target_arch = "wasm32"))]
      fs_write_semaphore: tokio::sync::Semaphore::new(MAX_CONCURRENT_FS_WRITES),
      reporter,
    }
  }

  pub async fn ensure_package(
    self: &Arc<Self>,
    package_nv: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
  ) -> Result<(), EnsurePackageError> {
    self
      .ensure_package_inner(package_nv, dist)
      .await
      .map_err(|source| EnsurePackageError {
        package_nv: Box::new(package_nv.clone()),
        source,
      })
  }

  async fn ensure_package_inner(
    self: &Arc<Self>,
    package_nv: &PackageNv,
    dist: &NpmPackageVersionDistInfo,
  ) -> Result<(), Arc<JsErrorBox>> {
    let cache_item = {
      let mut mem_cache = self.memory_cache.lock();
      if let Some(cache_item) = mem_cache.get(package_nv) {
        cache_item.clone()
      } else {
        let value_creator = MultiRuntimeAsyncValueCreator::new({
          let tarball_cache = self.clone();
          let package_nv = package_nv.clone();
          let dist = dist.clone();
          Box::new(move || {
            tarball_cache.create_setup_future(package_nv.clone(), dist.clone())
          })
        });
        let cache_item = MemoryCacheItem::Pending(Arc::new(value_creator));
        mem_cache.insert(package_nv.clone(), cache_item.clone());
        cache_item
      }
    };

    match cache_item {
      MemoryCacheItem::Cached => Ok(()),
      MemoryCacheItem::Errored(err) => Err(err),
      MemoryCacheItem::Pending(creator) => {
        let result = creator.get().await;
        match result {
          Ok(_) => {
            *self.memory_cache.lock().get_mut(package_nv).unwrap() =
              MemoryCacheItem::Cached;
            Ok(())
          }
          Err(err) => {
            *self.memory_cache.lock().get_mut(package_nv).unwrap() =
              MemoryCacheItem::Errored(err.clone());
            Err(err)
          }
        }
      }
    }
  }

  fn create_setup_future(
    self: &Arc<Self>,
    package_nv: PackageNv,
    dist: NpmPackageVersionDistInfo,
  ) -> LoadFuture {
    let tarball_cache = self.clone();
    let sys = self.sys.clone();
    let reporter = self.reporter.clone();
    async move {
      let registry_url = tarball_cache.npmrc.get_registry_url(&package_nv.name);
      let package_folder =
        tarball_cache.cache.package_folder_for_nv_and_url(&package_nv, registry_url);
      let should_use_cache = tarball_cache.cache.should_use_cache_for_package(&package_nv);
      let package_folder_exists = tarball_cache.sys.fs_exists_no_err(&package_folder);
      if should_use_cache && package_folder_exists {
        if let Some(reporter) = reporter {
          reporter.reused_cache(&package_nv);
        }
        return Ok(());
      } else if tarball_cache.cache.cache_setting() == &NpmCacheSetting::Only {
        return Err(JsErrorBox::new(
          "NotCached",
          format!(
            "npm package not found in cache: \"{}\", --cached-only is specified.",
            &package_nv.name
          )
        )
        );
      }

      if dist.tarball.is_empty() {
        return Err(JsErrorBox::generic("Tarball URL was empty."));
      }

      // If the manifest points the tarball at the canonical public npm registry
      // but a different registry is configured for this package (e.g. a private
      // proxy/mirror set via `.npmrc` or `NPM_CONFIG_REGISTRY`), download the
      // tarball from the configured registry instead. Some registries proxy npm
      // but don't rewrite `dist.tarball` in their packuments, so without this the
      // configured registry would be silently bypassed for the actual package
      // bytes. This mirrors npm's `replace-registry-host=npmjs` default.
      let tarball_uri = Url::parse(&dist.tarball).map_err(JsErrorBox::from_err)?;
      let tarball_uri =
        maybe_relocate_npm_registry_tarball(tarball_uri, registry_url);

      // IMPORTANT: npm registries may specify tarball URLs at different URLS than the
      // registry, so we MUST get the auth for the tarball URL and not the registry URL.
      // When the tarball path doesn't match a configured auth scope, fall back to the
      // package's scoped registry auth if the tarball is on the same origin (e.g. GitLab
      // instance-level registries serve tarballs from a different path than the registry).
      let maybe_registry_config = tarball_cache.npmrc.tarball_config_for_package(&tarball_uri, &package_nv.name);
      let maybe_auth_header = maybe_registry_config.and_then(|c| maybe_auth_header_value_for_npm_registry(c).ok()?);

      if let Some(reporter) = &reporter {
        reporter.download_started(&package_nv);

      }
      // The URL we actually download from (post-relocation). Reported in the
      // error paths below so they point at the registry Deno really contacted
      // rather than the original `dist.tarball`, which may have been relocated
      // away from `registry.npmjs.org`.
      let fetched_tarball_url = tarball_uri.to_string();
      let result = tarball_cache.http_client
        .download_with_retries_on_any_tokio_runtime(tarball_uri, maybe_auth_header, None, maybe_registry_config.map(|c| c.as_ref()))
        .await;
      if let Some(reporter) = &reporter {
        reporter.downloaded(&package_nv);
      }
      // The tarball URL had no matching auth, but the package's scoped registry
      // does have credentials. Registries signal this either with a 401, or
      // (notably GitLab instance-level npm registries) with a 404 to avoid
      // disclosing whether a private package exists. In both cases, point the
      // user at npm's "No auth for URI" guidance instead of an opaque error.
      let scoped_registry_has_auth = maybe_registry_config.is_none()
        && tarball_cache
          .npmrc
          .get_registry_config(&package_nv.name)
          .auth_token
          .is_some();
      let maybe_bytes = match result {
        Ok(response) => match response {
          NpmCacheHttpClientResponse::NotModified => unreachable!(), // no e-tag
          NpmCacheHttpClientResponse::NotFound => None,
          NpmCacheHttpClientResponse::Bytes(r) => Some(r.bytes),
        },
        Err(err) => {
          if err.status_code == Some(401) && scoped_registry_has_auth {
            return Err(scoped_registry_auth_error(&fetched_tarball_url, registry_url));
          }
          return Err(JsErrorBox::from_err(err))
        },
      };
      match maybe_bytes {
        Some(bytes) => {
          let extraction_mode = if should_use_cache || !package_folder_exists {
            TarballExtractionMode::SiblingTempDir
          } else {
            // The user ran with `--reload`, so overwrite the package instead of
            // deleting it since the package might get corrupted if a user kills
            // their deno process while it's deleting a package directory
            //
            // We can't rename this folder and delete it because the folder
            // may be in use by another process or may now contain hardlinks,
            // which will cause windows to throw an "AccessDenied" error when
            // renaming. So we settle for overwriting.
            TarballExtractionMode::Overwrite
          };
          // Phase 1: verify integrity + decompress (CPU-bound, no concurrency limit)
          let tar_data = spawn_blocking(move || {
            verify_and_decompress_tarball(&package_nv, &bytes, &dist)
          })
          .await
          .map_err(JsErrorBox::from_err)?
          .map_err(JsErrorBox::from_err)?;
          // Phase 2: write to disk (I/O-bound, limited concurrency to
          // avoid filesystem contention — especially on macOS APFS)
          #[cfg(not(target_arch = "wasm32"))]
          let _permit = tarball_cache
            .fs_write_semaphore
            .acquire()
            .await
            .map_err(|e| JsErrorBox::generic(e.to_string()))?;

          spawn_blocking(move || {
            write_extracted_tarball(
              &sys,
              &tar_data,
              &package_folder,
              extraction_mode,
            )
          })
          .await
          .map_err(JsErrorBox::from_err)?
          .map_err(JsErrorBox::from_err)
        }
        None => {
          // A 404 lands here (mapped to `NotFound`), so the 401 check above is
          // bypassed -- surface the auth hint here too when applicable.
          if scoped_registry_has_auth {
            return Err(scoped_registry_auth_error(&fetched_tarball_url, registry_url));
          }
          Err(JsErrorBox::generic(format!("Could not find npm package tarball at: {}", fetched_tarball_url)))
        }
      }
    }
    .map(|r| r.map_err(Arc::new))
    .boxed_local()
  }
}

/// The host of the canonical public npm registry.
const NPM_REGISTRY_HOST: &str = "registry.npmjs.org";

/// npm rewrites the host of tarball URLs that point at the public npm registry
/// to the configured registry (its `replace-registry-host` option defaults to
/// `npmjs`). We do the same, so that a configured private registry/proxy is used
/// to download the package bytes instead of being silently bypassed when a proxy
/// doesn't rewrite `dist.tarball` in its packuments.
///
/// Unlike npm, the base path of the configured registry is preserved (e.g.
/// Artifactory's `/api/npm/npm-remote/`), since that's where such proxies
/// actually serve tarballs.
fn maybe_relocate_npm_registry_tarball(
  tarball_uri: Url,
  registry_url: &Url,
) -> Url {
  // Only relocate tarballs served by the public npm registry.
  if tarball_uri.host_str() != Some(NPM_REGISTRY_HOST) {
    return tarball_uri;
  }
  // Nothing to do when the configured registry is also the public npm registry.
  if registry_url.host_str() == Some(NPM_REGISTRY_HOST) {
    return tarball_uri;
  }
  let mut base = registry_url.clone();
  // Ensure the base path ends in a slash so joining appends the tarball path
  // rather than replacing the registry's last path segment.
  if !base.path().ends_with('/') {
    let with_slash = format!("{}/", base.path());
    base.set_path(&with_slash);
  }
  // Preserve the tarball's path (the npm layout `/<name>/-/<file>.tgz`),
  // relocated under the configured registry.
  match base.join(tarball_uri.path().trim_start_matches('/')) {
    Ok(mut relocated) => {
      relocated.set_query(tarball_uri.query());
      relocated.set_fragment(tarball_uri.fragment());
      relocated
    }
    // If for some reason we can't build the URL, fall back to the original.
    Err(_) => tarball_uri,
  }
}

fn scoped_registry_auth_error(
  tarball_uri: &str,
  registry_url: &Url,
) -> JsErrorBox {
  JsErrorBox::generic(format!(
    concat!(
      "No auth for tarball URI, but present for scoped registry.\n\n",
      "Tarball URI: {}\n",
      "Scope URI: {}\n\n",
      "More info here: https://github.com/npm/cli/wiki/%22No-auth-for-URI,-but-auth-present-for-scoped-registry%22"
    ),
    tarball_uri, registry_url,
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn relocate(tarball: &str, registry: &str) -> String {
    maybe_relocate_npm_registry_tarball(
      Url::parse(tarball).unwrap(),
      &Url::parse(registry).unwrap(),
    )
    .to_string()
  }

  #[test]
  fn relocates_npmjs_tarball_to_configured_registry() {
    // Registry at the domain root.
    assert_eq!(
      relocate(
        "https://registry.npmjs.org/pako/-/pako-2.1.0.tgz",
        "https://mirror.example.com/",
      ),
      "https://mirror.example.com/pako/-/pako-2.1.0.tgz",
    );
    // Scoped package.
    assert_eq!(
      relocate(
        "https://registry.npmjs.org/@datadog/datadog-api-client/-/datadog-api-client-1.45.0.tgz",
        "https://mirror.example.com/",
      ),
      "https://mirror.example.com/@datadog/datadog-api-client/-/datadog-api-client-1.45.0.tgz",
    );
  }

  #[test]
  fn relocates_and_preserves_registry_base_path() {
    // e.g. Artifactory serves npm under a sub-path; the base path must be kept.
    assert_eq!(
      relocate(
        "https://registry.npmjs.org/@datadog/datadog-api-client/-/datadog-api-client-1.45.0.tgz",
        "https://artifactory.example.com/api/npm/npm-remote/",
      ),
      "https://artifactory.example.com/api/npm/npm-remote/@datadog/datadog-api-client/-/datadog-api-client-1.45.0.tgz",
    );
    // Also works when the configured registry URL lacks a trailing slash.
    assert_eq!(
      relocate(
        "https://registry.npmjs.org/pako/-/pako-2.1.0.tgz",
        "https://artifactory.example.com/api/npm/npm-remote",
      ),
      "https://artifactory.example.com/api/npm/npm-remote/pako/-/pako-2.1.0.tgz",
    );
  }

  #[test]
  fn does_not_relocate_when_registry_is_npmjs() {
    let tarball = "https://registry.npmjs.org/pako/-/pako-2.1.0.tgz";
    assert_eq!(relocate(tarball, "https://registry.npmjs.org/"), tarball,);
  }

  #[test]
  fn does_not_relocate_non_npmjs_tarball() {
    // The registry already rewrote the tarball to point at itself.
    let tarball = "https://artifactory.example.com/api/npm/npm-remote/pako/-/pako-2.1.0.tgz";
    assert_eq!(
      relocate(
        tarball,
        "https://artifactory.example.com/api/npm/npm-remote/"
      ),
      tarball,
    );
    // A tarball hosted somewhere unrelated is left untouched too.
    let other = "https://cdn.example.com/pako-2.1.0.tgz";
    assert_eq!(
      relocate(other, "https://artifactory.example.com/api/npm/npm-remote/"),
      other,
    );
  }
}
