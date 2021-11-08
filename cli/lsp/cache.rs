// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::cache::CacherLoader;
use crate::cache::FetchCacher;
use crate::flags::Flags;
use crate::proc_state::ProcState;
use crate::resolver::ImportMapResolver;
use crate::tokio_util::create_basic_runtime;

use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use import_map::ImportMap;
use std::path::PathBuf;
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
    maybe_import_map: Option<ImportMap>,
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
        let maybe_resolver =
          maybe_import_map.as_ref().map(ImportMapResolver::new);
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
            None,
            cache.as_mut_loader(),
            maybe_resolver.as_ref().map(|r| r.as_resolver()),
            None,
            None,
          )
          .await;

          if tx.send(graph.valid().map_err(|err| err.into())).is_err() {
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
