// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::thread;

use deno_ast::MediaType;
use deno_config::glob::FilePatterns;
use deno_config::workspace::WorkspaceDirLintConfig;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::unsync::spawn;
use deno_core::unsync::spawn_blocking;
use deno_core::unsync::JoinHandle;
use deno_core::url::Url;
use deno_core::ModuleSpecifier;
use deno_graph::source::ResolveError;
use deno_graph::Resolution;
use deno_graph::ResolutionError;
use deno_graph::SpecifierError;
use deno_lint::linter::LintConfig as DenoLintConfig;
use deno_resolver::workspace::sloppy_imports_resolve;
use deno_runtime::deno_node;
use deno_runtime::tokio_util::create_basic_runtime;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use import_map::ImportMap;
use import_map::ImportMapErrorKind;
use log::error;
use lsp_types::Uri;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::analysis;
use super::client::Client;
use super::config::Config;
use super::documents::Document;
use super::documents::DocumentModule;
use super::documents::DocumentModules;
use super::language_server;
use super::language_server::StateSnapshot;
use super::performance::Performance;
use super::tsc;
use super::tsc::MaybeAmbientModules;
use super::tsc::TsServer;
use crate::graph_util;
use crate::graph_util::enhanced_resolution_error_message;
use crate::lsp::logging::lsp_warn;
use crate::lsp::lsp_custom::DiagnosticBatchNotificationParams;
use crate::sys::CliSys;
use crate::tools::lint::CliLinter;
use crate::tools::lint::CliLinterOptions;
use crate::tools::lint::LintRuleProvider;
use crate::tsc::DiagnosticCategory;
use crate::util::path::to_percent_decoded_str;

pub type ScopedAmbientModules = HashMap<Option<Arc<Url>>, MaybeAmbientModules>;

#[derive(Debug)]
pub struct DiagnosticServerUpdateMessage {
  pub snapshot: Arc<StateSnapshot>,
}

#[derive(Debug)]
struct DiagnosticRecord {
  pub uri: Arc<Uri>,
  pub versioned: VersionedDiagnostics,
}

#[derive(Clone, Default, Debug)]
struct VersionedDiagnostics {
  pub version: i32,
  pub diagnostics: Vec<lsp::Diagnostic>,
}

type DiagnosticVec = Vec<DiagnosticRecord>;

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub enum DiagnosticSource {
  Deno,
  Lint,
  Ts,
  DeferredDeno,
}

impl DiagnosticSource {
  pub fn as_lsp_source(&self) -> &'static str {
    match self {
      Self::Deno => "deno",
      Self::Lint => "deno-lint",
      Self::Ts => "deno-ts",
      Self::DeferredDeno => "deno",
    }
  }
}

type DiagnosticsBySource = HashMap<DiagnosticSource, VersionedDiagnostics>;

#[derive(Debug)]
struct DiagnosticsPublisher {
  client: Client,
  state: Arc<DiagnosticsState>,
  diagnostics_by_uri: AsyncMutex<HashMap<Arc<Uri>, DiagnosticsBySource>>,
}

impl DiagnosticsPublisher {
  pub fn new(client: Client, state: Arc<DiagnosticsState>) -> Self {
    Self {
      client,
      state,
      diagnostics_by_uri: Default::default(),
    }
  }

  pub async fn publish(
    &self,
    source: DiagnosticSource,
    diagnostics: DiagnosticVec,
    token: &CancellationToken,
  ) -> usize {
    let mut diagnostics_by_uri = self.diagnostics_by_uri.lock().await;
    let mut seen_specifiers = HashSet::with_capacity(diagnostics.len());
    let mut messages_sent = 0;

    for record in diagnostics {
      if token.is_cancelled() {
        return messages_sent;
      }

      seen_specifiers.insert(record.uri.clone());

      let diagnostics_by_source =
        diagnostics_by_uri.entry(record.uri.clone()).or_default();
      let version = record.versioned.version;
      let source_diagnostics = diagnostics_by_source.entry(source).or_default();
      *source_diagnostics = record.versioned;

      // DO NOT filter these by version. We want to display even out
      // of date diagnostics in order to prevent flickering. The user's
      // lsp client will eventually catch up.
      let all_specifier_diagnostics = diagnostics_by_source
        .values()
        .flat_map(|d| &d.diagnostics)
        .cloned()
        .collect::<Vec<_>>();

      self
        .state
        .update(&record.uri, version, &all_specifier_diagnostics);
      self
        .client
        .publish_diagnostics(
          record.uri.as_ref().clone(),
          all_specifier_diagnostics,
          Some(version),
        )
        .await;
      messages_sent += 1;
    }

    // now check all the specifiers to clean up any ones with old diagnostics
    let mut uris_to_remove = Vec::new();
    for (uri, diagnostics_by_source) in diagnostics_by_uri.iter_mut() {
      if seen_specifiers.contains(uri) {
        continue;
      }
      if token.is_cancelled() {
        break;
      }
      let maybe_removed_value = diagnostics_by_source.remove(&source);
      if diagnostics_by_source.is_empty() {
        uris_to_remove.push(uri.clone());
        if let Some(removed_value) = maybe_removed_value {
          // clear out any diagnostics for this specifier
          self.state.update(uri, removed_value.version, &[]);
          self
            .client
            .publish_diagnostics(
              uri.as_ref().clone(),
              Vec::new(),
              Some(removed_value.version),
            )
            .await;
          messages_sent += 1;
        }
      }
    }

    // clean up specifiers with no diagnostics
    for specifier in uris_to_remove {
      diagnostics_by_uri.remove(&specifier);
    }

    messages_sent
  }

  pub async fn clear(&self) {
    let mut all_diagnostics = self.diagnostics_by_uri.lock().await;
    all_diagnostics.clear();
  }
}

type DiagnosticMap = HashMap<Arc<Uri>, VersionedDiagnostics>;

#[derive(Clone, Default, Debug)]
struct TsDiagnosticsStore(Arc<deno_core::parking_lot::Mutex<DiagnosticMap>>);

impl TsDiagnosticsStore {
  pub fn get(
    &self,
    uri: &Uri,
    document_version: Option<i32>,
  ) -> Vec<lsp::Diagnostic> {
    let ts_diagnostics = self.0.lock();
    if let Some(versioned) = ts_diagnostics.get(uri) {
      // only get the diagnostics if they're up to date
      if document_version == Some(versioned.version) {
        return versioned.diagnostics.clone();
      }
    }
    Vec::new()
  }

  pub fn invalidate(&self, uris: &[&Uri]) {
    let mut ts_diagnostics = self.0.lock();
    for uri in uris {
      ts_diagnostics.remove(*uri);
    }
  }

  pub fn invalidate_all(&self) {
    self.0.lock().clear();
  }

  fn update(&self, diagnostics: &DiagnosticVec) {
    let mut stored_ts_diagnostics = self.0.lock();
    *stored_ts_diagnostics = diagnostics
      .iter()
      .map(|record| (record.uri.clone(), record.versioned.clone()))
      .collect();
  }
}

pub fn should_send_diagnostic_batch_index_notifications() -> bool {
  deno_lib::args::has_flag_env_var(
    "DENO_DONT_USE_INTERNAL_LSP_DIAGNOSTIC_SYNC_FLAG",
  )
}

#[derive(Clone, Debug)]
struct DiagnosticBatchCounter(Option<Arc<AtomicUsize>>);

impl Default for DiagnosticBatchCounter {
  fn default() -> Self {
    if should_send_diagnostic_batch_index_notifications() {
      Self(Some(Default::default()))
    } else {
      Self(None)
    }
  }
}

impl DiagnosticBatchCounter {
  pub fn inc(&self) -> Option<usize> {
    self
      .0
      .as_ref()
      .map(|value| value.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1)
  }

  pub fn get(&self) -> Option<usize> {
    self
      .0
      .as_ref()
      .map(|value| value.load(std::sync::atomic::Ordering::SeqCst))
  }
}

#[derive(Debug)]
enum ChannelMessage {
  Update(ChannelUpdateMessage),
  Clear,
}

#[derive(Debug)]
struct ChannelUpdateMessage {
  message: DiagnosticServerUpdateMessage,
  batch_index: Option<usize>,
}

#[derive(Debug)]
struct DocumentDiagnosticsState {
  version: i32,
  no_cache_diagnostics: Vec<lsp::Diagnostic>,
}

#[derive(Debug, Default)]
pub struct DiagnosticsState {
  documents: RwLock<HashMap<Uri, DocumentDiagnosticsState>>,
}

impl DiagnosticsState {
  fn update(&self, uri: &Uri, version: i32, diagnostics: &[lsp::Diagnostic]) {
    let mut specifiers = self.documents.write();
    let current_version = specifiers.get(uri).map(|s| s.version);
    if let Some(current_version) = current_version {
      if version < current_version {
        return;
      }
    }
    let mut no_cache_diagnostics = vec![];
    for diagnostic in diagnostics {
      if diagnostic.code
        == Some(lsp::NumberOrString::String("no-cache".to_string()))
        || diagnostic.code
          == Some(lsp::NumberOrString::String("not-installed-jsr".to_string()))
        || diagnostic.code
          == Some(lsp::NumberOrString::String("not-installed-npm".to_string()))
      {
        no_cache_diagnostics.push(diagnostic.clone());
      }
    }
    specifiers.insert(
      uri.clone(),
      DocumentDiagnosticsState {
        version,
        no_cache_diagnostics,
      },
    );
  }

