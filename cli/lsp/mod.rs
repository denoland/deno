// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use deno_core::error::AnyError;
use lspower::LspService;
use lspower::Server;

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
mod sources;
mod text;
mod tsc;
mod urls;

pub async fn start() -> Result<(), AnyError> {
  let stdin = tokio::io::stdin();
  let stdout = tokio::io::stdout();

  let (service, messages) =
    LspService::new(language_server::LanguageServer::new);
  Server::new(stdin, stdout)
    .interleave(messages)
    .serve(service)
    .await;

  Ok(())
}
