// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cache::CacherLoader;
use crate::cache::FetchCacher;
use crate::config_file::ConfigFile;
use crate::flags::Flags;
use crate::graph_util::graph_valid;
use crate::http_cache;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::create_basic_runtime;
use import_map::ImportMap;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

type Request = (
  Vec<(ModuleSpecifier, deno_graph::ModuleKind)>,
  oneshot::Sender<Result<(), AnyError>>,
);

/// A "server" that handles requests from the language server to cache modules
/// in its own thread.
#[derive(Debug)]
pub(crate) struct CacheServer(mpsc::UnboundedSender<Request>);

impl CacheServer {
  pub async fn new(
    maybe_cache_path: Option<PathBuf>,
    maybe_import_map: Option<Arc<ImportMap>>,
    maybe_config_file: Option<ConfigFile>,
    maybe_ca_stores: Option<Vec<String>>,
    maybe_ca_file: Option<String>,
    unsafely_ignore_certificate_errors: Option<Vec<String>>,
  ) -> Self {
    let (tx, mut rx) = mpsc::unbounded_channel::<Request>();
    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();
      runtime.block_on(async {
        let ps = ProcState::build(Arc::new(Flags {
          cache_path: maybe_cache_path,
          ca_stores: maybe_ca_stores,
          ca_file: maybe_ca_file,
          unsafely_ignore_certificate_errors,
          ..Default::default()
        }))
        .await
        .unwrap();
        let maybe_import_map_resolver =
          maybe_import_map.map(ImportMapResolver::new);
        let maybe_jsx_resolver = maybe_config_file.as_ref().and_then(|cf| {
          cf.to_maybe_jsx_import_source_module()
            .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
        });
        let maybe_resolver = if maybe_jsx_resolver.is_some() {
          maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
        } else {
          maybe_import_map_resolver
            .as_ref()
            .map(|im| im.as_resolver())
        };
        let maybe_imports = maybe_config_file
          .and_then(|cf| cf.to_maybe_imports().ok())
          .flatten();
        let mut cache = FetchCacher::new(
          ps.dir.gen_cache.clone(),
          ps.file_fetcher.clone(),
          Permissions::allow_all(),
          Permissions::allow_all(),
        );

        while let Some((roots, tx)) = rx.recv().await {
          let graph = deno_graph::create_graph(
            roots,
            false,
            maybe_imports.clone(),
            cache.as_mut_loader(),
            maybe_resolver,
            None,
            None,
            None,
          )
          .await;

          if tx.send(graph_valid(&graph, true, false)).is_err() {
            log::warn!("cannot send to client");
          }
        }
      })
    });

    Self(tx)
  }

  /// Attempt to cache the supplied module specifiers and their dependencies in
  /// the current DENO_DIR, returning any errors, so they can be returned to the
  /// client.
  pub async fn cache(
    &self,
    roots: Vec<(ModuleSpecifier, deno_graph::ModuleKind)>,
  ) -> Result<(), AnyError> {
    let (tx, rx) = oneshot::channel::<Result<(), AnyError>>();
    if self.0.send((roots, tx)).is_err() {
      return Err(anyhow!("failed to send request to cache thread"));
    }
    rx.await?
  }
}

/// Calculate a version for for a given path.
pub(crate) fn calculate_fs_version(path: &Path) -> Option<String> {
  let metadata = fs::metadata(path).ok()?;
  if let Ok(modified) = metadata.modified() {
    if let Ok(n) = modified.duration_since(SystemTime::UNIX_EPOCH) {
      Some(n.as_millis().to_string())
    } else {
      Some("1".to_string())
    }
  } else {
    Some("1".to_string())
  }
}

/// Populate the metadata map based on the supplied headers
fn parse_metadata(
  headers: &HashMap<String, String>,
) -> HashMap<MetadataKey, String> {
  let mut metadata = HashMap::new();
  if let Some(warning) = headers.get("x-deno-warning").cloned() {
    metadata.insert(MetadataKey::Warning, warning);
  }
  metadata
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) enum MetadataKey {
  /// Represent the `x-deno-warning` header associated with the document
  Warning,
}

#[derive(Debug, Clone)]
struct Metadata {
  values: Arc<HashMap<MetadataKey, String>>,
  version: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct CacheMetadata {
  cache: http_cache::HttpCache,
  metadata: Arc<Mutex<HashMap<ModuleSpecifier, Metadata>>>,
}

impl CacheMetadata {
  pub fn new(location: &Path) -> Self {
    Self {
      cache: http_cache::HttpCache::new(location),
      metadata: Default::default(),
    }
  }

  /// Return the meta data associated with the specifier. Unlike the `get()`
  /// method, redirects of the supplied specifier will not be followed.
  pub fn get(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<Arc<HashMap<MetadataKey, String>>> {
    if specifier.scheme() == "file" {
      return None;
    }
    let version = self
      .cache
      .get_cache_filename(specifier)
      .and_then(|ref path| calculate_fs_version(path));
    let metadata = self.metadata.lock().get(specifier).cloned();
    if metadata.as_ref().and_then(|m| m.version.clone()) != version {
      self.refresh(specifier).map(|m| m.values)
    } else {
      metadata.map(|m| m.values)
    }
  }

  fn refresh(&self, specifier: &ModuleSpecifier) -> Option<Metadata> {
    if specifier.scheme() == "file" {
      return None;
    }
    let cache_filename = self.cache.get_cache_filename(specifier)?;
    let specifier_metadata =
      http_cache::Metadata::read(&cache_filename).ok()?;
    let values = Arc::new(parse_metadata(&specifier_metadata.headers));
    let version = calculate_fs_version(&cache_filename);
    let mut metadata_map = self.metadata.lock();
    let metadata = Metadata { values, version };
    metadata_map.insert(specifier.clone(), metadata.clone());
    Some(metadata)
  }

  pub fn set_location(&mut self, location: &Path) {
    self.cache = http_cache::HttpCache::new(location);
    self.metadata.lock().clear();
  }
}
