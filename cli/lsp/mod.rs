// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use tower_lsp::LspService;
use tower_lsp::LspServiceBuilder;
use tower_lsp::Server;

pub use repl::ReplCompletionItem;
pub use repl::ReplLanguageServer;
use crate::lsp::language_server::LanguageServer;

mod analysis;
mod cache;
mod capabilities;
mod client;
mod code_lens;
mod completions;
mod config;
mod diagnostics;
mod documents;
pub mod language_server;
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

  let (service, socket) = LspService::build(|client| {
    language_server::LanguageServer::new(client::Client::from_tower(client))
  })
      .custom_method(lsp_custom::CACHE_REQUEST, LanguageServer::cache_request)
      .custom_method(lsp_custom::PERFORMANCE_REQUEST, LanguageServer::performance_request)
      .custom_method(lsp_custom::RELOAD_IMPORT_REGISTRIES_REQUEST, LanguageServer::reload_import_registries_request)
      .custom_method(lsp_custom::VIRTUAL_TEXT_DOCUMENT, LanguageServer::virtual_text_document)
      .finish();

  Server::new(stdin, stdout, socket)
    .serve(service)
    .await;

  Ok(())
}
