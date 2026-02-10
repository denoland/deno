// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(nayeemrmn): Move to `cli/lsp/tsc/go.rs`.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use deno_config::deno_json::CompilerOptions;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_graph::source::Resolver;
use deno_resolver::deno_json::CompilerOptionsKey;
use indexmap::IndexSet;
use lsp_types::Uri;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::documents::DocumentModule;
use super::language_server::StateSnapshot;
use crate::cache::DenoDir;
use crate::http_util::HttpClientProvider;
use crate::lsp::completions::CompletionItemData;
use crate::lsp::documents::Document;
use crate::lsp::logging::lsp_log;
use crate::lsp::logging::lsp_warn;
use crate::lsp::resolver::SingleReferrerGraphResolver;
use crate::lsp::urls::uri_to_url;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TsGoCompletionItemData {
  pub uri: Uri,
  pub data: serde_json::Value,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TsGoProjectConfig {
  compiler_options: Arc<CompilerOptions>,
  files: IndexSet<Arc<Uri>>,
  user_preferences: super::tsc::UserPreferences,
  format_options: super::tsc::FormatCodeSettings,
  compiler_options_key: CompilerOptionsKey,
  notebook_uri: Option<Arc<Uri>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TsGoWorkspaceConfig {
  by_compiler_options_key: BTreeMap<CompilerOptionsKey, TsGoProjectConfig>,
  by_notebook_uri: BTreeMap<Arc<Uri>, TsGoProjectConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TsGoWorkspaceChange {
  file_changes: Vec<TsGoFileChange>,
  new_configuration: Option<TsGoWorkspaceConfig>,
}

impl TsGoWorkspaceChange {
  fn coalesce(&mut self, incoming: Self) {
    for change in incoming.file_changes {
      if let Some(existing_change) =
        self.file_changes.iter_mut().find(|c| c.uri == change.uri)
      {
        // Modified should never override Opened or Closed.
        if change.kind != TsGoFileChangeKind::Modified {
          existing_change.kind = change.kind;
        }
      } else {
        self.file_changes.push(change);
      }
    }
    if incoming.new_configuration.is_some() {
      self.new_configuration = incoming.new_configuration;
    }
  }
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TsGoRequestParams {
  request: TsGoRequest,
  workspace_change: Option<TsGoWorkspaceChange>,
}

fn fill_workspace_config_file_names(
  workspace_config: &mut TsGoWorkspaceConfig,
  snapshot: &StateSnapshot,
) {
  let scopes_with_node_specifier =
    snapshot.document_modules.scopes_with_node_specifier();

  // Insert global scripts.
  for (compiler_options_key, compiler_options_data) in
    snapshot.compiler_options_resolver.entries()
  {
    let files = &mut workspace_config
      .by_compiler_options_key
      .get_mut(compiler_options_key)
      .expect("workspace_config was made from snapshot")
      .files;
    let scope = compiler_options_data
      .workspace_dir_or_source_url
      .as_ref()
      .and_then(|s| snapshot.config.tree.scope_for_specifier(s))
      .cloned();
    let scoped_resolver =
      snapshot.resolver.get_scoped_resolver(scope.as_deref());
    if scopes_with_node_specifier.contains(&scope) {
      files.insert(Arc::new(
        Uri::from_str("deno:/asset/reference_types_node.d.ts").unwrap(),
      ));
    }
    for (referrer, relative_specifiers) in compiler_options_data
      .ts_config_files
      .iter()
      .map(|(r, f)| {
        let relative_specifiers =
          Box::new(f.iter().map(|f| &f.relative_specifier))
            as Box<dyn Iterator<Item = &String>>;
        (r.as_ref(), relative_specifiers)
      })
      .chain(
        compiler_options_data
          .compiler_options_types
          .iter()
          .map(|(r, t)| (r, Box::new(t.iter()) as _)),
      )
    {
      let resolver = SingleReferrerGraphResolver {
        valid_referrer: referrer,
        module_resolution_mode: ResolutionMode::Import,
        cli_resolver: scoped_resolver.as_cli_resolver(),
        jsx_import_source_config: compiler_options_data
          .jsx_import_source_config
          .as_deref(),
      };
      for relative_specifier in relative_specifiers {
        let Ok(mut specifier) = resolver
          .resolve(
            relative_specifier,
            &deno_graph::Range {
              specifier: referrer.clone(),
              range: deno_graph::PositionRange::zeroed(),
              resolution_mode: None,
            },
            deno_graph::source::ResolutionKind::Types,
          )
          .inspect_err(|err| {
            lsp_warn!(
              "Failed to resolve {relative_specifier} from `compilerOptions.types`: {err:#}"
            );
          })
        else {
          continue;
        };
        if let Ok(req_ref) =
          deno_semver::npm::NpmPackageReqReference::from_specifier(&specifier)
        {
          let Some((resolved, _)) = scoped_resolver.npm_to_file_url(
            &req_ref,
            referrer,
            NodeResolutionKind::Types,
            ResolutionMode::Import,
          ) else {
            lsp_log!("Failed to resolve {req_ref} to a file URL.");
            continue;
          };
          specifier = resolved;
        }
        let Some(module) = snapshot.document_modules.module_for_specifier(
          &specifier,
          scope.as_deref(),
          Some(compiler_options_key),
        ) else {
          continue;
        };
        files.insert(module.uri.clone());
      }
    }
  }

  // roots for notebook scopes
  for (notebook_uri, cell_uris) in
    snapshot.document_modules.documents.cells_by_notebook_uri()
  {
    let mut files = IndexSet::default();
    let scope = snapshot
      .document_modules
      .primary_scope(notebook_uri)
      .flatten();
    let compiler_options_key = snapshot
      .compiler_options_resolver
      .entry_for_specifier(&uri_to_url(notebook_uri))
      .0;

    // Copy over the globals from the containing regular scopes.
    if let Some(project_config) = workspace_config
      .by_compiler_options_key
      .get(compiler_options_key)
    {
      files.extend(project_config.files.iter().cloned());
    }

    // Add the cells as roots.
    files.extend(cell_uris.iter().filter_map(|u| {
      let document = snapshot.document_modules.documents.get(u)?;
      let module = snapshot
        .document_modules
        .module(&document, scope.map(|s| s.as_ref()))?;
      Some(module.uri.clone())
    }));

    workspace_config
      .by_notebook_uri
      .get_mut(notebook_uri)
      .expect("workspace_config was made from snapshot")
      .files = files;
  }

  // finally include the documents
  for modules in snapshot
    .document_modules
    .workspace_file_modules_by_scope()
    .into_values()
  {
    for module in modules {
      let is_open = module.open_data.is_some();
      let types_uri = (|| {
        let types_specifier = module
          .types_dependency
          .as_ref()?
          .dependency
          .maybe_specifier()?;
        snapshot
          .document_modules
          .resolve_dependency(
            types_specifier,
            &module.specifier,
            module.resolution_mode,
            module.scope.as_deref(),
            Some(&module.compiler_options_key),
          )?
          .2
      })();
      let files = &mut workspace_config
        .by_compiler_options_key
        .get_mut(&module.compiler_options_key)
        .expect("workspace_config was made from snapshot")
        .files;
      // If there is a types dep, use that as the root instead. But if the doc
      // is open, include both as roots.
      if let Some(types_uri) = &types_uri {
        files.insert(types_uri.clone());
      }
      if types_uri.is_none() || is_open {
        files.insert(module.uri.clone());
      }
    }
  }
}

struct TsGoServerInner {
  pending_change: Mutex<Option<TsGoWorkspaceChange>>,
  stdin: Mutex<std::process::ChildStdin>,
  pending_requests:
    Mutex<HashMap<i64, oneshot::Sender<Result<serde_json::Value, String>>>>,
  next_request_id: AtomicI64,
  #[allow(dead_code)]
  child: Mutex<Child>,
}

impl std::fmt::Debug for TsGoServerInner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TsGoServerInner")
      .field("pending_change", &self.pending_change)
      .finish_non_exhaustive()
  }
}

fn write_lsp_message(
  stdin: &mut std::process::ChildStdin,
  message: &serde_json::Value,
) -> std::io::Result<()> {
  let content = serde_json::to_string(message)?;
  write!(
    stdin,
    "Content-Length: {}\r\n\r\n{}",
    content.len(),
    content
  )?;
  stdin.flush()
}

fn read_lsp_message(
  reader: &mut std::io::BufReader<std::process::ChildStdout>,
) -> std::io::Result<serde_json::Value> {
  let mut content_length: Option<usize> = None;
  loop {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let line = line.trim();
    if line.is_empty() {
      break;
    }
    if let Some(len_str) = line.strip_prefix("Content-Length: ") {
      content_length = Some(len_str.parse().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
      })?);
    }
  }
  let content_length = content_length.ok_or_else(|| {
    std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      "Missing Content-Length header",
    )
  })?;
  let mut buf = vec![0u8; content_length];
  std::io::Read::read_exact(reader, &mut buf)?;
  serde_json::from_slice(&buf)
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

