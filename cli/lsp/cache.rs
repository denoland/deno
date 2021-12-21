// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::cache::CacherLoader;
use crate::cache::FetchCacher;
use crate::config_file::ConfigFile;
use crate::flags::Flags;
use crate::graph_util::graph_valid;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::resolver::JsxResolver;

use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use deno_runtime::tokio_util::create_basic_runtime;
use import_map::ImportMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

type Request = (Vec<ModuleSpecifier>, oneshot::Sender<Result<(), AnyError>>);

/// A "server" that handles requests from the language server to cache modules
/// in its own thread.
#[derive(Debug)]
pub(crate) struct CacheServer(mpsc::UnboundedSender<Request>);

impl CacheServer {
  pub async fn new(
    maybe_cache_path: Option<PathBuf>,
    maybe_import_map: Option<Arc<ImportMap>>,
    maybe_config_file: Option<ConfigFile>,
  ) -> Self {
    let (tx, mut rx) = mpsc::unbounded_channel::<Request>();
    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();
      runtime.block_on(async {
        let ps = ProcState::build(Flags {
          cache_path: maybe_cache_path,
          ..Default::default()
        })
        .await
        .unwrap();
        let maybe_import_map_resolver =
          maybe_import_map.map(ImportMapResolver::new);
        let maybe_jsx_resolver = maybe_config_file
          .as_ref()
          .map(|cf| {
            cf.to_maybe_jsx_import_source_module()
              .map(|im| JsxResolver::new(im, maybe_import_map_resolver.clone()))
          })
          .flatten();
        let maybe_resolver = if maybe_jsx_resolver.is_some() {
          maybe_jsx_resolver.as_ref().map(|jr| jr.as_resolver())
        } else {
          maybe_import_map_resolver
            .as_ref()
            .map(|im| im.as_resolver())
        };
        let maybe_imports = maybe_config_file
          .map(|cf| cf.to_maybe_imports().ok())
          .flatten()
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
          )
          .await;

          if tx.send(graph_valid(&graph, true)).is_err() {
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
    roots: Vec<ModuleSpecifier>,
  ) -> Result<(), AnyError> {
    let (tx, rx) = oneshot::channel::<Result<(), AnyError>>();
    if self.0.send((roots, tx)).is_err() {
      return Err(anyhow!("failed to send request to cache thread"));
    }
    rx.await?
  }
}
