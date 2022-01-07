// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::disk_cache::DiskCache;
use crate::errors::get_error_class_name;
use crate::file_fetcher::FileFetcher;
use crate::http_cache::url_to_filename;

use deno_core::error::AnyError;
use deno_core::futures::future;
use deno_core::futures::FutureExt;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::ModuleSpecifier;
use deno_core::url::Host;
use deno_core::url::Url;
use deno_graph::source::CacheInfo;
use deno_graph::source::LoadFuture;
use deno_graph::source::LoadResponse;
use deno_graph::source::Loader;
use deno_runtime::permissions::Permissions;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::path::Prefix;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionedCacheData {
  pub version_hash: String,
  pub text: String,
}

pub(crate) enum CacheType {
  TypeScriptBuildInfo,
}

impl CacheType {
  fn as_extension(&self) -> &'static str {
    match self {
      CacheType::TypeScriptBuildInfo => "buildinfo",
    }
  }
}

pub(crate) enum VersionedCacheType {
  Declaration,
  Emit,
  SourceMap,
}

impl VersionedCacheType {
  fn as_extension(&self) -> &'static str {
    match self {
      VersionedCacheType::Declaration => "dtsinfo",
      VersionedCacheType::Emit => "emitinfo",
      VersionedCacheType::SourceMap => "mapinfo",
    }
  }
}

/// A trait which provides a concise implementation to getting and setting
/// values in a cache.
pub(crate) trait Cacher {
  fn get(&self, filename: &Path) -> std::io::Result<Vec<u8>>;
  fn set(&mut self, filename: &Path, data: &[u8]) -> std::io::Result<()>;

  /// Get a string value from the cache.
  fn get_value(
    &self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    let filename = get_cache_filename_with_extension(specifier, cache_type.as_extension())?;
    self.get(&filename)
      .ok()
      .map(|b| String::from_utf8(b).ok())
      .flatten()
  }

  /// Set a string value in the cache.
  fn set_value(
    &mut self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
    value: String,
  ) -> Result<(), AnyError> {
    let filename = get_cache_filename_with_extension(specifier, cache_type.as_extension())
      .unwrap();
    self.set(&filename, value.as_bytes())
      .map_err(|e| e.into())
  }

  /// Gets a versioned value from the cache.
  fn get_versioned(&self, cache_type: VersionedCacheType, specifier: &ModuleSpecifier) -> Option<VersionedCacheData> {
    let filename = get_cache_filename_with_extension(specifier, cache_type.as_extension())?;
    let bytes = self.get(&filename).ok()?;
    serde_json::from_slice(&bytes).ok()
  }

  /// Sets a versioned value in the cache.
  fn set_versioned(
    &mut self,
    cache_type: VersionedCacheType,
    specifier: &ModuleSpecifier,
    data: VersionedCacheData,
  ) -> Result<(), AnyError> {
    let filename = get_cache_filename_with_extension(specifier, cache_type.as_extension())
      .unwrap();
    let bytes = serde_json::to_vec(&data)?;
    self.set(&filename, &bytes).map_err(|e| e.into())
  }
}

/// Combines the cacher trait along with the deno_graph Loader trait to provide
/// a single interface to be able to load and cache modules when building a
/// graph.
pub(crate) trait CacherLoader: Cacher + Loader {
  fn as_cacher(&self) -> &dyn Cacher;
  fn as_mut_loader(&mut self) -> &mut dyn Loader;
  fn as_mut_cacher(&mut self) -> &mut dyn Cacher;
}

/// A "wrapper" for the FileFetcher and DiskCache for the Deno CLI that provides
/// a concise interface to the DENO_DIR when building module graphs.
pub(crate) struct FetchCacher {
  disk_cache: DiskCache,
  dynamic_permissions: Permissions,
  file_fetcher: Arc<FileFetcher>,
  root_permissions: Permissions,
}