  pub fn clear(&self, uri: &Uri) {
    self.documents.write().remove(uri);
  }

  pub fn has_no_cache_diagnostics(&self, uri: &Uri) -> bool {
    self
      .documents
      .read()
      .get(uri)
      .map(|s| !s.no_cache_diagnostics.is_empty())
      .unwrap_or(false)
  }

  pub fn no_cache_diagnostics(&self, uri: &Uri) -> Vec<lsp::Diagnostic> {
    self
      .documents
      .read()
      .get(uri)
      .map(|s| s.no_cache_diagnostics.clone())
      .unwrap_or_default()
  }
}

#[derive(Debug, Default)]
struct AmbientModules {
  regex: Option<regex::Regex>,
  dirty: bool,
}

#[derive(Debug, Default)]
struct DeferredDiagnostics {
  diagnostics: Option<Vec<DeferredDiagnosticRecord>>,
  ambient_modules_by_scope: HashMap<Option<Arc<Url>>, AmbientModules>,
}

impl DeferredDiagnostics {
  fn invalidate(&mut self, uris: &[&Uri]) {
    if let Some(diagnostics) = &mut self.diagnostics {
      diagnostics.retain(|d| !uris.contains(&d.uri.as_ref()));
    }
    for ambient in self.ambient_modules_by_scope.values_mut() {
      ambient.dirty = true;
    }
  }

  fn invalidate_all(&mut self) {
    self.diagnostics = None;
    for ambient in self.ambient_modules_by_scope.values_mut() {
      ambient.dirty = true;
    }
  }

  fn take_filtered_diagnostics(&mut self) -> Option<DiagnosticVec> {
    let diagnostics = self.diagnostics.take()?;
    for diagnostic in &diagnostics {
      let Some(ambient) = self.ambient_modules_by_scope.get(&diagnostic.scope)
      else {
        self.diagnostics = Some(diagnostics);
        return None;
      };
      if ambient.dirty {
        self.diagnostics = Some(diagnostics);
        return None;
      }
    }

    Some(
      diagnostics
        .into_iter()
        .map(|diagnostic| {
          let ambient = self
            .ambient_modules_by_scope
            .get(&diagnostic.scope)
            .unwrap(); // checked above, but gross
          let filtered = if let Some(regex) = &ambient.regex {
            diagnostic
              .diagnostics
              .into_iter()
              .filter_map(|(import_url, diag)| {
                if regex.is_match(import_url.as_str()) {
                  None
                } else {
                  Some(diag)
                }
              })
              .collect()
          } else {
            diagnostic.diagnostics.into_iter().map(|d| d.1).collect()
          };
          DiagnosticRecord {
            uri: diagnostic.uri,
            versioned: VersionedDiagnostics {
              version: diagnostic.version,
              diagnostics: filtered,
            },
          }
        })
        .collect(),
    )
  }

  fn update_ambient_modules(&mut self, new: ScopedAmbientModules) {
    for (scope, value) in new {
      let ambient = self.ambient_modules_by_scope.entry(scope).or_default();
      ambient.dirty = false;
      if let Some(value) = value {
        if value.is_empty() {
          ambient.regex = None;
          continue;
        }
        let mut regex_string = String::with_capacity(value.len() * 8);
        regex_string.push('(');
        let last = value.len() - 1;
        for (idx, part) in value.into_iter().enumerate() {
          let trimmed = part.trim_matches('"');
          let escaped = regex::escape(trimmed);
          let regex = escaped.replace("\\*", ".*");
          regex_string.push_str(&regex);
          if idx != last {
            regex_string.push('|');
          }
        }
        regex_string.push_str(")$");
        if let Ok(regex) = regex::Regex::new(&regex_string).inspect_err(|e| {
          lsp_warn!("failed to compile ambient modules pattern: {e} (pattern is {regex_string:?})");
        }) {
          ambient.regex = Some(regex);
        } else {
          ambient.regex = None;
        }
      }
    }
  }
}

pub struct DiagnosticsServer {
  channel: Option<mpsc::UnboundedSender<ChannelMessage>>,
  ts_diagnostics: TsDiagnosticsStore,
  client: Client,
  performance: Arc<Performance>,
  ts_server: Arc<TsServer>,
  batch_counter: DiagnosticBatchCounter,
  state: Arc<DiagnosticsState>,
  deferred_diagnostics: Arc<deno_core::parking_lot::Mutex<DeferredDiagnostics>>,
}

impl std::fmt::Debug for DiagnosticsServer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("DiagnosticsServer")
      .field("channel", &self.channel)
      .field("ts_diagnostics", &self.ts_diagnostics)
      .field("client", &self.client)
      .field("performance", &self.performance)
      .field("ts_server", &self.ts_server)
      .field("batch_counter", &self.batch_counter)
      .field("state", &self.state)
      .field("deferred_diagnostics", &*self.deferred_diagnostics.lock())
      .finish()
  }
}

impl DiagnosticsServer {
  pub fn new(
    client: Client,
    performance: Arc<Performance>,
    ts_server: Arc<TsServer>,
    state: Arc<DiagnosticsState>,
  ) -> Self {
    DiagnosticsServer {
      channel: Default::default(),
      ts_diagnostics: Default::default(),
      client,
      performance,
      ts_server,
      batch_counter: Default::default(),
      state,
      deferred_diagnostics: Arc::new(
        Mutex::new(DeferredDiagnostics::default()),
      ),
    }
  }

  pub fn get_ts_diagnostics(
    &self,
    uri: &Uri,
    document_version: Option<i32>,
  ) -> Vec<lsp::Diagnostic> {
    self.ts_diagnostics.get(uri, document_version)
  }

  pub fn invalidate(&self, uris: &[&Uri]) {
    self.ts_diagnostics.invalidate(uris);
    self.deferred_diagnostics.lock().invalidate(uris);
  }

  pub fn invalidate_all(&self) {
    self.ts_diagnostics.invalidate_all();
    self.deferred_diagnostics.lock().invalidate_all();
    if let Some(tx) = &self.channel {
      let _ = tx.send(ChannelMessage::Clear);
    }
  }

