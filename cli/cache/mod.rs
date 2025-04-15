// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_cache_dir::file_fetcher::CacheSetting;
use deno_cache_dir::file_fetcher::FetchNoFollowErrorKind;
use deno_cache_dir::file_fetcher::FileOrRedirect;
use deno_core::futures::FutureExt;
use deno_core::ModuleSpecifier;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_runtime::deno_permissions::PermissionsContainer;
use node_resolver::InNpmPackageChecker;

use crate::args::jsr_url;
use crate::file_fetcher::CliFetchNoFollowErrorKind;
use crate::file_fetcher::CliFileFetcher;
use crate::file_fetcher::FetchNoFollowOptions;
use crate::file_fetcher::FetchPermissionsOptionRef;
use crate::sys::CliSys;

mod cache_db;
mod caches;
mod check;
mod code_cache;
mod deno_dir;
mod disk_cache;
mod emit;
mod fast_check;
mod incremental;
mod module_info;
mod node;
mod parsed_source;

pub use cache_db::CacheDBHash;
pub use caches::Caches;
pub use check::TypeCheckCache;
pub use code_cache::CodeCache;
/// Permissions used to save a file in the disk caches.
pub use deno_cache_dir::CACHE_PERM;
pub use deno_dir::DenoDir;
pub use deno_dir::DenoDirProvider;
pub use disk_cache::DiskCache;
pub use emit::EmitCache;
pub use fast_check::FastCheckCache;
pub use incremental::IncrementalCache;
pub use module_info::ModuleInfoCache;
pub use node::NodeAnalysisCache;
pub use parsed_source::LazyGraphSourceParser;
pub use parsed_source::ParsedSourceCache;

pub type GlobalHttpCache = deno_cache_dir::GlobalHttpCache<CliSys>;
pub type LocalLspHttpCache = deno_cache_dir::LocalLspHttpCache<CliSys>;
pub use deno_cache_dir::HttpCache;
use deno_error::JsErrorBox;

pub struct FetchCacherOptions {
  pub file_header_overrides: HashMap<ModuleSpecifier, HashMap<String, String>>,
  pub permissions: PermissionsContainer,
  /// If we're publishing for `deno publish`.
  pub is_deno_publish: bool,
}

/// A "wrapper" for the FileFetcher and DiskCache for the Deno CLI that provides
/// a concise interface to the DENO_DIR when building module graphs.
pub struct FetchCacher {
  pub file_header_overrides: HashMap<ModuleSpecifier, HashMap<String, String>>,
  file_fetcher: Arc<CliFileFetcher>,
  global_http_cache: Arc<GlobalHttpCache>,
  in_npm_pkg_checker: DenoInNpmPackageChecker,
  module_info_cache: Arc<ModuleInfoCache>,
  permissions: PermissionsContainer,
  sys: CliSys,
  is_deno_publish: bool,
  cache_info_enabled: bool,
}

impl FetchCacher {
  pub fn new(
    file_fetcher: Arc<CliFileFetcher>,
    global_http_cache: Arc<GlobalHttpCache>,
    in_npm_pkg_checker: DenoInNpmPackageChecker,
    module_info_cache: Arc<ModuleInfoCache>,
    sys: CliSys,
    options: FetchCacherOptions,
  ) -> Self {
    Self {
      file_fetcher,
      global_http_cache,
      in_npm_pkg_checker,
      module_info_cache,
      sys,
      file_header_overrides: options.file_header_overrides,
      permissions: options.permissions,
      is_deno_publish: options.is_deno_publish,
      cache_info_enabled: false,
    }
  }

  /// The cache information takes a bit of time to fetch and it's
  /// not always necessary. It should only be enabled for deno info.
  pub fn enable_loading_cache_info(&mut self) {
    self.cache_info_enabled = true;
  }

  /// Only use this for `deno info`.
  fn get_local_path(&self, specifier: &ModuleSpecifier) -> Option<PathBuf> {
    // TODO(@kitsonk) fix when deno_graph does not query cache for synthetic
    // modules
    if specifier.scheme() == "flags" {
      None
    } else if specifier.scheme() == "file" {
      specifier.to_file_path().ok()
    } else {
      self.global_http_cache.local_path_for_url(specifier).ok()
    }
  }
}

impl Loader for FetchCacher {
  fn get_cache_info(&self, specifier: &ModuleSpecifier) -> Option<CacheInfo> {
    if !self.cache_info_enabled {
      return None;
    }

    #[allow(deprecated)]
    let local = self.get_local_path(specifier)?;
    if local.is_file() {
      Some(CacheInfo { local: Some(local) })
    } else {
      None
    }
  }