impl FetchCacher {
  pub fn new(
    disk_cache: DiskCache,
    file_fetcher: FileFetcher,
    root_permissions: Permissions,
    dynamic_permissions: Permissions,
  ) -> Self {
    let file_fetcher = Arc::new(file_fetcher);

    Self {
      disk_cache,
      dynamic_permissions,
      file_fetcher,
      root_permissions,
    }
  }
}

impl Loader for FetchCacher {
  fn get_cache_info(&self, specifier: &ModuleSpecifier) -> Option<CacheInfo> {
    let local = self.file_fetcher.get_local_path(specifier)?;
    if local.is_file() {
      let location = &self.disk_cache.location;
      let emit = get_cache_filename_with_extension(specifier, VersionedCacheType::Emit.as_extension())
        .map(|p| location.join(p))
        .filter(|p| p.is_file()); // todo: dsherret: this condition is racy so we should remove it
      let map = get_cache_filename_with_extension(specifier, VersionedCacheType::SourceMap.as_extension())
        .map(|p| location.join(p))
        .filter(|p| p.is_file()); // todo: dsherret: this condition is racy so we should remove it
      Some(CacheInfo {
        local: Some(local),
        emit,
        map,
      })
    } else {
      None
    }
  }

  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    is_dynamic: bool,
  ) -> LoadFuture {
    let specifier = specifier.clone();
    let mut permissions = if is_dynamic {
      self.dynamic_permissions.clone()
    } else {
      self.root_permissions.clone()
    };
    let file_fetcher = self.file_fetcher.clone();

    async move {
      let load_result = file_fetcher
        .fetch(&specifier, &mut permissions)
        .await
        .map_or_else(
          |err| {
            if let Some(err) = err.downcast_ref::<std::io::Error>() {
              if err.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
              }
            } else if get_error_class_name(&err) == "NotFound" {
              return Ok(None);
            }
            Err(err)
          },
          |file| {
            Ok(Some(LoadResponse {
              specifier: file.specifier,
              maybe_headers: file.maybe_headers,
              content: file.source,
            }))
          },
        );

      (specifier, load_result)
    }
    .boxed()
  }
}

impl Cacher for FetchCacher {
  fn get(&self, filename: &Path) -> std::io::Result<Vec<u8>> {
    self.disk_cache.get(filename)
  }

  fn set(&mut self, filename: &Path, data: &[u8]) -> std::io::Result<()> {
    self.disk_cache.set(filename, data)
  }
}

impl CacherLoader for FetchCacher {
  fn as_cacher(&self) -> &dyn Cacher {
    self
  }

  fn as_mut_loader(&mut self) -> &mut dyn Loader {
    self
  }

  fn as_mut_cacher(&mut self) -> &mut dyn Cacher {
    self
  }
}

/// An in memory cache that is used by the runtime `Deno.emit()` API to provide
/// the same behavior as the disk cache when sources are provided.
#[derive(Debug)]
pub(crate) struct MemoryCacher {
  sources: HashMap<String, Arc<String>>,
  declarations: HashMap<ModuleSpecifier, VersionedCacheData>,
  build_infos: HashMap<ModuleSpecifier, String>,
  emits: HashMap<ModuleSpecifier, VersionedCacheData>,
  maps: HashMap<ModuleSpecifier, VersionedCacheData>,
}

impl MemoryCacher {
  pub fn new(sources: HashMap<String, Arc<String>>) -> Self {
    Self {
      sources,
      declarations: HashMap::default(),
      emits: HashMap::default(),
      maps: HashMap::default(),
      build_infos: HashMap::default(),
    }
  }
}

impl Loader for MemoryCacher {
  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    _is_dynamic: bool,
  ) -> LoadFuture {
    let mut specifier_str = specifier.to_string();
    if !self.sources.contains_key(&specifier_str) {
      specifier_str = specifier_str.replace("file:///", "/");
      if !self.sources.contains_key(&specifier_str) {
        specifier_str = specifier_str[3..].to_string();
      }
    }
    let response = self.sources.get(&specifier_str).map(|c| LoadResponse {
      specifier: specifier.clone(),
      maybe_headers: None,
      content: c.to_owned(),
    });
    Box::pin(future::ready((specifier.clone(), Ok(response))))
  }
}