  #[allow(unused_must_use)]
  pub fn start(&mut self) {
    let (tx, mut rx) = mpsc::unbounded_channel::<ChannelMessage>();
    self.channel = Some(tx);
    let client = self.client.clone();
    let state = self.state.clone();
    let performance = self.performance.clone();
    let ts_diagnostics_store = self.ts_diagnostics.clone();
    let ts_server = self.ts_server.clone();
    let deferred_diagnostics_state = self.deferred_diagnostics.clone();

    let _join_handle = thread::spawn(move || {
      let runtime = create_basic_runtime();

      runtime.block_on(async {
        let mut token = CancellationToken::new();
        let mut ts_handle: Option<JoinHandle<()>> = None;
        let mut lint_handle: Option<JoinHandle<()>> = None;
        let mut deps_handle: Option<JoinHandle<()>> = None;
        let diagnostics_publisher =
          Arc::new(DiagnosticsPublisher::new(client.clone(), state.clone()));

        loop {
          match rx.recv().await {
            // channel has closed
            None => break,
            Some(message) => {
              let message = match message {
                ChannelMessage::Update(message) => message,
                ChannelMessage::Clear => {
                  token.cancel();
                  token = CancellationToken::new();
                  diagnostics_publisher.clear().await;
                  continue;
                }
              };
              let ChannelUpdateMessage {
                message: DiagnosticServerUpdateMessage { snapshot },
                batch_index,
              } = message;

              // cancel the previous run
              token.cancel();
              token = CancellationToken::new();

              let previous_ts_handle = ts_handle.take();
              ts_handle = Some(spawn({
                let performance = performance.clone();
                let diagnostics_publisher = diagnostics_publisher.clone();
                let ts_server = ts_server.clone();
                let token = token.clone();
                let ts_diagnostics_store = ts_diagnostics_store.clone();
                let snapshot = snapshot.clone();
                let config = snapshot.config.clone();
                let deferred_diagnostics_state =
                  deferred_diagnostics_state.clone();
                async move {
                  if let Some(previous_handle) = previous_ts_handle {
                    // Wait on the previous run to complete in order to prevent
                    // multiple threads queueing up a lot of tsc requests.
                    // Do not race this with cancellation because we want a
                    // chain of events to wait for all the previous diagnostics to complete
                    previous_handle.await;
                  }

                  // Debounce timer delay. 150ms between keystrokes is about 45 WPM, so we
                  // want something that is longer than that, but not too long to
                  // introduce detectable UI delay; 200ms is a decent compromise.
                  const DELAY: Duration = Duration::from_millis(200);
                  tokio::select! {
                    _ = token.cancelled() => { return; }
                    _ = tokio::time::sleep(DELAY) => {}
                  };

                  let mark = performance.mark("lsp.update_diagnostics_ts");
                  let (diagnostics, ambient_modules_by_scope) =
                    generate_ts_diagnostics(
                      snapshot.clone(),
                      &config,
                      &ts_server,
                      token.clone(),
                    )
                    .await
                    .map_err(|err| {
                      if !token.is_cancelled() {
                        error!(
                          "Error generating TypeScript diagnostics: {}",
                          err
                        );
                        token.cancel();
                      }
                    })
                    .unwrap_or_default();
                  deferred_diagnostics_state
                    .lock()
                    .update_ambient_modules(ambient_modules_by_scope);
                  let mut messages_len = 0;
                  if !token.is_cancelled() {
                    ts_diagnostics_store.update(&diagnostics);
                    {
                      let value = {
                        let mut deferred_diagnostics_state =
                          deferred_diagnostics_state.lock();
                        deferred_diagnostics_state.take_filtered_diagnostics()
                      };
                      if let Some(deferred) = value {
                        messages_len += diagnostics_publisher
                          .publish(
                            DiagnosticSource::DeferredDeno,
                            deferred,
                            &token,
                          )
                          .await;
                      }
                    }
                    messages_len += diagnostics_publisher
                      .publish(DiagnosticSource::Ts, diagnostics, &token)
                      .await;

                    if !token.is_cancelled() {
                      performance.measure(mark);
                    }
                  }

                  if let Some(batch_index) = batch_index {
                    diagnostics_publisher
                      .client
                      .send_diagnostic_batch_notification(
                        DiagnosticBatchNotificationParams {
                          batch_index,
                          messages_len,
                        },
                      );
                  }
                }
              }));

              let previous_deps_handle = deps_handle.take();
              deps_handle = Some(spawn({
                let performance = performance.clone();
                let diagnostics_publisher = diagnostics_publisher.clone();
                let token = token.clone();
                let snapshot = snapshot.clone();
                let config = snapshot.config.clone();
                let deferred_diagnostics_state =
                  deferred_diagnostics_state.clone();
                async move {
                  if let Some(previous_handle) = previous_deps_handle {
                    previous_handle.await;
                  }
                  let mark = performance.mark("lsp.update_diagnostics_deps");
                  let (diagnostics, deferred) = spawn_blocking({
                    let token = token.clone();
                    let snapshot = snapshot.clone();
                    move || generate_deno_diagnostics(&snapshot, &config, token)
                  })
                  .await
                  .unwrap();

                  let mut messages_len = 0;
                  if !token.is_cancelled() {
                    {
                      let value = {
                        let mut deferred_diagnostics_state =
                          deferred_diagnostics_state.lock();
                        deferred_diagnostics_state.diagnostics = Some(deferred);
                        deferred_diagnostics_state.take_filtered_diagnostics()
                      };
                      if let Some(deferred) = value {
                        messages_len += diagnostics_publisher
                          .publish(
                            DiagnosticSource::DeferredDeno,
                            deferred,
                            &token,
                          )
                          .await;
                      }
                    }

                    messages_len += diagnostics_publisher
                      .publish(DiagnosticSource::Deno, diagnostics, &token)
                      .await;

                    if !token.is_cancelled() {
                      performance.measure(mark);
                    }
                  }

                  if let Some(batch_index) = batch_index {
                    diagnostics_publisher
                      .client
                      .send_diagnostic_batch_notification(
                        DiagnosticBatchNotificationParams {
                          batch_index,
                          messages_len,
                        },
                      );
                  }
                }
              }));

              let previous_lint_handle = lint_handle.take();
              lint_handle = Some(spawn({
                let performance = performance.clone();
                let diagnostics_publisher = diagnostics_publisher.clone();
                let token = token.clone();
                let snapshot = snapshot.clone();
                let config = snapshot.config.clone();
                async move {
                  if let Some(previous_handle) = previous_lint_handle {
                    previous_handle.await;
                  }
                  let mark = performance.mark("lsp.update_diagnostics_lint");
                  let diagnostics = spawn_blocking({
                    let token = token.clone();
                    let snapshot = snapshot.clone();
                    move || generate_lint_diagnostics(&snapshot, &config, token)
                  })
                  .await
                  .unwrap();

                  let mut messages_len = 0;
                  if !token.is_cancelled() {
                    messages_len = diagnostics_publisher
                      .publish(DiagnosticSource::Lint, diagnostics, &token)
                      .await;

                    if !token.is_cancelled() {
                      performance.measure(mark);
                    }
                  }

                  if let Some(batch_index) = batch_index {
                    diagnostics_publisher
                      .client
                      .send_diagnostic_batch_notification(
                        DiagnosticBatchNotificationParams {
                          batch_index,
                          messages_len,
                        },
                      );
                  }
                }
              }));
            }
          }
        }
      })
    });
  }

  pub fn latest_batch_index(&self) -> Option<usize> {
    self.batch_counter.get()
  }

  pub fn update(
    &self,
    message: DiagnosticServerUpdateMessage,
  ) -> Result<(), AnyError> {
    // todo(dsherret): instead of queuing up messages, it would be better to
    // instead only store the latest message (ex. maybe using a
    // tokio::sync::watch::channel)
    if let Some(tx) = &self.channel {
      tx.send(ChannelMessage::Update(ChannelUpdateMessage {
        message,
        batch_index: self.batch_counter.inc(),
      }))
      .map_err(|err| err.into())
    } else {
      Err(anyhow!("diagnostics server not started"))
    }
  }
}

impl<'a> From<&'a crate::tsc::DiagnosticCategory> for lsp::DiagnosticSeverity {
  fn from(category: &'a crate::tsc::DiagnosticCategory) -> Self {
    match category {
      crate::tsc::DiagnosticCategory::Error => lsp::DiagnosticSeverity::ERROR,
      crate::tsc::DiagnosticCategory::Warning => {
        lsp::DiagnosticSeverity::WARNING
      }
      crate::tsc::DiagnosticCategory::Suggestion => {
        lsp::DiagnosticSeverity::HINT
      }
      crate::tsc::DiagnosticCategory::Message => {
        lsp::DiagnosticSeverity::INFORMATION
      }
    }
  }
}

impl<'a> From<&'a crate::tsc::Position> for lsp::Position {
  fn from(pos: &'a crate::tsc::Position) -> Self {
    Self {
      line: pos.line as u32,
      character: pos.character as u32,
    }
  }
}

fn get_diagnostic_message(diagnostic: &crate::tsc::Diagnostic) -> String {
  if let Some(message) = diagnostic.message_text.clone() {
    message
  } else if let Some(message_chain) = diagnostic.message_chain.clone() {
    message_chain.format_message(0)
  } else {
    "[missing message]".to_string()
  }
}

fn to_lsp_range(
  start: &crate::tsc::Position,
  end: &crate::tsc::Position,
) -> lsp::Range {
  lsp::Range {
    start: start.into(),
    end: end.into(),
  }
}

fn to_lsp_related_information(
  related_information: &Option<Vec<crate::tsc::Diagnostic>>,
  module: &DocumentModule,
  document_modules: &DocumentModules,
) -> Option<Vec<lsp::DiagnosticRelatedInformation>> {
  related_information.as_ref().map(|related| {
    related
      .iter()
      .filter_map(|ri| {
        if let (Some(file_name), Some(start), Some(end)) =
          (&ri.file_name, &ri.start, &ri.end)
        {
          let uri = resolve_url(file_name)
            .ok()
            .and_then(|s| {
              document_modules.module_for_specifier(&s, module.scope.as_deref())
            })
            .map(|m| m.uri.as_ref().clone())
            .unwrap_or_else(|| Uri::from_str("unknown:").unwrap());
          Some(lsp::DiagnosticRelatedInformation {
            location: lsp::Location {
              uri,
              range: to_lsp_range(start, end),
            },
            message: get_diagnostic_message(ri),
          })
        } else {
          None
        }
      })
      .collect()
  })
}

fn ts_json_to_diagnostics(
  diagnostics: Vec<crate::tsc::Diagnostic>,
  module: &DocumentModule,
  document_modules: &DocumentModules,
) -> Vec<lsp::Diagnostic> {
  diagnostics
    .iter()
    .filter_map(|d| {
      if let (Some(start), Some(end)) = (&d.start, &d.end) {
        Some(lsp::Diagnostic {
          range: to_lsp_range(start, end),
          severity: Some((&d.category).into()),
          code: Some(lsp::NumberOrString::Number(d.code as i32)),
          code_description: None,
          source: Some(DiagnosticSource::Ts.as_lsp_source().to_string()),
          message: get_diagnostic_message(d),
          related_information: to_lsp_related_information(
            &d.related_information,
            module,
            document_modules,
          ),
          tags: match d.code {
            // These are codes that indicate the variable is unused.
            2695 | 6133 | 6138 | 6192 | 6196 | 6198 | 6199 | 6205 | 7027
            | 7028 => Some(vec![lsp::DiagnosticTag::UNNECESSARY]),
            // These are codes that indicated the variable is deprecated.
            2789 | 6385 | 6387 => Some(vec![lsp::DiagnosticTag::DEPRECATED]),
            _ => None,
          },
          data: None,
        })
      } else {
        None
      }
    })
    .collect()
}

