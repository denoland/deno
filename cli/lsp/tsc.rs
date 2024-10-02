// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use super::analysis::CodeActionData;
use super::code_lens;
use super::config;
use super::config::LspTsConfig;
use super::documents::AssetOrDocument;
use super::documents::Document;
use super::documents::DocumentsFilter;
use super::language_server;
use super::language_server::StateSnapshot;
use super::performance::Performance;
use super::performance::PerformanceMark;
use super::refactor::RefactorCodeActionData;
use super::refactor::ALL_KNOWN_REFACTOR_ACTION_KINDS;
use super::refactor::EXTRACT_CONSTANT;
use super::refactor::EXTRACT_INTERFACE;
use super::refactor::EXTRACT_TYPE;
use super::semantic_tokens;
use super::semantic_tokens::SemanticTokensBuilder;
use super::text::LineIndex;
use super::urls::uri_to_url;
use super::urls::url_to_uri;
use super::urls::INVALID_SPECIFIER;
use super::urls::INVALID_URI;

use crate::args::jsr_url;
use crate::args::FmtOptionsConfig;
use crate::lsp::logging::lsp_warn;
use crate::tsc;
use crate::tsc::ResolveArgs;
use crate::tsc::MISSING_DEPENDENCY_SPECIFIER;
use crate::util::path::relative_specifier;
use crate::util::path::to_percent_decoded_str;
use crate::util::result::InfallibleResultExt;
use crate::util::v8::convert;
use deno_core::convert::Smi;
use deno_core::convert::ToV8;
use deno_core::error::StdAnyError;
use deno_core::futures::stream::FuturesOrdered;
use deno_core::futures::StreamExt;

use dashmap::DashMap;
use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context as _;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_core::resolve_url;
use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::PollEventLoopOptions;
use deno_core::RuntimeOptions;
use deno_path_util::url_to_file_path;
use deno_runtime::inspector_server::InspectorServer;
use deno_runtime::tokio_util::create_basic_runtime;
use indexmap::IndexMap;
use indexmap::IndexSet;
use lazy_regex::lazy_regex;
use log::error;
use once_cell::sync::Lazy;
use regex::Captures;
use regex::Regex;
use serde_repr::Deserialize_repr;
use serde_repr::Serialize_repr;
use std::cell::RefCell;
use std::cmp;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use text_size::TextRange;
use text_size::TextSize;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tower_lsp::jsonrpc::Error as LspError;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types as lsp;

static BRACKET_ACCESSOR_RE: Lazy<Regex> =
  lazy_regex!(r#"^\[['"](.+)[\['"]\]$"#);
static CAPTION_RE: Lazy<Regex> =
  lazy_regex!(r"<caption>(.*?)</caption>\s*\r?\n((?:\s|\S)*)");
static CODEBLOCK_RE: Lazy<Regex> = lazy_regex!(r"^\s*[~`]{3}"m);
static EMAIL_MATCH_RE: Lazy<Regex> = lazy_regex!(r"(.+)\s<([-.\w]+@[-.\w]+)>");
static HTTP_RE: Lazy<Regex> = lazy_regex!(r#"(?i)^https?:"#);
static JSDOC_LINKS_RE: Lazy<Regex> = lazy_regex!(
  r"(?i)\{@(link|linkplain|linkcode) (https?://[^ |}]+?)(?:[| ]([^{}\n]+?))?\}"
);
static PART_KIND_MODIFIER_RE: Lazy<Regex> = lazy_regex!(r",|\s+");
static PART_RE: Lazy<Regex> = lazy_regex!(r"^(\S+)\s*-?\s*");
static SCOPE_RE: Lazy<Regex> = lazy_regex!(r"scope_(\d)");

const FILE_EXTENSION_KIND_MODIFIERS: &[&str] =
  &[".d.ts", ".ts", ".tsx", ".js", ".jsx", ".json"];

type Request = (
  TscRequest,
  Option<ModuleSpecifier>,
  Arc<StateSnapshot>,
  oneshot::Sender<Result<String, AnyError>>,
  CancellationToken,
  Option<PendingChange>,
);

#[derive(Debug, Clone, Copy, Serialize_repr)]
#[repr(u8)]
pub enum IndentStyle {
  #[allow(dead_code)]
  None = 0,
  Block = 1,
  #[allow(dead_code)]
  Smart = 2,
}

/// Relevant subset of https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6658.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatCodeSettings {
  base_indent_size: Option<u8>,
  indent_size: Option<u8>,
  tab_size: Option<u8>,
  new_line_character: Option<String>,
  convert_tabs_to_spaces: Option<bool>,
  indent_style: Option<IndentStyle>,
  trim_trailing_whitespace: Option<bool>,
  insert_space_after_comma_delimiter: Option<bool>,
  insert_space_after_semicolon_in_for_statements: Option<bool>,
  insert_space_before_and_after_binary_operators: Option<bool>,
  insert_space_after_constructor: Option<bool>,
  insert_space_after_keywords_in_control_flow_statements: Option<bool>,
  insert_space_after_function_keyword_for_anonymous_functions: Option<bool>,
  insert_space_after_opening_and_before_closing_nonempty_parenthesis:
    Option<bool>,
  insert_space_after_opening_and_before_closing_nonempty_brackets: Option<bool>,
  insert_space_after_opening_and_before_closing_nonempty_braces: Option<bool>,
  insert_space_after_opening_and_before_closing_template_string_braces:
    Option<bool>,
  insert_space_after_opening_and_before_closing_jsx_expression_braces:
    Option<bool>,
  insert_space_after_type_assertion: Option<bool>,
  insert_space_before_function_parenthesis: Option<bool>,
  place_open_brace_on_new_line_for_functions: Option<bool>,
  place_open_brace_on_new_line_for_control_blocks: Option<bool>,
  insert_space_before_type_annotation: Option<bool>,
  indent_multi_line_object_literal_beginning_on_blank_line: Option<bool>,
  semicolons: Option<SemicolonPreference>,
  indent_switch_case: Option<bool>,
}

impl From<&FmtOptionsConfig> for FormatCodeSettings {
  fn from(config: &FmtOptionsConfig) -> Self {
    FormatCodeSettings {
      base_indent_size: Some(0),
      indent_size: Some(config.indent_width.unwrap_or(2)),
      tab_size: Some(config.indent_width.unwrap_or(2)),
      new_line_character: Some("\n".to_string()),
      convert_tabs_to_spaces: Some(!config.use_tabs.unwrap_or(false)),
      indent_style: Some(IndentStyle::Block),
      trim_trailing_whitespace: Some(false),
      insert_space_after_comma_delimiter: Some(true),
      insert_space_after_semicolon_in_for_statements: Some(true),
      insert_space_before_and_after_binary_operators: Some(true),
      insert_space_after_constructor: Some(false),
      insert_space_after_keywords_in_control_flow_statements: Some(true),
      insert_space_after_function_keyword_for_anonymous_functions: Some(true),
      insert_space_after_opening_and_before_closing_nonempty_parenthesis: Some(
        false,
      ),
      insert_space_after_opening_and_before_closing_nonempty_brackets: Some(
        false,
      ),
      insert_space_after_opening_and_before_closing_nonempty_braces: Some(true),
      insert_space_after_opening_and_before_closing_template_string_braces:
        Some(false),
      insert_space_after_opening_and_before_closing_jsx_expression_braces: Some(
        false,
      ),
      insert_space_after_type_assertion: Some(false),
      insert_space_before_function_parenthesis: Some(false),
      place_open_brace_on_new_line_for_functions: Some(false),
      place_open_brace_on_new_line_for_control_blocks: Some(false),
      insert_space_before_type_annotation: Some(false),
      indent_multi_line_object_literal_beginning_on_blank_line: Some(false),
      semicolons: match config.semi_colons {
        Some(false) => Some(SemicolonPreference::Remove),
        _ => Some(SemicolonPreference::Insert),
      },
      indent_switch_case: Some(true),
    }
  }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SemicolonPreference {
  Insert,
  Remove,
}

// Allow due to false positive https://github.com/rust-lang/rust-clippy/issues/13170
#[allow(clippy::needless_borrows_for_generic_args)]
fn normalize_diagnostic(
  diagnostic: &mut crate::tsc::Diagnostic,
  specifier_map: &TscSpecifierMap,
) -> Result<(), AnyError> {
  if let Some(file_name) = &mut diagnostic.file_name {
    *file_name = specifier_map.normalize(&file_name)?.to_string();
  }
  for ri in diagnostic.related_information.iter_mut().flatten() {
    normalize_diagnostic(ri, specifier_map)?;
  }
  Ok(())
}

pub struct TsServer {
  performance: Arc<Performance>,
  sender: mpsc::UnboundedSender<Request>,
  receiver: Mutex<Option<mpsc::UnboundedReceiver<Request>>>,
  specifier_map: Arc<TscSpecifierMap>,
  inspector_server: Mutex<Option<Arc<InspectorServer>>>,
  pending_change: Mutex<Option<PendingChange>>,
}

impl std::fmt::Debug for TsServer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TsServer")
      .field("performance", &self.performance)
      .field("sender", &self.sender)
      .field("receiver", &self.receiver)
      .field("specifier_map", &self.specifier_map)
      .field("inspector_server", &self.inspector_server.lock().is_some())
      .finish()
  }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum ChangeKind {
  Opened = 0,
  Modified = 1,
  Closed = 2,
}

impl<'a> ToV8<'a> for ChangeKind {
  type Error = Infallible;
  fn to_v8(
    self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    Smi(self as u8).to_v8(scope)
  }
}

impl Serialize for ChangeKind {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_i32(*self as i32)
  }
}

#[derive(Debug)]
#[cfg_attr(test, derive(Serialize))]
pub struct PendingChange {
  pub modified_scripts: Vec<(String, ChangeKind)>,
  pub project_version: usize,
  pub new_configs_by_scope: Option<BTreeMap<ModuleSpecifier, Arc<LspTsConfig>>>,
}

impl<'a> ToV8<'a> for PendingChange {
  type Error = Infallible;
  fn to_v8(
    self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let modified_scripts = {
      let mut modified_scripts_v8 =
        Vec::with_capacity(self.modified_scripts.len());
      for (specifier, kind) in &self.modified_scripts {
        let specifier = v8::String::new(scope, specifier).unwrap().into();
        let kind = kind.to_v8(scope).unwrap_infallible();
        let pair =
          v8::Array::new_with_elements(scope, &[specifier, kind]).into();
        modified_scripts_v8.push(pair);
      }
      v8::Array::new_with_elements(scope, &modified_scripts_v8).into()
    };
    let project_version =
      v8::Integer::new_from_unsigned(scope, self.project_version as u32).into();
    let new_configs_by_scope =
      if let Some(new_configs_by_scope) = self.new_configs_by_scope {
        serde_v8::to_v8(
          scope,
          new_configs_by_scope.into_iter().collect::<Vec<_>>(),
        )
        .unwrap_or_else(|err| {
          lsp_warn!("Couldn't serialize ts configs: {err}");
          v8::null(scope).into()
        })
      } else {
        v8::null(scope).into()
      };

    Ok(
      v8::Array::new_with_elements(
        scope,
        &[modified_scripts, project_version, new_configs_by_scope],
      )
      .into(),
    )
  }
}

impl PendingChange {
  fn coalesce(
    &mut self,
    new_version: usize,
    modified_scripts: Vec<(String, ChangeKind)>,
    new_configs_by_scope: Option<BTreeMap<ModuleSpecifier, Arc<LspTsConfig>>>,
  ) {
    use ChangeKind::*;
    self.project_version = self.project_version.max(new_version);
    if let Some(new_configs_by_scope) = new_configs_by_scope {
      self.new_configs_by_scope = Some(new_configs_by_scope);
    }
    for (spec, new) in modified_scripts {
      if let Some((_, current)) =
        self.modified_scripts.iter_mut().find(|(s, _)| s == &spec)
      {
        // already a pending change for this specifier,
        // coalesce the change kinds
        match (*current, new) {
          (_, Closed) => {
            *current = Closed;
          }
          (Opened | Closed, Opened) => {
            *current = Opened;
          }
          (Modified, Opened) => {
            lsp_warn!("Unexpected change from Modified -> Opened");
            *current = Opened;
          }
          (Opened, Modified) => {
            // Opening may change the set of files in the project
            *current = Opened;
          }
          (Closed, Modified) => {
            lsp_warn!("Unexpected change from Closed -> Modifed");
            // Shouldn't happen, but if it does treat it as closed
            // since it's "stronger" than modifying an open doc
            *current = Closed;
          }
          (Modified, Modified) => {
            // no change
          }
        }
      } else {
        self.modified_scripts.push((spec, new));
      }
    }
  }
}

impl TsServer {
  pub fn new(performance: Arc<Performance>) -> Self {
    let (tx, request_rx) = mpsc::unbounded_channel::<Request>();
    Self {
      performance,
      sender: tx,
      receiver: Mutex::new(Some(request_rx)),
      specifier_map: Arc::new(TscSpecifierMap::new()),
      inspector_server: Mutex::new(None),
      pending_change: Mutex::new(None),
    }
  }

  pub fn start(
    &self,
    inspector_server_addr: Option<String>,
  ) -> Result<(), AnyError> {
    let maybe_inspector_server = match inspector_server_addr {
      Some(addr) => {
        let addr: SocketAddr = addr.parse().with_context(|| {
          format!("Invalid inspector server address \"{}\"", &addr)
        })?;
        let server = InspectorServer::new(addr, "deno-lsp-tsc")?;
        Some(Arc::new(server))
      }
      None => None,
    };
    self
      .inspector_server
      .lock()
      .clone_from(&maybe_inspector_server);
    // TODO(bartlomieju): why is the join_handle ignored here? Should we store it
    // on the `TsServer` struct.
    let receiver = self.receiver.lock().take().unwrap();
    let performance = self.performance.clone();
    let specifier_map = self.specifier_map.clone();
    let _join_handle = thread::spawn(move || {
      run_tsc_thread(
        receiver,
        performance,
        specifier_map,
        maybe_inspector_server,
      )
    });
    Ok(())
  }

  pub fn project_changed<'a>(
    &self,
    snapshot: Arc<StateSnapshot>,
    modified_scripts: impl IntoIterator<Item = (&'a ModuleSpecifier, ChangeKind)>,
    new_configs_by_scope: Option<BTreeMap<ModuleSpecifier, Arc<LspTsConfig>>>,
  ) {
    let modified_scripts = modified_scripts
      .into_iter()
      .map(|(spec, change)| (self.specifier_map.denormalize(spec), change))
      .collect::<Vec<_>>();
    match &mut *self.pending_change.lock() {
      Some(pending_change) => {
        pending_change.coalesce(
          snapshot.project_version,
          modified_scripts,
          new_configs_by_scope,
        );
      }
      pending => {
        let pending_change = PendingChange {
          modified_scripts,
          project_version: snapshot.project_version,
          new_configs_by_scope,
        };
        *pending = Some(pending_change);
      }
    }
  }

  pub async fn get_diagnostics(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifiers: Vec<ModuleSpecifier>,
    token: CancellationToken,
  ) -> Result<IndexMap<String, Vec<crate::tsc::Diagnostic>>, AnyError> {
    let mut diagnostics_map = IndexMap::with_capacity(specifiers.len());
    let mut specifiers_by_scope = BTreeMap::new();
    for specifier in specifiers {
      let scope = if snapshot.documents.is_valid_file_referrer(&specifier) {
        snapshot
          .config
          .tree
          .scope_for_specifier(&specifier)
          .cloned()
      } else {
        snapshot
          .documents
          .get(&specifier)
          .and_then(|d| d.scope().cloned())
      };
      let specifiers = specifiers_by_scope.entry(scope).or_insert(vec![]);
      specifiers.push(self.specifier_map.denormalize(&specifier));
    }
    let mut results = FuturesOrdered::new();
    for (scope, specifiers) in specifiers_by_scope {
      let req =
        TscRequest::GetDiagnostics((specifiers, snapshot.project_version));
      results.push_back(self.request_with_cancellation::<IndexMap<String, Vec<crate::tsc::Diagnostic>>>(snapshot.clone(), req, scope, token.clone()));
    }
    while let Some(raw_diagnostics) = results.next().await {
      let raw_diagnostics = raw_diagnostics
        .inspect_err(|err| {
          if !token.is_cancelled() {
            lsp_warn!("Error generating TypeScript diagnostics: {err}");
          }
        })
        .unwrap_or_default();
      for (mut specifier, mut diagnostics) in raw_diagnostics {
        specifier = self.specifier_map.normalize(&specifier)?.to_string();
        for diagnostic in &mut diagnostics {
          normalize_diagnostic(diagnostic, &self.specifier_map)?;
        }
        diagnostics_map.insert(specifier, diagnostics);
      }
    }
    Ok(diagnostics_map)
  }

  pub async fn cleanup_semantic_cache(&self, snapshot: Arc<StateSnapshot>) {
    for scope in snapshot
      .config
      .tree
      .data_by_scope()
      .keys()
      .map(Some)
      .chain(std::iter::once(None))
    {
      let req = TscRequest::CleanupSemanticCache;
      self
        .request::<()>(snapshot.clone(), req, scope.cloned())
        .await
        .map_err(|err| {
          log::error!("Failed to request to tsserver {}", err);
          LspError::invalid_request()
        })
        .ok();
    }
  }