impl TsGoServerInner {
  async fn init(tsgo_path: &Path) -> Result<Self, AnyError> {
    let mut child = Command::new(tsgo_path)
      .args(["--lsp", "--stdio"])
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::inherit())
      .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    let pending_requests: Arc<
      Mutex<HashMap<i64, oneshot::Sender<Result<serde_json::Value, String>>>>,
    > = Arc::new(Mutex::new(HashMap::new()));

    let pending_requests_clone = pending_requests.clone();
    std::thread::spawn(move || {
      let mut reader = std::io::BufReader::new(stdout);
      loop {
        let message = match read_lsp_message(&mut reader) {
          Ok(msg) => msg,
          Err(e) => {
            lsp_warn!("Error reading from tsgo: {}", e);
            break;
          }
        };
        if let Some(id) = message.get("id") {
          let id = id.as_i64().unwrap_or(-1);
          let mut pending = pending_requests_clone.lock();
          if let Some(sender) = pending.remove(&id) {
            let result = if let Some(error) = message.get("error") {
              let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
              Err(message.to_string())
            } else {
              Ok(
                message
                  .get("result")
                  .cloned()
                  .unwrap_or(serde_json::Value::Null),
              )
            };
            let _ = sender.send(result);
          }
        }
      }
    });

    let initialize_request = json!({
      "jsonrpc": "2.0",
      "id": 0,
      "method": "initialize",
      "params": {
        "processId": std::process::id(),
        "capabilities": {},
        "rootUri": null,
        "workspaceFolders": null,
      },
    });