fn generate_lint_diagnostics(
  snapshot: &language_server::StateSnapshot,
  config: &Config,
  token: CancellationToken,
) -> DiagnosticVec {
  let config_data_by_scope = config.tree.data_by_scope();
  let mut records = Vec::new();
  for document in snapshot.document_modules.documents.open_docs() {
    // TODO(nayeemrmn): Support linting notebooks cells. Will require stitching
    // cells from the same notebook into one module, linting it and then
    // splitting/relocating the diagnostics to each cell.
    if document.notebook_uri.is_some() {
      continue;
    }
    let Some(module) = snapshot
      .document_modules
      .primary_module(&Document::Open(document.clone()))
    else {
      continue;
    };
    if module.specifier.scheme() != "file" {
      continue;
    }
    if !config.specifier_enabled(&module.specifier) {
      continue;
    }
    let settings = config.workspace_settings_for_specifier(&module.specifier);
    if !settings.lint {
      continue;
    }
    // exit early if cancelled
    if token.is_cancelled() {
      break;
    }
    // ignore any npm package files
    if snapshot.resolver.in_node_modules(&module.specifier) {
      continue;
    }
    let (lint_config, linter) = module
      .scope
      .as_ref()
      .and_then(|s| config_data_by_scope.get(s))
      .map(|d| (d.lint_config.clone(), d.linter.clone()))
      .unwrap_or_else(|| {
        (
          Arc::new(WorkspaceDirLintConfig {
            rules: Default::default(),
            plugins: Default::default(),
            files: FilePatterns::new_with_base(PathBuf::from("/")),
          }),
          Arc::new(CliLinter::new(CliLinterOptions {
            configured_rules: {
              let lint_rule_provider = LintRuleProvider::new(None);
              lint_rule_provider.resolve_lint_rules(Default::default(), None)
            },
            fix: false,
            deno_lint_config: DenoLintConfig {
              default_jsx_factory: None,
              default_jsx_fragment_factory: None,
            },
            // TODO(bartlomieju): handle linter plugins here before landing
            maybe_plugin_runner: None,
          })),
        )
      });
    records.push(DiagnosticRecord {
      uri: document.uri.clone(),
      versioned: VersionedDiagnostics {
        version: document.version,
        diagnostics: generate_document_lint_diagnostics(
          &module,
          &lint_config,
          &linter,
          token.clone(),
        ),
      },
    });
  }
  records
}

fn generate_document_lint_diagnostics(
  module: &DocumentModule,
  lint_config: &WorkspaceDirLintConfig,
  linter: &CliLinter,
  token: CancellationToken,
) -> Vec<lsp::Diagnostic> {
  if !module.is_diagnosable()
    || !lint_config.files.matches_specifier(&module.specifier)
  {
    return Vec::new();
  }
  match &module
    .open_data
    .as_ref()
    .and_then(|d| d.parsed_source.as_ref())
  {
    Some(Ok(parsed_source)) => {
      if let Ok(references) =
        analysis::get_lint_references(parsed_source, linter, token)
      {
        references
          .into_iter()
          .map(|r| r.to_diagnostic())
          .collect::<Vec<_>>()
      } else {
        Vec::new()
      }
    }
    Some(Err(_)) => Vec::new(),
    None => {
      error!("Missing file contents for: {}", &module.specifier);
      Vec::new()
    }
  }
}