  pub async fn find_references(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
  ) -> Result<Option<Vec<ReferencedSymbol>>, AnyError> {
    let req = TscRequest::FindReferences((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    let mut results = FuturesOrdered::new();
    for scope in snapshot
      .config
      .tree
      .data_by_scope()
      .keys()
      .map(Some)
      .chain(std::iter::once(None))
    {
      results.push_back(self.request::<Option<Vec<ReferencedSymbol>>>(
        snapshot.clone(),
        req.clone(),
        scope.cloned(),
      ));
    }
    let mut all_symbols = IndexSet::new();
    while let Some(symbols) = results.next().await {
      let symbols = symbols
        .inspect_err(|err| {
          let err = err.to_string();
          if !err.contains("Could not find source file") {
            lsp_warn!("Unable to get references from TypeScript: {err}");
          }
        })
        .unwrap_or_default();
      let Some(mut symbols) = symbols else {
        continue;
      };
      for symbol in &mut symbols {
        symbol.normalize(&self.specifier_map)?;
      }
      all_symbols.extend(symbols);
    }
    if all_symbols.is_empty() {
      return Ok(None);
    }
    Ok(Some(all_symbols.into_iter().collect()))
  }

  pub async fn get_navigation_tree(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    scope: Option<ModuleSpecifier>,
  ) -> Result<NavigationTree, AnyError> {
    let req = TscRequest::GetNavigationTree((self
      .specifier_map
      .denormalize(&specifier),));
    self.request(snapshot, req, scope).await
  }

  pub async fn get_supported_code_fixes(
    &self,
    snapshot: Arc<StateSnapshot>,
  ) -> Result<Vec<String>, LspError> {
    let req = TscRequest::GetSupportedCodeFixes;
    self.request(snapshot, req, None).await.map_err(|err| {
      log::error!("Unable to get fixable diagnostics: {}", err);
      LspError::internal_error()
    })
  }

  pub async fn get_quick_info(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Option<QuickInfo>, LspError> {
    let req = TscRequest::GetQuickInfoAtPosition((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    self.request(snapshot, req, scope).await.map_err(|err| {
      log::error!("Unable to get quick info: {}", err);
      LspError::internal_error()
    })
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn get_code_fixes(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    range: Range<u32>,
    codes: Vec<i32>,
    format_code_settings: FormatCodeSettings,
    preferences: UserPreferences,
    scope: Option<ModuleSpecifier>,
  ) -> Vec<CodeFixAction> {
    let req = TscRequest::GetCodeFixesAtPosition(Box::new((
      self.specifier_map.denormalize(&specifier),
      range.start,
      range.end,
      codes,
      format_code_settings,
      preferences,
    )));
    let result = self
      .request::<Vec<CodeFixAction>>(snapshot, req, scope)
      .await
      .and_then(|mut actions| {
        for action in &mut actions {
          action.normalize(&self.specifier_map)?;
        }
        Ok(actions)
      });
    match result {
      Ok(items) => items,
      Err(err) => {
        // sometimes tsc reports errors when retrieving code actions
        // because they don't reflect the current state of the document
        // so we will log them to the output, but we won't send an error
        // message back to the client.
        log::error!("Error getting actions from TypeScript: {}", err);
        Vec::new()
      }
    }
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn get_applicable_refactors(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    range: Range<u32>,
    preferences: Option<UserPreferences>,
    trigger_kind: Option<lsp::CodeActionTriggerKind>,
    only: String,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Vec<ApplicableRefactorInfo>, LspError> {
    let trigger_kind = trigger_kind.map(|reason| match reason {
      lsp::CodeActionTriggerKind::INVOKED => "invoked",
      lsp::CodeActionTriggerKind::AUTOMATIC => "implicit",
      _ => unreachable!(),
    });
    let req = TscRequest::GetApplicableRefactors(Box::new((
      self.specifier_map.denormalize(&specifier),
      range.into(),
      preferences.unwrap_or_default(),
      trigger_kind,
      only,
    )));
    self.request(snapshot, req, scope).await.map_err(|err| {
      log::error!("Failed to request to tsserver {}", err);
      LspError::invalid_request()
    })
  }

  pub async fn get_combined_code_fix(
    &self,
    snapshot: Arc<StateSnapshot>,
    code_action_data: &CodeActionData,
    format_code_settings: FormatCodeSettings,
    preferences: UserPreferences,
    scope: Option<ModuleSpecifier>,
  ) -> Result<CombinedCodeActions, LspError> {
    let req = TscRequest::GetCombinedCodeFix(Box::new((
      CombinedCodeFixScope {
        r#type: "file",
        file_name: self.specifier_map.denormalize(&code_action_data.specifier),
      },
      code_action_data.fix_id.clone(),
      format_code_settings,
      preferences,
    )));
    self
      .request::<CombinedCodeActions>(snapshot, req, scope)
      .await
      .and_then(|mut actions| {
        actions.normalize(&self.specifier_map)?;
        Ok(actions)
      })
      .map_err(|err| {
        log::error!("Unable to get combined fix from TypeScript: {}", err);
        LspError::internal_error()
      })
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn get_edits_for_refactor(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    format_code_settings: FormatCodeSettings,
    range: Range<u32>,
    refactor_name: String,
    action_name: String,
    preferences: Option<UserPreferences>,
    scope: Option<ModuleSpecifier>,
  ) -> Result<RefactorEditInfo, LspError> {
    let req = TscRequest::GetEditsForRefactor(Box::new((
      self.specifier_map.denormalize(&specifier),
      format_code_settings,
      range.into(),
      refactor_name,
      action_name,
      preferences,
    )));
    self
      .request::<RefactorEditInfo>(snapshot, req, scope)
      .await
      .and_then(|mut info| {
        info.normalize(&self.specifier_map)?;
        Ok(info)
      })
      .map_err(|err| {
        log::error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })
  }

  pub async fn get_edits_for_file_rename(
    &self,
    snapshot: Arc<StateSnapshot>,
    old_specifier: ModuleSpecifier,
    new_specifier: ModuleSpecifier,
    format_code_settings: FormatCodeSettings,
    user_preferences: UserPreferences,
  ) -> Result<Vec<FileTextChanges>, AnyError> {
    let req = TscRequest::GetEditsForFileRename(Box::new((
      self.specifier_map.denormalize(&old_specifier),
      self.specifier_map.denormalize(&new_specifier),
      format_code_settings,
      user_preferences,
    )));
    let mut results = FuturesOrdered::new();
    for scope in snapshot
      .config
      .tree
      .data_by_scope()
      .keys()
      .map(Some)
      .chain(std::iter::once(None))
    {
      results.push_back(self.request::<Vec<FileTextChanges>>(
        snapshot.clone(),
        req.clone(),
        scope.cloned(),
      ));
    }
    let mut all_changes = IndexSet::new();
    while let Some(changes) = results.next().await {
      let mut changes = changes
        .inspect_err(|err| {
          lsp_warn!(
            "Unable to get edits for file rename from TypeScript: {err}"
          );
        })
        .unwrap_or_default();
      for changes in &mut changes {
        changes.normalize(&self.specifier_map)?;
        for text_changes in &mut changes.text_changes {
          text_changes.new_text =
            to_percent_decoded_str(&text_changes.new_text);
        }
      }
      all_changes.extend(changes);
    }
    Ok(all_changes.into_iter().collect())
  }

  pub async fn get_document_highlights(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    files_to_search: Vec<ModuleSpecifier>,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Option<Vec<DocumentHighlights>>, LspError> {
    let req = TscRequest::GetDocumentHighlights(Box::new((
      self.specifier_map.denormalize(&specifier),
      position,
      files_to_search
        .into_iter()
        .map(|s| self.specifier_map.denormalize(&s))
        .collect::<Vec<_>>(),
    )));
    self.request(snapshot, req, scope).await.map_err(|err| {
      log::error!("Unable to get document highlights from TypeScript: {}", err);
      LspError::internal_error()
    })
  }

  pub async fn get_definition(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Option<DefinitionInfoAndBoundSpan>, LspError> {
    let req = TscRequest::GetDefinitionAndBoundSpan((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    self
      .request::<Option<DefinitionInfoAndBoundSpan>>(snapshot, req, scope)
      .await
      .and_then(|mut info| {
        if let Some(info) = &mut info {
          info.normalize(&self.specifier_map)?;
        }
        Ok(info)
      })
      .map_err(|err| {
        log::error!("Unable to get definition from TypeScript: {}", err);
        LspError::internal_error()
      })
  }

  pub async fn get_type_definition(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Option<Vec<DefinitionInfo>>, LspError> {
    let req = TscRequest::GetTypeDefinitionAtPosition((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    self
      .request::<Option<Vec<DefinitionInfo>>>(snapshot, req, scope)
      .await
      .and_then(|mut infos| {
        for info in infos.iter_mut().flatten() {
          info.normalize(&self.specifier_map)?;
        }
        Ok(infos)
      })
      .map_err(|err| {
        log::error!("Unable to get type definition from TypeScript: {}", err);
        LspError::internal_error()
      })
  }

  pub async fn get_completions(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    options: GetCompletionsAtPositionOptions,
    format_code_settings: FormatCodeSettings,
    scope: Option<ModuleSpecifier>,
  ) -> Option<CompletionInfo> {
    let req = TscRequest::GetCompletionsAtPosition(Box::new((
      self.specifier_map.denormalize(&specifier),
      position,
      options,
      format_code_settings,
    )));
    match self.request(snapshot, req, scope).await {
      Ok(maybe_info) => maybe_info,
      Err(err) => {
        log::error!("Unable to get completion info from TypeScript: {:#}", err);
        None
      }
    }
  }

  pub async fn get_completion_details(
    &self,
    snapshot: Arc<StateSnapshot>,
    args: GetCompletionDetailsArgs,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Option<CompletionEntryDetails>, AnyError> {
    let req = TscRequest::GetCompletionEntryDetails(Box::new((
      self.specifier_map.denormalize(&args.specifier),
      args.position,
      args.name,
      args.format_code_settings.unwrap_or_default(),
      args.source,
      args.preferences,
      args.data,
    )));
    self
      .request::<Option<CompletionEntryDetails>>(snapshot, req, scope)
      .await
      .and_then(|mut details| {
        if let Some(details) = &mut details {
          details.normalize(&self.specifier_map)?;
        }
        Ok(details)
      })
  }

  pub async fn get_implementations(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
  ) -> Result<Option<Vec<ImplementationLocation>>, AnyError> {
    let req = TscRequest::GetImplementationAtPosition((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    let mut results = FuturesOrdered::new();
    for scope in snapshot
      .config
      .tree
      .data_by_scope()
      .keys()
      .map(Some)
      .chain(std::iter::once(None))
    {
      results.push_back(self.request::<Option<Vec<ImplementationLocation>>>(
        snapshot.clone(),
        req.clone(),
        scope.cloned(),
      ));
    }
    let mut all_locations = IndexSet::new();
    while let Some(locations) = results.next().await {
      let locations = locations
        .inspect_err(|err| {
          let err = err.to_string();
          if !err.contains("Could not find source file") {
            lsp_warn!("Unable to get implementations from TypeScript: {err}");
          }
        })
        .unwrap_or_default();
      let Some(mut locations) = locations else {
        continue;
      };
      for location in &mut locations {
        location.normalize(&self.specifier_map)?;
      }
      all_locations.extend(locations);
    }
    if all_locations.is_empty() {
      return Ok(None);
    }
    Ok(Some(all_locations.into_iter().collect()))
  }

  pub async fn get_outlining_spans(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Vec<OutliningSpan>, LspError> {
    let req = TscRequest::GetOutliningSpans((self
      .specifier_map
      .denormalize(&specifier),));
    self.request(snapshot, req, scope).await.map_err(|err| {
      log::error!("Failed to request to tsserver {}", err);
      LspError::invalid_request()
    })
  }

  pub async fn provide_call_hierarchy_incoming_calls(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
  ) -> Result<Vec<CallHierarchyIncomingCall>, AnyError> {
    let req = TscRequest::ProvideCallHierarchyIncomingCalls((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    let mut results = FuturesOrdered::new();
    for scope in snapshot
      .config
      .tree
      .data_by_scope()
      .keys()
      .map(Some)
      .chain(std::iter::once(None))
    {
      results.push_back(self.request::<Vec<CallHierarchyIncomingCall>>(
        snapshot.clone(),
        req.clone(),
        scope.cloned(),
      ));
    }
    let mut all_calls = IndexSet::new();
    while let Some(calls) = results.next().await {
      let mut calls = calls
        .inspect_err(|err| {
          let err = err.to_string();
          if !err.contains("Could not find source file") {
            lsp_warn!("Unable to get incoming calls from TypeScript: {err}");
          }
        })
        .unwrap_or_default();
      for call in &mut calls {
        call.normalize(&self.specifier_map)?;
      }
      all_calls.extend(calls)
    }
    Ok(all_calls.into_iter().collect())
  }

  pub async fn provide_call_hierarchy_outgoing_calls(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Vec<CallHierarchyOutgoingCall>, LspError> {
    let req = TscRequest::ProvideCallHierarchyOutgoingCalls((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    self
      .request::<Vec<CallHierarchyOutgoingCall>>(snapshot, req, scope)
      .await
      .and_then(|mut calls| {
        for call in &mut calls {
          call.normalize(&self.specifier_map)?;
        }
        Ok(calls)
      })
      .map_err(|err| {
        log::error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })
  }

  pub async fn prepare_call_hierarchy(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Option<OneOrMany<CallHierarchyItem>>, LspError> {
    let req = TscRequest::PrepareCallHierarchy((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    self
      .request::<Option<OneOrMany<CallHierarchyItem>>>(snapshot, req, scope)
      .await
      .and_then(|mut items| {
        match &mut items {
          Some(OneOrMany::One(item)) => {
            item.normalize(&self.specifier_map)?;
          }
          Some(OneOrMany::Many(items)) => {
            for item in items {
              item.normalize(&self.specifier_map)?;
            }
          }
          None => {}
        }
        Ok(items)
      })
      .map_err(|err| {
        log::error!("Failed to request to tsserver {}", err);
        LspError::invalid_request()
      })
  }

  pub async fn find_rename_locations(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
  ) -> Result<Option<Vec<RenameLocation>>, AnyError> {
    let req = TscRequest::FindRenameLocations((
      self.specifier_map.denormalize(&specifier),
      position,
      false,
      false,
      false,
    ));
    let mut results = FuturesOrdered::new();
    for scope in snapshot
      .config
      .tree
      .data_by_scope()
      .keys()
      .map(Some)
      .chain(std::iter::once(None))
    {
      results.push_back(self.request::<Option<Vec<RenameLocation>>>(
        snapshot.clone(),
        req.clone(),
        scope.cloned(),
      ));
    }
    let mut all_locations = IndexSet::new();
    while let Some(locations) = results.next().await {
      let locations = locations
        .inspect_err(|err| {
          let err = err.to_string();
          if !err.contains("Could not find source file") {
            lsp_warn!("Unable to get rename locations from TypeScript: {err}");
          }
        })
        .unwrap_or_default();
      let Some(mut locations) = locations else {
        continue;
      };
      for symbol in &mut locations {
        symbol.normalize(&self.specifier_map)?;
      }
      all_locations.extend(locations);
    }
    if all_locations.is_empty() {
      return Ok(None);
    }
    Ok(Some(all_locations.into_iter().collect()))
  }

  pub async fn get_smart_selection_range(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    scope: Option<ModuleSpecifier>,
  ) -> Result<SelectionRange, LspError> {
    let req = TscRequest::GetSmartSelectionRange((
      self.specifier_map.denormalize(&specifier),
      position,
    ));
    self.request(snapshot, req, scope).await.map_err(|err| {
      log::error!("Failed to request to tsserver {}", err);
      LspError::invalid_request()
    })
  }

  pub async fn get_encoded_semantic_classifications(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    range: Range<u32>,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Classifications, LspError> {
    let req = TscRequest::GetEncodedSemanticClassifications((
      self.specifier_map.denormalize(&specifier),
      TextSpan {
        start: range.start,
        length: range.end - range.start,
      },
      "2020",
    ));
    self.request(snapshot, req, scope).await.map_err(|err| {
      log::error!("Failed to request to tsserver {}", err);
      LspError::invalid_request()
    })
  }

  pub async fn get_signature_help_items(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    position: u32,
    options: SignatureHelpItemsOptions,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Option<SignatureHelpItems>, LspError> {
    let req = TscRequest::GetSignatureHelpItems((
      self.specifier_map.denormalize(&specifier),
      position,
      options,
    ));
    self.request(snapshot, req, scope).await.map_err(|err| {
      log::error!("Failed to request to tsserver: {}", err);
      LspError::invalid_request()
    })
  }

  pub async fn get_navigate_to_items(
    &self,
    snapshot: Arc<StateSnapshot>,
    args: GetNavigateToItemsArgs,
  ) -> Result<Vec<NavigateToItem>, AnyError> {
    let req = TscRequest::GetNavigateToItems((
      args.search,
      args.max_result_count,
      args.file.map(|f| match resolve_url(&f) {
        Ok(s) => self.specifier_map.denormalize(&s),
        Err(_) => f,
      }),
    ));
    let mut results = FuturesOrdered::new();
    for scope in snapshot
      .config
      .tree
      .data_by_scope()
      .keys()
      .map(Some)
      .chain(std::iter::once(None))
    {
      results.push_back(self.request::<Vec<NavigateToItem>>(
        snapshot.clone(),
        req.clone(),
        scope.cloned(),
      ));
    }
    let mut all_items = IndexSet::new();
    while let Some(items) = results.next().await {
      let mut items = items
        .inspect_err(|err| {
          lsp_warn!("Unable to get 'navigate to' items from TypeScript: {err}");
        })
        .unwrap_or_default();
      for item in &mut items {
        item.normalize(&self.specifier_map)?;
      }
      all_items.extend(items)
    }
    Ok(all_items.into_iter().collect())
  }

  pub async fn provide_inlay_hints(
    &self,
    snapshot: Arc<StateSnapshot>,
    specifier: ModuleSpecifier,
    text_span: TextSpan,
    user_preferences: UserPreferences,
    scope: Option<ModuleSpecifier>,
  ) -> Result<Option<Vec<InlayHint>>, LspError> {
    let req = TscRequest::ProvideInlayHints((
      self.specifier_map.denormalize(&specifier),
      text_span,
      user_preferences,
    ));
    self.request(snapshot, req, scope).await.map_err(|err| {
      log::error!("Unable to get inlay hints: {}", err);
      LspError::internal_error()
    })
  }

  async fn request<R>(
    &self,
    snapshot: Arc<StateSnapshot>,
    req: TscRequest,
    scope: Option<ModuleSpecifier>,
  ) -> Result<R, AnyError>
  where
    R: de::DeserializeOwned,
  {
    let mark = self
      .performance
      .mark(format!("tsc.request.{}", req.method()));
    let r = self
      .request_with_cancellation(snapshot, req, scope, Default::default())
      .await;
    self.performance.measure(mark);
    r
  }

  async fn request_with_cancellation<R>(
    &self,
    snapshot: Arc<StateSnapshot>,
    req: TscRequest,
    scope: Option<ModuleSpecifier>,
    token: CancellationToken,
  ) -> Result<R, AnyError>
  where
    R: de::DeserializeOwned,
  {
    // When an LSP request is cancelled by the client, the future this is being
    // executed under and any local variables here will be dropped at the next
    // await point. To pass on that cancellation to the TS thread, we make this
    // wrapper which cancels the request's token on drop.
    struct DroppableToken(CancellationToken);
    impl Drop for DroppableToken {
      fn drop(&mut self) {
        self.0.cancel();
      }
    }
    let token = token.child_token();
    let droppable_token = DroppableToken(token.clone());
    let (tx, mut rx) = oneshot::channel::<Result<String, AnyError>>();
    let change = self.pending_change.lock().take();

    if self
      .sender
      .send((req, scope, snapshot, tx, token.clone(), change))
      .is_err()
    {
      return Err(anyhow!("failed to send request to tsc thread"));
    }
    tokio::select! {
      value = &mut rx => {
        let value = value??;
        drop(droppable_token);
        Ok(serde_json::from_str(&value)?)
      }
      _ = token.cancelled() => {
        Err(anyhow!("request cancelled"))
      }
    }
  }
}

/// An lsp representation of an asset in memory, that has either been retrieved
/// from static assets built into Rust, or static assets built into tsc.
#[derive(Debug, Clone)]
pub struct AssetDocument {
  specifier: ModuleSpecifier,
  text: Arc<str>,
  line_index: Arc<LineIndex>,
  maybe_navigation_tree: Option<Arc<NavigationTree>>,
}

impl AssetDocument {
  pub fn new(specifier: ModuleSpecifier, text: impl AsRef<str>) -> Self {
    let text = text.as_ref();
    Self {
      specifier,
      text: text.into(),
      line_index: Arc::new(LineIndex::new(text)),
      maybe_navigation_tree: None,
    }
  }

  pub fn specifier(&self) -> &ModuleSpecifier {
    &self.specifier
  }

  pub fn with_navigation_tree(&self, tree: Arc<NavigationTree>) -> Self {
    Self {
      maybe_navigation_tree: Some(tree),
      ..self.clone()
    }
  }

  pub fn text(&self) -> Arc<str> {
    self.text.clone()
  }

  pub fn line_index(&self) -> Arc<LineIndex> {
    self.line_index.clone()
  }

  pub fn maybe_navigation_tree(&self) -> Option<Arc<NavigationTree>> {
    self.maybe_navigation_tree.clone()
  }
}

type AssetsMap = HashMap<ModuleSpecifier, AssetDocument>;

fn new_assets_map() -> Arc<Mutex<AssetsMap>> {
  let assets = tsc::LAZILY_LOADED_STATIC_ASSETS
    .iter()
    .map(|(k, v)| {
      let url_str = format!("asset:///{k}");
      let specifier = resolve_url(&url_str).unwrap();
      let asset = AssetDocument::new(specifier.clone(), v);
      (specifier, asset)
    })
    .collect::<AssetsMap>();
  Arc::new(Mutex::new(assets))
}

/// Snapshot of Assets.
#[derive(Debug, Clone)]
pub struct AssetsSnapshot(Arc<Mutex<AssetsMap>>);

impl Default for AssetsSnapshot {
  fn default() -> Self {
    Self(new_assets_map())
  }
}

impl AssetsSnapshot {
  pub fn contains_key(&self, k: &ModuleSpecifier) -> bool {
    self.0.lock().contains_key(k)
  }

  pub fn get(&self, k: &ModuleSpecifier) -> Option<AssetDocument> {
    self.0.lock().get(k).cloned()
  }
}

/// Assets are never updated and so we can safely use this struct across
/// multiple threads without needing to worry about race conditions.
#[derive(Debug, Clone)]
pub struct Assets {
  ts_server: Arc<TsServer>,
  assets: Arc<Mutex<AssetsMap>>,
}

impl Assets {
  pub fn new(ts_server: Arc<TsServer>) -> Self {
    Self {
      ts_server,
      assets: new_assets_map(),
    }
  }

  /// Initializes with the assets in the isolate.
  pub async fn initialize(&self, state_snapshot: Arc<StateSnapshot>) {
    let assets = get_isolate_assets(&self.ts_server, state_snapshot).await;
    let mut assets_map = self.assets.lock();
    for asset in assets {
      if !assets_map.contains_key(asset.specifier()) {
        assets_map.insert(asset.specifier().clone(), asset);
      }
    }
  }

  pub fn snapshot(&self) -> AssetsSnapshot {
    // it's ok to not make a complete copy for snapshotting purposes
    // because assets are static
    AssetsSnapshot(self.assets.clone())
  }

  pub fn get(&self, specifier: &ModuleSpecifier) -> Option<AssetDocument> {
    self.assets.lock().get(specifier).cloned()
  }

  pub fn cache_navigation_tree(
    &self,
    specifier: &ModuleSpecifier,
    navigation_tree: Arc<NavigationTree>,
  ) -> Result<(), AnyError> {
    let mut assets = self.assets.lock();
    let doc = assets
      .get_mut(specifier)
      .ok_or_else(|| anyhow!("Missing asset."))?;
    *doc = doc.with_navigation_tree(navigation_tree);
    Ok(())
  }
}

/// Get all the assets stored in the tsc isolate.
async fn get_isolate_assets(
  ts_server: &TsServer,
  state_snapshot: Arc<StateSnapshot>,
) -> Vec<AssetDocument> {
  let req = TscRequest::GetAssets;
  let res: Value = ts_server.request(state_snapshot, req, None).await.unwrap();
  let response_assets = match res {
    Value::Array(value) => value,
    _ => unreachable!(),
  };
  let mut assets = Vec::with_capacity(response_assets.len());

  for asset in response_assets {
    let mut obj = match asset {
      Value::Object(obj) => obj,
      _ => unreachable!(),
    };
    let specifier_str = obj.get("specifier").unwrap().as_str().unwrap();
    let specifier = ModuleSpecifier::parse(specifier_str).unwrap();
    let text = match obj.remove("text").unwrap() {
      Value::String(text) => text,
      _ => unreachable!(),
    };
    assets.push(AssetDocument::new(specifier, text));
  }

  assets
}

fn get_tag_body_text(
  tag: &JsDocTagInfo,
  language_server: &language_server::Inner,
) -> Option<String> {
  tag.text.as_ref().map(|display_parts| {
    // TODO(@kitsonk) check logic in vscode about handling this API change in
    // tsserver
    let text = display_parts_to_string(display_parts, language_server);
    match tag.name.as_str() {
      "example" => {
        if CAPTION_RE.is_match(&text) {
          CAPTION_RE
            .replace(&text, |c: &Captures| {
              format!("{}\n\n{}", &c[1], make_codeblock(&c[2]))
            })
            .to_string()
        } else {
          make_codeblock(&text)
        }
      }
      "author" => EMAIL_MATCH_RE
        .replace(&text, |c: &Captures| format!("{} {}", &c[1], &c[2]))
        .to_string(),
      "default" => make_codeblock(&text),
      _ => replace_links(&text),
    }
  })
}

fn get_tag_documentation(
  tag: &JsDocTagInfo,
  language_server: &language_server::Inner,
) -> String {
  match tag.name.as_str() {
    "augments" | "extends" | "param" | "template" => {
      if let Some(display_parts) = &tag.text {
        // TODO(@kitsonk) check logic in vscode about handling this API change
        // in tsserver
        let text = display_parts_to_string(display_parts, language_server);
        let body: Vec<&str> = PART_RE.split(&text).collect();
        if body.len() == 3 {
          let param = body[1];
          let doc = body[2];
          let label = format!("*@{}* `{}`", tag.name, param);
          if doc.is_empty() {
            return label;
          }
          if doc.contains('\n') {
            return format!("{}  \n{}", label, replace_links(doc));
          } else {
            return format!("{} - {}", label, replace_links(doc));
          }
        }
      }
    }
    _ => (),
  }
  let label = format!("*@{}*", tag.name);
  let maybe_text = get_tag_body_text(tag, language_server);
  if let Some(text) = maybe_text {
    if text.contains('\n') {
      format!("{label}  \n{text}")
    } else {
      format!("{label} - {text}")
    }
  } else {
    label
  }
}

fn make_codeblock(text: &str) -> String {
  if CODEBLOCK_RE.is_match(text) {
    text.to_string()
  } else {
    format!("```\n{text}\n```")
  }
}

/// Replace JSDoc like links (`{@link http://example.com}`) with markdown links
fn replace_links<S: AsRef<str>>(text: S) -> String {
  JSDOC_LINKS_RE
    .replace_all(text.as_ref(), |c: &Captures| match &c[1] {
      "linkcode" => format!(
        "[`{}`]({})",
        if c.get(3).is_none() {
          &c[2]
        } else {
          c[3].trim()
        },
        &c[2]
      ),
      _ => format!(
        "[{}]({})",
        if c.get(3).is_none() {
          &c[2]
        } else {
          c[3].trim()
        },
        &c[2]
      ),
    })
    .to_string()
}

fn parse_kind_modifier(kind_modifiers: &str) -> HashSet<&str> {
  PART_KIND_MODIFIER_RE.split(kind_modifiers).collect()
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
  One(T),
  Many(Vec<T>),
}

/// Aligns with ts.ScriptElementKind
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub enum ScriptElementKind {
  #[serde(rename = "")]
  Unknown,
  #[serde(rename = "warning")]
  Warning,
  #[serde(rename = "keyword")]
  Keyword,
  #[serde(rename = "script")]
  ScriptElement,
  #[serde(rename = "module")]
  ModuleElement,
  #[serde(rename = "class")]
  ClassElement,
  #[serde(rename = "local class")]
  LocalClassElement,
  #[serde(rename = "interface")]
  InterfaceElement,
  #[serde(rename = "type")]
  TypeElement,
  #[serde(rename = "enum")]
  EnumElement,
  #[serde(rename = "enum member")]
  EnumMemberElement,
  #[serde(rename = "var")]
  VariableElement,
  #[serde(rename = "local var")]
  LocalVariableElement,
  #[serde(rename = "using")]
  VariableUsingElement,
  #[serde(rename = "await using")]
  VariableAwaitUsingElement,
  #[serde(rename = "function")]
  FunctionElement,
  #[serde(rename = "local function")]
  LocalFunctionElement,
  #[serde(rename = "method")]
  MemberFunctionElement,
  #[serde(rename = "getter")]
  MemberGetAccessorElement,
  #[serde(rename = "setter")]
  MemberSetAccessorElement,
  #[serde(rename = "property")]
  MemberVariableElement,
  #[serde(rename = "accessor")]
  MemberAccessorVariableElement,
  #[serde(rename = "constructor")]
  ConstructorImplementationElement,
  #[serde(rename = "call")]
  CallSignatureElement,
  #[serde(rename = "index")]
  IndexSignatureElement,
  #[serde(rename = "construct")]
  ConstructSignatureElement,
  #[serde(rename = "parameter")]
  ParameterElement,
  #[serde(rename = "type parameter")]
  TypeParameterElement,
  #[serde(rename = "primitive type")]
  PrimitiveType,
  #[serde(rename = "label")]
  Label,
  #[serde(rename = "alias")]
  Alias,
  #[serde(rename = "const")]
  ConstElement,
  #[serde(rename = "let")]
  LetElement,
  #[serde(rename = "directory")]
  Directory,
  #[serde(rename = "external module name")]
  ExternalModuleName,
  #[serde(rename = "JSX attribute")]
  JsxAttribute,
  #[serde(rename = "string")]
  String,
  #[serde(rename = "link")]
  Link,
  #[serde(rename = "link name")]
  LinkName,
  #[serde(rename = "link text")]
  LinkText,
}

impl Default for ScriptElementKind {
  fn default() -> Self {
    Self::Unknown
  }
}

/// This mirrors the method `convertKind` in `completions.ts` in vscode (extensions/typescript-language-features)
/// https://github.com/microsoft/vscode/blob/bd2df940d74b51105aefb11304e028d2fb56a9dc/extensions/typescript-language-features/src/languageFeatures/completions.ts#L440
impl From<ScriptElementKind> for lsp::CompletionItemKind {
  fn from(kind: ScriptElementKind) -> Self {
    match kind {
      ScriptElementKind::PrimitiveType | ScriptElementKind::Keyword => {
        lsp::CompletionItemKind::KEYWORD
      }
      ScriptElementKind::ConstElement
      | ScriptElementKind::LetElement
      | ScriptElementKind::VariableElement
      | ScriptElementKind::LocalVariableElement
      | ScriptElementKind::Alias
      | ScriptElementKind::ParameterElement => {
        lsp::CompletionItemKind::VARIABLE
      }
      ScriptElementKind::MemberVariableElement
      | ScriptElementKind::MemberGetAccessorElement
      | ScriptElementKind::MemberSetAccessorElement => {
        lsp::CompletionItemKind::FIELD
      }
      ScriptElementKind::FunctionElement
      | ScriptElementKind::LocalFunctionElement => {
        lsp::CompletionItemKind::FUNCTION
      }
      ScriptElementKind::MemberFunctionElement
      | ScriptElementKind::ConstructSignatureElement
      | ScriptElementKind::CallSignatureElement
      | ScriptElementKind::IndexSignatureElement => {
        lsp::CompletionItemKind::METHOD
      }
      ScriptElementKind::EnumElement => lsp::CompletionItemKind::ENUM,
      ScriptElementKind::EnumMemberElement => {
        lsp::CompletionItemKind::ENUM_MEMBER
      }
      ScriptElementKind::ModuleElement
      | ScriptElementKind::ExternalModuleName => {
        lsp::CompletionItemKind::MODULE
      }
      ScriptElementKind::ClassElement | ScriptElementKind::TypeElement => {
        lsp::CompletionItemKind::CLASS
      }
      ScriptElementKind::InterfaceElement => lsp::CompletionItemKind::INTERFACE,
      ScriptElementKind::Warning => lsp::CompletionItemKind::TEXT,
      ScriptElementKind::ScriptElement => lsp::CompletionItemKind::FILE,
      ScriptElementKind::Directory => lsp::CompletionItemKind::FOLDER,
      ScriptElementKind::String => lsp::CompletionItemKind::CONSTANT,
      ScriptElementKind::LocalClassElement
      | ScriptElementKind::ConstructorImplementationElement
      | ScriptElementKind::TypeParameterElement
      | ScriptElementKind::Label
      | ScriptElementKind::JsxAttribute
      | ScriptElementKind::Link
      | ScriptElementKind::LinkName
      | ScriptElementKind::LinkText
      | ScriptElementKind::VariableUsingElement
      | ScriptElementKind::VariableAwaitUsingElement
      | ScriptElementKind::MemberAccessorVariableElement
      | ScriptElementKind::Unknown => lsp::CompletionItemKind::PROPERTY,
    }
  }
}

/// This mirrors `fromProtocolScriptElementKind` in vscode
impl From<ScriptElementKind> for lsp::SymbolKind {
  fn from(kind: ScriptElementKind) -> Self {
    match kind {
      ScriptElementKind::ModuleElement => Self::MODULE,
      // this is only present in `getSymbolKind` in `workspaceSymbols` in
      // vscode, but seems strange it isn't consistent.
      ScriptElementKind::TypeElement => Self::CLASS,
      ScriptElementKind::ClassElement => Self::CLASS,
      ScriptElementKind::EnumElement => Self::ENUM,
      ScriptElementKind::EnumMemberElement => Self::ENUM_MEMBER,
      ScriptElementKind::InterfaceElement => Self::INTERFACE,
      ScriptElementKind::IndexSignatureElement => Self::METHOD,
      ScriptElementKind::CallSignatureElement => Self::METHOD,
      ScriptElementKind::MemberFunctionElement => Self::METHOD,
      // workspaceSymbols in vscode treats them as fields, which does seem more
      // semantically correct while `fromProtocolScriptElementKind` treats them
      // as properties.
      ScriptElementKind::MemberVariableElement => Self::FIELD,
      ScriptElementKind::MemberGetAccessorElement => Self::FIELD,
      ScriptElementKind::MemberSetAccessorElement => Self::FIELD,
      ScriptElementKind::VariableElement => Self::VARIABLE,
      ScriptElementKind::LetElement => Self::VARIABLE,
      ScriptElementKind::ConstElement => Self::VARIABLE,
      ScriptElementKind::LocalVariableElement => Self::VARIABLE,
      ScriptElementKind::Alias => Self::VARIABLE,
      ScriptElementKind::FunctionElement => Self::FUNCTION,
      ScriptElementKind::LocalFunctionElement => Self::FUNCTION,
      ScriptElementKind::ConstructSignatureElement => Self::CONSTRUCTOR,
      ScriptElementKind::ConstructorImplementationElement => Self::CONSTRUCTOR,
      ScriptElementKind::TypeParameterElement => Self::TYPE_PARAMETER,
      ScriptElementKind::String => Self::STRING,
      _ => Self::VARIABLE,
    }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TextSpan {
  pub start: u32,
  pub length: u32,
}

impl TextSpan {
  pub fn from_range(
    range: &lsp::Range,
    line_index: Arc<LineIndex>,
  ) -> Result<Self, AnyError> {
    let start = line_index.offset_tsc(range.start)?;
    let length = line_index.offset_tsc(range.end)? - start;
    Ok(Self { start, length })
  }

  pub fn to_range(&self, line_index: Arc<LineIndex>) -> lsp::Range {
    lsp::Range {
      start: line_index.position_tsc(self.start.into()),
      end: line_index.position_tsc(TextSize::from(self.start + self.length)),
    }
  }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SymbolDisplayPart {
  text: String,
  kind: String,
  // This is only on `JSDocLinkDisplayPart` which extends `SymbolDisplayPart`
  // but is only used as an upcast of a `SymbolDisplayPart` and not explicitly
  // returned by any API, so it is safe to add it as an optional value.
  #[serde(skip_serializing_if = "Option::is_none")]
  target: Option<DocumentSpan>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsDocTagInfo {
  name: String,
  text: Option<Vec<SymbolDisplayPart>>,
}

// Note: the tsc protocol contains fields that are part of the protocol but
// not currently used.  They are commented out in the structures so it is clear
// that they exist.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickInfo {
  // kind: ScriptElementKind,
  // kind_modifiers: String,
  text_span: TextSpan,
  display_parts: Option<Vec<SymbolDisplayPart>>,
  documentation: Option<Vec<SymbolDisplayPart>>,
  tags: Option<Vec<JsDocTagInfo>>,
}

#[derive(Default)]
struct Link {
  name: Option<String>,
  target: Option<DocumentSpan>,
  text: Option<String>,
  linkcode: bool,
}

/// Takes `SymbolDisplayPart` items and converts them into a string, handling
/// any `{@link Symbol}` and `{@linkcode Symbol}` JSDoc tags and linking them
/// to the their source location.
fn display_parts_to_string(
  parts: &[SymbolDisplayPart],
  language_server: &language_server::Inner,
) -> String {
  let mut out = Vec::<String>::new();

  let mut current_link: Option<Link> = None;
  for part in parts {
    match part.kind.as_str() {
      "link" => {
        if let Some(link) = current_link.as_mut() {
          if let Some(target) = &link.target {
            if let Some(specifier) = target.to_target(language_server) {
              let link_text = link.text.clone().unwrap_or_else(|| {
                link
                  .name
                  .clone()
                  .map(|ref n| n.replace('`', "\\`"))
                  .unwrap_or_else(|| "".to_string())
              });
              let link_str = if link.linkcode {
                format!("[`{link_text}`]({specifier})")
              } else {
                format!("[{link_text}]({specifier})")
              };
              out.push(link_str);
            }
          } else {
            let maybe_text = link.text.clone().or_else(|| link.name.clone());
            if let Some(text) = maybe_text {
              if HTTP_RE.is_match(&text) {
                let parts: Vec<&str> = text.split(' ').collect();
                if parts.len() == 1 {
                  out.push(parts[0].to_string());
                } else {
                  let link_text = parts[1..].join(" ").replace('`', "\\`");
                  let link_str = if link.linkcode {
                    format!("[`{}`]({})", link_text, parts[0])
                  } else {
                    format!("[{}]({})", link_text, parts[0])
                  };
                  out.push(link_str);
                }
              } else {
                out.push(text.replace('`', "\\`"));
              }
            }
          }
          current_link = None;
        } else {
          current_link = Some(Link {
            linkcode: part.text.as_str() == "{@linkcode ",
            ..Default::default()
          });
        }
      }
      "linkName" => {
        if let Some(link) = current_link.as_mut() {
          link.name = Some(part.text.clone());
          link.target.clone_from(&part.target);
        }
      }
      "linkText" => {
        if let Some(link) = current_link.as_mut() {
          link.name = Some(part.text.clone());
        }
      }
      _ => out.push(
        // should decode percent-encoding string when hovering over the right edge of module specifier like below
        // module "file:///path/to/🦕"
        to_percent_decoded_str(&part.text),
        // NOTE: The reason why an example above that lacks `.ts` extension is caused by the implementation of tsc itself.
        // The request `tsc.request.getQuickInfoAtPosition` receives the payload from tsc host as follows.
        // {
        //   text_span: {
        //     start: 19,
        //     length: 9,
        //   },
        //   displayParts:
        //     [
        //       {
        //         text: "module",
        //         kind: "keyword",
        //         target: null,
        //       },
        //       {
        //         text: " ",
        //         kind: "space",
        //         target: null,
        //       },
        //       {
        //         text: "\"file:///path/to/%F0%9F%A6%95\"",
        //         kind: "stringLiteral",
        //         target: null,
        //       },
        //     ],
        //   documentation: [],
        //   tags: null,
        // }
        //
        // related issue: https://github.com/denoland/deno/issues/16058
      ),
    }
  }

  replace_links(out.join(""))
}

impl QuickInfo {
  pub fn to_hover(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> lsp::Hover {
    let mut parts = Vec::<lsp::MarkedString>::new();
    if let Some(display_string) = self
      .display_parts
      .clone()
      .map(|p| display_parts_to_string(&p, language_server))
    {
      parts.push(lsp::MarkedString::from_language_code(
        "typescript".to_string(),
        display_string,
      ));
    }
    if let Some(documentation) = self
      .documentation
      .clone()
      .map(|p| display_parts_to_string(&p, language_server))
    {
      parts.push(lsp::MarkedString::from_markdown(documentation));
    }
    if let Some(tags) = &self.tags {
      let tags_preview = tags
        .iter()
        .map(|tag_info| get_tag_documentation(tag_info, language_server))
        .collect::<Vec<String>>()
        .join("  \n\n");
      if !tags_preview.is_empty() {
        parts.push(lsp::MarkedString::from_markdown(format!(
          "\n\n{tags_preview}"
        )));
      }
    }
    lsp::Hover {
      contents: lsp::HoverContents::Array(parts),
      range: Some(self.text_span.to_range(line_index)),
    }
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSpan {
  text_span: TextSpan,
  pub file_name: String,
  original_text_span: Option<TextSpan>,
  // original_file_name: Option<String>,
  context_span: Option<TextSpan>,
  original_context_span: Option<TextSpan>,
}

impl DocumentSpan {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.file_name = specifier_map.normalize(&self.file_name)?.to_string();
    Ok(())
  }
}

impl DocumentSpan {
  pub fn to_link(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> Option<lsp::LocationLink> {
    let target_specifier = resolve_url(&self.file_name).ok()?;
    let target_asset_or_doc =
      language_server.get_maybe_asset_or_document(&target_specifier)?;
    let target_line_index = target_asset_or_doc.line_index();
    let file_referrer = target_asset_or_doc.file_referrer();
    let target_uri = language_server
      .url_map
      .specifier_to_uri(&target_specifier, file_referrer)
      .ok()?;
    let (target_range, target_selection_range) =
      if let Some(context_span) = &self.context_span {
        (
          context_span.to_range(target_line_index.clone()),
          self.text_span.to_range(target_line_index),
        )
      } else {
        (
          self.text_span.to_range(target_line_index.clone()),
          self.text_span.to_range(target_line_index),
        )
      };
    let origin_selection_range =
      if let Some(original_context_span) = &self.original_context_span {
        Some(original_context_span.to_range(line_index))
      } else {
        self
          .original_text_span
          .as_ref()
          .map(|original_text_span| original_text_span.to_range(line_index))
      };
    let link = lsp::LocationLink {
      origin_selection_range,
      target_uri,
      target_range,
      target_selection_range,
    };
    Some(link)
  }

  /// Convert the `DocumentSpan` into a specifier that can be sent to the client
  /// to link to the target document span. Used for converting JSDoc symbol
  /// links to markdown links.
  fn to_target(
    &self,
    language_server: &language_server::Inner,
  ) -> Option<ModuleSpecifier> {
    let specifier = resolve_url(&self.file_name).ok()?;
    let asset_or_doc =
      language_server.get_maybe_asset_or_document(&specifier)?;
    let line_index = asset_or_doc.line_index();
    let range = self.text_span.to_range(line_index);
    let file_referrer = asset_or_doc.file_referrer();
    let target_uri = language_server
      .url_map
      .specifier_to_uri(&specifier, file_referrer)
      .ok()?;
    let mut target = uri_to_url(&target_uri);
    target.set_fragment(Some(&format!(
      "L{},{}",
      range.start.line + 1,
      range.start.character + 1
    )));

    Some(target)
  }
}

#[derive(Debug, Clone, Deserialize)]
pub enum MatchKind {
  #[serde(rename = "exact")]
  Exact,
  #[serde(rename = "prefix")]
  Prefix,
  #[serde(rename = "substring")]
  Substring,
  #[serde(rename = "camelCase")]
  CamelCase,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateToItem {
  name: String,
  kind: ScriptElementKind,
  kind_modifiers: String,
  // match_kind: MatchKind,
  // is_case_sensitive: bool,
  file_name: String,
  text_span: TextSpan,
  container_name: Option<String>,
  // container_kind: ScriptElementKind,
}

impl NavigateToItem {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.file_name = specifier_map.normalize(&self.file_name)?.to_string();
    Ok(())
  }
}

impl NavigateToItem {
  pub fn to_symbol_information(
    &self,
    language_server: &language_server::Inner,
  ) -> Option<lsp::SymbolInformation> {
    let specifier = resolve_url(&self.file_name).ok()?;
    let asset_or_doc =
      language_server.get_asset_or_document(&specifier).ok()?;
    let line_index = asset_or_doc.line_index();
    let file_referrer = asset_or_doc.file_referrer();
    let uri = language_server
      .url_map
      .specifier_to_uri(&specifier, file_referrer)
      .ok()?;
    let range = self.text_span.to_range(line_index);
    let location = lsp::Location { uri, range };

    let mut tags: Option<Vec<lsp::SymbolTag>> = None;
    let kind_modifiers = parse_kind_modifier(&self.kind_modifiers);
    if kind_modifiers.contains("deprecated") {
      tags = Some(vec![lsp::SymbolTag::DEPRECATED]);
    }

    // The field `deprecated` is deprecated but SymbolInformation does not have
    // a default, therefore we have to supply the deprecated deprecated
    // field. It is like a bad version of Inception.
    #[allow(deprecated)]
    Some(lsp::SymbolInformation {
      name: self.name.clone(),
      kind: self.kind.clone().into(),
      tags,
      deprecated: None,
      location,
      container_name: self.container_name.clone(),
    })
  }
}

#[derive(Debug, Clone, Deserialize)]
pub enum InlayHintKind {
  Type,
  Parameter,
  Enum,
}

impl InlayHintKind {
  pub fn to_lsp(&self) -> Option<lsp::InlayHintKind> {
    match self {
      Self::Enum => None,
      Self::Parameter => Some(lsp::InlayHintKind::PARAMETER),
      Self::Type => Some(lsp::InlayHintKind::TYPE),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlayHint {
  pub text: String,
  pub position: u32,
  pub kind: InlayHintKind,
  pub whitespace_before: Option<bool>,
  pub whitespace_after: Option<bool>,
}

impl InlayHint {
  pub fn to_lsp(&self, line_index: Arc<LineIndex>) -> lsp::InlayHint {
    lsp::InlayHint {
      position: line_index.position_tsc(self.position.into()),
      label: lsp::InlayHintLabel::String(self.text.clone()),
      kind: self.kind.to_lsp(),
      padding_left: self.whitespace_before,
      padding_right: self.whitespace_after,
      text_edits: None,
      tooltip: None,
      data: None,
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationTree {
  pub text: String,
  pub kind: ScriptElementKind,
  pub kind_modifiers: String,
  pub spans: Vec<TextSpan>,
  pub name_span: Option<TextSpan>,
  pub child_items: Option<Vec<NavigationTree>>,
}

impl NavigationTree {
  pub fn to_code_lens(
    &self,
    line_index: Arc<LineIndex>,
    specifier: &ModuleSpecifier,
    source: &code_lens::CodeLensSource,
  ) -> lsp::CodeLens {
    let range = if let Some(name_span) = &self.name_span {
      name_span.to_range(line_index)
    } else if !self.spans.is_empty() {
      let span = &self.spans[0];
      span.to_range(line_index)
    } else {
      lsp::Range::default()
    };
    lsp::CodeLens {
      range,
      command: None,
      data: Some(json!({
        "specifier": specifier,
        "source": source
      })),
    }
  }

  pub fn collect_document_symbols(
    &self,
    line_index: Arc<LineIndex>,
    document_symbols: &mut Vec<lsp::DocumentSymbol>,
  ) -> bool {
    let mut should_include = self.should_include_entry();
    if !should_include
      && self
        .child_items
        .as_ref()
        .map(|v| v.is_empty())
        .unwrap_or(true)
    {
      return false;
    }

    let children = self
      .child_items
      .as_deref()
      .unwrap_or(&[] as &[NavigationTree]);
    for span in self.spans.iter() {
      let range = TextRange::at(span.start.into(), span.length.into());
      let mut symbol_children = Vec::<lsp::DocumentSymbol>::new();
      for child in children.iter() {
        let should_traverse_child = child
          .spans
          .iter()
          .map(|child_span| {
            TextRange::at(child_span.start.into(), child_span.length.into())
          })
          .any(|child_range| range.intersect(child_range).is_some());
        if should_traverse_child {
          let included_child = child
            .collect_document_symbols(line_index.clone(), &mut symbol_children);
          should_include = should_include || included_child;
        }
      }

      if should_include {
        let mut selection_span = span;
        if let Some(name_span) = self.name_span.as_ref() {
          let name_range =
            TextRange::at(name_span.start.into(), name_span.length.into());
          if range.contains_range(name_range) {
            selection_span = name_span;
          }
        }

        let name = match self.kind {
          ScriptElementKind::MemberGetAccessorElement => {
            format!("(get) {}", self.text)
          }
          ScriptElementKind::MemberSetAccessorElement => {
            format!("(set) {}", self.text)
          }
          _ => self.text.clone(),
        };

        let mut tags: Option<Vec<lsp::SymbolTag>> = None;
        let kind_modifiers = parse_kind_modifier(&self.kind_modifiers);
        if kind_modifiers.contains("deprecated") {
          tags = Some(vec![lsp::SymbolTag::DEPRECATED]);
        }

        let children = if !symbol_children.is_empty() {
          Some(symbol_children)
        } else {
          None
        };

        // The field `deprecated` is deprecated but DocumentSymbol does not have
        // a default, therefore we have to supply the deprecated deprecated
        // field. It is like a bad version of Inception.
        #[allow(deprecated)]
        document_symbols.push(lsp::DocumentSymbol {
          name,
          kind: self.kind.clone().into(),
          range: span.to_range(line_index.clone()),
          selection_range: selection_span.to_range(line_index.clone()),
          tags,
          children,
          detail: None,
          deprecated: None,
        })
      }
    }

    should_include
  }

  fn should_include_entry(&self) -> bool {
    if let ScriptElementKind::Alias = self.kind {
      return false;
    }

    !self.text.is_empty() && self.text != "<function>" && self.text != "<class>"
  }

  pub fn walk<F>(&self, callback: &F)
  where
    F: Fn(&NavigationTree, Option<&NavigationTree>),
  {
    callback(self, None);
    if let Some(child_items) = &self.child_items {
      for child in child_items {
        child.walk_child(callback, self);
      }
    }
  }

  fn walk_child<F>(&self, callback: &F, parent: &NavigationTree)
  where
    F: Fn(&NavigationTree, Option<&NavigationTree>),
  {
    callback(self, Some(parent));
    if let Some(child_items) = &self.child_items {
      for child in child_items {
        child.walk_child(callback, self);
      }
    }
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationLocation {
  #[serde(flatten)]
  pub document_span: DocumentSpan,
  // ImplementationLocation props
  // kind: ScriptElementKind,
  // display_parts: Vec<SymbolDisplayPart>,
}

impl ImplementationLocation {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.document_span.normalize(specifier_map)?;
    Ok(())
  }

  pub fn to_location(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> lsp::Location {
    let specifier = resolve_url(&self.document_span.file_name)
      .unwrap_or_else(|_| ModuleSpecifier::parse("deno://invalid").unwrap());
    let file_referrer = language_server.documents.get_file_referrer(&specifier);
    let uri = language_server
      .url_map
      .specifier_to_uri(&specifier, file_referrer.as_deref())
      .unwrap_or_else(|_| INVALID_URI.clone());
    lsp::Location {
      uri,
      range: self.document_span.text_span.to_range(line_index),
    }
  }

  pub fn to_link(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> Option<lsp::LocationLink> {
    self.document_span.to_link(line_index, language_server)
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameLocation {
  #[serde(flatten)]
  document_span: DocumentSpan,
  // RenameLocation props
  // prefix_text: Option<String>,
  // suffix_text: Option<String>,
}

impl RenameLocation {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.document_span.normalize(specifier_map)?;
    Ok(())
  }
}

pub struct RenameLocations {
  pub locations: Vec<RenameLocation>,
}

impl RenameLocations {
  pub fn into_workspace_edit(
    self,
    new_name: &str,
    language_server: &language_server::Inner,
  ) -> Result<lsp::WorkspaceEdit, AnyError> {
    let mut text_document_edit_map = IndexMap::new();
    let mut includes_non_files = false;
    for location in self.locations.iter() {
      let specifier = resolve_url(&location.document_span.file_name)?;
      if specifier.scheme() != "file" {
        includes_non_files = true;
        continue;
      }
      let file_referrer =
        language_server.documents.get_file_referrer(&specifier);
      let uri = language_server
        .url_map
        .specifier_to_uri(&specifier, file_referrer.as_deref())?;
      let asset_or_doc = language_server.get_asset_or_document(&specifier)?;

      // ensure TextDocumentEdit for `location.file_name`.
      if !text_document_edit_map.contains_key(&uri) {
        text_document_edit_map.insert(
          uri.clone(),
          lsp::TextDocumentEdit {
            text_document: lsp::OptionalVersionedTextDocumentIdentifier {
              uri: uri.clone(),
              version: asset_or_doc.document_lsp_version(),
            },
            edits:
              Vec::<lsp::OneOf<lsp::TextEdit, lsp::AnnotatedTextEdit>>::new(),
          },
        );
      }

      // push TextEdit for ensured `TextDocumentEdit.edits`.
      let document_edit = text_document_edit_map.get_mut(&uri).unwrap();
      document_edit.edits.push(lsp::OneOf::Left(lsp::TextEdit {
        range: location
          .document_span
          .text_span
          .to_range(asset_or_doc.line_index()),
        new_text: new_name.to_string(),
      }));
    }

    if includes_non_files {
      language_server.client.show_message(lsp::MessageType::WARNING, "The renamed symbol had references in non-file schemed modules. These have not been modified.");
    }

    Ok(lsp::WorkspaceEdit {
      change_annotations: None,
      changes: None,
      document_changes: Some(lsp::DocumentChanges::Edits(
        text_document_edit_map.values().cloned().collect(),
      )),
    })
  }
}

#[derive(Debug, Deserialize)]
pub enum HighlightSpanKind {
  #[serde(rename = "none")]
  None,
  #[serde(rename = "definition")]
  Definition,
  #[serde(rename = "reference")]
  Reference,
  #[serde(rename = "writtenReference")]
  WrittenReference,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightSpan {
  // file_name: Option<String>,
  // is_in_string: Option<bool>,
  text_span: TextSpan,
  // context_span: Option<TextSpan>,
  kind: HighlightSpanKind,
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionInfo {
  // kind: ScriptElementKind,
  // name: String,
  // container_kind: Option<ScriptElementKind>,
  // container_name: Option<String>,
  #[serde(flatten)]
  pub document_span: DocumentSpan,
}

impl DefinitionInfo {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.document_span.normalize(specifier_map)?;
    Ok(())
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionInfoAndBoundSpan {
  pub definitions: Option<Vec<DefinitionInfo>>,
  // text_span: TextSpan,
}

impl DefinitionInfoAndBoundSpan {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    for definition in self.definitions.iter_mut().flatten() {
      definition.normalize(specifier_map)?;
    }
    Ok(())
  }

  pub fn to_definition(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> Option<lsp::GotoDefinitionResponse> {
    if let Some(definitions) = &self.definitions {
      let mut location_links = Vec::<lsp::LocationLink>::new();
      for di in definitions {
        if let Some(link) = di
          .document_span
          .to_link(line_index.clone(), language_server)
        {
          location_links.push(link);
        }
      }
      Some(lsp::GotoDefinitionResponse::Link(location_links))
    } else {
      None
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentHighlights {
  // file_name: String,
  highlight_spans: Vec<HighlightSpan>,
}

impl DocumentHighlights {
  pub fn to_highlight(
    &self,
    line_index: Arc<LineIndex>,
  ) -> Vec<lsp::DocumentHighlight> {
    self
      .highlight_spans
      .iter()
      .map(|hs| lsp::DocumentHighlight {
        range: hs.text_span.to_range(line_index.clone()),
        kind: match hs.kind {
          HighlightSpanKind::WrittenReference => {
            Some(lsp::DocumentHighlightKind::WRITE)
          }
          _ => Some(lsp::DocumentHighlightKind::READ),
        },
      })
      .collect()
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TextChange {
  pub span: TextSpan,
  pub new_text: String,
}

impl TextChange {
  pub fn as_text_edit(&self, line_index: Arc<LineIndex>) -> lsp::TextEdit {
    lsp::TextEdit {
      range: self.span.to_range(line_index),
      new_text: self.new_text.clone(),
    }
  }

  pub fn as_text_or_annotated_text_edit(
    &self,
    line_index: Arc<LineIndex>,
  ) -> lsp::OneOf<lsp::TextEdit, lsp::AnnotatedTextEdit> {
    lsp::OneOf::Left(lsp::TextEdit {
      range: self.span.to_range(line_index),
      new_text: self.new_text.clone(),
    })
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct FileTextChanges {
  pub file_name: String,
  pub text_changes: Vec<TextChange>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub is_new_file: Option<bool>,
}

impl FileTextChanges {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.file_name = specifier_map.normalize(&self.file_name)?.to_string();
    Ok(())
  }

  pub fn to_text_document_edit(
    &self,
    language_server: &language_server::Inner,
  ) -> Result<lsp::TextDocumentEdit, AnyError> {
    let specifier = resolve_url(&self.file_name)?;
    let asset_or_doc = language_server.get_asset_or_document(&specifier)?;
    let edits = self
      .text_changes
      .iter()
      .map(|tc| tc.as_text_or_annotated_text_edit(asset_or_doc.line_index()))
      .collect();
    Ok(lsp::TextDocumentEdit {
      text_document: lsp::OptionalVersionedTextDocumentIdentifier {
        uri: url_to_uri(&specifier)?,
        version: asset_or_doc.document_lsp_version(),
      },
      edits,
    })
  }

  pub fn to_text_document_change_ops(
    &self,
    language_server: &language_server::Inner,
  ) -> Result<Vec<lsp::DocumentChangeOperation>, AnyError> {
    let mut ops = Vec::<lsp::DocumentChangeOperation>::new();
    let specifier = resolve_url(&self.file_name)?;
    let maybe_asset_or_document = if !self.is_new_file.unwrap_or(false) {
      let asset_or_doc = language_server.get_asset_or_document(&specifier)?;
      Some(asset_or_doc)
    } else {
      None
    };
    let line_index = maybe_asset_or_document
      .as_ref()
      .map(|d| d.line_index())
      .unwrap_or_else(|| Arc::new(LineIndex::new("")));

    if self.is_new_file.unwrap_or(false) {
      ops.push(lsp::DocumentChangeOperation::Op(lsp::ResourceOp::Create(
        lsp::CreateFile {
          uri: url_to_uri(&specifier)?,
          options: Some(lsp::CreateFileOptions {
            ignore_if_exists: Some(true),
            overwrite: None,
          }),
          annotation_id: None,
        },
      )));
    }

    let edits = self
      .text_changes
      .iter()
      .map(|tc| tc.as_text_or_annotated_text_edit(line_index.clone()))
      .collect();
    ops.push(lsp::DocumentChangeOperation::Edit(lsp::TextDocumentEdit {
      text_document: lsp::OptionalVersionedTextDocumentIdentifier {
        uri: url_to_uri(&specifier)?,
        version: maybe_asset_or_document.and_then(|d| d.document_lsp_version()),
      },
      edits,
    }));

    Ok(ops)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Classifications {
  spans: Vec<u32>,
}

impl Classifications {
  pub fn to_semantic_tokens(
    &self,
    line_index: Arc<LineIndex>,
  ) -> LspResult<lsp::SemanticTokens> {
    // https://github.com/microsoft/vscode/blob/1.89.0/extensions/typescript-language-features/src/languageFeatures/semanticTokens.ts#L89-L115
    let token_count = self.spans.len() / 3;
    let mut builder = SemanticTokensBuilder::new();
    for i in 0..token_count {
      let src_offset = 3 * i;
      let offset = self.spans[src_offset];
      let length = self.spans[src_offset + 1];
      let ts_classification = self.spans[src_offset + 2];

      let token_type =
        Classifications::get_token_type_from_classification(ts_classification);
      let token_modifiers =
        Classifications::get_token_modifier_from_classification(
          ts_classification,
        );

      let start_pos = line_index.position_tsc(offset.into());
      let end_pos = line_index.position_tsc(TextSize::from(offset + length));

      for line in start_pos.line..(end_pos.line + 1) {
        let start_character = if line == start_pos.line {
          start_pos.character
        } else {
          0
        };
        let end_character = if line == end_pos.line {
          end_pos.character
        } else {
          line_index.line_length_utf16(line).into()
        };
        builder.push(
          line,
          start_character,
          end_character - start_character,
          token_type,
          token_modifiers,
        );
      }
    }
    Ok(builder.build(None))
  }

  fn get_token_type_from_classification(ts_classification: u32) -> u32 {
    assert!(ts_classification > semantic_tokens::MODIFIER_MASK);
    (ts_classification >> semantic_tokens::TYPE_OFFSET) - 1
  }

  fn get_token_modifier_from_classification(ts_classification: u32) -> u32 {
    ts_classification & semantic_tokens::MODIFIER_MASK
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefactorActionInfo {
  name: String,
  description: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  not_applicable_reason: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  kind: Option<String>,
}

impl RefactorActionInfo {
  pub fn get_action_kind(&self) -> lsp::CodeActionKind {
    if let Some(kind) = &self.kind {
      kind.clone().into()
    } else {
      let maybe_match = ALL_KNOWN_REFACTOR_ACTION_KINDS
        .iter()
        .find(|action| action.matches(&self.name));
      maybe_match
        .map(|action| action.kind.clone())
        .unwrap_or(lsp::CodeActionKind::REFACTOR)
    }
  }

  pub fn is_preferred(&self, all_actions: &[RefactorActionInfo]) -> bool {
    if EXTRACT_CONSTANT.matches(&self.name) {
      let get_scope = |name: &str| -> Option<u32> {
        if let Some(captures) = SCOPE_RE.captures(name) {
          captures[1].parse::<u32>().ok()
        } else {
          None
        }
      };

      return if let Some(scope) = get_scope(&self.name) {
        all_actions
          .iter()
          .filter(|other| {
            !std::ptr::eq(&self, other) && EXTRACT_CONSTANT.matches(&other.name)
          })
          .all(|other| {
            if let Some(other_scope) = get_scope(&other.name) {
              scope < other_scope
            } else {
              true
            }
          })
      } else {
        false
      };
    }
    if EXTRACT_TYPE.matches(&self.name) || EXTRACT_INTERFACE.matches(&self.name)
    {
      return true;
    }
    false
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicableRefactorInfo {
  name: String,
  // description: String,
  // #[serde(skip_serializing_if = "Option::is_none")]
  // inlineable: Option<bool>,
  actions: Vec<RefactorActionInfo>,
}

impl ApplicableRefactorInfo {
  pub fn to_code_actions(
    &self,
    specifier: &ModuleSpecifier,
    range: &lsp::Range,
  ) -> Vec<lsp::CodeAction> {
    let mut code_actions = Vec::<lsp::CodeAction>::new();
    // All typescript refactoring actions are inlineable
    for action in self.actions.iter() {
      code_actions
        .push(self.as_inline_code_action(action, specifier, range, &self.name));
    }
    code_actions
  }

  fn as_inline_code_action(
    &self,
    action: &RefactorActionInfo,
    specifier: &ModuleSpecifier,
    range: &lsp::Range,
    refactor_name: &str,
  ) -> lsp::CodeAction {
    let disabled = action.not_applicable_reason.as_ref().map(|reason| {
      lsp::CodeActionDisabled {
        reason: reason.clone(),
      }
    });

    lsp::CodeAction {
      title: action.description.to_string(),
      kind: Some(action.get_action_kind()),
      is_preferred: Some(action.is_preferred(&self.actions)),
      disabled,
      data: Some(
        serde_json::to_value(RefactorCodeActionData {
          specifier: specifier.clone(),
          range: *range,
          refactor_name: refactor_name.to_owned(),
          action_name: action.name.clone(),
        })
        .unwrap(),
      ),
      ..Default::default()
    }
  }
}

pub fn file_text_changes_to_workspace_edit(
  changes: &[FileTextChanges],
  language_server: &language_server::Inner,
) -> LspResult<Option<lsp::WorkspaceEdit>> {
  let mut all_ops = Vec::<lsp::DocumentChangeOperation>::new();
  for change in changes {
    let ops = match change.to_text_document_change_ops(language_server) {
      Ok(op) => op,
      Err(err) => {
        error!("Unable to convert changes to edits: {}", err);
        return Err(LspError::internal_error());
      }
    };
    all_ops.extend(ops);
  }

  Ok(Some(lsp::WorkspaceEdit {
    document_changes: Some(lsp::DocumentChanges::Operations(all_ops)),
    ..Default::default()
  }))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefactorEditInfo {
  pub edits: Vec<FileTextChanges>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rename_location: Option<u32>,
}

impl RefactorEditInfo {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    for changes in &mut self.edits {
      changes.normalize(specifier_map)?;
    }
    Ok(())
  }

  pub fn to_workspace_edit(
    &self,
    language_server: &language_server::Inner,
  ) -> LspResult<Option<lsp::WorkspaceEdit>> {
    file_text_changes_to_workspace_edit(&self.edits, language_server)
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeAction {
  description: String,
  changes: Vec<FileTextChanges>,
  #[serde(skip_serializing_if = "Option::is_none")]
  commands: Option<Vec<Value>>,
}

impl CodeAction {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    for changes in &mut self.changes {
      changes.normalize(specifier_map)?;
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeFixAction {
  pub description: String,
  pub changes: Vec<FileTextChanges>,
  // These are opaque types that should just be passed back when applying the
  // action.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub commands: Option<Vec<Value>>,
  pub fix_name: String,
  // It appears currently that all fixIds are strings, but the protocol
  // specifies an opaque type, the problem is that we need to use the id as a
  // hash key, and `Value` does not implement hash (and it could provide a false
  // positive depending on JSON whitespace, so we deserialize it but it might
  // break in the future)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fix_id: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fix_all_description: Option<String>,
}

impl CodeFixAction {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    for changes in &mut self.changes {
      changes.normalize(specifier_map)?;
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombinedCodeActions {
  pub changes: Vec<FileTextChanges>,
  pub commands: Option<Vec<Value>>,
}

impl CombinedCodeActions {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    for changes in &mut self.changes {
      changes.normalize(specifier_map)?;
    }
    Ok(())
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencedSymbol {
  pub definition: ReferencedSymbolDefinitionInfo,
  pub references: Vec<ReferencedSymbolEntry>,
}

impl ReferencedSymbol {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.definition.normalize(specifier_map)?;
    for reference in &mut self.references {
      reference.normalize(specifier_map)?;
    }
    Ok(())
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencedSymbolDefinitionInfo {
  #[serde(flatten)]
  pub definition_info: DefinitionInfo,
}

impl ReferencedSymbolDefinitionInfo {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.definition_info.normalize(specifier_map)?;
    Ok(())
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencedSymbolEntry {
  #[serde(default)]
  pub is_definition: bool,
  #[serde(flatten)]
  pub entry: ReferenceEntry,
}

impl ReferencedSymbolEntry {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.entry.normalize(specifier_map)?;
    Ok(())
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceEntry {
  // is_write_access: bool,
  // is_in_string: Option<bool>,
  #[serde(flatten)]
  pub document_span: DocumentSpan,
}

impl ReferenceEntry {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.document_span.normalize(specifier_map)?;
    Ok(())
  }
}

impl ReferenceEntry {
  pub fn to_location(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
  ) -> lsp::Location {
    let specifier = resolve_url(&self.document_span.file_name)
      .unwrap_or_else(|_| INVALID_SPECIFIER.clone());
    let file_referrer = language_server.documents.get_file_referrer(&specifier);
    let uri = language_server
      .url_map
      .specifier_to_uri(&specifier, file_referrer.as_deref())
      .unwrap_or_else(|_| INVALID_URI.clone());
    lsp::Location {
      uri,
      range: self.document_span.text_span.to_range(line_index),
    }
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyItem {
  name: String,
  kind: ScriptElementKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  kind_modifiers: Option<String>,
  file: String,
  span: TextSpan,
  selection_span: TextSpan,
  #[serde(skip_serializing_if = "Option::is_none")]
  container_name: Option<String>,
}

impl CallHierarchyItem {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.file = specifier_map.normalize(&self.file)?.to_string();
    Ok(())
  }

  pub fn try_resolve_call_hierarchy_item(
    &self,
    language_server: &language_server::Inner,
    maybe_root_path: Option<&Path>,
  ) -> Option<lsp::CallHierarchyItem> {
    let target_specifier = resolve_url(&self.file).ok()?;
    let target_asset_or_doc =
      language_server.get_maybe_asset_or_document(&target_specifier)?;

    Some(self.to_call_hierarchy_item(
      target_asset_or_doc.line_index(),
      language_server,
      maybe_root_path,
    ))
  }

  pub fn to_call_hierarchy_item(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
    maybe_root_path: Option<&Path>,
  ) -> lsp::CallHierarchyItem {
    let target_specifier =
      resolve_url(&self.file).unwrap_or_else(|_| INVALID_SPECIFIER.clone());
    let file_referrer = language_server
      .documents
      .get_file_referrer(&target_specifier);
    let uri = language_server
      .url_map
      .specifier_to_uri(&target_specifier, file_referrer.as_deref())
      .unwrap_or_else(|_| INVALID_URI.clone());

    let use_file_name = self.is_source_file_item();
    let maybe_file_path = if uri.scheme().is_some_and(|s| s.as_str() == "file")
    {
      url_to_file_path(&uri_to_url(&uri)).ok()
    } else {
      None
    };
    let name = if use_file_name {
      if let Some(file_path) = maybe_file_path.as_ref() {
        file_path.file_name().unwrap().to_string_lossy().to_string()
      } else {
        uri.as_str().to_string()
      }
    } else {
      self.name.clone()
    };
    let detail = if use_file_name {
      if let Some(file_path) = maybe_file_path.as_ref() {
        // TODO: update this to work with multi root workspaces
        let parent_dir = file_path.parent().unwrap();
        if let Some(root_path) = maybe_root_path {
          parent_dir
            .strip_prefix(root_path)
            .unwrap_or(parent_dir)
            .to_string_lossy()
            .to_string()
        } else {
          parent_dir.to_string_lossy().to_string()
        }
      } else {
        String::new()
      }
    } else {
      self.container_name.as_ref().cloned().unwrap_or_default()
    };

    let mut tags: Option<Vec<lsp::SymbolTag>> = None;
    if let Some(modifiers) = self.kind_modifiers.as_ref() {
      let kind_modifiers = parse_kind_modifier(modifiers);
      if kind_modifiers.contains("deprecated") {
        tags = Some(vec![lsp::SymbolTag::DEPRECATED]);
      }
    }

    lsp::CallHierarchyItem {
      name,
      tags,
      uri,
      detail: Some(detail),
      kind: self.kind.clone().into(),
      range: self.span.to_range(line_index.clone()),
      selection_range: self.selection_span.to_range(line_index),
      data: None,
    }
  }

  fn is_source_file_item(&self) -> bool {
    self.kind == ScriptElementKind::ScriptElement
      || self.kind == ScriptElementKind::ModuleElement
        && self.selection_span.start == 0
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyIncomingCall {
  from: CallHierarchyItem,
  from_spans: Vec<TextSpan>,
}

impl CallHierarchyIncomingCall {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.from.normalize(specifier_map)?;
    Ok(())
  }

  pub fn try_resolve_call_hierarchy_incoming_call(
    &self,
    language_server: &language_server::Inner,
    maybe_root_path: Option<&Path>,
  ) -> Option<lsp::CallHierarchyIncomingCall> {
    let target_specifier = resolve_url(&self.from.file).ok()?;
    let target_asset_or_doc =
      language_server.get_maybe_asset_or_document(&target_specifier)?;

    Some(lsp::CallHierarchyIncomingCall {
      from: self.from.to_call_hierarchy_item(
        target_asset_or_doc.line_index(),
        language_server,
        maybe_root_path,
      ),
      from_ranges: self
        .from_spans
        .iter()
        .map(|span| span.to_range(target_asset_or_doc.line_index()))
        .collect(),
    })
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyOutgoingCall {
  to: CallHierarchyItem,
  from_spans: Vec<TextSpan>,
}

impl CallHierarchyOutgoingCall {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    self.to.normalize(specifier_map)?;
    Ok(())
  }

  pub fn try_resolve_call_hierarchy_outgoing_call(
    &self,
    line_index: Arc<LineIndex>,
    language_server: &language_server::Inner,
    maybe_root_path: Option<&Path>,
  ) -> Option<lsp::CallHierarchyOutgoingCall> {
    let target_specifier = resolve_url(&self.to.file).ok()?;
    let target_asset_or_doc =
      language_server.get_maybe_asset_or_document(&target_specifier)?;

    Some(lsp::CallHierarchyOutgoingCall {
      to: self.to.to_call_hierarchy_item(
        target_asset_or_doc.line_index(),
        language_server,
        maybe_root_path,
      ),
      from_ranges: self
        .from_spans
        .iter()
        .map(|span| span.to_range(line_index.clone()))
        .collect(),
    })
  }
}

/// Used to convert completion code actions into a command and additional text
/// edits to pass in the completion item.
fn parse_code_actions(
  maybe_code_actions: Option<&Vec<CodeAction>>,
  data: &CompletionItemData,
  specifier: &ModuleSpecifier,
  language_server: &language_server::Inner,
) -> Result<(Option<lsp::Command>, Option<Vec<lsp::TextEdit>>), AnyError> {
  if let Some(code_actions) = maybe_code_actions {
    let mut additional_text_edits: Vec<lsp::TextEdit> = Vec::new();
    let mut has_remaining_commands_or_edits = false;
    for ts_action in code_actions {
      if ts_action.commands.is_some() {
        has_remaining_commands_or_edits = true;
      }

      let asset_or_doc =
        language_server.get_asset_or_document(&data.specifier)?;
      for change in &ts_action.changes {
        if data.specifier.as_str() == change.file_name {
          additional_text_edits.extend(change.text_changes.iter().map(|tc| {
            let mut text_edit = tc.as_text_edit(asset_or_doc.line_index());
            if let Some(specifier_rewrite) = &data.specifier_rewrite {
              text_edit.new_text = text_edit
                .new_text
                .replace(&specifier_rewrite.0, &specifier_rewrite.1);
            }
            text_edit
          }));
        } else {
          has_remaining_commands_or_edits = true;
        }
      }
    }

    let mut command: Option<lsp::Command> = None;
    if has_remaining_commands_or_edits {
      let actions: Vec<Value> = code_actions
        .iter()
        .map(|ca| {
          let changes: Vec<FileTextChanges> = ca
            .changes
            .clone()
            .into_iter()
            .filter(|ch| ch.file_name == data.specifier.as_str())
            .collect();
          json!({
            "commands": ca.commands,
            "description": ca.description,
            "changes": changes,
          })
        })
        .collect();
      command = Some(lsp::Command {
        title: "".to_string(),
        command: "_typescript.applyCompletionCodeAction".to_string(),
        arguments: Some(vec![json!(specifier.to_string()), json!(actions)]),
      });
    }

    if additional_text_edits.is_empty() {
      Ok((command, None))
    } else {
      Ok((command, Some(additional_text_edits)))
    }
  } else {
    Ok((None, None))
  }
}

// Based on https://github.com/microsoft/vscode/blob/1.81.1/extensions/typescript-language-features/src/languageFeatures/util/snippetForFunctionCall.ts#L49.
fn get_parameters_from_parts(parts: &[SymbolDisplayPart]) -> Vec<String> {
  let mut parameters = Vec::with_capacity(3);
  let mut is_in_fn = false;
  let mut paren_count = 0;
  let mut brace_count = 0;
  for (idx, part) in parts.iter().enumerate() {
    if ["methodName", "functionName", "text", "propertyName"]
      .contains(&part.kind.as_str())
    {
      if paren_count == 0 && brace_count == 0 {
        is_in_fn = true;
      }
    } else if part.kind == "parameterName" {
      if paren_count == 1 && brace_count == 0 && is_in_fn {
        let is_optional =
          matches!(parts.get(idx + 1), Some(next) if next.text == "?");
        // Skip `this` and optional parameters.
        if !is_optional && part.text != "this" {
          parameters.push(format!(
            "${{{}:{}}}",
            parameters.len() + 1,
            &part.text
          ));
        }
      }
    } else if part.kind == "punctuation" {
      if part.text == "(" {
        paren_count += 1;
      } else if part.text == ")" {
        paren_count -= 1;
        if paren_count <= 0 && is_in_fn {
          break;
        }
      } else if part.text == "..." && paren_count == 1 {
        // Found rest parameter. Do not fill in any further arguments.
        break;
      } else if part.text == "{" {
        brace_count += 1;
      } else if part.text == "}" {
        brace_count -= 1;
      }
    }
  }
  parameters
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionEntryDetails {
  display_parts: Vec<SymbolDisplayPart>,
  documentation: Option<Vec<SymbolDisplayPart>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  tags: Option<Vec<JsDocTagInfo>>,
  name: String,
  kind: ScriptElementKind,
  kind_modifiers: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  code_actions: Option<Vec<CodeAction>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  source_display: Option<Vec<SymbolDisplayPart>>,
}

impl CompletionEntryDetails {
  fn normalize(
    &mut self,
    specifier_map: &TscSpecifierMap,
  ) -> Result<(), AnyError> {
    for action in self.code_actions.iter_mut().flatten() {
      action.normalize(specifier_map)?;
    }
    Ok(())
  }

  pub fn as_completion_item(
    &self,
    original_item: &lsp::CompletionItem,
    data: &CompletionItemData,
    specifier: &ModuleSpecifier,
    language_server: &language_server::Inner,
  ) -> Result<lsp::CompletionItem, AnyError> {
    let detail = if original_item.detail.is_some() {
      original_item.detail.clone()
    } else if !self.display_parts.is_empty() {
      Some(replace_links(display_parts_to_string(
        &self.display_parts,
        language_server,
      )))
    } else {
      None
    };
    let documentation = if let Some(parts) = &self.documentation {
      // NOTE: similar as `QuickInfo::to_hover()`
      let mut value = display_parts_to_string(parts, language_server);
      if let Some(tags) = &self.tags {
        let tags_preview = tags
          .iter()
          .map(|tag_info| get_tag_documentation(tag_info, language_server))
          .collect::<Vec<String>>()
          .join("  \n\n");
        if !tags_preview.is_empty() {
          value = format!("{value}\n\n{tags_preview}");
        }
      }
      Some(lsp::Documentation::MarkupContent(lsp::MarkupContent {
        kind: lsp::MarkupKind::Markdown,
        value,
      }))
    } else {
      None
    };
    let mut text_edit = original_item.text_edit.clone();
    if let Some(specifier_rewrite) = &data.specifier_rewrite {
      if let Some(text_edit) = &mut text_edit {
        match text_edit {
          lsp::CompletionTextEdit::Edit(text_edit) => {
            text_edit.new_text = text_edit
              .new_text
              .replace(&specifier_rewrite.0, &specifier_rewrite.1);
          }
          lsp::CompletionTextEdit::InsertAndReplace(insert_replace_edit) => {
            insert_replace_edit.new_text = insert_replace_edit
              .new_text
              .replace(&specifier_rewrite.0, &specifier_rewrite.1);
          }
        }
      }
    }
    let (command, additional_text_edits) = parse_code_actions(
      self.code_actions.as_ref(),
      data,
      specifier,
      language_server,
    )?;
    let mut insert_text_format = original_item.insert_text_format;
    let insert_text = if data.use_code_snippet {
      insert_text_format = Some(lsp::InsertTextFormat::SNIPPET);
      Some(format!(
        "{}({})",
        original_item
          .insert_text
          .as_ref()
          .unwrap_or(&original_item.label),
        get_parameters_from_parts(&self.display_parts).join(", "),
      ))
    } else {
      original_item.insert_text.clone()
    };

    Ok(lsp::CompletionItem {
      data: None,
      detail,
      documentation,
      command,
      text_edit,
      additional_text_edits,
      insert_text,
      insert_text_format,
      // NOTE(bartlomieju): it's not entirely clear to me why we need to do that,
      // but when `completionItem/resolve` is called, we get a list of commit chars
      // even though we might have returned an empty list in `completion` request.
      commit_characters: None,
      ..original_item.clone()
    })
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionInfo {
  entries: Vec<CompletionEntry>,
  // this is only used by Microsoft's telemetrics, which Deno doesn't use and
  // there are issues with the value not matching the type definitions.
  // flags: Option<CompletionInfoFlags>,
  is_global_completion: bool,
  is_member_completion: bool,
  is_new_identifier_location: bool,
  metadata: Option<Value>,
  optional_replacement_span: Option<TextSpan>,
}

impl CompletionInfo {
  pub fn as_completion_response(
    &self,
    line_index: Arc<LineIndex>,
    settings: &config::CompletionSettings,
    specifier: &ModuleSpecifier,
    position: u32,
    language_server: &language_server::Inner,
  ) -> lsp::CompletionResponse {
    let items = self
      .entries
      .iter()
      .flat_map(|entry| {
        entry.as_completion_item(
          line_index.clone(),
          self,
          settings,
          specifier,
          position,
          language_server,
        )
      })
      .collect();
    let is_incomplete = self
      .metadata
      .clone()
      .map(|v| {
        v.as_object()
          .unwrap()
          .get("isIncomplete")
          .unwrap_or(&json!(false))
          .as_bool()
          .unwrap()
      })
      .unwrap_or(false);
    lsp::CompletionResponse::List(lsp::CompletionList {
      is_incomplete,
      items,
    })
  }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItemData {
  pub specifier: ModuleSpecifier,
  pub position: u32,
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  /// If present, the code action / text edit corresponding to this item should
  /// be rewritten by replacing the first string with the second. Intended for
  /// auto-import specifiers to be reverse-import-mapped.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub specifier_rewrite: Option<(String, String)>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<Value>,
  pub use_code_snippet: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletionEntryDataImport {
  module_specifier: String,
  file_name: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionEntry {
  name: String,
  kind: ScriptElementKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  kind_modifiers: Option<String>,
  sort_text: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  insert_text: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_snippet: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  replacement_span: Option<TextSpan>,
  #[serde(skip_serializing_if = "Option::is_none")]
  has_action: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  source: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  source_display: Option<Vec<SymbolDisplayPart>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  label_details: Option<CompletionEntryLabelDetails>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_recommended: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_from_unchecked_file: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_package_json_import: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  is_import_statement_completion: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  data: Option<Value>,
}

impl CompletionEntry {
  fn get_commit_characters(
    &self,
    info: &CompletionInfo,
    settings: &config::CompletionSettings,
  ) -> Option<Vec<String>> {
    if info.is_new_identifier_location {
      return None;
    }

    let mut commit_characters = vec![];
    match self.kind {
      ScriptElementKind::MemberGetAccessorElement
      | ScriptElementKind::MemberSetAccessorElement
      | ScriptElementKind::ConstructSignatureElement
      | ScriptElementKind::CallSignatureElement
      | ScriptElementKind::IndexSignatureElement
      | ScriptElementKind::EnumElement
      | ScriptElementKind::InterfaceElement => {
        commit_characters.push(".");
        commit_characters.push(";");
      }
      ScriptElementKind::ModuleElement
      | ScriptElementKind::Alias
      | ScriptElementKind::ConstElement
      | ScriptElementKind::LetElement
      | ScriptElementKind::VariableElement
      | ScriptElementKind::LocalVariableElement
      | ScriptElementKind::MemberVariableElement
      | ScriptElementKind::ClassElement
      | ScriptElementKind::FunctionElement
      | ScriptElementKind::MemberFunctionElement
      | ScriptElementKind::Keyword
      | ScriptElementKind::ParameterElement => {
        commit_characters.push(".");
        commit_characters.push(",");
        commit_characters.push(";");
        if !settings.complete_function_calls {
          commit_characters.push("(");
        }
      }
      _ => (),
    }

    if commit_characters.is_empty() {
      None
    } else {
      Some(commit_characters.into_iter().map(String::from).collect())
    }
  }

  fn get_filter_text(&self) -> Option<String> {
    if self.name.starts_with('#') {
      if let Some(insert_text) = &self.insert_text {
        if insert_text.starts_with("this.#") {
          return Some(insert_text.replace("this.#", ""));
        } else {
          return Some(insert_text.clone());
        }
      } else {
        return None;
      }
    }

    if let Some(insert_text) = &self.insert_text {
      if insert_text.starts_with("this.") {
        return None;
      }
      if insert_text.starts_with('[') {
        return Some(
          BRACKET_ACCESSOR_RE
            .replace(insert_text, |caps: &Captures| format!(".{}", &caps[1]))
            .to_string(),
        );
      }
    }

    self.insert_text.clone()
  }

  pub fn as_completion_item(
    &self,
    line_index: Arc<LineIndex>,
    info: &CompletionInfo,
    settings: &config::CompletionSettings,
    specifier: &ModuleSpecifier,
    position: u32,
    language_server: &language_server::Inner,
  ) -> Option<lsp::CompletionItem> {
    let mut label = self.name.clone();
    let mut label_details: Option<lsp::CompletionItemLabelDetails> = None;
    let mut kind: Option<lsp::CompletionItemKind> =
      Some(self.kind.clone().into());
    let mut specifier_rewrite = None;

    let mut sort_text = if self.source.is_some() {
      format!("\u{ffff}{}", self.sort_text)
    } else {
      self.sort_text.clone()
    };

    let preselect = self.is_recommended;
    let use_code_snippet = settings.complete_function_calls
      && (kind == Some(lsp::CompletionItemKind::FUNCTION)
        || kind == Some(lsp::CompletionItemKind::METHOD));
    let commit_characters = self.get_commit_characters(info, settings);
    let mut insert_text = self.insert_text.clone();
    let insert_text_format = match self.is_snippet {
      Some(true) => Some(lsp::InsertTextFormat::SNIPPET),
      _ => None,
    };
    let range = self.replacement_span.clone();
    let mut filter_text = self.get_filter_text();
    let mut tags = None;
    let mut detail = None;

    if let Some(kind_modifiers) = &self.kind_modifiers {
      let kind_modifiers = parse_kind_modifier(kind_modifiers);
      if kind_modifiers.contains("optional") {
        if insert_text.is_none() {
          insert_text = Some(label.clone());
        }
        if filter_text.is_none() {
          filter_text = Some(label.clone());
        }
        label += "?";
      }
      if kind_modifiers.contains("deprecated") {
        tags = Some(vec![lsp::CompletionItemTag::DEPRECATED]);
      }
      if kind_modifiers.contains("color") {
        kind = Some(lsp::CompletionItemKind::COLOR);
      }
      if self.kind == ScriptElementKind::ScriptElement {
        for ext_modifier in FILE_EXTENSION_KIND_MODIFIERS {
          if kind_modifiers.contains(ext_modifier) {
            detail = if self.name.to_lowercase().ends_with(ext_modifier) {
              Some(self.name.clone())
            } else {
              Some(format!("{}{}", self.name, ext_modifier))
            };
            break;
          }
        }
      }
    }

    if let Some(source) = &self.source {
      let mut display_source = source.clone();
      if let Some(data) = &self.data {
        if let Ok(import_data) =
          serde_json::from_value::<CompletionEntryDataImport>(data.clone())
        {
          if let Ok(import_specifier) = resolve_url(&import_data.file_name) {
            if let Some(new_module_specifier) = language_server
              .get_ts_response_import_mapper(specifier)
              .check_specifier(&import_specifier, specifier)
              .or_else(|| relative_specifier(specifier, &import_specifier))
            {
              display_source.clone_from(&new_module_specifier);
              if new_module_specifier != import_data.module_specifier {
                specifier_rewrite =
                  Some((import_data.module_specifier, new_module_specifier));
              }
            } else if source.starts_with(jsr_url().as_str()) {
              return None;
            }
          }
        }
      }
      // We want relative or bare (import-mapped or otherwise) specifiers to
      // appear at the top.
      if resolve_url(&display_source).is_err() {
        sort_text += "_0";
      } else {
        sort_text += "_1";
      }
      label_details
        .get_or_insert_with(Default::default)
        .description = Some(display_source);
    }

    let text_edit =
      if let (Some(text_span), Some(new_text)) = (range, &insert_text) {
        let range = text_span.to_range(line_index);
        let insert_replace_edit = lsp::InsertReplaceEdit {
          new_text: new_text.clone(),
          insert: range,
          replace: range,
        };
        Some(insert_replace_edit.into())
      } else {
        None
      };

    let tsc = CompletionItemData {
      specifier: specifier.clone(),
      position,
      name: self.name.clone(),
      source: self.source.clone(),
      specifier_rewrite,
      data: self.data.clone(),
      use_code_snippet,
    };

    Some(lsp::CompletionItem {
      label,
      label_details,
      kind,
      sort_text: Some(sort_text),
      preselect,
      text_edit,
      filter_text,
      insert_text,
      insert_text_format,
      detail,
      tags,
      commit_characters,
      data: Some(json!({ "tsc": tsc })),
      ..Default::default()
    })
  }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletionEntryLabelDetails {
  #[serde(skip_serializing_if = "Option::is_none")]
  detail: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub enum OutliningSpanKind {
  #[serde(rename = "comment")]
  Comment,
  #[serde(rename = "region")]
  Region,
  #[serde(rename = "code")]
  Code,
  #[serde(rename = "imports")]
  Imports,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutliningSpan {
  text_span: TextSpan,
  // hint_span: TextSpan,
  // banner_text: String,
  // auto_collapse: bool,
  kind: OutliningSpanKind,
}

const FOLD_END_PAIR_CHARACTERS: &[u8] = &[b'}', b']', b')', b'`'];

impl OutliningSpan {
  pub fn to_folding_range(
    &self,
    line_index: Arc<LineIndex>,
    content: &[u8],
    line_folding_only: bool,
  ) -> lsp::FoldingRange {
    let range = self.text_span.to_range(line_index.clone());
    lsp::FoldingRange {
      start_line: range.start.line,
      start_character: if line_folding_only {
        None
      } else {
        Some(range.start.character)
      },
      end_line: self.adjust_folding_end_line(
        &range,
        line_index,
        content,
        line_folding_only,
      ),
      end_character: if line_folding_only {
        None
      } else {
        Some(range.end.character)
      },
      kind: self.get_folding_range_kind(&self.kind),
      collapsed_text: None,
    }
  }

  fn adjust_folding_end_line(
    &self,
    range: &lsp::Range,
    line_index: Arc<LineIndex>,
    content: &[u8],
    line_folding_only: bool,
  ) -> u32 {
    if line_folding_only && range.end.line > 0 && range.end.character > 0 {
      let offset_end: usize = line_index.offset(range.end).unwrap().into();
      let fold_end_char = content[offset_end - 1];
      if FOLD_END_PAIR_CHARACTERS.contains(&fold_end_char) {
        return cmp::max(range.end.line - 1, range.start.line);
      }
    }

    range.end.line
  }

  fn get_folding_range_kind(
    &self,
    span_kind: &OutliningSpanKind,
  ) -> Option<lsp::FoldingRangeKind> {
    match span_kind {
      OutliningSpanKind::Comment => Some(lsp::FoldingRangeKind::Comment),
      OutliningSpanKind::Region => Some(lsp::FoldingRangeKind::Region),
      OutliningSpanKind::Imports => Some(lsp::FoldingRangeKind::Imports),
      _ => None,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpItems {
  items: Vec<SignatureHelpItem>,
  // applicable_span: TextSpan,
  selected_item_index: u32,
  argument_index: u32,
  // argument_count: u32,
}

impl SignatureHelpItems {
  pub fn into_signature_help(
    self,
    language_server: &language_server::Inner,
  ) -> lsp::SignatureHelp {
    lsp::SignatureHelp {
      signatures: self
        .items
        .into_iter()
        .map(|item| item.into_signature_information(language_server))
        .collect(),
      active_parameter: Some(self.argument_index),
      active_signature: Some(self.selected_item_index),
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpItem {
  // is_variadic: bool,
  prefix_display_parts: Vec<SymbolDisplayPart>,
  suffix_display_parts: Vec<SymbolDisplayPart>,
  // separator_display_parts: Vec<SymbolDisplayPart>,
  parameters: Vec<SignatureHelpParameter>,
  documentation: Vec<SymbolDisplayPart>,
  // tags: Vec<JsDocTagInfo>,
}

impl SignatureHelpItem {
  pub fn into_signature_information(
    self,
    language_server: &language_server::Inner,
  ) -> lsp::SignatureInformation {
    let prefix_text =
      display_parts_to_string(&self.prefix_display_parts, language_server);
    let params_text = self
      .parameters
      .iter()
      .map(|param| {
        display_parts_to_string(&param.display_parts, language_server)
      })
      .collect::<Vec<String>>()
      .join(", ");
    let suffix_text =
      display_parts_to_string(&self.suffix_display_parts, language_server);
    let documentation =
      display_parts_to_string(&self.documentation, language_server);
    lsp::SignatureInformation {
      label: format!("{prefix_text}{params_text}{suffix_text}"),
      documentation: Some(lsp::Documentation::MarkupContent(
        lsp::MarkupContent {
          kind: lsp::MarkupKind::Markdown,
          value: documentation,
        },
      )),
      parameters: Some(
        self
          .parameters
          .into_iter()
          .map(|param| param.into_parameter_information(language_server))
          .collect(),
      ),
      active_parameter: None,
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpParameter {
  // name: String,
  documentation: Vec<SymbolDisplayPart>,
  display_parts: Vec<SymbolDisplayPart>,
  // is_optional: bool,
}

impl SignatureHelpParameter {
  pub fn into_parameter_information(
    self,
    language_server: &language_server::Inner,
  ) -> lsp::ParameterInformation {
    let documentation =
      display_parts_to_string(&self.documentation, language_server);
    lsp::ParameterInformation {
      label: lsp::ParameterLabel::Simple(display_parts_to_string(
        &self.display_parts,
        language_server,
      )),
      documentation: Some(lsp::Documentation::MarkupContent(
        lsp::MarkupContent {
          kind: lsp::MarkupKind::Markdown,
          value: documentation,
        },
      )),
    }
  }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRange {
  text_span: TextSpan,
  #[serde(skip_serializing_if = "Option::is_none")]
  parent: Option<Box<SelectionRange>>,
}

impl SelectionRange {
  pub fn to_selection_range(
    &self,
    line_index: Arc<LineIndex>,
  ) -> lsp::SelectionRange {
    lsp::SelectionRange {
      range: self.text_span.to_range(line_index.clone()),
      parent: self.parent.as_ref().map(|parent_selection| {
        Box::new(parent_selection.to_selection_range(line_index))
      }),
    }
  }
}

#[derive(Debug, Default)]
pub struct TscSpecifierMap {
  normalized_specifiers: DashMap<String, ModuleSpecifier>,
  denormalized_specifiers: DashMap<ModuleSpecifier, String>,
}

impl TscSpecifierMap {
  pub fn new() -> Self {
    Self::default()
  }

  /// Convert the specifier to one compatible with tsc. Cache the resulting
  /// mapping in case it needs to be reversed.
  // TODO(nayeemrmn): Factor in out-of-band media type here.
  pub fn denormalize(&self, specifier: &ModuleSpecifier) -> String {
    let original = specifier;
    if let Some(specifier) = self.denormalized_specifiers.get(original) {
      return specifier.to_string();
    }
    let mut specifier = original.to_string();
    let media_type = MediaType::from_specifier(original);
    // If the URL-inferred media type doesn't correspond to tsc's path-inferred
    // media type, force it to be the same by appending an extension.
    if MediaType::from_path(Path::new(specifier.as_str())) != media_type {
      specifier += media_type.as_ts_extension();
    }
    if specifier != original.as_str() {
      self
        .normalized_specifiers
        .insert(specifier.clone(), original.clone());
    }
    specifier
  }

  /// Convert the specifier from one compatible with tsc. Cache the resulting
  /// mapping in case it needs to be reversed.
  pub fn normalize<S: AsRef<str>>(
    &self,
    specifier: S,
  ) -> Result<ModuleSpecifier, AnyError> {
    let original = specifier.as_ref();
    if let Some(specifier) = self.normalized_specifiers.get(original) {
      return Ok(specifier.clone());
    }
    let specifier_str = original.replace(".d.ts.d.ts", ".d.ts");
    let specifier = match ModuleSpecifier::parse(&specifier_str) {
      Ok(s) => s,
      Err(err) => return Err(err.into()),
    };
    if specifier.as_str() != original {
      self
        .denormalized_specifiers
        .insert(specifier.clone(), original.to_string());
    }
    Ok(specifier)
  }
}

// TODO(bartlomieju): we have similar struct in `cli/tsc/mod.rs` - maybe at least change
// the name of the struct to avoid confusion?
struct State {
  last_id: usize,
  performance: Arc<Performance>,
  // the response from JS, as a JSON string
  response_tx: Option<oneshot::Sender<Result<String, AnyError>>>,
  state_snapshot: Arc<StateSnapshot>,
  specifier_map: Arc<TscSpecifierMap>,
  last_scope: Option<ModuleSpecifier>,
  token: CancellationToken,
  pending_requests: Option<UnboundedReceiver<Request>>,
  mark: Option<PerformanceMark>,
}

impl State {
  fn new(
    state_snapshot: Arc<StateSnapshot>,
    specifier_map: Arc<TscSpecifierMap>,
    performance: Arc<Performance>,
    pending_requests: UnboundedReceiver<Request>,
  ) -> Self {
    Self {
      last_id: 1,
      performance,
      response_tx: None,
      state_snapshot,
      specifier_map,
      last_scope: None,
      token: Default::default(),
      mark: None,
      pending_requests: Some(pending_requests),
    }
  }

  fn get_document(&self, specifier: &ModuleSpecifier) -> Option<Arc<Document>> {
    self
      .state_snapshot
      .documents
      .get_or_load(specifier, self.last_scope.as_ref())
  }

  fn get_asset_or_document(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<AssetOrDocument> {
    let snapshot = &self.state_snapshot;
    if specifier.scheme() == "asset" {
      snapshot.assets.get(specifier).map(AssetOrDocument::Asset)
    } else {
      let document = self.get_document(specifier);
      document.map(AssetOrDocument::Document)
    }
  }

  fn script_version(&self, specifier: &ModuleSpecifier) -> Option<String> {
    if specifier.scheme() == "asset" {
      if self.state_snapshot.assets.contains_key(specifier) {
        Some("1".to_string())
      } else {
        None
      }
    } else {
      let document = self.get_document(specifier);
      document.map(|d| d.script_version())
    }
  }
}

#[op2(fast)]
fn op_is_cancelled(state: &mut OpState) -> bool {
  let state = state.borrow_mut::<State>();
  state.token.is_cancelled()
}

#[op2(fast)]
fn op_is_node_file(state: &mut OpState, #[string] path: String) -> bool {
  let state = state.borrow::<State>();
  let mark = state.performance.mark("tsc.op.op_is_node_file");
  let r = match ModuleSpecifier::parse(&path) {
    Ok(specifier) => state.state_snapshot.resolver.in_node_modules(&specifier),
    Err(_) => false,
  };
  state.performance.measure(mark);
  r
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadResponse {
  data: Arc<str>,
  script_kind: i32,
  version: Option<String>,
  is_cjs: bool,
}

#[op2]
fn op_load<'s>(
  scope: &'s mut v8::HandleScope,
  state: &mut OpState,
  #[string] specifier: &str,
) -> Result<v8::Local<'s, v8::Value>, AnyError> {
  let state = state.borrow_mut::<State>();
  let mark = state
    .performance
    .mark_with_args("tsc.op.op_load", specifier);
  let specifier = state.specifier_map.normalize(specifier)?;
  let maybe_load_response =
    if specifier.as_str() == MISSING_DEPENDENCY_SPECIFIER {
      None
    } else {
      let asset_or_document = state.get_asset_or_document(&specifier);
      asset_or_document.map(|doc| LoadResponse {
        data: doc.text(),
        script_kind: crate::tsc::as_ts_script_kind(doc.media_type()),
        version: state.script_version(&specifier),
        is_cjs: matches!(
          doc.media_type(),
          MediaType::Cjs | MediaType::Cts | MediaType::Dcts
        ),
      })
    };

  let serialized = serde_v8::to_v8(scope, maybe_load_response)?;

  state.performance.measure(mark);
  Ok(serialized)
}

#[op2(fast)]
fn op_release(
  state: &mut OpState,
  #[string] specifier: &str,
) -> Result<(), AnyError> {
  let state = state.borrow_mut::<State>();
  let mark = state
    .performance
    .mark_with_args("tsc.op.op_release", specifier);
  let specifier = state.specifier_map.normalize(specifier)?;
  state.state_snapshot.documents.release(&specifier);
  state.performance.measure(mark);
  Ok(())
}

#[op2]
#[serde]
fn op_resolve(
  state: &mut OpState,
  #[string] base: String,
  is_base_cjs: bool,
  #[serde] specifiers: Vec<String>,
) -> Result<Vec<Option<(String, String)>>, AnyError> {
  op_resolve_inner(
    state,
    ResolveArgs {
      base,
      is_base_cjs,
      specifiers,
    },
  )
}

struct TscRequestArray {
  request: TscRequest,
  scope: Option<String>,
  id: Smi<usize>,
  change: convert::OptionNull<PendingChange>,
}

impl<'a> ToV8<'a> for TscRequestArray {
  type Error = StdAnyError;

  fn to_v8(
    self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let id = self.id.to_v8(scope).unwrap_infallible();

    let (method_name, args) = self.request.to_server_request(scope)?;

    let method_name = deno_core::FastString::from_static(method_name)
      .v8_string(scope)
      .into();
    let args = args.unwrap_or_else(|| v8::Array::new(scope, 0).into());
    let scope_url = serde_v8::to_v8(scope, self.scope)
      .map_err(AnyError::from)
      .map_err(StdAnyError::from)?;

    let change = self.change.to_v8(scope).unwrap_infallible();

    Ok(
      v8::Array::new_with_elements(
        scope,
        &[id, method_name, args, scope_url, change],
      )
      .into(),
    )
  }
}

#[op2(async)]
#[to_v8]
async fn op_poll_requests(
  state: Rc<RefCell<OpState>>,
) -> convert::OptionNull<TscRequestArray> {
  let mut pending_requests = {
    let mut state = state.borrow_mut();
    let state = state.try_borrow_mut::<State>().unwrap();
    state.pending_requests.take().unwrap()
  };

  let Some((request, scope, snapshot, response_tx, token, change)) =
    pending_requests.recv().await
  else {
    return None.into();
  };

  let mut state = state.borrow_mut();
  let state = state.try_borrow_mut::<State>().unwrap();
  state.pending_requests = Some(pending_requests);
  state.state_snapshot = snapshot;
  state.token = token;
  state.response_tx = Some(response_tx);
  let id = state.last_id;
  state.last_id += 1;
  state.last_scope.clone_from(&scope);
  let mark = state
    .performance
    .mark_with_args(format!("tsc.host.{}", request.method()), &request);
  state.mark = Some(mark);

  Some(TscRequestArray {
    request,
    scope: scope.map(|s| s.into()),
    id: Smi(id),
    change: change.into(),
  })
  .into()
}

#[inline]
fn op_resolve_inner(
  state: &mut OpState,
  args: ResolveArgs,
) -> Result<Vec<Option<(String, String)>>, AnyError> {
  let state = state.borrow_mut::<State>();
  let mark = state.performance.mark_with_args("tsc.op.op_resolve", &args);
  let referrer = state.specifier_map.normalize(&args.base)?;
  let specifiers = state
    .state_snapshot
    .documents
    .resolve(&args.specifiers, &referrer, state.last_scope.as_ref())
    .into_iter()
    .map(|o| {
      o.map(|(s, mt)| {
        (
          state.specifier_map.denormalize(&s),
          mt.as_ts_extension().to_string(),
        )
      })
    })
    .collect();
  state.performance.measure(mark);
  Ok(specifiers)
}

#[op2(fast)]
fn op_respond(
  state: &mut OpState,
  #[string] response: String,
  #[string] error: String,
) {
  let state = state.borrow_mut::<State>();
  state.performance.measure(state.mark.take().unwrap());
  state.last_scope = None;
  let response = if !error.is_empty() {
    Err(anyhow!("tsc error: {error}"))
  } else {
    Ok(response)
  };

  let was_sent = state.response_tx.take().unwrap().send(response).is_ok();
  // Don't print the send error if the token is cancelled, it's expected
  // to fail in that case and this commonly occurs.
  if !was_sent && !state.token.is_cancelled() {
    lsp_warn!("Unable to send result to client.");
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScriptNames {
  unscoped: IndexSet<String>,
  by_scope: BTreeMap<ModuleSpecifier, IndexSet<String>>,
}

#[op2]
#[serde]
fn op_script_names(state: &mut OpState) -> ScriptNames {
  let state = state.borrow_mut::<State>();
  let mark = state.performance.mark("tsc.op.op_script_names");
  let mut result = ScriptNames {
    unscoped: IndexSet::new(),
    by_scope: BTreeMap::from_iter(
      state
        .state_snapshot
        .config
        .tree
        .data_by_scope()
        .keys()
        .map(|s| (s.clone(), IndexSet::new())),
    ),
  };

  let scopes_with_node_specifier =
    state.state_snapshot.documents.scopes_with_node_specifier();
  if scopes_with_node_specifier.contains(&None) {
    result
      .unscoped
      .insert("asset:///node_types.d.ts".to_string());
  }
  for (scope, script_names) in &mut result.by_scope {
    if scopes_with_node_specifier.contains(&Some(scope.clone())) {
      script_names.insert("asset:///node_types.d.ts".to_string());
    }
  }

  // inject these next because they're global
  for (scope, script_names) in &mut result.by_scope {
    for (_, specifiers) in state
      .state_snapshot
      .resolver
      .graph_imports_by_referrer(scope)
    {
      for specifier in specifiers {
        script_names.insert(specifier.to_string());
      }
    }
  }

  // finally include the documents
  let docs = state
    .state_snapshot
    .documents
    .documents(DocumentsFilter::AllDiagnosable);
  for doc in &docs {
    let specifier = doc.specifier();
    let is_open = doc.is_open();
    if is_open || specifier.scheme() == "file" {
      let script_names = doc
        .scope()
        .and_then(|s| result.by_scope.get_mut(s))
        .unwrap_or(&mut result.unscoped);
      let types_specifier = (|| {
        let documents = &state.state_snapshot.documents;
        let types = doc.maybe_types_dependency().maybe_specifier()?;
        let (types, _) = documents.resolve_dependency(
          types,
          specifier,
          doc.file_referrer(),
        )?;
        let types_doc = documents.get_or_load(&types, doc.file_referrer())?;
        Some(types_doc.specifier().clone())
      })();
      // If there is a types dep, use that as the root instead. But if the doc
      // is open, include both as roots.
      if let Some(types_specifier) = &types_specifier {
        script_names.insert(types_specifier.to_string());
      }
      if types_specifier.is_none() || is_open {
        script_names.insert(specifier.to_string());
      }
    }
  }

  for script_names in result
    .by_scope
    .values_mut()
    .chain(std::iter::once(&mut result.unscoped))
  {
    *script_names = std::mem::take(script_names)
      .into_iter()
      .map(|s| match ModuleSpecifier::parse(&s) {
        Ok(s) => state.specifier_map.denormalize(&s),
        Err(_) => s,
      })
      .collect();
  }
  state.performance.measure(mark);
  result
}

#[op2]
#[string]
fn op_script_version(
  state: &mut OpState,
  #[string] specifier: &str,
) -> Result<Option<String>, AnyError> {
  let state = state.borrow_mut::<State>();
  let mark = state.performance.mark("tsc.op.op_script_version");
  let specifier = state.specifier_map.normalize(specifier)?;
  let r = state.script_version(&specifier);
  state.performance.measure(mark);
  Ok(r)
}

#[op2(fast)]
#[number]
fn op_project_version(state: &mut OpState) -> usize {
  let state: &mut State = state.borrow_mut::<State>();
  let mark = state.performance.mark("tsc.op.op_project_version");
  let r = state.state_snapshot.project_version;
  state.performance.measure(mark);
  r
}

struct TscRuntime {
  js_runtime: JsRuntime,
  server_main_loop_fn_global: v8::Global<v8::Function>,
}

impl TscRuntime {
  fn new(mut js_runtime: JsRuntime) -> Self {
    let server_main_loop_fn_global = {
      let context = js_runtime.main_context();
      let scope = &mut js_runtime.handle_scope();
      let context_local = v8::Local::new(scope, context);
      let global_obj = context_local.global(scope);
      let server_main_loop_fn_str =
        v8::String::new_external_onebyte_static(scope, b"serverMainLoop")
          .unwrap();
      let server_main_loop_fn = v8::Local::try_from(
        global_obj
          .get(scope, server_main_loop_fn_str.into())
          .unwrap(),
      )
      .unwrap();
      v8::Global::new(scope, server_main_loop_fn)
    };
    Self {
      server_main_loop_fn_global,
      js_runtime,
    }
  }
}

fn run_tsc_thread(
  request_rx: UnboundedReceiver<Request>,
  performance: Arc<Performance>,
  specifier_map: Arc<TscSpecifierMap>,
  maybe_inspector_server: Option<Arc<InspectorServer>>,
) {
  let has_inspector_server = maybe_inspector_server.is_some();
  // Create and setup a JsRuntime based on a snapshot. It is expected that the
  // supplied snapshot is an isolate that contains the TypeScript language
  // server.
  let mut tsc_runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![deno_tsc::init_ops(
      performance,
      specifier_map,
      request_rx,
    )],
    startup_snapshot: Some(tsc::compiler_snapshot()),
    inspector: has_inspector_server,
    ..Default::default()
  });

  if let Some(server) = maybe_inspector_server {
    server.register_inspector(
      "ext:deno_tsc/99_main_compiler.js".to_string(),
      &mut tsc_runtime,
      false,
    );
  }

  let tsc_future = async {
    // start_tsc(&mut tsc_runtime, false).unwrap();
    let tsc_runtime =
      Rc::new(tokio::sync::Mutex::new(TscRuntime::new(tsc_runtime)));
    let tsc_runtime_ = tsc_runtime.clone();

    let event_loop_fut = async {
      loop {
        if let Err(e) = tsc_runtime_
          .lock()
          .await
          .js_runtime
          .run_event_loop(PollEventLoopOptions {
            wait_for_inspector: false,
            pump_v8_message_loop: true,
          })
          .await
        {
          log::error!("Error in TSC event loop: {e}");
        }
      }
    };
    let main_loop_fut = {
      let enable_debug = std::env::var("DENO_TSC_DEBUG")
        .map(|s| {
          let s = s.trim();
          s == "1" || s.eq_ignore_ascii_case("true")
        })
        .unwrap_or(false);
      let mut runtime = tsc_runtime.lock().await;
      let main_loop = runtime.server_main_loop_fn_global.clone();
      let args = {
        let scope = &mut runtime.js_runtime.handle_scope();
        let enable_debug_local =
          v8::Local::<v8::Value>::from(v8::Boolean::new(scope, enable_debug));
        [v8::Global::new(scope, enable_debug_local)]
      };

      runtime.js_runtime.call_with_args(&main_loop, &args)
    };

    tokio::select! {
      biased;
      _ = event_loop_fut => {},
      res = main_loop_fut => {
        if let Err(err) = res {
          log::error!("Error in TSC main loop: {err}");
        }
      }
    }
  }
  .boxed_local();

  let runtime = create_basic_runtime();
  runtime.block_on(tsc_future)
}

deno_core::extension!(deno_tsc,
  ops = [
    op_is_cancelled,
    op_is_node_file,
    op_load,
    op_release,
    op_resolve,
    op_respond,
    op_script_names,
    op_script_version,
    op_project_version,
    op_poll_requests,
  ],
  options = {
    performance: Arc<Performance>,
    specifier_map: Arc<TscSpecifierMap>,
    request_rx: UnboundedReceiver<Request>,
  },
  state = |state, options| {
    state.put(State::new(
      Default::default(),
      options.specifier_map,
      options.performance,
      options.request_rx,
    ));
  },
);

#[derive(Debug, Clone, Deserialize_repr, Serialize_repr)]
#[repr(u32)]
pub enum CompletionTriggerKind {
  Invoked = 1,
  TriggerCharacter = 2,
  TriggerForIncompleteCompletions = 3,
}

impl From<lsp::CompletionTriggerKind> for CompletionTriggerKind {
  fn from(kind: lsp::CompletionTriggerKind) -> Self {
    match kind {
      lsp::CompletionTriggerKind::INVOKED => Self::Invoked,
      lsp::CompletionTriggerKind::TRIGGER_CHARACTER => Self::TriggerCharacter,
      lsp::CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS => {
        Self::TriggerForIncompleteCompletions
      }
      _ => Self::Invoked,
    }
  }
}

pub type QuotePreference = config::QuoteStyle;

pub type ImportModuleSpecifierPreference = config::ImportModuleSpecifier;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum ImportModuleSpecifierEnding {
  Auto,
  Minimal,
  Index,
  Js,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum IncludeInlayParameterNameHints {
  None,
  Literals,
  All,
}

impl From<&config::InlayHintsParamNamesEnabled>
  for IncludeInlayParameterNameHints
{
  fn from(setting: &config::InlayHintsParamNamesEnabled) -> Self {
    match setting {
      config::InlayHintsParamNamesEnabled::All => Self::All,
      config::InlayHintsParamNamesEnabled::Literals => Self::Literals,
      config::InlayHintsParamNamesEnabled::None => Self::None,
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum IncludePackageJsonAutoImports {
  Auto,
  On,
  Off,
}

pub type JsxAttributeCompletionStyle = config::JsxAttributeCompletionStyle;

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompletionsAtPositionOptions {
  #[serde(flatten)]
  pub user_preferences: UserPreferences,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_character: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_kind: Option<CompletionTriggerKind>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub disable_suggestions: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub quote_preference: Option<QuotePreference>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_for_module_exports: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_for_import_statements: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_snippet_text: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_automatic_optional_chain_completions: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_insert_text: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_class_member_snippets: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_completions_with_object_literal_method_snippets: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub use_label_details_in_completion_entries: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub allow_incomplete_completions: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub import_module_specifier_preference:
    Option<ImportModuleSpecifierPreference>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub import_module_specifier_ending: Option<ImportModuleSpecifierEnding>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub allow_text_changes_in_new_files: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provide_prefix_and_suffix_text_for_rename: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_package_json_auto_imports: Option<IncludePackageJsonAutoImports>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub provide_refactor_not_applicable_reason: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub jsx_attribute_completion_style: Option<JsxAttributeCompletionStyle>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_parameter_name_hints:
    Option<IncludeInlayParameterNameHints>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_parameter_name_hints_when_argument_matches_name:
    Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_function_parameter_type_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_variable_type_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_variable_type_hints_when_type_matches_name: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_property_declaration_type_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_function_like_return_type_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub include_inlay_enum_member_value_hints: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub allow_rename_of_import_path: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub auto_import_file_exclude_patterns: Option<Vec<String>>,
}

impl UserPreferences {
  pub fn from_config_for_specifier(
    config: &config::Config,
    specifier: &ModuleSpecifier,
  ) -> Self {
    let fmt_options = config.tree.fmt_config_for_specifier(specifier);
    let fmt_config = &fmt_options.options;
    let base_preferences = Self {
      allow_incomplete_completions: Some(true),
      allow_text_changes_in_new_files: Some(specifier.scheme() == "file"),
      // TODO(nayeemrmn): Investigate why we use `Index` here.
      import_module_specifier_ending: Some(ImportModuleSpecifierEnding::Index),
      include_completions_with_snippet_text: Some(
        config.snippet_support_capable(),
      ),
      provide_refactor_not_applicable_reason: Some(true),
      quote_preference: Some(fmt_config.into()),
      use_label_details_in_completion_entries: Some(true),
      ..Default::default()
    };
    let Some(language_settings) =
      config.language_settings_for_specifier(specifier)
    else {
      return base_preferences;
    };
    Self {
      auto_import_file_exclude_patterns: Some(
        language_settings
          .preferences
          .auto_import_file_exclude_patterns
          .clone(),
      ),
      include_automatic_optional_chain_completions: Some(
        language_settings.suggest.enabled
          && language_settings
            .suggest
            .include_automatic_optional_chain_completions,
      ),
      include_completions_for_import_statements: Some(
        language_settings.suggest.enabled
          && language_settings
            .suggest
            .include_completions_for_import_statements,
      ),
      include_completions_for_module_exports: Some(
        language_settings.suggest.enabled
          && language_settings.suggest.auto_imports,
      ),
      include_completions_with_class_member_snippets: Some(
        language_settings.suggest.enabled
          && language_settings.suggest.class_member_snippets.enabled
          && config.snippet_support_capable(),
      ),
      include_completions_with_insert_text: Some(
        language_settings.suggest.enabled,
      ),
      include_completions_with_object_literal_method_snippets: Some(
        language_settings.suggest.enabled
          && language_settings
            .suggest
            .object_literal_method_snippets
            .enabled
          && config.snippet_support_capable(),
      ),
      import_module_specifier_preference: Some(
        language_settings.preferences.import_module_specifier,
      ),
      include_inlay_parameter_name_hints: Some(
        (&language_settings.inlay_hints.parameter_names.enabled).into(),
      ),
      include_inlay_parameter_name_hints_when_argument_matches_name: Some(
        !language_settings
          .inlay_hints
          .parameter_names
          .suppress_when_argument_matches_name,
      ),
      include_inlay_function_parameter_type_hints: Some(
        language_settings.inlay_hints.parameter_types.enabled,
      ),
      include_inlay_variable_type_hints: Some(
        language_settings.inlay_hints.variable_types.enabled,
      ),
      include_inlay_variable_type_hints_when_type_matches_name: Some(
        !language_settings
          .inlay_hints
          .variable_types
          .suppress_when_type_matches_name,
      ),
      include_inlay_property_declaration_type_hints: Some(
        language_settings
          .inlay_hints
          .property_declaration_types
          .enabled,
      ),
      include_inlay_function_like_return_type_hints: Some(
        language_settings
          .inlay_hints
          .function_like_return_types
          .enabled,
      ),
      include_inlay_enum_member_value_hints: Some(
        language_settings.inlay_hints.enum_member_values.enabled,
      ),
      jsx_attribute_completion_style: Some(
        language_settings.preferences.jsx_attribute_completion_style,
      ),
      provide_prefix_and_suffix_text_for_rename: Some(
        language_settings.preferences.use_aliases_for_renames,
      ),
      // Only use workspace settings for quote style if there's no `deno.json`.
      quote_preference: if config
        .tree
        .workspace_dir_for_specifier(specifier)
        .is_some_and(|ctx| ctx.maybe_deno_json().is_some())
      {
        base_preferences.quote_preference
      } else {
        Some(language_settings.preferences.quote_style)
      },
      ..base_preferences
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpItemsOptions {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_reason: Option<SignatureHelpTriggerReason>,
}

#[derive(Debug, Clone, Serialize)]
pub enum SignatureHelpTriggerKind {
  #[serde(rename = "characterTyped")]
  CharacterTyped,
  #[serde(rename = "invoked")]
  Invoked,
  #[serde(rename = "retrigger")]
  Retrigger,
  #[serde(rename = "unknown")]
  Unknown,
}

impl From<lsp::SignatureHelpTriggerKind> for SignatureHelpTriggerKind {
  fn from(kind: lsp::SignatureHelpTriggerKind) -> Self {
    match kind {
      lsp::SignatureHelpTriggerKind::INVOKED => Self::Invoked,
      lsp::SignatureHelpTriggerKind::TRIGGER_CHARACTER => Self::CharacterTyped,
      lsp::SignatureHelpTriggerKind::CONTENT_CHANGE => Self::Retrigger,
      _ => Self::Unknown,
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureHelpTriggerReason {
  pub kind: SignatureHelpTriggerKind,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub trigger_character: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompletionDetailsArgs {
  pub specifier: ModuleSpecifier,
  pub position: u32,
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub format_code_settings: Option<FormatCodeSettings>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub source: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub preferences: Option<UserPreferences>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub data: Option<Value>,
}

impl From<&CompletionItemData> for GetCompletionDetailsArgs {
  fn from(item_data: &CompletionItemData) -> Self {
    Self {
      specifier: item_data.specifier.clone(),
      position: item_data.position,
      name: item_data.name.clone(),
      source: item_data.source.clone(),
      preferences: None,
      format_code_settings: None,
      data: item_data.data.clone(),
    }
  }
}

#[derive(Debug)]
pub struct GetNavigateToItemsArgs {
  pub search: String,
  pub max_result_count: Option<u32>,
  pub file: Option<String>,
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct TscTextRange {
  pos: u32,
  end: u32,
}

impl From<Range<u32>> for TscTextRange {
  fn from(range: Range<u32>) -> Self {
    Self {
      pos: range.start,
      end: range.end,
    }
  }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CombinedCodeFixScope {
  r#type: &'static str,
  file_name: String,
}

#[derive(Serialize, Clone, Copy)]
pub struct JsNull;

#[derive(Debug, Clone, Serialize)]
pub enum TscRequest {
  GetDiagnostics((Vec<String>, usize)),
  GetAssets,

  CleanupSemanticCache,
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6230
  FindReferences((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6235
  GetNavigationTree((String,)),
  GetSupportedCodeFixes,
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6214
  GetQuickInfoAtPosition((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6257
  GetCodeFixesAtPosition(
    Box<(
      String,
      u32,
      u32,
      Vec<i32>,
      FormatCodeSettings,
      UserPreferences,
    )>,
  ),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6274
  GetApplicableRefactors(
    Box<(
      String,
      TscTextRange,
      UserPreferences,
      Option<&'static str>,
      String,
    )>,
  ),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6258
  GetCombinedCodeFix(
    Box<(
      CombinedCodeFixScope,
      String,
      FormatCodeSettings,
      UserPreferences,
    )>,
  ),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6275
  GetEditsForRefactor(
    Box<(
      String,
      FormatCodeSettings,
      TscTextRange,
      String,
      String,
      Option<UserPreferences>,
    )>,
  ),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6281
  GetEditsForFileRename(
    Box<(String, String, FormatCodeSettings, UserPreferences)>,
  ),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6231
  GetDocumentHighlights(Box<(String, u32, Vec<String>)>),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6226
  GetDefinitionAndBoundSpan((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6227
  GetTypeDefinitionAtPosition((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6193
  GetCompletionsAtPosition(
    Box<(
      String,
      u32,
      GetCompletionsAtPositionOptions,
      FormatCodeSettings,
    )>,
  ),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6205
  #[allow(clippy::type_complexity)]
  GetCompletionEntryDetails(
    Box<(
      String,
      u32,
      String,
      FormatCodeSettings,
      Option<String>,
      Option<UserPreferences>,
      Option<Value>,
    )>,
  ),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6228
  GetImplementationAtPosition((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6240
  GetOutliningSpans((String,)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6237
  ProvideCallHierarchyIncomingCalls((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6238
  ProvideCallHierarchyOutgoingCalls((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6236
  PrepareCallHierarchy((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6221
  FindRenameLocations((String, u32, bool, bool, bool)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6224
  GetSmartSelectionRange((String, u32)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6183
  GetEncodedSemanticClassifications((String, TextSpan, &'static str)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6217
  GetSignatureHelpItems((String, u32, SignatureHelpItemsOptions)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6233
  GetNavigateToItems((String, Option<u32>, Option<String>)),
  // https://github.com/denoland/deno/blob/v1.37.1/cli/tsc/dts/typescript.d.ts#L6239
  ProvideInlayHints((String, TextSpan, UserPreferences)),
}

impl TscRequest {
  /// Converts the request into a tuple containing the method name and the
  /// arguments (in the form of a V8 value) to be passed to the server request
  /// function
  fn to_server_request<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
  ) -> Result<(&'static str, Option<v8::Local<'s, v8::Value>>), AnyError> {
    let args = match self {
      TscRequest::GetDiagnostics(args) => {
        ("$getDiagnostics", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::FindReferences(args) => {
        ("findReferences", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::GetNavigationTree(args) => {
        ("getNavigationTree", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::GetSupportedCodeFixes => ("$getSupportedCodeFixes", None),
      TscRequest::GetQuickInfoAtPosition(args) => (
        "getQuickInfoAtPosition",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetCodeFixesAtPosition(args) => (
        "getCodeFixesAtPosition",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetApplicableRefactors(args) => (
        "getApplicableRefactors",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetCombinedCodeFix(args) => {
        ("getCombinedCodeFix", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::GetEditsForRefactor(args) => {
        ("getEditsForRefactor", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::GetEditsForFileRename(args) => {
        ("getEditsForFileRename", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::GetDocumentHighlights(args) => {
        ("getDocumentHighlights", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::GetDefinitionAndBoundSpan(args) => (
        "getDefinitionAndBoundSpan",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetTypeDefinitionAtPosition(args) => (
        "getTypeDefinitionAtPosition",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetCompletionsAtPosition(args) => (
        "getCompletionsAtPosition",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetCompletionEntryDetails(args) => (
        "getCompletionEntryDetails",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetImplementationAtPosition(args) => (
        "getImplementationAtPosition",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetOutliningSpans(args) => {
        ("getOutliningSpans", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::ProvideCallHierarchyIncomingCalls(args) => (
        "provideCallHierarchyIncomingCalls",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::ProvideCallHierarchyOutgoingCalls(args) => (
        "provideCallHierarchyOutgoingCalls",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::PrepareCallHierarchy(args) => {
        ("prepareCallHierarchy", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::FindRenameLocations(args) => {
        ("findRenameLocations", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::GetSmartSelectionRange(args) => (
        "getSmartSelectionRange",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetEncodedSemanticClassifications(args) => (
        "getEncodedSemanticClassifications",
        Some(serde_v8::to_v8(scope, args)?),
      ),
      TscRequest::GetSignatureHelpItems(args) => {
        ("getSignatureHelpItems", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::GetNavigateToItems(args) => {
        ("getNavigateToItems", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::ProvideInlayHints(args) => {
        ("provideInlayHints", Some(serde_v8::to_v8(scope, args)?))
      }
      TscRequest::CleanupSemanticCache => ("cleanupSemanticCache", None),
      TscRequest::GetAssets => ("$getAssets", None),
    };

    Ok(args)
  }

  fn method(&self) -> &'static str {
    match self {
      TscRequest::GetDiagnostics(_) => "$getDiagnostics",
      TscRequest::CleanupSemanticCache => "cleanupSemanticCache",
      TscRequest::FindReferences(_) => "findReferences",
      TscRequest::GetNavigationTree(_) => "getNavigationTree",
      TscRequest::GetSupportedCodeFixes => "$getSupportedCodeFixes",
      TscRequest::GetQuickInfoAtPosition(_) => "getQuickInfoAtPosition",
      TscRequest::GetCodeFixesAtPosition(_) => "getCodeFixesAtPosition",
      TscRequest::GetApplicableRefactors(_) => "getApplicableRefactors",
      TscRequest::GetCombinedCodeFix(_) => "getCombinedCodeFix",
      TscRequest::GetEditsForRefactor(_) => "getEditsForRefactor",
      TscRequest::GetEditsForFileRename(_) => "getEditsForFileRename",
      TscRequest::GetDocumentHighlights(_) => "getDocumentHighlights",
      TscRequest::GetDefinitionAndBoundSpan(_) => "getDefinitionAndBoundSpan",
      TscRequest::GetTypeDefinitionAtPosition(_) => {
        "getTypeDefinitionAtPosition"
      }
      TscRequest::GetCompletionsAtPosition(_) => "getCompletionsAtPosition",
      TscRequest::GetCompletionEntryDetails(_) => "getCompletionEntryDetails",
      TscRequest::GetImplementationAtPosition(_) => {
        "getImplementationAtPosition"
      }
      TscRequest::GetOutliningSpans(_) => "getOutliningSpans",
      TscRequest::ProvideCallHierarchyIncomingCalls(_) => {
        "provideCallHierarchyIncomingCalls"
      }
      TscRequest::ProvideCallHierarchyOutgoingCalls(_) => {
        "provideCallHierarchyOutgoingCalls"
      }
      TscRequest::PrepareCallHierarchy(_) => "prepareCallHierarchy",
      TscRequest::FindRenameLocations(_) => "findRenameLocations",
      TscRequest::GetSmartSelectionRange(_) => "getSmartSelectionRange",
      TscRequest::GetEncodedSemanticClassifications(_) => {
        "getEncodedSemanticClassifications"
      }
      TscRequest::GetSignatureHelpItems(_) => "getSignatureHelpItems",
      TscRequest::GetNavigateToItems(_) => "getNavigateToItems",
      TscRequest::ProvideInlayHints(_) => "provideInlayHints",
      TscRequest::GetAssets => "$getAssets",
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::cache::HttpCache;
  use crate::http_util::HeadersMap;
  use crate::lsp::cache::LspCache;
  use crate::lsp::config::Config;
  use crate::lsp::config::WorkspaceSettings;
  use crate::lsp::documents::Documents;
  use crate::lsp::documents::LanguageId;
  use crate::lsp::resolver::LspResolver;
  use crate::lsp::text::LineIndex;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  async fn setup(
    ts_config: Value,
    sources: &[(&str, &str, i32, LanguageId)],
  ) -> (TempDir, TsServer, Arc<StateSnapshot>, LspCache) {
    let temp_dir = TempDir::new();
    let cache = LspCache::new(Some(temp_dir.url().join(".deno_dir").unwrap()));
    let mut config = Config::default();
    config
      .tree
      .inject_config_file(
        deno_config::deno_json::ConfigFile::new(
          &json!({
            "compilerOptions": ts_config,
          })
          .to_string(),
          temp_dir.url().join("deno.json").unwrap(),
          &Default::default(),
        )
        .unwrap(),
      )
      .await;
    let resolver =
      Arc::new(LspResolver::from_config(&config, &cache, None).await);
    let mut documents = Documents::default();
    documents.update_config(&config, &resolver, &cache, &Default::default());
    for (relative_specifier, source, version, language_id) in sources {
      let specifier = temp_dir.url().join(relative_specifier).unwrap();
      documents.open(specifier, *version, *language_id, (*source).into(), None);
    }
    let snapshot = Arc::new(StateSnapshot {
      project_version: 0,
      documents: Arc::new(documents),
      assets: Default::default(),
      config: Arc::new(config),
      resolver,
    });
    let performance = Arc::new(Performance::default());
    let ts_server = TsServer::new(performance);
    ts_server.start(None).unwrap();
    ts_server.project_changed(
      snapshot.clone(),
      [],
      Some(
        snapshot
          .config
          .tree
          .data_by_scope()
          .iter()
          .map(|(s, d)| (s.clone(), d.ts_config.clone()))
          .collect(),
      ),
    );
    (temp_dir, ts_server, snapshot, cache)
  }

  fn setup_op_state(state_snapshot: Arc<StateSnapshot>) -> OpState {
    let (_tx, rx) = mpsc::unbounded_channel();
    let state =
      State::new(state_snapshot, Default::default(), Default::default(), rx);
    let mut op_state = OpState::new(None);
    op_state.put(state);
    op_state
  }

  #[test]
  fn test_replace_links() {
    let actual = replace_links(r"test {@link http://deno.land/x/mod.ts} test");
    assert_eq!(
      actual,
      r"test [http://deno.land/x/mod.ts](http://deno.land/x/mod.ts) test"
    );
    let actual =
      replace_links(r"test {@link http://deno.land/x/mod.ts a link} test");
    assert_eq!(actual, r"test [a link](http://deno.land/x/mod.ts) test");
    let actual =
      replace_links(r"test {@linkcode http://deno.land/x/mod.ts a link} test");
    assert_eq!(actual, r"test [`a link`](http://deno.land/x/mod.ts) test");
  }

  #[tokio::test]
  async fn test_get_diagnostics() {
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "noEmit": true,
        "lib": [],
      }),
      &[(
        "a.ts",
        r#"console.log("hello deno");"#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot, vec![specifier.clone()], Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!({
        specifier.clone(): [
          {
            "start": {
              "line": 0,
              "character": 0,
            },
            "end": {
              "line": 0,
              "character": 7
            },
            "fileName": specifier,
            "messageText": "Cannot find name 'console'. Do you need to change your target library? Try changing the \'lib\' compiler option to include 'dom'.",
            "sourceLine": "console.log(\"hello deno\");",
            "category": 1,
            "code": 2584
          }
        ]
      })
    );
  }

  #[tokio::test]
  async fn test_get_diagnostics_lib() {
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "jsx": "react",
        "lib": ["esnext", "dom", "deno.ns"],
        "noEmit": true,
      }),
      &[(
        "a.ts",
        r#"console.log(document.location);"#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot, vec![specifier.clone()], Default::default())
      .await
      .unwrap();
    assert_eq!(json!(diagnostics), json!({ specifier: [] }));
  }

  #[tokio::test]
  async fn test_module_resolution() {
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot, vec![specifier.clone()], Default::default())
      .await
      .unwrap();
    assert_eq!(json!(diagnostics), json!({ specifier: [] }));
  }

  #[tokio::test]
  async fn test_bad_module_specifiers() {
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "a.ts",
        r#"
        import { A } from ".";
        "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot, vec![specifier.clone()], Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!({
        specifier.clone(): [{
          "start": {
            "line": 1,
            "character": 8
          },
          "end": {
            "line": 1,
            "character": 30
          },
          "fileName": specifier,
          "messageText": "\'A\' is declared but its value is never read.",
          "sourceLine": "        import { A } from \".\";",
          "category": 2,
          "code": 6133,
        }]
      })
    );
  }

  #[tokio::test]
  async fn test_remote_modules() {
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot, vec![specifier.clone()], Default::default())
      .await
      .unwrap();
    assert_eq!(json!(diagnostics), json!({ specifier: [] }));
  }

  #[tokio::test]
  async fn test_partial_modules() {
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "a.ts",
        r#"
        import {
          Application,
          Context,
          Router,
          Status,
        } from "https://deno.land/x/oak@v6.3.2/mod.ts";

        import * as test from
      "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot, vec![specifier.clone()], Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!({
        specifier.clone(): [{
          "start": {
            "line": 1,
            "character": 8
          },
          "end": {
            "line": 6,
            "character": 55,
          },
          "fileName": specifier.clone(),
          "messageText": "All imports in import declaration are unused.",
          "sourceLine": "        import {",
          "category": 2,
          "code": 6192,
        }, {
          "start": {
            "line": 8,
            "character": 29
          },
          "end": {
            "line": 8,
            "character": 29
          },
          "fileName": specifier,
          "messageText": "Expression expected.",
          "sourceLine": "        import * as test from",
          "category": 1,
          "code": 1109
        }]
      })
    );
  }

  #[tokio::test]
  async fn test_no_debug_failure() {
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "a.ts",
        r#"const url = new URL("b.js", import."#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(snapshot, vec![specifier.clone()], Default::default())
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!({
        specifier.clone(): [
          {
            "start": {
              "line": 0,
              "character": 35,
            },
            "end": {
              "line": 0,
              "character": 35
            },
            "fileName": specifier,
            "messageText": "Identifier expected.",
            "sourceLine": "const url = new URL(\"b.js\", import.",
            "category": 1,
            "code": 1003,
          }
        ]
      })
    );
  }

  #[tokio::test]
  async fn test_request_assets() {
    let (_, ts_server, snapshot, _) = setup(json!({}), &[]).await;
    let assets = get_isolate_assets(&ts_server, snapshot).await;
    let mut asset_names = assets
      .iter()
      .map(|a| {
        a.specifier()
          .to_string()
          .replace("asset:///lib.", "")
          .replace(".d.ts", "")
      })
      .collect::<Vec<_>>();
    let mut expected_asset_names: Vec<String> = serde_json::from_str(
      include_str!(concat!(env!("OUT_DIR"), "/lib_file_names.json")),
    )
    .unwrap();
    asset_names.sort();

    // if this test fails, update build.rs
    expected_asset_names.sort();
    assert_eq!(asset_names, expected_asset_names);

    // get some notification when the size of the assets grows
    let mut total_size = 0;
    for asset in assets {
      total_size += asset.text().len();
    }
    assert!(total_size > 0);
    assert!(total_size < 2_000_000); // currently as of TS 4.6, it's 0.7MB
  }

  #[tokio::test]
  async fn test_modify_sources() {
    let (temp_dir, ts_server, snapshot, cache) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[(
        "a.ts",
        r#"
          import * as a from "https://deno.land/x/example/a.ts";
          if (a.a === "b") {
            console.log("fail");
          }
        "#,
        1,
        LanguageId::TypeScript,
      )],
    )
    .await;
    let specifier_dep =
      resolve_url("https://deno.land/x/example/a.ts").unwrap();
    cache
      .global()
      .set(
        &specifier_dep,
        HeadersMap::default(),
        b"export const b = \"b\";\n",
      )
      .unwrap();
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(
        snapshot.clone(),
        vec![specifier.clone()],
        Default::default(),
      )
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!({
        specifier.clone(): [
          {
            "start": {
              "line": 2,
              "character": 16,
            },
            "end": {
              "line": 2,
              "character": 17
            },
            "fileName": specifier,
            "messageText": "Property \'a\' does not exist on type \'typeof import(\"https://deno.land/x/example/a\")\'.",
            "sourceLine": "          if (a.a === \"b\") {",
            "code": 2339,
            "category": 1,
          }
        ]
      })
    );
    cache
      .global()
      .set(
        &specifier_dep,
        HeadersMap::default(),
        b"export const b = \"b\";\n\nexport const a = \"b\";\n",
      )
      .unwrap();
    let snapshot = {
      Arc::new(StateSnapshot {
        project_version: snapshot.project_version + 1,
        ..snapshot.as_ref().clone()
      })
    };
    ts_server.project_changed(
      snapshot.clone(),
      [(&specifier_dep, ChangeKind::Opened)],
      None,
    );
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let diagnostics = ts_server
      .get_diagnostics(
        snapshot.clone(),
        vec![specifier.clone()],
        Default::default(),
      )
      .await
      .unwrap();
    assert_eq!(
      json!(diagnostics),
      json!({
        specifier: []
      })
    );
  }

  #[test]
  fn test_completion_entry_filter_text() {
    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "['foo']".to_string(),
      insert_text: Some("['foo']".to_string()),
      ..Default::default()
    };
    let actual = fixture.get_filter_text();
    assert_eq!(actual, Some(".foo".to_string()));

    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "#abc".to_string(),
      ..Default::default()
    };
    let actual = fixture.get_filter_text();
    assert_eq!(actual, None);

    let fixture = CompletionEntry {
      kind: ScriptElementKind::MemberVariableElement,
      name: "#abc".to_string(),
      insert_text: Some("this.#abc".to_string()),
      ..Default::default()
    };
    let actual = fixture.get_filter_text();
    assert_eq!(actual, Some("abc".to_string()));
  }

  #[tokio::test]
  async fn test_completions() {
    let fixture = r#"
      import { B } from "https://deno.land/x/b/mod.ts";

      const b = new B();

      console.
    "#;
    let line_index = LineIndex::new(fixture);
    let position = line_index
      .offset_tsc(lsp::Position {
        line: 5,
        character: 16,
      })
      .unwrap();
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[("a.ts", fixture, 1, LanguageId::TypeScript)],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let info = ts_server
      .get_completions(
        snapshot.clone(),
        specifier.clone(),
        position,
        GetCompletionsAtPositionOptions {
          user_preferences: UserPreferences {
            include_completions_with_insert_text: Some(true),
            ..Default::default()
          },
          trigger_character: Some(".".to_string()),
          trigger_kind: None,
        },
        Default::default(),
        Some(temp_dir.url()),
      )
      .await
      .unwrap();
    assert_eq!(info.entries.len(), 22);
    let details = ts_server
      .get_completion_details(
        snapshot.clone(),
        GetCompletionDetailsArgs {
          specifier,
          position,
          name: "log".to_string(),
          format_code_settings: None,
          source: None,
          preferences: None,
          data: None,
        },
        Some(temp_dir.url()),
      )
      .await
      .unwrap()
      .unwrap();
    assert_eq!(
      json!(details),
      json!({
        "name": "log",
        "kindModifiers": "declare",
        "kind": "method",
        "displayParts": [
          {
            "text": "(",
            "kind": "punctuation"
          },
          {
            "text": "method",
            "kind": "text"
          },
          {
            "text": ")",
            "kind": "punctuation"
          },
          {
            "text": " ",
            "kind": "space"
          },
          {
            "text": "Console",
            "kind": "interfaceName"
          },
          {
            "text": ".",
            "kind": "punctuation"
          },
          {
            "text": "log",
            "kind": "methodName"
          },
          {
            "text": "(",
            "kind": "punctuation"
          },
          {
            "text": "...",
            "kind": "punctuation"
          },
          {
            "text": "data",
            "kind": "parameterName"
          },
          {
            "text": ":",
            "kind": "punctuation"
          },
          {
            "text": " ",
            "kind": "space"
          },
          {
            "text": "any",
            "kind": "keyword"
          },
          {
            "text": "[",
            "kind": "punctuation"
          },
          {
            "text": "]",
            "kind": "punctuation"
          },
          {
            "text": ")",
            "kind": "punctuation"
          },
          {
            "text": ":",
            "kind": "punctuation"
          },
          {
            "text": " ",
            "kind": "space"
          },
          {
            "text": "void",
            "kind": "keyword"
          }
        ],
        "documentation": []
      })
    );
  }

  #[tokio::test]
  async fn test_completions_fmt() {
    let fixture_a = r#"
      console.log(someLongVaria)
    "#;
    let fixture_b = r#"
      export const someLongVariable = 1
    "#;
    let line_index = LineIndex::new(fixture_a);
    let position = line_index
      .offset_tsc(lsp::Position {
        line: 1,
        character: 33,
      })
      .unwrap();
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[
        ("a.ts", fixture_a, 1, LanguageId::TypeScript),
        ("b.ts", fixture_b, 1, LanguageId::TypeScript),
      ],
    )
    .await;
    let specifier = temp_dir.url().join("a.ts").unwrap();
    let fmt_options_config = FmtOptionsConfig {
      semi_colons: Some(false),
      single_quote: Some(true),
      ..Default::default()
    };
    let info = ts_server
      .get_completions(
        snapshot.clone(),
        specifier.clone(),
        position,
        GetCompletionsAtPositionOptions {
          user_preferences: UserPreferences {
            quote_preference: Some((&fmt_options_config).into()),
            include_completions_for_module_exports: Some(true),
            include_completions_with_insert_text: Some(true),
            ..Default::default()
          },
          ..Default::default()
        },
        FormatCodeSettings::from(&fmt_options_config),
        Some(temp_dir.url()),
      )
      .await
      .unwrap();
    let entry = info
      .entries
      .iter()
      .find(|e| &e.name == "someLongVariable")
      .unwrap();
    let details = ts_server
      .get_completion_details(
        snapshot.clone(),
        GetCompletionDetailsArgs {
          specifier,
          position,
          name: entry.name.clone(),
          format_code_settings: Some(FormatCodeSettings::from(
            &fmt_options_config,
          )),
          source: entry.source.clone(),
          preferences: Some(UserPreferences {
            quote_preference: Some((&fmt_options_config).into()),
            ..Default::default()
          }),
          data: entry.data.clone(),
        },
        Some(temp_dir.url()),
      )
      .await
      .unwrap()
      .unwrap();
    let actions = details.code_actions.unwrap();
    let action = actions
      .iter()
      .find(|a| &a.description == r#"Add import from "./b.ts""#)
      .unwrap();
    let changes = action.changes.first().unwrap();
    let change = changes.text_changes.first().unwrap();
    assert_eq!(
      change.new_text,
      "import type { someLongVariable } from './b.ts'\n"
    );
  }

  #[test]
  fn test_classification_to_semantic_tokens_multiline_tokens() {
    let line_index = Arc::new(LineIndex::new("  to\nken  \n"));
    let classifications = Classifications {
      spans: vec![2, 6, 2057],
    };
    let semantic_tokens =
      classifications.to_semantic_tokens(line_index).unwrap();
    assert_eq!(
      &semantic_tokens.data,
      &[
        lsp::SemanticToken {
          delta_line: 0,
          delta_start: 2,
          length: 3,
          token_type: 7,
          token_modifiers_bitset: 9,
        },
        lsp::SemanticToken {
          delta_line: 1,
          delta_start: 0,
          length: 3,
          token_type: 7,
          token_modifiers_bitset: 9,
        },
      ]
    );
  }

  #[tokio::test]
  async fn test_get_edits_for_file_rename() {
    let (temp_dir, ts_server, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[
        ("a.ts", r#"import "./b.ts";"#, 1, LanguageId::TypeScript),
        ("b.ts", r#""#, 1, LanguageId::TypeScript),
      ],
    )
    .await;
    let changes = ts_server
      .get_edits_for_file_rename(
        snapshot,
        temp_dir.url().join("b.ts").unwrap(),
        temp_dir.url().join("🦕.ts").unwrap(),
        FormatCodeSettings::default(),
        UserPreferences::default(),
      )
      .await
      .unwrap();
    assert_eq!(
      changes,
      vec![FileTextChanges {
        file_name: temp_dir.url().join("a.ts").unwrap().to_string(),
        text_changes: vec![TextChange {
          span: TextSpan {
            start: 8,
            length: 6,
          },
          new_text: "./🦕.ts".to_string(),
        }],
        is_new_file: None,
      }]
    );
  }

  #[test]
  fn include_suppress_inlay_hint_settings() {
    let mut settings = WorkspaceSettings::default();
    settings
      .typescript
      .inlay_hints
      .parameter_names
      .suppress_when_argument_matches_name = true;
    settings
      .typescript
      .inlay_hints
      .variable_types
      .suppress_when_type_matches_name = true;
    let mut config = config::Config::default();
    config.set_workspace_settings(settings, vec![]);
    let user_preferences = UserPreferences::from_config_for_specifier(
      &config,
      &ModuleSpecifier::parse("file:///foo.ts").unwrap(),
    );
    assert_eq!(
      user_preferences.include_inlay_variable_type_hints_when_type_matches_name,
      Some(false)
    );
    assert_eq!(
      user_preferences
        .include_inlay_parameter_name_hints_when_argument_matches_name,
      Some(false)
    );
  }

  #[tokio::test]
  async fn resolve_unknown_dependency() {
    let (temp_dir, _, snapshot, _) = setup(
      json!({
        "target": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      &[("a.ts", "", 1, LanguageId::TypeScript)],
    )
    .await;
    let mut state = setup_op_state(snapshot);
    let resolved = op_resolve_inner(
      &mut state,
      ResolveArgs {
        base: temp_dir.url().join("a.ts").unwrap().to_string(),
        is_base_cjs: false,
        specifiers: vec!["./b.ts".to_string()],
      },
    )
    .unwrap();
    assert_eq!(
      resolved,
      vec![Some((
        temp_dir.url().join("b.ts").unwrap().to_string(),
        MediaType::TypeScript.as_ts_extension().to_string()
      ))]
    );
  }

  #[test]
  fn coalesce_pending_change() {
    use ChangeKind::*;
    fn change<S: AsRef<str>>(
      project_version: usize,
      scripts: impl IntoIterator<Item = (S, ChangeKind)>,
      new_configs_by_scope: Option<BTreeMap<ModuleSpecifier, Arc<LspTsConfig>>>,
    ) -> PendingChange {
      PendingChange {
        project_version,
        modified_scripts: scripts
          .into_iter()
          .map(|(s, c)| (s.as_ref().into(), c))
          .collect(),
        new_configs_by_scope,
      }
    }
    let cases = [
      (
        // start
        change(1, [("file:///a.ts", Closed)], None),
        // new
        change(2, Some(("file:///b.ts", Opened)), None),
        // expected
        change(
          2,
          [("file:///a.ts", Closed), ("file:///b.ts", Opened)],
          None,
        ),
      ),
      (
        // start
        change(
          1,
          [("file:///a.ts", Closed), ("file:///b.ts", Opened)],
          None,
        ),
        // new
        change(
          2,
          // a gets closed then reopened, b gets opened then closed
          [("file:///a.ts", Opened), ("file:///b.ts", Closed)],
          None,
        ),
        // expected
        change(
          2,
          [("file:///a.ts", Opened), ("file:///b.ts", Closed)],
          None,
        ),
      ),
      (
        change(
          1,
          [("file:///a.ts", Opened), ("file:///b.ts", Modified)],
          None,
        ),
        // new
        change(
          2,
          // a gets opened then modified, b gets modified then closed
          [("file:///a.ts", Opened), ("file:///b.ts", Closed)],
          None,
        ),
        // expected
        change(
          2,
          [("file:///a.ts", Opened), ("file:///b.ts", Closed)],
          None,
        ),
      ),
    ];

    for (start, new, expected) in cases {
      let mut pending = start;
      pending.coalesce(new.project_version, new.modified_scripts, None);
      assert_eq!(json!(pending), json!(expected));
    }
  }
}