    write_lsp_message(&mut stdin, &initialize_request)?;

    let (tx, rx) = oneshot::channel();
    pending_requests.lock().insert(0, tx);

    let _init_response = rx
      .await
      .map_err(|_| anyhow!("Channel closed"))?
      .map_err(|e| anyhow!("{}", e))?;

    let inner = Self {
      pending_change: Mutex::new(None),
      stdin: Mutex::new(stdin),
      pending_requests: Mutex::new(
        Arc::try_unwrap(pending_requests)
          .map_err(|_| anyhow!("Failed to unwrap pending_requests"))?
          .into_inner(),
      ),
      next_request_id: AtomicI64::new(1),
      child: Mutex::new(child),
    };

    Ok(inner)
  }

  async fn request<R>(
    &self,
    request: TsGoRequest,
    _token: &CancellationToken,
  ) -> Result<R, AnyError>
  where
    R: DeserializeOwned,
  {
    let workspace_change = self.pending_change.lock().take();
    let params = TsGoRequestParams {
      request,
      workspace_change,
    };

    let request_id = self.next_request_id.fetch_add(1, Ordering::SeqCst);

    let message = json!({
      "jsonrpc": "2.0",
      "id": request_id,
      "method": "deno/request",
      "params": params,
    });

    let (tx, rx) = oneshot::channel();
    self.pending_requests.lock().insert(request_id, tx);

    {
      let mut stdin = self.stdin.lock();
      write_lsp_message(&mut stdin, &message)?;
    }

    let result = rx
      .await
      .map_err(|_| anyhow!("Channel closed"))?
      .map_err(|e| anyhow!("{}", e))?;

    Ok(serde_json::from_value(result)?)
  }
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
        TsGoServerInner::init(&tsgo_path).await.unwrap()
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
    let mut incoming = TsGoWorkspaceChange {
      file_changes: documents
        .iter()
        .map(|(d, k)| TsGoFileChange {
          uri: d.uri().clone(),
          kind: (*k).into(),
        })
        .collect(),
      new_configuration: configuration_changed.then(|| {
        let by_compiler_options_key = snapshot
          .compiler_options_resolver
          .entries()
          .map(|(k, d)| {
            let (user_preferences, format_options) = d
              .workspace_dir_or_source_url
              .as_ref()
              .map(|s| {
                (
                  super::tsc::UserPreferences::from_config_for_specifier(
                    &snapshot.config,
                    s,
                  ),
                  (&snapshot.config.tree.fmt_config_for_specifier(s).options)
                    .into(),
                )
              })
              .unwrap_or_default();
            (
              k.clone(),
              TsGoProjectConfig {
                compiler_options: d.compiler_options.clone(),
                files: Default::default(),
                user_preferences,
                format_options,
                compiler_options_key: k.clone(),
                notebook_uri: None,
              },
            )
          })
          .collect::<BTreeMap<_, _>>();
        let by_notebook_uri = snapshot
          .document_modules
          .documents
          .cells_by_notebook_uri()
          .keys()
          .map(|u| {
            let compiler_options_key = snapshot
              .compiler_options_resolver
              .entry_for_specifier(&uri_to_url(u))
              .0;
            let project_config =
              by_compiler_options_key.get(compiler_options_key).unwrap();
            (
              u.clone(),
              TsGoProjectConfig {
                compiler_options: project_config.compiler_options.clone(),
                files: Default::default(),
                user_preferences: project_config.user_preferences.clone(),
                format_options: project_config.format_options.clone(),
                compiler_options_key: compiler_options_key.clone(),
                notebook_uri: Some(u.clone()),
              },
            )
          })
          .collect::<BTreeMap<_, _>>();
        TsGoWorkspaceConfig {
          by_compiler_options_key,
          by_notebook_uri,
        }
      }),
    };
    if let Some(workspace_config) = &mut incoming.new_configuration {
      fill_workspace_config_file_names(workspace_config, &snapshot);
    }
    let mut pending_change = inner.pending_change.lock();
    if let Some(existing) = pending_change.as_mut() {
      existing.coalesce(incoming);
    } else {
      *pending_change = Some(incoming);
    }
  }

  async fn request<R>(
    &self,
    request: TsGoRequest,
    _snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<R, AnyError>
  where
    R: DeserializeOwned,
  {
    let inner = self.inner().await;
    inner.request(request, token).await
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
          args: json!([&module.uri]),
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
            "textDocument": { "uri": &module.uri },
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
          args: json!([&module.uri]),
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
          args: json!([&module.uri]),
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
          args: json!([&module.uri, position]),
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
            "textDocument": { "uri": &module.uri },
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
          args: json!([&module.uri, position]),
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
          args: json!([&module.uri, position]),
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
          args: json!([&module.uri, position]),
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
          args: json!([&module.uri, position, context]),
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
            "textDocument": { "uri": &module.uri },
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
          args: json!([&module.uri]),
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
          args: json!([&module.uri, position]),
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
            "textDocument": { "uri": &module.uri },
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
            "textDocument": { "uri": &module.uri },
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
          args: json!([&module.uri, position, context]),
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
            "textDocument": { "uri": &module.uri },
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
