// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(nayeemrmn): Move to `cli/lsp/tsc/go.rs`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use deno_core::error::AnyError;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::documents::DocumentModule;
use super::language_server::StateSnapshot;
use crate::lsp::performance::Performance;

#[derive(Debug)]
pub struct TsGoServer {
  enable_tracing: Arc<AtomicBool>,
  performance: Arc<Performance>,
}

impl TsGoServer {
  pub fn new(performance: Arc<Performance>) -> Self {
    Self {
      enable_tracing: Default::default(),
      performance,
    }
  }

  pub fn is_started(&self) -> bool {
    todo!()
  }

  pub async fn provide_diagnostics(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<lsp::DocumentDiagnosticReport, AnyError> {
    todo!()
  }

  pub async fn provide_references(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: lsp::ReferenceContext,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::Location>>, AnyError> {
    todo!()
  }

  pub async fn provide_code_lenses(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CodeLens>>, AnyError> {
    todo!()
  }

  pub async fn provide_document_symbols(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::DocumentSymbolResponse>, AnyError> {
    todo!()
  }

  pub async fn provide_hover(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::Hover>, AnyError> {
    todo!()
  }

  pub async fn provide_code_actions(
    &self,
    module: &DocumentModule,
    range: lsp::Range,
    context: &lsp::CodeActionContext,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::CodeActionResponse>, AnyError> {
    todo!()
  }

  pub async fn provide_document_highlights(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::DocumentHighlight>>, AnyError> {
    todo!()
  }

  pub async fn provide_definition(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::GotoDefinitionResponse>, AnyError> {
    todo!()
  }

  pub async fn provide_type_definition(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoTypeDefinitionResponse>, AnyError> {
    todo!()
  }

  pub async fn provide_completion(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: Option<&lsp::CompletionContext>,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::CompletionResponse>, AnyError> {
    todo!()
  }

  pub async fn resolve_completion_item(
    &self,
    module: &DocumentModule,
    item: lsp::CompletionItem,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<lsp::CompletionItem, AnyError> {
    todo!()
  }

  pub async fn provide_implementations(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoImplementationResponse>, AnyError> {
    todo!()
  }

  pub async fn provide_folding_range(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::FoldingRange>>, AnyError> {
    todo!()
  }

  pub async fn provide_call_hierarchy_incoming_calls(
    &self,
    module: &DocumentModule,
    item: &lsp::CallHierarchyItem,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyIncomingCall>>, AnyError> {
    todo!()
  }

  pub async fn provide_call_hierarchy_outgoing_calls(
    &self,
    module: &DocumentModule,
    item: &lsp::CallHierarchyItem,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyOutgoingCall>>, AnyError> {
    todo!()
  }

  pub async fn provide_prepare_call_hierarchy(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyItem>>, AnyError> {
    todo!()
  }

  pub async fn provide_rename(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    new_name: &str,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    todo!()
  }

  pub async fn provide_selection_ranges(
    &self,
    module: &DocumentModule,
    positions: &[lsp::Position],
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SelectionRange>>, AnyError> {
    todo!()
  }

  pub async fn provide_signature_help(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: Option<&lsp::SignatureHelpContext>,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::SignatureHelp>, AnyError> {
    todo!()
  }

  pub async fn provide_inlay_hint(
    &self,
    module: &DocumentModule,
    range: lsp::Range,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::InlayHint>>, AnyError> {
    todo!()
  }
}
