// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::unsync::spawn;
use tower_lsp::LspService;
use tower_lsp::Server;

use crate::lsp::language_server::LanguageServer;
use crate::util::sync::AsyncFlag;
pub use repl::ReplCompletionItem;
pub use repl::ReplLanguageServer;

use self::diagnostics::should_send_diagnostic_batch_index_notifications;

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
mod tsc;
mod urls;

pub async fn start() -> Result<(), AnyError> {
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();

  let shutdown_flag = AsyncFlag::default();
  let builder = LspService::build(|client| {
    language_server::LanguageServer::new(
      client::Client::from_tower(client),
      shutdown_flag.clone(),
    )
  })
  .custom_method(
    lsp_custom::PERFORMANCE_REQUEST,
    LanguageServer::performance_request,
  )
  .custom_method(lsp_custom::TASK_REQUEST, LanguageServer::task_definitions)
  // TODO(nayeemrmn): Rename this to `deno/taskDefinitions` in vscode_deno and
  // remove this alias.
  .custom_method("deno/task", LanguageServer::task_definitions)
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

  let (service, socket) = builder.finish();

  // TODO(nayeemrmn): This shutdown flag is a workaround for
  // https://github.com/denoland/deno/issues/20700. Remove when
  // https://github.com/ebkalderon/tower-lsp/issues/399 is fixed.
  // Force end the server 8 seconds after receiving a shutdown request.
  tokio::select! {
    biased;
    _ = Server::new(stdin, stdout, socket).serve(service) => {}
    _ = spawn(async move {
      shutdown_flag.wait_raised().await;
      tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    }) => {}
  }

  Ok(())
}