  fn load(
    &self,
    specifier: &ModuleSpecifier,
    options: deno_graph::source::LoadOptions,
  ) -> LoadFuture {
    use deno_graph::source::CacheSetting as LoaderCacheSetting;

    if specifier.scheme() == "file"
      && specifier.path().contains("/node_modules/")
    {
      // The specifier might be in a completely different symlinked tree than
      // what the node_modules url is in (ex. `/my-project-1/node_modules`
      // symlinked to `/my-project-2/node_modules`), so first we checked if the path
      // is in a node_modules dir to avoid needlessly canonicalizing, then now compare
      // against the canonicalized specifier.
      let specifier = node_resolver::resolve_specifier_into_node_modules(
        &self.sys, specifier,
      );
      if self.in_npm_pkg_checker.in_npm_package(&specifier) {
        return Box::pin(std::future::ready(Ok(Some(
          LoadResponse::External { specifier },
        ))));
      }
    }

    if self.is_deno_publish
      && matches!(specifier.scheme(), "http" | "https")
      && !specifier.as_str().starts_with(jsr_url().as_str())
    {
      // mark non-JSR remote modules as external so we don't need --allow-import
      // permissions as these will error out later when publishing
      return Box::pin(std::future::ready(Ok(Some(LoadResponse::External {
        specifier: specifier.clone(),
      }))));
    }

    let file_fetcher = self.file_fetcher.clone();
    let file_header_overrides = self.file_header_overrides.clone();
    let permissions = self.permissions.clone();
    let specifier = specifier.clone();
    let is_statically_analyzable = !options.was_dynamic_root;

    async move {
      let maybe_cache_setting = match options.cache_setting {
        LoaderCacheSetting::Use => None,
        LoaderCacheSetting::Reload => {
          if matches!(file_fetcher.cache_setting(), CacheSetting::Only) {
            return Err(deno_graph::source::LoadError::Other(Arc::new(JsErrorBox::generic(
              "Could not resolve version constraint using only cached data. Try running again without --cached-only"
            ))));
          }
          Some(CacheSetting::ReloadAll)
        }
        LoaderCacheSetting::Only => Some(CacheSetting::Only),
      };
      file_fetcher
        .fetch_no_follow(
          &specifier,
          FetchPermissionsOptionRef::Restricted(&permissions,
          if is_statically_analyzable {
            deno_runtime::deno_permissions::CheckSpecifierKind::Static
          } else {
            deno_runtime::deno_permissions::CheckSpecifierKind::Dynamic
          }),
          FetchNoFollowOptions {
          maybe_auth: None,
          maybe_accept: None,
          maybe_cache_setting: maybe_cache_setting.as_ref(),
          maybe_checksum: options.maybe_checksum.as_ref(),
        })
        .await
        .map(|file_or_redirect| {
          match file_or_redirect {
            FileOrRedirect::File(file) => {
              let maybe_headers =
              match (file.maybe_headers, file_header_overrides.get(&specifier)) {
                (Some(headers), Some(overrides)) => {
                  Some(headers.into_iter().chain(overrides.clone()).collect())
                }
                (Some(headers), None) => Some(headers),
                (None, Some(overrides)) => Some(overrides.clone()),
                (None, None) => None,
              };
            Ok(Some(LoadResponse::Module {
              specifier: file.url,
              maybe_headers,
              content: file.source,
            }))
            },
            FileOrRedirect::Redirect(redirect_specifier) => {
              Ok(Some(LoadResponse::Redirect {
                specifier: redirect_specifier,
              }))
            },
          }
        })
        .unwrap_or_else(|err| {
          let err = err.into_kind();
          match err {
            CliFetchNoFollowErrorKind::FetchNoFollow(err) => {
              let err = err.into_kind();
              match err {
                FetchNoFollowErrorKind::NotFound(_) => Ok(None),
                FetchNoFollowErrorKind::UrlToFilePath { .. } |
                FetchNoFollowErrorKind::ReadingBlobUrl { .. } |
                FetchNoFollowErrorKind::ReadingFile { .. } |
                FetchNoFollowErrorKind::FetchingRemote { .. } |
                FetchNoFollowErrorKind::ClientError { .. } |
                FetchNoFollowErrorKind::NoRemote { .. } |
                FetchNoFollowErrorKind::DataUrlDecode { .. } |
                FetchNoFollowErrorKind::RedirectResolution { .. } |
                FetchNoFollowErrorKind::CacheRead { .. } |
                FetchNoFollowErrorKind::CacheSave  { .. } |
                FetchNoFollowErrorKind::UnsupportedScheme  { .. } |
                FetchNoFollowErrorKind::RedirectHeaderParse { .. } |
                FetchNoFollowErrorKind::InvalidHeader { .. } => Err(deno_graph::source::LoadError::Other(Arc::new(JsErrorBox::from_err(err)))),
                FetchNoFollowErrorKind::NotCached { .. } => {
                  if options.cache_setting == LoaderCacheSetting::Only {
                    Ok(None)
                  } else {
                    Err(deno_graph::source::LoadError::Other(Arc::new(JsErrorBox::from_err(err))))
                  }
                },
                FetchNoFollowErrorKind::ChecksumIntegrity(err) => {
                  // convert to the equivalent deno_graph error so that it
                  // enhances it if this is passed to deno_graph
                  Err(
                    deno_graph::source::LoadError::ChecksumIntegrity(deno_graph::source::ChecksumIntegrityError {
                      actual: err.actual,
                      expected: err.expected,
                    }),
                  )
                }
              }
            },
            CliFetchNoFollowErrorKind::PermissionCheck(permission_check_error) => Err(deno_graph::source::LoadError::Other(Arc::new(JsErrorBox::from_err(permission_check_error)))),
          }
        })
    }
    .boxed_local()
  }

  fn cache_module_info(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    source: &Arc<[u8]>,
    module_info: &deno_graph::ModuleInfo,
  ) {
    log::debug!("Caching module info for {}", specifier);
    let source_hash = CacheDBHash::from_hashable(source);
    let result = self.module_info_cache.set_module_info(
      specifier,
      media_type,
      source_hash,
      module_info,
    );
    if let Err(err) = result {
      log::debug!(
        "Error saving module cache info for {}. {:#}",
        specifier,
        err
      );
    }
  }
}