impl Cacher for MemoryCacher {
  fn get(&self, _filename: &Path) -> std::io::Result<Vec<u8>> {
    unreachable!();
  }

  fn set(&mut self, _filename: &Path, _data: &[u8]) -> std::io::Result<()> {
    unreachable!();
  }

  fn get_value(
    &self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    match cache_type {
      CacheType::TypeScriptBuildInfo => {
        self.build_infos.get(specifier).cloned()
      }
    }
  }

  fn set_value(
    &mut self,
    cache_type: CacheType,
    specifier: &ModuleSpecifier,
    value: String,
  ) -> Result<(), AnyError> {
    match cache_type {
      CacheType::TypeScriptBuildInfo => {
        self.build_infos.insert(specifier.clone(), value)
      }
    };
    Ok(())
  }

  fn get_versioned(&self, cache_type: VersionedCacheType, specifier: &ModuleSpecifier) -> Option<VersionedCacheData> {
    match cache_type {
      VersionedCacheType::Declaration => self.declarations.get(specifier).cloned(),
      VersionedCacheType::Emit => self.emits.get(specifier).cloned(),
      VersionedCacheType::SourceMap => self.maps.get(specifier).cloned(),
    }
  }

  fn set_versioned(
    &mut self,
    cache_type: VersionedCacheType,
    specifier: &ModuleSpecifier,
    data: VersionedCacheData,
  ) -> Result<(), AnyError> {
    match cache_type {
      VersionedCacheType::Declaration => {
        self.declarations.insert(specifier.clone(), data);
      },
      VersionedCacheType::Emit => {
        self.emits.insert(specifier.clone(), data);
      },
      VersionedCacheType::SourceMap => {
        self.maps.insert(specifier.clone(), data);
      },
    }
    Ok(())
  }
}

impl CacherLoader for MemoryCacher {
  fn as_cacher(&self) -> &dyn Cacher {
    self
  }

  fn as_mut_loader(&mut self) -> &mut dyn Loader {
    self
  }

  fn as_mut_cacher(&mut self) -> &mut dyn Cacher {
    self
  }
}

fn get_cache_filename(url: &Url) -> Option<PathBuf> {
  let mut out = PathBuf::new();

  let scheme = url.scheme();
  out.push(scheme);

  match scheme {
    "wasm" => {
      let host = url.host_str().unwrap();
      let host_port = match url.port() {
        // Windows doesn't support ":" in filenames, so we represent port using a
        // special string.
        Some(port) => format!("{}_PORT{}", host, port),
        None => host.to_string(),
      };
      out.push(host_port);

      for path_seg in url.path_segments().unwrap() {
        out.push(path_seg);
      }
    }
    "http" | "https" | "data" | "blob" => out = url_to_filename(url)?,
    "file" => {
      let path = match url.to_file_path() {
        Ok(path) => path,
        Err(_) => return None,
      };
      let mut path_components = path.components();

      if cfg!(target_os = "windows") {
        if let Some(Component::Prefix(prefix_component)) =
          path_components.next()
        {
          // Windows doesn't support ":" in filenames, so we need to extract disk prefix
          // Example: file:///C:/deno/js/unit_test_runner.ts
          // it should produce: file\c\deno\js\unit_test_runner.ts
          match prefix_component.kind() {
            Prefix::Disk(disk_byte) | Prefix::VerbatimDisk(disk_byte) => {
              let disk = (disk_byte as char).to_string();
              out.push(disk);
            }
            Prefix::UNC(server, share)
            | Prefix::VerbatimUNC(server, share) => {
              out.push("UNC");
              let host = Host::parse(server.to_str().unwrap()).unwrap();
              let host = host.to_string().replace(":", "_");
              out.push(host);
              out.push(share);
            }
            _ => unreachable!(),
          }
        }
      }

      // Must be relative, so strip forward slash
      let mut remaining_components = path_components.as_path();
      if let Ok(stripped) = remaining_components.strip_prefix("/") {
        remaining_components = stripped;
      };

      out = out.join(remaining_components);
    }
    _ => return None,
  };

  Some(out)
}