async fn generate_ts_diagnostics(
  snapshot: Arc<language_server::StateSnapshot>,
  config: &Config,
  ts_server: &tsc::TsServer,
  token: CancellationToken,
) -> Result<(DiagnosticVec, ScopedAmbientModules), AnyError> {
  let mut records = Vec::new();
  let mut ambient_modules_by_scope = HashMap::new();
  let mut enabled_modules_by_scope = BTreeMap::<_, Vec<_>>::new();
  let mut disabled_documents = Vec::new();
  for document in snapshot.document_modules.documents.open_docs() {
    if document.is_diagnosable() {
      if let Some(module) = snapshot
        .document_modules
        .primary_module(&Document::Open(document.clone()))
      {
        if config.specifier_enabled(&module.specifier) {
          enabled_modules_by_scope
            .entry((module.scope.clone(), module.notebook_uri.clone()))
            .or_default()
            .push(module);
          continue;
        }
      }
    }
    disabled_documents.push(document.clone());
  }
  // add an empty diagnostic publish for disabled documents in order
  // to clear those diagnostics if they exist
  for document in disabled_documents {
    records.push(DiagnosticRecord {
      uri: document.uri.clone(),
      versioned: VersionedDiagnostics {
        version: document.version,
        diagnostics: Vec::new(),
      },
    });
  }
  let mut enabled_modules_with_diagnostics = Vec::new();
  for ((scope, notebook_uri), enabled_modules) in enabled_modules_by_scope {
    let (diagnostics_list, ambient_modules) = ts_server
      .get_diagnostics(
        snapshot.clone(),
        enabled_modules.iter().map(|m| m.specifier.as_ref()),
        scope.as_ref(),
        notebook_uri.as_ref(),
        &token,
      )
      .await?;
    enabled_modules_with_diagnostics
      .extend(enabled_modules.into_iter().zip(diagnostics_list));
    if notebook_uri.is_none() {
      ambient_modules_by_scope.insert(scope, ambient_modules);
    }
  }
  for (module, mut diagnostics) in enabled_modules_with_diagnostics {
    let suggestion_actions_settings = snapshot
      .config
      .language_settings_for_specifier(&module.specifier)
      .map(|s| s.suggestion_actions.clone())
      .unwrap_or_default();
    if !suggestion_actions_settings.enabled {
      diagnostics.retain(|d| {
        d.category != DiagnosticCategory::Suggestion
          // Still show deprecated and unused diagnostics.
          // https://github.com/microsoft/vscode/blob/ce50bd4876af457f64d83cfd956bc916535285f4/extensions/typescript-language-features/src/languageFeatures/diagnostics.ts#L113-L114
          || d.reports_deprecated == Some(true)
          || d.reports_unnecessary == Some(true)
      });
    }
    let diagnostics =
      ts_json_to_diagnostics(diagnostics, &module, &snapshot.document_modules);
    records.push(DiagnosticRecord {
      uri: module.uri.clone(),
      versioned: VersionedDiagnostics {
        version: module.open_data.as_ref().unwrap().version,
        diagnostics,
      },
    });
  }
  Ok((records, ambient_modules_by_scope))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDataSpecifier {
  pub specifier: ModuleSpecifier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataStrSpecifier {
  pub specifier: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataRedirect {
  pub redirect: ModuleSpecifier,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataNoLocal {
  pub to: ModuleSpecifier,
  pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDataImportMapRemap {
  pub from: String,
  pub to: String,
}

/// An enum which represents diagnostic errors which originate from Deno itself.
pub enum DenoDiagnostic {
  /// A `x-deno-warning` is associated with the specifier and should be displayed
  /// as a warning to the user.
  DenoWarn(String),
  /// An informational diagnostic that indicates an existing specifier can be
  /// remapped to an import map import specifier.
  ImportMapRemap { from: String, to: String },
  /// The import assertion type is incorrect.
  InvalidAttributeType(String),
  /// A module requires an attribute type to be a valid import.
  NoAttributeType,
  /// A remote module was not found in the cache.
  NoCache(ModuleSpecifier),
  /// A jsr package reference was not found in the cache.
  NotInstalledJsr(PackageReq, ModuleSpecifier),
  /// An npm package reference was not found in the cache.
  NotInstalledNpm(PackageReq, ModuleSpecifier),
  /// An npm package reference was not exported by its package.
  NoExportNpm(NpmPackageReqReference),
  /// A local module was not found on the local file system.
  NoLocal(ModuleSpecifier),
  /// An error occurred when resolving the specifier string.
  ResolutionError(deno_graph::ResolutionError),
  /// Invalid `node:` specifier.
  InvalidNodeSpecifier(ModuleSpecifier),
  /// Bare specifier is used for `node:` specifier
  BareNodeSpecifier(String),
}

impl DenoDiagnostic {
  fn code(&self) -> &str {
    match self {
      Self::DenoWarn(_) => "deno-warn",
      Self::ImportMapRemap { .. } => "import-map-remap",
      Self::InvalidAttributeType(_) => "invalid-attribute-type",
      Self::NoAttributeType => "no-attribute-type",
      Self::NoCache(_) => "no-cache",
      Self::NotInstalledJsr(_, _) => "not-installed-jsr",
      Self::NotInstalledNpm(_, _) => "not-installed-npm",
      Self::NoExportNpm(_) => "no-export-npm",
      Self::NoLocal(_) => "no-local",
      Self::ResolutionError(err) => {
        if graph_util::get_resolution_error_bare_node_specifier(err).is_some() {
          "import-node-prefix-missing"
        } else {
          match err {
            ResolutionError::InvalidDowngrade { .. } => "invalid-downgrade",
            ResolutionError::InvalidJsrHttpsTypesImport { .. } => {
              "invalid-jsr-https-types-import"
            }
            ResolutionError::InvalidLocalImport { .. } => {
              "invalid-local-import"
            }
            ResolutionError::InvalidSpecifier { error, .. } => match error {
              SpecifierError::ImportPrefixMissing { .. } => {
                "import-prefix-missing"
              }
              SpecifierError::InvalidUrl(_) => "invalid-url",
            },
            ResolutionError::ResolverError { .. } => "resolver-error",
          }
        }
      }
      Self::InvalidNodeSpecifier(_) => "resolver-error",
      Self::BareNodeSpecifier(_) => "import-node-prefix-missing",
    }
  }

  /// A "static" method which for a diagnostic that originated from the
  /// structure returns a code action which can resolve the diagnostic.
  pub fn get_code_action(
    uri: &Uri,
    specifier: &ModuleSpecifier,
    diagnostic: &lsp::Diagnostic,
  ) -> Result<lsp::CodeAction, AnyError> {
    if let Some(lsp::NumberOrString::String(code)) = &diagnostic.code {
      let code_action = match code.as_str() {
        "import-map-remap" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let DiagnosticDataImportMapRemap { from, to } =
            serde_json::from_value(data)?;
          lsp::CodeAction {
            title: format!("Update \"{from}\" to \"{to}\" to use import map."),
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(lsp::WorkspaceEdit {
              changes: Some(HashMap::from([(
                uri.clone(),
                vec![lsp::TextEdit {
                  new_text: format!("\"{to}\""),
                  range: diagnostic.range,
                }],
              )])),
              ..Default::default()
            }),
            ..Default::default()
          }
        }
        "no-attribute-type" => lsp::CodeAction {
          title: "Insert import attribute.".to_string(),
          kind: Some(lsp::CodeActionKind::QUICKFIX),
          diagnostics: Some(vec![diagnostic.clone()]),
          edit: Some(lsp::WorkspaceEdit {
            changes: Some(HashMap::from([(
              uri.clone(),
              vec![lsp::TextEdit {
                new_text: " with { type: \"json\" }".to_string(),
                range: lsp::Range {
                  start: diagnostic.range.end,
                  end: diagnostic.range.end,
                },
              }],
            )])),
            ..Default::default()
          }),
          ..Default::default()
        },
        "no-cache" | "not-installed-jsr" | "not-installed-npm" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataSpecifier = serde_json::from_value(data)?;
          let title = if matches!(
            code.as_str(),
            "not-installed-jsr" | "not-installed-npm"
          ) {
            format!("Install \"{}\" and its dependencies.", data.specifier)
          } else {
            format!("Cache \"{}\" and its dependencies.", data.specifier)
          };
          lsp::CodeAction {
            title,
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            command: Some(lsp::Command {
              title: "".to_string(),
              command: "deno.cache".to_string(),
              arguments: Some(vec![json!([data.specifier]), json!(&specifier)]),
            }),
            ..Default::default()
          }
        }
        "no-local" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataNoLocal = serde_json::from_value(data)?;
          lsp::CodeAction {
            title: data.message,
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(lsp::WorkspaceEdit {
              changes: Some(HashMap::from([(
                uri.clone(),
                vec![lsp::TextEdit {
                  new_text: format!(
                    "\"{}\"",
                    relative_specifier(&data.to, specifier)
                  ),
                  range: diagnostic.range,
                }],
              )])),
              ..Default::default()
            }),
            ..Default::default()
          }
        }
        "redirect" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataRedirect = serde_json::from_value(data)?;
          lsp::CodeAction {
            title: "Update specifier to its redirected specifier.".to_string(),
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(lsp::WorkspaceEdit {
              changes: Some(HashMap::from([(
                uri.clone(),
                vec![lsp::TextEdit {
                  new_text: format!(
                    "\"{}\"",
                    specifier_text_for_redirected(&data.redirect, specifier)
                  ),
                  range: diagnostic.range,
                }],
              )])),
              ..Default::default()
            }),
            ..Default::default()
          }
        }
        "import-node-prefix-missing" => {
          let data = diagnostic
            .data
            .clone()
            .ok_or_else(|| anyhow!("Diagnostic is missing data"))?;
          let data: DiagnosticDataStrSpecifier = serde_json::from_value(data)?;
          lsp::CodeAction {
            title: format!("Update specifier to node:{}", data.specifier),
            kind: Some(lsp::CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(lsp::WorkspaceEdit {
              changes: Some(HashMap::from([(
                uri.clone(),
                vec![lsp::TextEdit {
                  new_text: format!("\"node:{}\"", data.specifier),
                  range: diagnostic.range,
                }],
              )])),
              ..Default::default()
            }),
            ..Default::default()
          }
        }
        _ => {
          return Err(anyhow!(
            "Unsupported diagnostic code (\"{}\") provided.",
            code
          ))
        }
      };
      Ok(code_action)
    } else {
      Err(anyhow!("Unsupported diagnostic code provided."))
    }
  }

  /// Given a reference to the code from an LSP diagnostic, determine if the
  /// diagnostic is fixable or not
  pub fn is_fixable(diagnostic: &lsp_types::Diagnostic) -> bool {
    if let Some(lsp::NumberOrString::String(code)) = &diagnostic.code {
      match code.as_str() {
        "import-map-remap"
        | "no-cache"
        | "not-installed-jsr"
        | "not-installed-npm"
        | "no-attribute-type"
        | "redirect"
        | "import-node-prefix-missing" => true,
        "no-local" => diagnostic.data.is_some(),
        _ => false,
      }
    } else {
      false
    }
  }

  /// Convert to an lsp Diagnostic when the range the diagnostic applies to is
  /// provided.
  pub fn to_lsp_diagnostic(&self, range: &lsp::Range) -> lsp::Diagnostic {
    fn no_local_message(
      specifier: &ModuleSpecifier,
      suggestion_message: Option<String>,
    ) -> String {
      let mut message = format!(
        "Unable to load a local module: {}\n",
        to_percent_decoded_str(specifier.as_ref())
      );
      if let Some(suggestion_message) = suggestion_message {
        message.push_str(&suggestion_message);
        message.push('.');
      } else {
        message.push_str("Please check the file path.");
      }
      message
    }

    let (severity, message, data) = match self {
      Self::DenoWarn(message) => (lsp::DiagnosticSeverity::WARNING, message.to_string(), None),
      Self::ImportMapRemap { from, to } => (lsp::DiagnosticSeverity::HINT, format!("The import specifier can be remapped to \"{to}\" which will resolve it via the active import map."), Some(json!({ "from": from, "to": to }))),
      Self::InvalidAttributeType(assert_type) => (lsp::DiagnosticSeverity::ERROR, format!("The module is a JSON module and expected an attribute type of \"json\". Instead got \"{assert_type}\"."), None),
      Self::NoAttributeType => (lsp::DiagnosticSeverity::ERROR, "The module is a JSON module and not being imported with an import attribute. Consider adding `with { type: \"json\" }` to the import statement.".to_string(), None),
      Self::NoCache(specifier) => (lsp::DiagnosticSeverity::ERROR, format!("Uncached or missing remote URL: {specifier}"), Some(json!({ "specifier": specifier }))),
      Self::NotInstalledJsr(pkg_req, specifier) => (lsp::DiagnosticSeverity::ERROR, format!("JSR package \"{pkg_req}\" is not installed or doesn't exist."), Some(json!({ "specifier": specifier }))),
      Self::NotInstalledNpm(pkg_req, specifier) => (lsp::DiagnosticSeverity::ERROR, format!("npm package \"{pkg_req}\" is not installed or doesn't exist."), Some(json!({ "specifier": specifier }))),
      Self::NoExportNpm(pkg_ref) => (lsp::DiagnosticSeverity::ERROR, format!("NPM package \"{}\" does not define an export \"{}\".", pkg_ref.req(), pkg_ref.sub_path().unwrap_or(".")), None),
      Self::NoLocal(specifier) => {
        let sloppy_resolution = sloppy_imports_resolve(specifier, deno_resolver::workspace::ResolutionKind::Execution, CliSys::default());
        let data = sloppy_resolution.as_ref().map(|(resolved, sloppy_reason)| {
          json!({
            "specifier": specifier,
            "to": resolved,
            "message": sloppy_reason.quick_fix_message_for_specifier(resolved),
          })
        });
        (lsp::DiagnosticSeverity::ERROR, no_local_message(specifier, sloppy_resolution.as_ref().map(|(resolved, sloppy_reason)| sloppy_reason.suggestion_message_for_specifier(resolved))), data)
      },
      Self::ResolutionError(err) => {
        let mut message;
        message = enhanced_resolution_error_message(err);
        if let deno_graph::ResolutionError::ResolverError {error, ..} = err{
          if let ResolveError::ImportMap(importmap) = (*error).as_ref() {
            if let ImportMapErrorKind::UnmappedBareSpecifier(specifier, _) = &**importmap {
              if specifier.chars().next().unwrap_or('\0') == '@'{
                let hint = format!("\nHint: Use [deno add {}] to add the dependency.", specifier);
                message.push_str(hint.as_str());
              }
            }
          }
        }
        (
        lsp::DiagnosticSeverity::ERROR,
        message,
        graph_util::get_resolution_error_bare_node_specifier(err)
          .map(|specifier| json!({ "specifier": specifier }))
      )},
      Self::InvalidNodeSpecifier(specifier) => (lsp::DiagnosticSeverity::ERROR, format!("Unknown Node built-in module: {}", specifier.path()), None),
      Self::BareNodeSpecifier(specifier) => (lsp::DiagnosticSeverity::WARNING, format!("\"{}\" is resolved to \"node:{}\". If you want to use a built-in Node module, add a \"node:\" prefix.", specifier, specifier), Some(json!({ "specifier": specifier }))),
    };
    lsp::Diagnostic {
      range: *range,
      severity: Some(severity),
      code: Some(lsp::NumberOrString::String(self.code().to_string())),
      source: Some(DiagnosticSource::Deno.as_lsp_source().to_string()),
      message,
      data,
      ..Default::default()
    }
  }
}

fn specifier_text_for_redirected(redirect: &Url, referrer: &Url) -> String {
  if redirect.scheme() == "file" && referrer.scheme() == "file" {
    // use a relative specifier when it's going to a file url
    relative_specifier(redirect, referrer)
  } else {
    redirect.to_string()
  }
}

fn relative_specifier(specifier: &Url, referrer: &Url) -> String {
  match referrer.make_relative(specifier) {
    Some(relative) => {
      if relative.starts_with('.') {
        relative
      } else {
        format!("./{}", relative)
      }
    }
    None => specifier.to_string(),
  }
}

fn maybe_ambient_import_specifier(
  diagnostic: &DenoDiagnostic,
) -> Option<String> {
  match diagnostic {
    DenoDiagnostic::NoCache(url) | DenoDiagnostic::NoLocal(url) => {
      Some(url.to_string())
    }
    DenoDiagnostic::ResolutionError(err) => {
      maybe_ambient_specifier_resolution_err(err)
    }
    _ => None,
  }
}

fn maybe_ambient_specifier_resolution_err(
  err: &ResolutionError,
) -> Option<String> {
  match err {
    ResolutionError::InvalidDowngrade { .. }
    | ResolutionError::InvalidJsrHttpsTypesImport { .. }
    | ResolutionError::InvalidLocalImport { .. } => None,
    ResolutionError::InvalidSpecifier { error, .. } => match error {
      SpecifierError::InvalidUrl(..) => None,
      SpecifierError::ImportPrefixMissing { specifier, .. } => {
        Some(specifier.to_string())
      }
    },
    ResolutionError::ResolverError { error, .. } => match &**error {
      ResolveError::Specifier(specifier_error) => match specifier_error {
        SpecifierError::InvalidUrl(..) => None,
        SpecifierError::ImportPrefixMissing { specifier, .. } => {
          Some(specifier.to_string())
        }
      },
      ResolveError::ImportMap(import_map_error) => {
        match import_map_error.as_kind() {
          ImportMapErrorKind::UnmappedBareSpecifier(spec, _) => {
            Some(spec.clone())
          }
          ImportMapErrorKind::JsonParse(_)
          | ImportMapErrorKind::ImportMapNotObject
          | ImportMapErrorKind::ImportsFieldNotObject
          | ImportMapErrorKind::ScopesFieldNotObject
          | ImportMapErrorKind::ScopePrefixNotObject(_)
          | ImportMapErrorKind::BlockedByNullEntry(_)
          | ImportMapErrorKind::SpecifierResolutionFailure { .. }
          | ImportMapErrorKind::SpecifierBacktracksAbovePrefix { .. } => None,
        }
      }
      ResolveError::Other(..) => None,
    },
  }
}

fn diagnose_resolution(
  snapshot: &language_server::StateSnapshot,
  dependency_key: &str,
  resolution: &Resolution,
  is_dynamic: bool,
  maybe_assert_type: Option<&str>,
  referrer_module: &DocumentModule,
  import_map: Option<&ImportMap>,
) -> (Vec<DenoDiagnostic>, Vec<DenoDiagnostic>) {
  let mut diagnostics = vec![];
  let mut deferred_diagnostics = vec![];
  match resolution {
    Resolution::Ok(resolved) => {
      let specifier = &resolved.specifier;
      let scoped_resolver = snapshot
        .resolver
        .get_scoped_resolver(referrer_module.scope.as_deref());
      let managed_npm_resolver =
        scoped_resolver.as_maybe_managed_npm_resolver();
      for (_, headers) in scoped_resolver.redirect_chain_headers(specifier) {
        if let Some(message) = headers.get("x-deno-warning") {
          diagnostics.push(DenoDiagnostic::DenoWarn(message.clone()));
        }
      }
      if let Some(module) = snapshot
        .document_modules
        .module_for_specifier(specifier, referrer_module.scope.as_deref())
      {
        if let Some(headers) = &module.headers {
          if let Some(message) = headers.get("x-deno-warning") {
            diagnostics.push(DenoDiagnostic::DenoWarn(message.clone()));
          }
        }
        if module.media_type == MediaType::Json {
          match maybe_assert_type {
            // The module has the correct assertion type, no diagnostic
            Some("json") => (),
            // The dynamic import statement is missing an attribute type, which
            // we might not be able to statically detect, therefore we will
            // not provide a potentially incorrect diagnostic.
            None if is_dynamic => (),
            // The module has an incorrect assertion type, diagnostic
            Some(assert_type) => diagnostics.push(
              DenoDiagnostic::InvalidAttributeType(assert_type.to_string()),
            ),
            // The module is missing an attribute type, diagnostic
            None => diagnostics.push(DenoDiagnostic::NoAttributeType),
          }
        }
      } else if let Ok(pkg_ref) =
        JsrPackageReqReference::from_specifier(specifier)
      {
        let req = pkg_ref.into_inner().req;
        diagnostics
          .push(DenoDiagnostic::NotInstalledJsr(req, specifier.clone()));
      } else if let Ok(pkg_ref) =
        NpmPackageReqReference::from_specifier(specifier)
      {
        if let Some(npm_resolver) = managed_npm_resolver {
          // show diagnostics for npm package references that aren't cached
          let req = pkg_ref.req();
          if !npm_resolver.is_pkg_req_folder_cached(req) {
            diagnostics.push(DenoDiagnostic::NotInstalledNpm(
              req.clone(),
              specifier.clone(),
            ));
          } else if scoped_resolver
            .npm_to_file_url(
              &pkg_ref,
              &referrer_module.specifier,
              referrer_module.resolution_mode,
            )
            .is_none()
          {
            diagnostics.push(DenoDiagnostic::NoExportNpm(pkg_ref.clone()));
          }
        }
      } else if let Some(module_name) = specifier.as_str().strip_prefix("node:")
      {
        if !deno_node::is_builtin_node_module(module_name) {
          diagnostics
            .push(DenoDiagnostic::InvalidNodeSpecifier(specifier.clone()));
        } else if module_name == dependency_key {
          let mut is_mapped = false;
          if let Some(import_map) = import_map {
            if let Resolution::Ok(resolved) = &resolution {
              if import_map.resolve(module_name, &resolved.specifier).is_ok() {
                is_mapped = true;
              }
            }
          }
          // show diagnostics for bare node specifiers that aren't mapped by import map
          if !is_mapped {
            diagnostics
              .push(DenoDiagnostic::BareNodeSpecifier(module_name.to_string()));
          }
        } else if let Some(npm_resolver) = managed_npm_resolver {
          // check that a @types/node package exists in the resolver
          let types_node_req = PackageReq::from_str("@types/node").unwrap();
          if !npm_resolver.is_pkg_req_folder_cached(&types_node_req) {
            diagnostics.push(DenoDiagnostic::NotInstalledNpm(
              types_node_req,
              ModuleSpecifier::parse("npm:@types/node").unwrap(),
            ));
          }
        }
      } else {
        // When the document is not available, it means that it cannot be found
        // in the cache or locally on the disk, so we want to issue a diagnostic
        // about that.
        // these may be invalid, however, if this is an ambient module with
        // no real source (as in the case of a virtual module).
        let deno_diagnostic = match specifier.scheme() {
          "file" => DenoDiagnostic::NoLocal(specifier.clone()),
          _ => DenoDiagnostic::NoCache(specifier.clone()),
        };
        deferred_diagnostics.push(deno_diagnostic);
      }
    }
    // The specifier resolution resulted in an error, so we want to issue a
    // diagnostic for that.
    Resolution::Err(err) => {
      if maybe_ambient_specifier_resolution_err(err).is_none() {
        diagnostics.push(DenoDiagnostic::ResolutionError(*err.clone()))
      } else {
        deferred_diagnostics
          .push(DenoDiagnostic::ResolutionError(*err.clone()));
      }
    }
    _ => (),
  }
  (diagnostics, deferred_diagnostics)
}

/// Generate diagnostics related to a dependency. The dependency is analyzed to
/// determine if it can be remapped to the active import map as well as surface
/// any diagnostics related to the resolved code or type dependency.
fn diagnose_dependency(
  diagnostics: &mut Vec<lsp::Diagnostic>,
  deferred_diagnostics: &mut Vec<(String, lsp::Diagnostic)>,
  snapshot: &language_server::StateSnapshot,
  referrer_module: &DocumentModule,
  dependency_key: &str,
  dependency: &deno_graph::Dependency,
) {
  /// Given a specifier and a referring specifier, determine if a value in the
  /// import map could be used as an import specifier that resolves using the
  /// import map.
  ///
  /// This was inlined from the import_map crate in order to ignore more
  /// entries.
  fn import_map_lookup(
    import_map: &ImportMap,
    specifier: &Url,
    referrer: &Url,
  ) -> Option<String> {
    let specifier_str = specifier.as_str();
    for entry in import_map.entries_for_referrer(referrer) {
      if let Some(address) = entry.value {
        let address_str = address.as_str();
        if referrer.as_str().starts_with(address_str) {
          // ignore when the referrer has a common base with the
          // import map entry (ex. `./src/a.ts` importing `./src/b.ts`
          // and there's a `"$src/": "./src/"` import map entry)
          continue;
        }
        if address_str == specifier_str {
          return Some(entry.raw_key.to_string());
        }
        if address_str.ends_with('/') && specifier_str.starts_with(address_str)
        {
          return Some(specifier_str.replace(address_str, entry.raw_key));
        }
      }
    }
    None
  }

  if snapshot
    .resolver
    .in_node_modules(&referrer_module.specifier)
  {
    return; // ignore, surface typescript errors instead
  }

  let config_data = referrer_module
    .scope
    .as_ref()
    .and_then(|s| snapshot.config.tree.data_for_specifier(s));
  let import_map = config_data.and_then(|d| d.resolver.maybe_import_map());
  if let Some(import_map) = import_map {
    let resolved = dependency
      .maybe_code
      .ok()
      .or_else(|| dependency.maybe_type.ok());
    if let Some(resolved) = resolved {
      if let Some(to) = import_map_lookup(
        import_map,
        &resolved.specifier,
        &referrer_module.specifier,
      ) {
        if dependency_key != to {
          diagnostics.push(
            DenoDiagnostic::ImportMapRemap {
              from: dependency_key.to_string(),
              to,
            }
            .to_lsp_diagnostic(&language_server::to_lsp_range(&resolved.range)),
          );
        }
      }
    }
  }

  let import_ranges: Vec<_> = dependency
    .imports
    .iter()
    .map(|i| language_server::to_lsp_range(&i.specifier_range))
    .collect();
  // TODO(nayeemrmn): This is a crude way of detecting `@ts-types` which has
  // a different specifier and therefore needs a separate call to
  // `diagnose_resolution()`. It would be much cleaner if that were modelled as
  // a separate dependency: https://github.com/denoland/deno_graph/issues/247.
  let is_types_deno_types = !dependency.maybe_type.is_none()
    && !dependency.imports.iter().any(|i| {
      dependency
        .maybe_type
        .includes(i.specifier_range.range.start)
        .is_some()
    });

  let (resolution_diagnostics, deferred) = diagnose_resolution(
    snapshot,
    dependency_key,
    if dependency.maybe_code.is_none()
        // If not @ts-types, diagnose the types if the code errored because
        // it's likely resolving into the node_modules folder, which might be
        // erroring correctly due to resolution only being for bundlers. Let this
        // fail at runtime if necessary, but don't bother erroring in the editor
        || !is_types_deno_types && matches!(dependency.maybe_type, Resolution::Ok(_))
          && matches!(dependency.maybe_code, Resolution::Err(_))
    {
      &dependency.maybe_type
    } else {
      &dependency.maybe_code
    },
    dependency.is_dynamic,
    dependency.maybe_attribute_type.as_deref(),
    referrer_module,
    import_map,
  );
  diagnostics.extend(resolution_diagnostics.iter().flat_map(|diag| {
    import_ranges
      .iter()
      .map(|range| diag.to_lsp_diagnostic(range))
  }));
  deferred_diagnostics.extend(
    deferred
      .iter()
      .filter_map(|diag| {
        maybe_ambient_import_specifier(diag).map(|spec| {
          import_ranges
            .iter()
            .map(move |range| (spec.clone(), diag.to_lsp_diagnostic(range)))
        })
      })
      .flatten(),
  );

  if is_types_deno_types {
    let range = match &dependency.maybe_type {
      Resolution::Ok(resolved) => {
        language_server::to_lsp_range(&resolved.range)
      }
      Resolution::Err(error) => language_server::to_lsp_range(error.range()),
      Resolution::None => unreachable!(),
    };
    let (resolution_diagnostics, deferred) = diagnose_resolution(
      snapshot,
      dependency_key,
      &dependency.maybe_type,
      dependency.is_dynamic,
      dependency.maybe_attribute_type.as_deref(),
      referrer_module,
      import_map,
    );
    diagnostics.extend(
      resolution_diagnostics
        .iter()
        .map(|diag| diag.to_lsp_diagnostic(&range)),
    );
    deferred_diagnostics.extend(Box::new(deferred.iter().filter_map(|diag| {
      maybe_ambient_import_specifier(diag)
        .map(|spec| (spec, diag.to_lsp_diagnostic(&range)))
    })));
  }
}

#[derive(Debug)]
struct DeferredDiagnosticRecord {
  uri: Arc<Uri>,
  version: i32,
  scope: Option<Arc<Url>>,
  diagnostics: Vec<(String, lsp::Diagnostic)>,
}

/// Generate diagnostics that come from Deno module resolution logic (like
/// dependencies) or other Deno specific diagnostics, like the ability to use
/// an import map to shorten an URL.
fn generate_deno_diagnostics(
  snapshot: &language_server::StateSnapshot,
  config: &Config,
  token: CancellationToken,
) -> (DiagnosticVec, Vec<DeferredDiagnosticRecord>) {
  let mut diagnostics_vec = Vec::new();
  let mut deferred_diagnostics = Vec::new();
  for document in snapshot.document_modules.documents.open_docs() {
    if token.is_cancelled() {
      break;
    }
    if !document.is_diagnosable() {
      continue;
    }
    let Some(module) = snapshot
      .document_modules
      .primary_module(&Document::Open(document.clone()))
    else {
      continue;
    };
    let mut diagnostics = Vec::new();
    let mut deferred = Vec::new();
    if config.specifier_enabled(&module.specifier) {
      for (dependency_key, dependency) in module.dependencies.iter() {
        diagnose_dependency(
          &mut diagnostics,
          &mut deferred,
          snapshot,
          &module,
          dependency_key,
          dependency,
        );
      }
    }
    diagnostics_vec.push(DiagnosticRecord {
      uri: document.uri.clone(),
      versioned: VersionedDiagnostics {
        version: document.version,
        diagnostics,
      },
    });
    deferred_diagnostics.push(DeferredDiagnosticRecord {
      uri: document.uri.clone(),
      scope: module.scope.clone(),
      version: document.version,
      diagnostics: deferred,
    });
  }

  (diagnostics_vec, deferred_diagnostics)
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;
  use std::sync::Arc;

  use deno_config::deno_json::ConfigFile;
  use deno_core::resolve_url;
  use deno_semver::package::PackageNv;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  use super::*;
  use crate::lsp::cache::LspCache;
  use crate::lsp::config::Config;
  use crate::lsp::config::Settings;
  use crate::lsp::config::WorkspaceSettings;
  use crate::lsp::documents::DocumentModules;
  use crate::lsp::documents::LanguageId;
  use crate::lsp::language_server::StateSnapshot;
  use crate::lsp::resolver::LspResolver;
  use crate::lsp::urls::uri_to_url;
  use crate::lsp::urls::url_to_uri;

  fn mock_config() -> Config {
    let root_url = Arc::new(resolve_url("file:///").unwrap());
    let root_uri = url_to_uri(&root_url).unwrap();
    Config {
      settings: Arc::new(Settings {
        unscoped: Arc::new(WorkspaceSettings {
          enable: Some(true),
          lint: true,
          ..Default::default()
        }),
        ..Default::default()
      }),
      workspace_folders: Arc::new(vec![(
        root_url,
        lsp::WorkspaceFolder {
          uri: root_uri,
          name: "".to_string(),
        },
      )]),
      ..Default::default()
    }
  }

  struct DefaultRegistry;

  #[async_trait::async_trait(?Send)]
  impl deno_lockfile::NpmPackageInfoProvider for DefaultRegistry {
    async fn get_npm_package_info(
      &self,
      values: &[PackageNv],
    ) -> Result<
      Vec<deno_lockfile::Lockfile5NpmInfo>,
      Box<dyn std::error::Error + Send + Sync>,
    > {
      Ok(values.iter().map(|_| Default::default()).collect())
    }
  }

  fn default_registry(
  ) -> Arc<dyn deno_lockfile::NpmPackageInfoProvider + Send + Sync> {
    Arc::new(DefaultRegistry)
  }

  async fn setup(
    sources: &[(&str, &str, i32, LanguageId)],
    maybe_import_map: Option<(&str, &str)>,
  ) -> (TempDir, StateSnapshot) {
    let temp_dir = TempDir::new();
    let root_url = temp_dir.url();
    let cache = LspCache::new(Some(root_url.join(".deno_dir").unwrap()));
    let mut config = Config::new_with_roots([root_url.clone()]);
    if let Some((relative_path, json_string)) = maybe_import_map {
      let base_url = root_url.join(relative_path).unwrap();
      let config_file = ConfigFile::new(json_string, base_url).unwrap();
      config
        .tree
        .inject_config_file(config_file, &default_registry())
        .await;
    }
    let resolver =
      Arc::new(LspResolver::from_config(&config, &cache, None).await);
    let mut document_modules = DocumentModules::default();
    document_modules.update_config(
      &config,
      &resolver,
      &cache,
      &Default::default(),
    );
    for (relative_path, source, version, language_id) in sources {
      let specifier = root_url.join(relative_path).unwrap();
      document_modules.open_document(
        url_to_uri(&specifier).unwrap(),
        *version,
        *language_id,
        (*source).into(),
        None,
      );
    }
    (
      temp_dir,
      StateSnapshot {
        project_version: 0,
        document_modules,
        config: Arc::new(config),
        resolver,
      },
    )
  }

  #[tokio::test]
  async fn test_enabled_then_disabled_specifier() {
    let (_, snapshot) = setup(
      &[(
        "a.ts",
        r#"import * as b from "./b.ts";
let a: any = "a";
let c: number = "a";
"#,
        1,
        LanguageId::TypeScript,
      )],
      None,
    )
    .await;
    let snapshot = Arc::new(snapshot);
    let ts_server = TsServer::new(Default::default());

    // test enabled
    {
      let enabled_config = mock_config();
      let diagnostics = generate_lint_diagnostics(
        &snapshot,
        &enabled_config,
        Default::default(),
      );
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 6);
      let diagnostics = generate_ts_diagnostics(
        snapshot.clone(),
        &enabled_config,
        &ts_server,
        Default::default(),
      )
      .await
      .unwrap()
      .0;
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 4);
      let diagnostics = generate_all_deno_diagnostics(
        &snapshot,
        &enabled_config,
        Default::default(),
      );
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 1);
    }

    // now test disabled specifier
    {
      let mut disabled_config = mock_config();
      disabled_config.set_workspace_settings(
        WorkspaceSettings {
          enable: Some(false),
          ..Default::default()
        },
        vec![],
      );

      let diagnostics = generate_lint_diagnostics(
        &snapshot,
        &disabled_config,
        Default::default(),
      );
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 0);
      let diagnostics = generate_ts_diagnostics(
        snapshot.clone(),
        &disabled_config,
        &ts_server,
        Default::default(),
      )
      .await
      .unwrap()
      .0;
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 0);
      let diagnostics = generate_all_deno_diagnostics(
        &snapshot,
        &disabled_config,
        Default::default(),
      );
      assert_eq!(get_diagnostics_for_single(diagnostics).len(), 0);
    }
  }

  fn get_diagnostics_for_single(
    diagnostic_vec: DiagnosticVec,
  ) -> Vec<lsp::Diagnostic> {
    if diagnostic_vec.is_empty() {
      return vec![];
    }
    assert_eq!(diagnostic_vec.len(), 1);
    diagnostic_vec
      .into_iter()
      .next()
      .unwrap()
      .versioned
      .diagnostics
  }

  fn generate_all_deno_diagnostics(
    snapshot: &StateSnapshot,
    config: &Config,
    token: CancellationToken,
  ) -> DiagnosticVec {
    let (diagnostics, deferred) =
      generate_deno_diagnostics(snapshot, config, token);

    let mut all_diagnostics = diagnostics
      .into_iter()
      .map(|d| (d.uri.clone(), d))
      .collect::<HashMap<_, _>>();
    for diag in deferred {
      let existing =
        all_diagnostics.entry(diag.uri.clone()).or_insert_with(|| {
          DiagnosticRecord {
            uri: diag.uri.clone(),
            versioned: VersionedDiagnostics {
              diagnostics: vec![],
              version: diag.version,
            },
          }
        });
      existing
        .versioned
        .diagnostics
        .extend(diag.diagnostics.into_iter().map(|(_, d)| d));
      assert_eq!(existing.versioned.version, diag.version);
    }
    all_diagnostics.into_values().collect()
  }

  #[tokio::test]
  async fn test_deno_diagnostics_with_import_map() {
    let (temp_dir, snapshot) = setup(
      &[
        (
          "std/assert/mod.ts",
          "export function assert() {}",
          1,
          LanguageId::TypeScript,
        ),
        (
          "a/file.ts",
          "import { assert } from \"../std/assert/mod.ts\";\n\nassert();\nexport function a() {}",
          1,
          LanguageId::TypeScript,
        ),
        (
          "a/file2.ts",
          "import { a } from './file.ts';\nconsole.log(a);\n",
          1,
          LanguageId::TypeScript,
        ),
      ],
      Some((
        "a/deno.json",
        r#"{
        "imports": {
          "/~/std/": "../std/",
          "$@/": "./",
          "$a/": "../a/"
        }
      }"#,
      )),
    )
    .await;
    let config = mock_config();
    let token = CancellationToken::new();
    let actual = generate_all_deno_diagnostics(&snapshot, &config, token);
    assert_eq!(actual.len(), 3);
    for record in actual {
      let specifier = uri_to_url(&record.uri);
      let relative_specifier =
        temp_dir.url().make_relative(&specifier).unwrap();
      match relative_specifier.as_str() {
        "std/assert/mod.ts" => {
          assert_eq!(json!(record.versioned.diagnostics), json!([]))
        }
        "a/file.ts" => assert_eq!(
          json!(record.versioned.diagnostics),
          json!([
            {
              "range": {
                "start": {
                  "line": 0,
                  "character": 23
                },
                "end": {
                  "line": 0,
                  "character": 45
                }
              },
              "severity": 4,
              "code": "import-map-remap",
              "source": "deno",
              "message": "The import specifier can be remapped to \"/~/std/assert/mod.ts\" which will resolve it via the active import map.",
              "data": {
                "from": "../std/assert/mod.ts",
                "to": "/~/std/assert/mod.ts"
              }
            }
          ])
        ),
        "a/file2.ts" => {
          assert_eq!(json!(record.versioned.diagnostics), json!([]))
        }
        _ => unreachable!("unexpected specifier {}", &specifier),
      }
    }
  }

  #[test]
  fn test_get_code_action_import_map_remap() {
    let uri = Uri::from_str("file:///a/file.ts").unwrap();
    let specifier = ModuleSpecifier::parse("file:///a/file.ts").unwrap();
    let result = DenoDiagnostic::get_code_action(&uri, &specifier, &lsp::Diagnostic {
      range: lsp::Range {
        start: lsp::Position { line: 0, character: 23 },
        end: lsp::Position { line: 0, character: 50 },
      },
      severity: Some(lsp::DiagnosticSeverity::HINT),
      code: Some(lsp::NumberOrString::String("import-map-remap".to_string())),
      source: Some("deno".to_string()),
      message: "The import specifier can be remapped to \"/~/std/assert/mod.ts\" which will resolve it via the active import map.".to_string(),
      data: Some(json!({
        "from": "../std/assert/mod.ts",
        "to": "/~/std/assert/mod.ts"
      })),
      ..Default::default()
    });
    assert!(result.is_ok());
    let actual = result.unwrap();
    assert_eq!(
      json!(actual),
      json!({
        "title": "Update \"../std/assert/mod.ts\" to \"/~/std/assert/mod.ts\" to use import map.",
        "kind": "quickfix",
        "diagnostics": [
          {
            "range": {
              "start": {
                "line": 0,
                "character": 23
              },
              "end": {
                "line": 0,
                "character": 50
              }
            },
            "severity": 4,
            "code": "import-map-remap",
            "source": "deno",
            "message": "The import specifier can be remapped to \"/~/std/assert/mod.ts\" which will resolve it via the active import map.",
            "data": {
              "from": "../std/assert/mod.ts",
              "to": "/~/std/assert/mod.ts"
            }
          }
        ],
        "edit": {
          "changes": {
            "file:///a/file.ts": [
              {
                "range": {
                  "start": {
                    "line": 0,
                    "character": 23
                  },
                  "end": {
                    "line": 0,
                    "character": 50
                  }
                },
                "newText": "\"/~/std/assert/mod.ts\""
              }
            ]
          }
        }
      })
    );
  }

  #[tokio::test]
  async fn duplicate_diagnostics_for_duplicate_imports() {
    let (_, snapshot) = setup(
      &[(
        "a.ts",
        r#"
        // @ts-types="bad.d.ts"
        import "bad.js";
        import "bad.js";
        "#,
        1,
        LanguageId::TypeScript,
      )],
      None,
    )
    .await;
    let config = mock_config();
    let token = CancellationToken::new();
    let actual = generate_all_deno_diagnostics(&snapshot, &config, token);
    assert_eq!(actual.len(), 1);
    let record = actual.first().unwrap();
    assert_eq!(
      json!(record.versioned.diagnostics),
      json!([
        {
          "range": {
            "start": {
              "line": 2,
              "character": 15
            },
            "end": {
              "line": 2,
              "character": 23
            }
          },
          "severity": 1,
          "code": "import-prefix-missing",
          "source": "deno",
          "message": "Relative import path \"bad.js\" not prefixed with / or ./ or ../",
        },
        {
          "range": {
            "start": {
              "line": 3,
              "character": 15
            },
            "end": {
              "line": 3,
              "character": 23
            }
          },
          "severity": 1,
          "code": "import-prefix-missing",
          "source": "deno",
          "message": "Relative import path \"bad.js\" not prefixed with / or ./ or ../",
        },
        {
          "range": {
            "start": {
              "line": 1,
              "character": 21
            },
            "end": {
              "line": 1,
              "character": 31
            }
          },
          "severity": 1,
          "code": "import-prefix-missing",
          "source": "deno",
          "message": "Relative import path \"bad.d.ts\" not prefixed with / or ./ or ../",
        },
      ])
    );
  }

  #[tokio::test]
  async fn unable_to_load_a_local_module() {
    let (temp_dir, snapshot) = setup(
      &[(
        "a.ts",
        r#"
        import { 東京 } from "./🦕.ts";
        "#,
        1,
        LanguageId::TypeScript,
      )],
      None,
    )
    .await;
    let config = mock_config();
    let token = CancellationToken::new();
    let actual = generate_all_deno_diagnostics(&snapshot, &config, token);
    assert_eq!(actual.len(), 1);
    let record = actual.first().unwrap();
    assert_eq!(
      json!(record.versioned.diagnostics),
      json!([
        {
          "range": {
            "start": {
              "line": 1,
              "character": 27
            },
            "end": {
              "line": 1,
              "character": 35
            }
          },
          "severity": 1,
          "code": "no-local",
          "source": "deno",
          "message": format!(
            "Unable to load a local module: {}🦕.ts\nPlease check the file path.",
            temp_dir.url(),
          ),
        }
      ])
    );
  }

  #[test]
  fn test_specifier_text_for_redirected() {
    #[track_caller]
    fn run_test(specifier: &str, referrer: &str, expected: &str) {
      let result = specifier_text_for_redirected(
        &ModuleSpecifier::parse(specifier).unwrap(),
        &ModuleSpecifier::parse(referrer).unwrap(),
      );
      assert_eq!(result, expected);
    }

    run_test("file:///a/a.ts", "file:///a/mod.ts", "./a.ts");
    run_test("file:///a/a.ts", "file:///a/sub_dir/mod.ts", "../a.ts");
    run_test(
      "file:///a/sub_dir/a.ts",
      "file:///a/mod.ts",
      "./sub_dir/a.ts",
    );
    run_test(
      "https://deno.land/x/example/mod.ts",
      "file:///a/sub_dir/a.ts",
      "https://deno.land/x/example/mod.ts",
    );
  }
}
