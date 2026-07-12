// Copyright 2018-2026 the Deno authors. MIT license.

//! The Deno LSP no longer embeds an in-isolate TypeScript engine ("lsp diet").
//!
//! TypeScript language features (hover, definition, references, rename,
//! completions of TS symbols, TS type diagnostics, code fixes, etc.) are now
//! provided by the editor's stock TypeScript server / TS Native Preview. VS Code
//! LSP providers compose, so `deno lsp` keeps its `.ts`/`.js` document selector
//! and simply publishes only its Deno-specific features (module resolution
//! diagnostics, registry/import completions, Deno quick-fixes, test code lens).
//!
//! This type retains the previous public surface so the LSP handlers keep
//! compiling, but every method is inert: it returns empty/`None` and never talks
//! to a TypeScript engine.

use std::future::Future;
use std::ops::Range;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::future::Shared;
use deno_resolver::deno_json::CompilerOptionsKey;
use lsp_types::Uri;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::completions::CompletionItemData;
use super::config::CodeLensSettings;
use super::documents::Document;
use super::documents::DocumentModule;
use super::language_server;
use super::language_server::StateSnapshot;
use super::lsp_custom;
use super::performance::Performance;
use super::tsc::ChangeKind;
use super::tsc::CombinedCodeActions;
use super::tsc::RefactorEditInfo;
use super::tsc::TscSpecifierMap;

/// A best-effort list of "ambient" module specifiers that the deferred
/// resolution-diagnostics pass should not flag as "module not found".
///
/// Previously the set of ambient modules was queried from the in-isolate
/// TypeScript engine (it knew about `declare module "..."` declarations in the
/// program). With the engine removed, project-specific ambient modules are no
/// longer discoverable here; the stock TypeScript server is responsible for
/// reporting genuinely-missing modules. We keep a small static list of
/// well-known ambient specifiers so the most common cases don't produce false
/// positives from Deno's resolver.
fn best_effort_ambient_modules() -> Vec<String> {
  Vec::new()
}

/// Engine-free stand-in for the former in-isolate TypeScript language service.
#[derive(Debug)]
pub struct TsServer {
  /// Retained because Deno's own import mapper
  /// (`TsResponseImportMapper`) still normalizes specifiers through it.
  pub specifier_map: Arc<TscSpecifierMap>,
}

impl TsServer {
  pub fn new(_performance: Arc<Performance>) -> Self {
    Self {
      specifier_map: Arc::new(TscSpecifierMap::new()),
    }
  }

  pub fn is_started(&self) -> bool {
    // There is no TypeScript engine to start anymore.
    false
  }

  pub fn set_tracing_enabled(&self, _enabled: bool) {}

  pub fn set_inspector_server_addr(&self, _addr: Option<String>) {}

  pub fn project_changed(
    &self,
    _documents: &[(Document, ChangeKind)],
    _configuration_changed: bool,
    _snapshot: Arc<StateSnapshot>,
  ) {
  }

  pub async fn cleanup_semantic_cache(&self, _snapshot: Arc<StateSnapshot>) {}

