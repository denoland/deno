// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use lspower::LspService;
use lspower::Server;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

mod analysis;
mod capabilities;
mod completions;
mod config;
mod diagnostics;
mod documents;
pub(crate) mod language_server;
mod path_to_regex;
mod performance;
mod registries;
mod semantic_tokens;
mod sources;
mod text;
mod tsc;
mod urls;

pub async fn start(lsp_debug_flag: Arc<AtomicBool>) -> Result<(), AnyError> {
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();

  let (service, messages) = LspService::new(|client| {
    language_server::LanguageServer::new(client, lsp_debug_flag)
  });
  Server::new(stdin, stdout)
    .interleave(messages)
    .serve(service)
    .await;

  Ok(())
}