fn get_cache_filename_with_extension(
  url: &Url,
  extension: &str,
) -> Option<PathBuf> {
  let base = get_cache_filename(url)?;

  match base.extension() {
    None => Some(base.with_extension(extension)),
    Some(ext) => {
      let original_extension = OsStr::to_str(ext).unwrap();
      let final_extension = format!("{}.{}", original_extension, extension);
      Some(base.with_extension(final_extension))
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_get_cache_filename() {
    let mut test_cases = vec![
      (
        "http://deno.land/std/http/file_server.ts",
        "http/deno.land/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ),
      (
        "http://localhost:8000/std/http/file_server.ts",
        "http/localhost_PORT8000/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ),
      (
        "https://deno.land/std/http/file_server.ts",
        "https/deno.land/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf",
      ),
      ("wasm://wasm/d1c677ea", "wasm/wasm/d1c677ea"),
    ];

    if cfg!(target_os = "windows") {
      test_cases.push(("file:///D:/a/1/s/format.ts", "file/D/a/1/s/format.ts"));
      // IPv4 localhost
      test_cases.push((
        "file://127.0.0.1/d$/a/1/s/format.ts",
        "file/UNC/127.0.0.1/d$/a/1/s/format.ts",
      ));
      // IPv6 localhost
      test_cases.push((
        "file://[0:0:0:0:0:0:0:1]/d$/a/1/s/format.ts",
        "file/UNC/[__1]/d$/a/1/s/format.ts",
      ));
      // shared folder
      test_cases.push((
        "file://comp/t-share/a/1/s/format.ts",
        "file/UNC/comp/t-share/a/1/s/format.ts",
      ));
    } else {
      test_cases.push((
        "file:///std/http/file_server.ts",
        "file/std/http/file_server.ts",
      ));
    }

    for test_case in &test_cases {
      let cache_filename = get_cache_filename(&Url::parse(test_case.0).unwrap());
      assert_eq!(cache_filename, Some(PathBuf::from(test_case.1)));
    }
  }

  #[test]
  fn test_get_cache_filename_with_extension() {
    let mut test_cases = vec![
      (
        "http://deno.land/std/http/file_server.ts",
        "js",
        "http/deno.land/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf.js",
      ),
      (
        "http://deno.land/std/http/file_server.ts",
        "js.map",
        "http/deno.land/d8300752800fe3f0beda9505dc1c3b5388beb1ee45afd1f1e2c9fc0866df15cf.js.map",
      ),
    ];

    if cfg!(target_os = "windows") {
      test_cases.push((
        "file:///D:/std/http/file_server",
        "js",
        "file/D/std/http/file_server.js",
      ));
    } else {
      test_cases.push((
        "file:///std/http/file_server",
        "js",
        "file/std/http/file_server.js",
      ));
    }

    for test_case in &test_cases {
      assert_eq!(
        get_cache_filename_with_extension(
          &Url::parse(test_case.0).unwrap(),
          test_case.1
        ),
        Some(PathBuf::from(test_case.2))
      )
    }
  }

  #[test]
  fn test_get_cache_filename_invalid_urls() {
    let mut test_cases = vec!["unknown://localhost/test.ts"];

    if cfg!(target_os = "windows") {
      test_cases.push("file://");
      test_cases.push("file:///");
    }

    for test_case in &test_cases {
      let cache_filename = get_cache_filename(&Url::parse(test_case).unwrap());
      assert_eq!(cache_filename, None);
    }
  }
}