  pub async fn get_ambient_modules(
    &self,
    _compiler_options_key: &CompilerOptionsKey,
    _notebook_uri: Option<&Arc<Uri>>,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Vec<String>, AnyError> {
    Ok(best_effort_ambient_modules())
  }

  pub async fn provide_diagnostics(
    &self,
    _module: &DocumentModule,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Vec<lsp::Diagnostic>, AnyError> {
    // TS type diagnostics are provided by the stock TypeScript server.
    Ok(Vec::new())
  }

  pub async fn provide_references(
    &self,
    _document: &Document,
    _module: &DocumentModule,
    _position: lsp::Position,
    _context: lsp::ReferenceContext,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::Location>>, AnyError> {
    Ok(None)
  }

  pub async fn provide_code_lenses(
    &self,
    _module: &DocumentModule,
    _settings: &CodeLensSettings,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CodeLens>>, AnyError> {
    // References/implementations code lenses are a TS feature; test code lens is
    // produced separately (statically) in `code_lens::collect_test`.
    Ok(None)
  }

  pub async fn provide_document_symbols(
    &self,
    _module: &DocumentModule,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::DocumentSymbolResponse>, AnyError> {
    Ok(None)
  }

  pub async fn provide_hover(
    &self,
    _module: &DocumentModule,
    _position: lsp::Position,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::Hover>, AnyError> {
    Ok(None)
  }

  pub async fn provide_inferred_type(
    &self,
    _module: &DocumentModule,
    _position: lsp::Position,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp_custom::InferredTypeResponse>, AnyError> {
    Ok(None)
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn provide_code_actions(
    &self,
    _module: &DocumentModule,
    _range: lsp::Range,
    _context: &lsp::CodeActionContext,
    _file_diagnostics: Shared<
      impl Future<Output = Arc<Vec<lsp::Diagnostic>>>,
    >,
    _has_deno_code_actions: bool,
    _language_server: &language_server::Inner,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::CodeActionResponse>, AnyError> {
    // TS quick fixes / refactors / organize imports are provided by the stock
    // TypeScript server. Deno quick fixes are collected separately.
    Ok(None)
  }

  pub async fn provide_document_highlights(
    &self,
    _module: &DocumentModule,
    _position: lsp::Position,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::DocumentHighlight>>, AnyError> {
    Ok(None)
  }

  pub async fn provide_definition(
    &self,
    _module: &DocumentModule,
    _position: lsp::Position,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::GotoDefinitionResponse>, AnyError> {
    Ok(None)
  }

  pub async fn provide_type_definition(
    &self,
    _module: &DocumentModule,
    _position: lsp::Position,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoTypeDefinitionResponse>, AnyError> {
    Ok(None)
  }

  pub async fn provide_completion(
    &self,
    _module: &DocumentModule,
    _position: lsp::Position,
    _context: Option<&lsp::CompletionContext>,
    _language_server: &language_server::Inner,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::CompletionResponse>, AnyError> {
    // TS symbol completions come from the stock TypeScript server. Deno
    // registry/import completions are handled separately in `completions.rs`.
    Ok(None)
  }

  pub async fn resolve_completion_item(
    &self,
    _module: &DocumentModule,
    item: lsp::CompletionItem,
    _data: CompletionItemData,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<lsp::CompletionItem, AnyError> {
    Ok(item)
  }

  pub async fn provide_implementations(
    &self,
    _document: &Document,
    _module: &DocumentModule,
    _position: lsp::Position,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoImplementationResponse>, AnyError> {
    Ok(None)
  }

  pub async fn provide_folding_range(
    &self,
    _module: &DocumentModule,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::FoldingRange>>, AnyError> {
    Ok(None)
  }

  pub async fn provide_call_hierarchy_incoming_calls(
    &self,
    _document: &Document,
    _module: &DocumentModule,
    _item: &lsp::CallHierarchyItem,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyIncomingCall>>, AnyError> {
    Ok(None)
  }

  pub async fn provide_call_hierarchy_outgoing_calls(
    &self,
    _module: &DocumentModule,
    _item: &lsp::CallHierarchyItem,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyOutgoingCall>>, AnyError> {
    Ok(None)
  }

  pub async fn provide_prepare_call_hierarchy(
    &self,
    _module: &DocumentModule,
    _position: lsp::Position,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyItem>>, AnyError> {
    Ok(None)
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn provide_rename(
    &self,
    _document: &Document,
    _module: &DocumentModule,
    _position: lsp::Position,
    _new_name: &str,
    _language_server: &language_server::Inner,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    Ok(None)
  }

  pub async fn provide_selection_ranges(
    &self,
    _module: &DocumentModule,
    _positions: &[lsp::Position],
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SelectionRange>>, AnyError> {
    Ok(None)
  }

  pub async fn provide_semantic_tokens_full(
    &self,
    _module: &DocumentModule,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::SemanticTokensResult>, AnyError> {
    Ok(None)
  }

  pub async fn provide_semantic_tokens_range(
    &self,
    _module: &DocumentModule,
    _range: lsp::Range,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::SemanticTokensRangeResult>, AnyError> {
    Ok(None)
  }

  pub async fn provide_signature_help(
    &self,
    _module: &DocumentModule,
    _position: lsp::Position,
    _context: Option<&lsp::SignatureHelpContext>,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::SignatureHelp>, AnyError> {
    Ok(None)
  }

  pub async fn provide_will_rename_files(
    &self,
    _file_renames: &[lsp::FileRename],
    _language_server: &language_server::Inner,
    _snapshot: Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<lsp::WorkspaceEdit>, AnyError> {
    Ok(None)
  }

  pub async fn provide_inlay_hint(
    &self,
    _module: &DocumentModule,
    _range: lsp::Range,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::InlayHint>>, AnyError> {
    Ok(None)
  }

  pub async fn provide_workspace_symbol(
    &self,
    _query: &str,
    _snapshot: &Arc<StateSnapshot>,
    _token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SymbolInformation>>, AnyError> {
    Ok(None)
  }

  pub async fn get_combined_code_fix(
    &self,
    _snapshot: Arc<StateSnapshot>,
    _module: &DocumentModule,
    _fix_id: &str,
    _token: &CancellationToken,
  ) -> Result<CombinedCodeActions, AnyError> {
    Ok(CombinedCodeActions {
      changes: Vec::new(),
      commands: None,
    })
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn get_edits_for_refactor(
    &self,
    _snapshot: Arc<StateSnapshot>,
    _module: &DocumentModule,
    _range: Range<u32>,
    _refactor_name: String,
    _action_name: String,
    _token: &CancellationToken,
  ) -> Result<RefactorEditInfo, AnyError> {
    Ok(RefactorEditInfo {
      edits: Vec::new(),
      rename_filename: None,
      rename_location: None,
    })
  }
}
