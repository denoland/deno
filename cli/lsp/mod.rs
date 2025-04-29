// Copyright 2018-2025 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::error::AnyError;
pub use repl::ReplCompletionItem;
pub use repl::ReplLanguageServer;
use tower_lsp::LspService;
use tower_lsp::Server;

use self::diagnostics::should_send_diagnostic_batch_index_notifications;
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
mod jsr;
pub mod language_server;
mod logging;
mod lsp_custom;
mod npm;
mod parent_process_checker;
mod path_to_regex;
mod performance;
mod refactor;
mod registries;
mod repl;
mod resolver;
mod search;
mod semantic_tokens;
mod testing;
mod text;
mod trace;
mod tsc;
mod urls;

pub async fn start(
  registry_provider: Arc<
    dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync,
  >,
) -> Result<(), AnyError> {
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();

  let builder = LspService::build(|client| {
    language_server::LanguageServer::new(
      client::Client::from_tower(client),
      registry_provider,
    )
  })
  .custom_method(
    lsp_custom::PERFORMANCE_REQUEST,
    LanguageServer::performance_request,
  )
  .custom_method(lsp_custom::TASK_REQUEST, LanguageServer::task_definitions)
  .custom_method(testing::TEST_RUN_REQUEST, LanguageServer::test_run_request)
  .custom_method(
    testing::TEST_RUN_CANCEL_REQUEST,
    LanguageServer::test_run_cancel_request,
  )
  .custom_method(
    lsp_custom::VIRTUAL_TEXT_DOCUMENT,
    LanguageServer::virtual_text_document,
  );

  let builder = if should_send_diagnostic_batch_index_notifications() {
    builder.custom_method(
      lsp_custom::LATEST_DIAGNOSTIC_BATCH_INDEX,
      LanguageServer::latest_diagnostic_batch_index_request,
    )
  } else {
    builder
  };

  let (service, socket, pending) = builder.finish();
  Server::new(stdin, stdout, socket, pending)
    .concurrency_level(32)
    .serve(service)
    .await;
  Ok(())
}
