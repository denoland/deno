// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(nayeemrmn): Move to `cli/lsp/tsc/go.rs`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use deno_config::deno_json::CompilerOptions;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_resolver::deno_json::CompilerOptionsKey;
use lsp_types::Uri;
use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::documents::DocumentModule;
use super::language_server::StateSnapshot;
use crate::cache::DenoDir;
use crate::http_util::HttpClientProvider;
use crate::lsp::completions::CompletionItemData;
use crate::lsp::performance::Performance;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TsGoCompletionItemData {
  pub uri: Uri,
  pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum TsGoRequest {
  ScopedRequest {
    name: String,
    args: serde_json::Value,
    compiler_options_key: CompilerOptionsKey,
    notebook_uri: Option<Arc<Uri>>,
  },
  UnscopedRequest {
    name: String,
    args: serde_json::Value,
  },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DidChangeConfigurationParams {
  pub by_compiler_options_by_key:
    Option<BTreeMap<CompilerOptionsKey, Arc<CompilerOptions>>>,
  pub by_notebook_uri: Option<BTreeMap<Arc<Uri>, CompilerOptionsKey>>,
}

#[derive(Debug)]
struct TsGoServerInner {}

#[derive(Debug)]
pub struct TsGoServer {
  enable_tracing: Arc<AtomicBool>,
  performance: Arc<Performance>,
  deno_dir: DenoDir,
  http_client_provider: Arc<HttpClientProvider>,
  inner: tokio::sync::OnceCell<TsGoServerInner>,
}

impl TsGoServer {
  pub fn new(
    performance: Arc<Performance>,
    deno_dir: &DenoDir,
    http_client_provider: &Arc<HttpClientProvider>,
  ) -> Self {
    Self {
      enable_tracing: Default::default(),
      performance,
      deno_dir: deno_dir.clone(),
      http_client_provider: http_client_provider.clone(),
      inner: Default::default(),
    }
  }

  async fn inner(&self) -> &TsGoServerInner {
    self
      .inner
      .get_or_init(async || {
        let tsgo_path = crate::tsc::ensure_tsgo(
          &self.deno_dir,
          self.http_client_provider.clone(),
        )
        .await
        .unwrap();
        todo!("{}", tsgo_path.display())
      })
      .await
  }

  pub fn is_started(&self) -> bool {
    self.inner.initialized()
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
    let response: Result<Option<lsp::CompletionResponse>, AnyError> = todo!();
    if let Ok(Some(response)) = &mut response {
      let items = match response {
        lsp::CompletionResponse::Array(items) => items,
        lsp::CompletionResponse::List(list) => &mut list.items,
      };
      for item in items {
        if let Some(data) = &mut item.data {
          let raw_data = std::mem::replace(data, serde_json::Value::Null);
          *data = serde_json::json!(CompletionItemData {
            documentation: None,
            tsc: None,
            tsgo: Some(TsGoCompletionItemData {
              uri: module.uri.as_ref().clone(),
              data: raw_data,
            })
          });
        }
      }
    }
    response
  }

  pub async fn resolve_completion_item(
    &self,
    module: &DocumentModule,
    mut item: lsp::CompletionItem,
    data: TsGoCompletionItemData,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<lsp::CompletionItem, AnyError> {
    item.data = Some(data.data);
    todo!("{:?}", item)
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

  pub async fn provide_workspace_symbol(
    &self,
    query: &str,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SymbolInformation>>, AnyError> {
    todo!()
  }
}
