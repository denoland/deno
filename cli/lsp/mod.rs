// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use lspower::LspService;
use lspower::Server;

pub use repl::ReplCompletionItem;
pub use repl::ReplLanguageServer;

mod analysis;
mod cache;
mod capabilities;
mod client;
mod code_lens;
mod completions;
mod config;
mod diagnostics;
mod documents;
pub(crate) mod language_server;
mod logging;
mod lsp_custom;
mod parent_process_checker;
mod path_to_regex;
mod performance;
mod refactor;
mod registries;
mod repl;
mod semantic_tokens;
mod text;
mod tsc;
mod urls;

pub async fn start() -> Result<(), AnyError> {
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();

  let (service, messages) = LspService::new(|client| {
    language_server::LanguageServer::new(client::Client::from_lspower(client))
  });
  Server::new(stdin, stdout)
    .interleave(messages)
    .serve(service)
    .await;

  Ok(())
}
