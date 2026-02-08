// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(nayeemrmn): Move to `cli/lsp/tsc/go.rs`.

use std::collections::BTreeMap;
use std::sync::Arc;

use deno_config::deno_json::CompilerOptions;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_resolver::deno_json::CompilerOptionsKey;
use lsp_types::Uri;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::documents::DocumentModule;
use super::language_server::StateSnapshot;
use crate::cache::DenoDir;
use crate::http_util::HttpClientProvider;
use crate::lsp::completions::CompletionItemData;
use crate::lsp::documents::Document;
use crate::lsp::urls::uri_to_url;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TsGoCompletionItemData {
  pub uri: Uri,
  pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum TsGoRequest {
  LanguageServiceMethod {
    name: String,
    args: serde_json::Value,
    compiler_options_key: CompilerOptionsKey,
    notebook_uri: Option<Arc<Uri>>,
  },
  GetAmbientModules {
    compiler_options_key: CompilerOptionsKey,
    notebook_uri: Option<Arc<Uri>>,
  },
  WorkspaceSymbol {
    query: String,
  },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
enum TsGoFileChangeKind {
  Opened,
  Closed,
  Modified,
}

impl From<super::tsc::ChangeKind> for TsGoFileChangeKind {
  fn from(value: super::tsc::ChangeKind) -> Self {
    match value {
      super::tsc::ChangeKind::Opened => Self::Opened,
      super::tsc::ChangeKind::Closed => Self::Closed,
      super::tsc::ChangeKind::Modified => Self::Modified,
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TsGoFileChange {
  uri: Arc<Uri>,
  kind: TsGoFileChangeKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TsGoConfigurationChange {
  pub new_compiler_options_by_key:
    BTreeMap<CompilerOptionsKey, Arc<CompilerOptions>>,
  pub new_notebook_keys: BTreeMap<Arc<Uri>, CompilerOptionsKey>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TsGoProjectChange {
  files: Vec<TsGoFileChange>,
  configuration: Option<TsGoConfigurationChange>,
}

#[derive(Debug)]
struct TsGoServerInner {
  pending_change: Mutex<Option<TsGoProjectChange>>,
}

#[derive(Debug)]
pub struct TsGoServer {
  deno_dir: DenoDir,
  http_client_provider: Arc<HttpClientProvider>,
  inner: tokio::sync::OnceCell<TsGoServerInner>,
}

impl TsGoServer {
  pub fn new(
    deno_dir: &DenoDir,
    http_client_provider: &Arc<HttpClientProvider>,
  ) -> Self {
    Self {
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

  pub fn project_changed(
    &self,
    documents: &[(Document, super::tsc::ChangeKind)],
    configuration_changed: bool,
    snapshot: Arc<StateSnapshot>,
  ) {
    let Some(inner) = self.inner.get() else {
      return;
    };
    let mut pending_change = inner.pending_change.lock();
    if pending_change.is_none() {
      *pending_change = Some(TsGoProjectChange {
        files: documents
          .iter()
          .map(|(d, k)| TsGoFileChange {
            uri: d.uri().clone(),
            kind: (*k).into(),
          })
          .collect(),
        configuration: configuration_changed.then(|| TsGoConfigurationChange {
          new_compiler_options_by_key: snapshot
            .compiler_options_resolver
            .entries()
            .map(|(k, d)| (k.clone(), d.compiler_options.clone()))
            .collect(),
          new_notebook_keys: snapshot
            .document_modules
            .documents
            .cells_by_notebook_uri()
            .keys()
            .map(|u| {
              let compiler_options_key = snapshot
                .compiler_options_resolver
                .entry_for_specifier(&uri_to_url(u))
                .0;
              (u.clone(), compiler_options_key.clone())
            })
            .collect(),
        }),
      })
    }
  }

  async fn request<R>(
    &self,
    request: TsGoRequest,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<R, AnyError>
  where
    R: DeserializeOwned,
  {
    let inner = self.inner().await;
    todo!("{:?}", &inner.pending_change)
  }

  pub async fn get_ambient_modules(
    &self,
    compiler_options_key: &CompilerOptionsKey,
    notebook_uri: Option<&Arc<Uri>>,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Vec<String>, AnyError> {
    self
      .request(
        TsGoRequest::GetAmbientModules {
          compiler_options_key: compiler_options_key.clone(),
          notebook_uri: notebook_uri.cloned(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_diagnostics(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<lsp::DocumentDiagnosticReport, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideDiagnostics".to_string(),
          args: json!([&module.specifier]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_references(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: lsp::ReferenceContext,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::Location>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideReferences".to_string(),
          args: json!([{
            "textDocument": { "uri": &module.specifier },
            "position": position,
            "context": context,
          }]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_code_lenses(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CodeLens>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideCodeLenses".to_string(),
          args: json!([&module.specifier]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_document_symbols(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::DocumentSymbolResponse>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideDocumentSymbols".to_string(),
          args: json!([&module.specifier]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_hover(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::Hover>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideHover".to_string(),
          args: json!([&module.specifier, position]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_code_actions(
    &self,
    module: &DocumentModule,
    range: lsp::Range,
    context: &lsp::CodeActionContext,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::CodeActionResponse>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideCodeActions".to_string(),
          args: json!([{
            "textDocument": { "uri": &module.specifier },
            "range": range,
            "context": context,
          }]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_document_highlights(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::DocumentHighlight>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideDocumentHighlights".to_string(),
          args: json!([&module.specifier, position]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_definition(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::GotoDefinitionResponse>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideDefinition".to_string(),
          args: json!([&module.specifier, position]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_type_definition(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoTypeDefinitionResponse>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideTypeDefinition".to_string(),
          args: json!([&module.specifier, position]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_completion(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: Option<&lsp::CompletionContext>,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::CompletionResponse>, AnyError> {
    let mut response: Result<Option<lsp::CompletionResponse>, AnyError> = self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideCompletion".to_string(),
          args: json!([&module.specifier, position, context]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await;
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
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ResolveCompletionItem".to_string(),
          args: json!([item]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_implementations(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoImplementationResponse>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideImplementations".to_string(),
          args: json!({
            "textDocument": { "uri": &module.specifier },
            "position": position,
          }),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_folding_range(
    &self,
    module: &DocumentModule,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::FoldingRange>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideFoldingRange".to_string(),
          args: json!([&module.specifier]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_call_hierarchy_incoming_calls(
    &self,
    module: &DocumentModule,
    item: &lsp::CallHierarchyItem,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyIncomingCall>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideCallHierarchyIncomingCalls".to_string(),
          args: json!([item]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_call_hierarchy_outgoing_calls(
    &self,
    module: &DocumentModule,
    item: &lsp::CallHierarchyItem,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyOutgoingCall>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideCallHierarchyOutgoingCalls".to_string(),
          args: json!([item]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_prepare_call_hierarchy(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyItem>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvidePrepareCallHierarchy".to_string(),
          args: json!([&module.specifier, position]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_rename(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    new_name: &str,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideRename".to_string(),
          args: json!([{
            "textDocument": { "uri": &module.specifier },
            "position": position,
            "newName": new_name,
          }]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_selection_ranges(
    &self,
    module: &DocumentModule,
    positions: &[lsp::Position],
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SelectionRange>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideSelectionRanges".to_string(),
          args: json!([{
            "textDocument": { "uri": &module.specifier },
            "positions": positions,
          }]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_signature_help(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: Option<&lsp::SignatureHelpContext>,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::SignatureHelp>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideSignatureHelp".to_string(),
          args: json!([&module.specifier, position, context]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_inlay_hint(
    &self,
    module: &DocumentModule,
    range: lsp::Range,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::InlayHint>>, AnyError> {
    self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideInlayHint".to_string(),
          args: json!([{
            "textDocument": { "uri": &module.specifier },
            "range": range,
          }]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await
  }

  pub async fn provide_workspace_symbol(
    &self,
    query: &str,
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SymbolInformation>>, AnyError> {
    self
      .request(
        TsGoRequest::WorkspaceSymbol {
          query: query.to_string(),
        },
        snapshot,
        token,
      )
      .await
  }
}
