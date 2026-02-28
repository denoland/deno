// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use deno_ast::MediaType;
use deno_config::deno_json::CompilerOptions;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_graph::source::Resolver;
use deno_path_util::url_from_directory_path;
use deno_resolver::deno_json::CompilerOptionsKey;
use deno_runtime::tokio_util::create_basic_runtime;
use indexmap::IndexSet;
use lsp_types::Uri;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tower_lsp::lsp_types as lsp;

use super::documents::DocumentModule;
use super::language_server::StateSnapshot;
use crate::cache::DenoDir;
use crate::http_util::HttpClientProvider;
use crate::lsp::completions::CompletionItemData;
use crate::lsp::documents::Document;
use crate::lsp::documents::ServerDocumentKind;
use crate::lsp::logging::lsp_log;
use crate::lsp::logging::lsp_warn;
use crate::lsp::resolver::SingleReferrerGraphResolver;
use crate::lsp::urls::uri_to_url;
use crate::tsc::IGNORED_DIAGNOSTIC_CODES;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TsGoCompletionItemData {
  pub uri: Uri,
  pub data: serde_json::Value,
}

/// This is different from compiler options from user config, it stores enums as
/// numbers.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsGoCompilerOptions(serde_json::Value);

impl TsGoCompilerOptions {
  fn from_compiler_options(compiler_options: CompilerOptions) -> Self {
    let mut value = compiler_options.0;
    let Some(object) = value.as_object_mut() else {
      return Self(serde_json::Value::Object(Default::default()));
    };
    let jsx = object.remove("jsx");
    if let Some(jsx) = jsx.as_ref().and_then(|v| v.as_str()) {
      let tsgo_jsx = match jsx {
        "preserve" => 1,
        "react-native" => 2,
        "react" => 3,
        "react-jsx" | "precompile" => 4,
        "react-jsxdev" => 5,
        _ => 0,
      };
      object.insert("jsx".to_string(), json!(tsgo_jsx));
    }
    let module = object.remove("module");
    if let Some(module) = module.as_ref().and_then(|v| v.as_str()) {
      let tsgo_module = match module {
        "commonjs" => 1,
        "amd" => 2,
        "umd" => 3,
        "system" => 4,
        "es6" | "es2015" => 5,
        "es2020" => 6,
        "es2022" => 7,
        "esnext" => 99,
        "node16" => 100,
        "node18" => 101,
        "node20" => 102,
        "nodenext" => 199,
        "preserve" => 200,
        _ => 199,
      };
      object.insert("module".to_string(), json!(tsgo_module));
    }
    let module_detection = object.remove("moduleDetection");
    if let Some(module_detection) =
      module_detection.as_ref().and_then(|v| v.as_str())
    {
      let tsgo_module_detection = match module_detection {
        "auto" => 1,
        "legacy" => 2,
        "force" => 3,
        _ => 3,
      };
      object
        .insert("moduleDetection".to_string(), json!(tsgo_module_detection));
    }
    let module_resolution = object.remove("moduleResolution");
    if let Some(module_resolution) =
      module_resolution.as_ref().and_then(|v| v.as_str())
    {
      let tsgo_module_resolution = match module_resolution {
        "classic" => 1,
        "node10" | "node" => 2,
        "node16" => 3,
        "nodenext" => 99,
        "bundler" => 100,
        _ => 99,
      };
      object.insert(
        "moduleResolution".to_string(),
        json!(tsgo_module_resolution),
      );
    }
    let new_line = object.remove("newLine");
    if let Some(new_line) = new_line.as_ref().and_then(|v| v.as_str()) {
      let tsgo_new_line = match new_line {
        "crlf" => 1,
        "lf" => 2,
        _ => 0,
      };
      object.insert("newLine".to_string(), json!(tsgo_new_line));
    }
    let target = object.remove("target");
    if let Some(target) = target.as_ref().and_then(|v| v.as_str()) {
      let tsgo_target = match target {
        "es3" => 0,
        "es5" => 1,
        "es6" | "es2015" => 2,
        "es2016" => 3,
        "es2017" => 4,
        "es2018" => 5,
        "es2019" => 6,
        "es2020" => 7,
        "es2021" => 8,
        "es2022" => 9,
        "es2023" => 10,
        "es2024" => 11,
        "esnext" => 99,
        _ => 99,
      };
      object.insert("target".to_string(), json!(tsgo_target));
    }
    Self(value)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TsGoCallbackParams {
  #[serde(rename_all = "camelCase")]
  GetDocument { uri: Uri },
  #[serde(rename_all = "camelCase")]
  ResolveModuleName {
    module_name: String,
    referrer_uri: Uri,
    import_attribute_type: Option<String>,
    resolution_mode: deno_typescript_go_client_rust::types::ResolutionMode,
    compiler_options_key: CompilerOptionsKey,
  },
  #[serde(rename_all = "camelCase")]
  ResolveJsxImportSource {
    compiler_options_key: CompilerOptionsKey,
  },
  #[serde(rename_all = "camelCase")]
  GetPackageScopeForPath { directory_path: PathBuf },
  #[serde(rename_all = "camelCase")]
  GetImpliedNodeFormatForFile {
    uri: Uri,
    compiler_options_key: CompilerOptionsKey,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TsGoProjectConfig {
  compiler_options: Arc<TsGoCompilerOptions>,
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

impl TsGoWorkspaceConfig {
  fn from_snapshot(snapshot: &StateSnapshot) -> Self {
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
            compiler_options: Arc::new(
              TsGoCompilerOptions::from_compiler_options(
                d.compiler_options.as_ref().clone(),
              ),
            ),
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
    let mut workspace_config = Self {
      by_compiler_options_key,
      by_notebook_uri,
    };
    fill_workspace_config_file_names(&mut workspace_config, snapshot);
    workspace_config
  }
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
enum TsGoRequest {
  #[serde(rename_all = "camelCase")]
  LanguageServiceMethod {
    name: String,
    args: serde_json::Value,
    compiler_options_key: CompilerOptionsKey,
    notebook_uri: Option<Arc<Uri>>,
  },
  #[serde(rename_all = "camelCase")]
  GetAmbientModules {
    compiler_options_key: CompilerOptionsKey,
    notebook_uri: Option<Arc<Uri>>,
  },
  #[serde(rename_all = "camelCase")]
  WorkspaceSymbol { query: String },
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

type PendingRequests =
  Mutex<HashMap<i64, oneshot::Sender<Result<serde_json::Value, String>>>>;

struct TsGoServerInner {
  snapshot: Arc<Mutex<Arc<StateSnapshot>>>,
  pending_change: Mutex<Option<TsGoWorkspaceChange>>,
  pending_change_lock: tokio::sync::RwLock<()>,
  stdin: Arc<Mutex<std::process::ChildStdin>>,
  pending_requests: Arc<PendingRequests>,
  next_request_id: AtomicI64,
  #[allow(dead_code)]
  child: Mutex<Child>,
  runtime_handle: tokio::runtime::Handle,
}

impl std::fmt::Debug for TsGoServerInner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TsGoServerInner")
      .field("pending_change", &self.pending_change)
      .field("next_request_id", &self.next_request_id)
      .finish_non_exhaustive()
  }
}

fn write_lsp_message(
  stdin: &mut std::process::ChildStdin,
  message: &serde_json::Value,
) -> std::io::Result<()> {
  use std::io::Write;
  let content = serde_json::to_string(message)?;
  write!(
    stdin,
    "Content-Length: {}\r\n\r\n{}",
    content.len(),
    content
  )?;
  stdin.flush()
}

async fn read_lsp_message(
  reader: &mut tokio::io::BufReader<tokio::process::ChildStdout>,
) -> std::io::Result<serde_json::Value> {
  let mut content_length: Option<usize> = None;
  loop {
    let mut line = String::new();
    reader.read_line(&mut line).await?;
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
  reader.read_exact(&mut buf).await?;
  serde_json::from_slice(&buf)
    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

impl TsGoServerInner {
  async fn init(tsgo_path: &Path, snapshot: Arc<StateSnapshot>) -> Self {
    let mut child = Command::new(tsgo_path)
      .args(["--lsp", "--stdio"])
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::inherit())
      .spawn()
      .unwrap();

    let stdin = Arc::new(Mutex::new(child.stdin.take().unwrap()));
    let stdout =
      tokio::process::ChildStdout::from_std(child.stdout.take().unwrap())
        .unwrap();
    let snapshot = Arc::new(Mutex::new(snapshot));

    let pending_requests: Arc<PendingRequests> =
      Arc::new(Mutex::new(HashMap::new()));

    let pending_requests_clone = pending_requests.clone();
    let stdin_clone = stdin.clone();
    let snapshot_clone = snapshot.clone();
    let read_loop = async move {
      let mut reader = tokio::io::BufReader::new(stdout);
      loop {
        let message = match read_lsp_message(&mut reader).await {
          Ok(msg) => msg,
          Err(e) => {
            lsp_warn!("Error reading from tsgo: {}", e);
            break;
          }
        };
        if let Some(method) = message.get("method").and_then(|m| m.as_str()) {
          let id = message.get("id");
          let params = message.get("params");
          match method {
            "deno/callback" => {
              let Some(id) = id else {
                lsp_warn!("Missing id in tsgo callback: {:#}", &message,);
                continue;
              };
              let params: TsGoCallbackParams = match params
                .and_then(|p| serde_json::from_value(p.clone()).ok())
              {
                Some(p) => p,
                None => {
                  let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                      "code": -32602,
                      "message": "Invalid params",
                    },
                  });
                  let mut stdin = stdin_clone.lock();
                  let _ = write_lsp_message(&mut stdin, &response);
                  continue;
                }
              };
              let result =
                Self::handle_callback(params, &snapshot_clone.lock());
              let response = match result {
                Ok(result) => json!({
                  "jsonrpc": "2.0",
                  "id": id,
                  "result": result,
                }),
                Err(err) => json!({
                  "jsonrpc": "2.0",
                  "id": id,
                  "error": {
                    "code": -32001,
                    "message": err.to_string(),
                  },
                }),
              };
              let mut stdin = stdin_clone.lock();
              let _ = write_lsp_message(&mut stdin, &response);
            }
            "client/registerCapability" => {
              let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null,
              });
              let mut stdin = stdin_clone.lock();
              let _ = write_lsp_message(&mut stdin, &response);
            }
            "window/logMessage" => {
              let Ok(params) = serde_json::from_value::<lsp::LogMessageParams>(json!(params)).inspect_err(|err| {
                lsp_warn!("Couldn't parse params for \"window/logMessage\" from tsgo: {err:#}");
              }) else {
                continue;
              };
              if matches!(
                params.typ,
                lsp::MessageType::ERROR | lsp::MessageType::WARNING
              ) || std::env::var("DENO_TSC_DEBUG").is_ok()
              {
                lsp_log!("[tsgo - {:?}] {}", params.typ, params.message);
              }
            }
            method => {
              if let Some(id) = id {
                let response = json!({
                  "jsonrpc": "2.0",
                  "id": id,
                  "error": {
                    "code": -32601,
                    "message": format!("Method not found: \"{method}\""),
                  },
                });
                let mut stdin = stdin_clone.lock();
                let _ = write_lsp_message(&mut stdin, &response);
              }
              lsp_warn!(
                "Received unknown notification from tsgo: {:#}",
                &message,
              );
            }
          }
        } else if let Some(id) = message.get("id") {
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
    };

    let (runtime_handle_tx, runtime_handle_rx) =
      std::sync::mpsc::channel::<tokio::runtime::Handle>();
    std::thread::spawn(move || {
      let rt = create_basic_runtime();
      let _ = runtime_handle_tx.send(rt.handle().clone());
      rt.block_on(read_loop);
    });
    let runtime_handle = runtime_handle_rx.recv().unwrap();

    let capabilities = {
      let snapshot = snapshot.lock();
      lsp::ClientCapabilities {
        text_document: Some(lsp::TextDocumentClientCapabilities {
          synchronization: None,
          formatting: None,
          range_formatting: None,
          on_type_formatting: None,
          publish_diagnostics: None,
          diagnostic: Some(Default::default()),
          ..snapshot
            .config
            .client_capabilities
            .text_document
            .clone()
            .unwrap_or_default()
        }),
        workspace: Some(lsp::WorkspaceClientCapabilities {
          symbol: snapshot
            .config
            .client_capabilities
            .workspace
            .as_ref()
            .and_then(|w| w.symbol.clone()),
          ..Default::default()
        }),
        ..Default::default()
      }
    };
    let initialize_request = json!({
      "jsonrpc": "2.0",
      "id": 0,
      "method": "initialize",
      "params": {
        "initializationOptions": {
          "disablePushDiagnostics": true,
        },
        "processId": std::process::id(),
        "capabilities": capabilities,
        "rootUri": null,
        "workspaceFolders": null,
      },
    });

    {
      let mut stdin = stdin.lock();
      write_lsp_message(&mut stdin, &initialize_request).unwrap();
    }

    let (tx, rx) = oneshot::channel();
    pending_requests.lock().insert(0, tx);

    let _init_response = rx.await.unwrap().unwrap();

    let initialized_notification = json!({
      "jsonrpc": "2.0",
      "method": "initialized",
      "params": {},
    });
    {
      let mut stdin = stdin.lock();
      write_lsp_message(&mut stdin, &initialized_notification).unwrap();
    }

    let pending_change = Mutex::new(Some(TsGoWorkspaceChange {
      file_changes: Vec::new(),
      new_configuration: Some(TsGoWorkspaceConfig::from_snapshot(
        snapshot.lock().as_ref(),
      )),
    }));

    Self {
      snapshot,
      pending_change,
      pending_change_lock: Default::default(),
      stdin,
      pending_requests,
      next_request_id: AtomicI64::new(1),
      child: Mutex::new(child),
      runtime_handle,
    }
  }

  fn handle_callback(
    params: TsGoCallbackParams,
    snapshot: &StateSnapshot,
  ) -> Result<serde_json::Value, AnyError> {
    match params {
      TsGoCallbackParams::GetDocument { uri } => {
        let document = snapshot
          .document_modules
          .documents
          .get(&uri)
          .ok_or_else(|| anyhow!("Document not found"))?;
        let text = document.text();
        Ok(json!({
          "text": &text,
          "lineStarts": document
            .line_index()
            .line_starts()
            .iter()
            .map(|&s| u32::from(s) as i32)
            .collect::<Vec<_>>(),
          "asciiOnly": text.is_ascii(),
        }))
      }
      TsGoCallbackParams::ResolveModuleName {
        module_name,
        referrer_uri,
        import_attribute_type,
        resolution_mode,
        compiler_options_key,
      } => {
        let referrer_module = snapshot
          .document_modules
          .module_for_tsgo_document(&referrer_uri, &compiler_options_key)
          .ok_or_else(|| anyhow!("Referrer module not found"))?;
        let Some((uri, media_type)) = snapshot
          .document_modules
          .resolve_dependency_document(
          &module_name,
          &referrer_module,
          match resolution_mode {
            deno_typescript_go_client_rust::types::ResolutionMode::None => {
              ResolutionMode::Import
            }
            deno_typescript_go_client_rust::types::ResolutionMode::CommonJS => {
              ResolutionMode::Require
            }
            deno_typescript_go_client_rust::types::ResolutionMode::ESM => {
              ResolutionMode::Import
            }
          },
          import_attribute_type.as_deref(),
        ) else {
          return Ok(json!(null));
        };
        Ok(json!({
          "uri": uri,
          "extension": media_type.as_ts_extension(),
        }))
      }
      TsGoCallbackParams::ResolveJsxImportSource {
        compiler_options_key,
      } => {
        let compiler_options_data = snapshot
          .compiler_options_resolver
          .for_key(&compiler_options_key)
          .unwrap();
        let specifier = compiler_options_data
          .jsx_import_source_config
          .as_ref()
          .and_then(|c| c.specifier());
        Ok(json!(specifier))
      }
      TsGoCallbackParams::GetPackageScopeForPath { directory_path } => {
        let Ok(directory_url) = url_from_directory_path(&directory_path) else {
          return Ok(json!(null));
        };
        let scoped_resolver =
          snapshot.resolver.get_scoped_resolver(Some(&directory_url));
        let Ok(Some(package_json)) = scoped_resolver
          .as_pkg_json_resolver()
          .get_closest_package_json(&directory_path.join("package.json"))
        else {
          return Ok(json!(null));
        };
        Ok(json!({
          "packageDirectoryPath": package_json.path.parent(),
          "packageJsonText": serde_json::to_string(&package_json).unwrap(),
        }))
      }
      TsGoCallbackParams::GetImpliedNodeFormatForFile {
        uri,
        compiler_options_key,
      } => {
        let referrer_module = snapshot
          .document_modules
          .module_for_tsgo_document(&uri, &compiler_options_key)
          .ok_or_else(|| anyhow!("Module not found"))?;
        let resolution_mode =
          match (referrer_module.resolution_mode, referrer_module.media_type) {
            (
              _,
              MediaType::Css
              | MediaType::Json
              | MediaType::Html
              | MediaType::Sql
              | MediaType::Wasm
              | MediaType::SourceMap
              | MediaType::Unknown,
            ) => deno_typescript_go_client_rust::types::ResolutionMode::None,
            (ResolutionMode::Import, _) => {
              deno_typescript_go_client_rust::types::ResolutionMode::ESM
            }
            (ResolutionMode::Require, _) => {
              deno_typescript_go_client_rust::types::ResolutionMode::CommonJS
            }
          };
        Ok(json!(resolution_mode))
      }
    }
  }

  async fn request<R>(
    &self,
    request: TsGoRequest,
    token: &CancellationToken,
  ) -> Result<R, AnyError>
  where
    R: DeserializeOwned,
  {
    let workspace_change = self.pending_change.lock().take();
    let (_read, _write) = if workspace_change.is_some() {
      (None, Some(self.pending_change_lock.write().await))
    } else {
      (Some(self.pending_change_lock.read().await), None)
    };
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

    // Spawn this task on the reader thread which should be mostly idle. It's
    // important that cancellations are passed through quickly.
    let token_clone = token.clone();
    let stdin = self.stdin.clone();
    let pending_requests = self.pending_requests.clone();
    let cancel_handle = self.runtime_handle.spawn(async move {
      token_clone.cancelled().await;
      let cancel_message = json!({
        "jsonrpc": "2.0",
        "method": "$/cancelRequest",
        "params": { "id": request_id },
      });
      {
        let mut stdin = stdin.lock();
        let _ = write_lsp_message(&mut stdin, &cancel_message);
      }
      pending_requests.lock().remove(&request_id);
    });

    let result = rx.await;
    cancel_handle.abort();
    let result = result
      .map_err(|_| {
        debug_assert!(token.is_cancelled());
        anyhow!("request cancelled")
      })?
      .map_err(|e| anyhow!("{}", e))?;

    Ok(serde_json::from_value(result)?)
  }
}

fn qualify_tsgo_diagnostic(diagnostic: &mut lsp::Diagnostic) {
  diagnostic.source = Some("deno-ts".to_string());
  if let Some(lsp::NumberOrString::Number(code)) = &diagnostic.code {
    diagnostic.message = crate::tsc::go::maybe_rewrite_message(
      std::mem::take(&mut diagnostic.message),
      *code as _,
    );
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

  async fn inner(&self, snapshot: Arc<StateSnapshot>) -> &TsGoServerInner {
    self
      .inner
      .get_or_init(async || {
        let tsgo_path = crate::tsc::ensure_tsgo(
          &self.deno_dir,
          self.http_client_provider.clone(),
        )
        .await
        .unwrap();
        TsGoServerInner::init(tsgo_path, snapshot).await
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
    *inner.snapshot.lock() = snapshot.clone();
    let incoming = TsGoWorkspaceChange {
      file_changes: documents
        .iter()
        .map(|(d, k)| TsGoFileChange {
          uri: d.uri().clone(),
          kind: (*k).into(),
        })
        .collect(),
      new_configuration: configuration_changed
        .then(|| TsGoWorkspaceConfig::from_snapshot(&snapshot)),
    };
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
    snapshot: Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<R, AnyError>
  where
    R: DeserializeOwned,
  {
    let inner = self.inner(snapshot).await;
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
    let mut report = self
      .request::<lsp::DocumentDiagnosticReport>(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideDiagnostics".to_string(),
          args: json!([&module.uri]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot,
        token,
      )
      .await?;
    if let lsp::DocumentDiagnosticReport::Full(report) = &mut report {
      report
        .full_document_diagnostic_report
        .items
        .retain(|diagnostic| {
          let Some(lsp::NumberOrString::Number(code)) = &diagnostic.code else {
            return true;
          };
          !IGNORED_DIAGNOSTIC_CODES.contains(&(*code as _))
        });
      for diagnostic in &mut report.full_document_diagnostic_report.items {
        qualify_tsgo_diagnostic(diagnostic);
      }
    }
    Ok(report)
  }

  pub async fn provide_references(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    context: lsp::ReferenceContext,
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::Location>>, AnyError> {
    let mut references: Result<Option<Vec<lsp::Location>>, _> = self
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
        snapshot.clone(),
        token,
      )
      .await;
    if let Ok(Some(locations)) = &mut references {
      for location in locations {
        normalize_location(location, snapshot)
      }
    }
    references
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::GotoDefinitionResponse>, AnyError> {
    let mut response: Result<Option<lsp::GotoDefinitionResponse>, _> = self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideDefinition".to_string(),
          args: json!([&module.uri, position]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot.clone(),
        token,
      )
      .await;
    if let Ok(Some(response)) = &mut response {
      normalize_goto_definition_response(response, snapshot);
    }
    response
  }

  pub async fn provide_type_definition(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoTypeDefinitionResponse>, AnyError> {
    let mut response: Result<
      Option<lsp::request::GotoTypeDefinitionResponse>,
      _,
    > = self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideTypeDefinition".to_string(),
          args: json!([&module.uri, position]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot.clone(),
        token,
      )
      .await;
    if let Ok(Some(response)) = &mut response {
      normalize_goto_definition_response(response, snapshot);
    }
    response
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
          *data = serde_json::json!(CompletionItemData::TsGo(
            TsGoCompletionItemData {
              uri: module.uri.as_ref().clone(),
              data: raw_data,
            }
          ));
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<lsp::request::GotoImplementationResponse>, AnyError> {
    let mut response: Result<
      Option<lsp::request::GotoImplementationResponse>,
      _,
    > = self
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
        snapshot.clone(),
        token,
      )
      .await;
    if let Ok(Some(response)) = &mut response {
      normalize_goto_definition_response(response, snapshot);
    }
    response
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyIncomingCall>>, AnyError> {
    let mut incoming_calls: Result<
      Option<Vec<lsp::CallHierarchyIncomingCall>>,
      _,
    > = self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideCallHierarchyIncomingCalls".to_string(),
          args: json!([item]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot.clone(),
        token,
      )
      .await;
    if let Ok(Some(incoming_calls)) = &mut incoming_calls {
      for incoming_call in incoming_calls {
        normalize_call_hierarchy_incoming_call(incoming_call, snapshot);
      }
    }
    incoming_calls
  }

  pub async fn provide_call_hierarchy_outgoing_calls(
    &self,
    module: &DocumentModule,
    item: &lsp::CallHierarchyItem,
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyOutgoingCall>>, AnyError> {
    let mut outgoing_calls: Result<
      Option<Vec<lsp::CallHierarchyOutgoingCall>>,
      _,
    > = self
      .request(
        TsGoRequest::LanguageServiceMethod {
          name: "ProvideCallHierarchyOutgoingCalls".to_string(),
          args: json!([item]),
          compiler_options_key: module.compiler_options_key.clone(),
          notebook_uri: module.notebook_uri.clone(),
        },
        snapshot.clone(),
        token,
      )
      .await;
    if let Ok(Some(outgoing_calls)) = &mut outgoing_calls {
      for outgoing_call in outgoing_calls {
        normalize_call_hierarchy_outgoing_call(outgoing_call, snapshot);
      }
    }
    outgoing_calls
  }

  pub async fn provide_prepare_call_hierarchy(
    &self,
    module: &DocumentModule,
    position: lsp::Position,
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::CallHierarchyItem>>, AnyError> {
    let mut call_hierarchy: Result<Option<Vec<lsp::CallHierarchyItem>>, _> =
      self
        .request(
          TsGoRequest::LanguageServiceMethod {
            name: "ProvidePrepareCallHierarchy".to_string(),
            args: json!([&module.uri, position]),
            compiler_options_key: module.compiler_options_key.clone(),
            notebook_uri: module.notebook_uri.clone(),
          },
          snapshot.clone(),
          token,
        )
        .await;
    if let Ok(Some(call_hierarchy_items)) = &mut call_hierarchy {
      for call_hierarchy_item in call_hierarchy_items {
        normalize_call_hierarchy_item(call_hierarchy_item, snapshot);
      }
    }
    call_hierarchy
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
    snapshot: &Arc<StateSnapshot>,
    token: &CancellationToken,
  ) -> Result<Option<Vec<lsp::SymbolInformation>>, AnyError> {
    let mut symbol_information: Result<Option<Vec<lsp::SymbolInformation>>, _> =
      self
        .request(
          TsGoRequest::WorkspaceSymbol {
            query: query.to_string(),
          },
          snapshot.clone(),
          token,
        )
        .await;
    if let Ok(Some(symbol_information)) = &mut symbol_information {
      for symbol_information in symbol_information {
        normalize_symbol_information(symbol_information, snapshot);
      }
    }
    symbol_information
  }
}

fn normalize_uri_and_positions<'a>(
  uri: &mut Uri,
  positions: impl IntoIterator<Item = &'a mut lsp::Position>,
  snapshot: &StateSnapshot,
) {
  let Some(document) = snapshot.document_modules.documents.get(uri) else {
    return;
  };
  let Document::Server(server_document) = document else {
    return;
  };
  let ServerDocumentKind::RawImportTypes { resource_uri, .. } =
    &server_document.kind
  else {
    return;
  };
  *uri = resource_uri.clone();
  for position in positions {
    *position = Default::default();
  }
}

fn normalize_location(location: &mut lsp::Location, snapshot: &StateSnapshot) {
  normalize_uri_and_positions(
    &mut location.uri,
    [&mut location.range.start, &mut location.range.end],
    snapshot,
  )
}

fn normalize_location_link(
  location_link: &mut lsp::LocationLink,
  snapshot: &StateSnapshot,
) {
  normalize_uri_and_positions(
    &mut location_link.target_uri,
    [
      &mut location_link.target_range.start,
      &mut location_link.target_range.end,
      &mut location_link.target_selection_range.start,
      &mut location_link.target_selection_range.end,
    ],
    snapshot,
  )
}

fn normalize_goto_definition_response(
  response: &mut lsp::GotoDefinitionResponse,
  snapshot: &StateSnapshot,
) {
  match response {
    lsp::GotoDefinitionResponse::Scalar(location) => {
      normalize_location(location, snapshot)
    }
    lsp::GotoDefinitionResponse::Array(locations) => {
      for location in locations {
        normalize_location(location, snapshot)
      }
    }
    lsp::GotoDefinitionResponse::Link(location_links) => {
      for location_link in location_links {
        normalize_location_link(location_link, snapshot)
      }
    }
  }
}

fn normalize_call_hierarchy_item(
  call_hierarchy_item: &mut lsp::CallHierarchyItem,
  snapshot: &StateSnapshot,
) {
  normalize_uri_and_positions(
    &mut call_hierarchy_item.uri,
    [
      &mut call_hierarchy_item.range.start,
      &mut call_hierarchy_item.range.end,
      &mut call_hierarchy_item.selection_range.start,
      &mut call_hierarchy_item.selection_range.end,
    ],
    snapshot,
  )
}

fn normalize_call_hierarchy_incoming_call(
  incoming_call: &mut lsp::CallHierarchyIncomingCall,
  snapshot: &StateSnapshot,
) {
  normalize_call_hierarchy_item(&mut incoming_call.from, snapshot);
}

fn normalize_call_hierarchy_outgoing_call(
  outgoing_call: &mut lsp::CallHierarchyOutgoingCall,
  snapshot: &StateSnapshot,
) {
  normalize_call_hierarchy_item(&mut outgoing_call.to, snapshot);
}

fn normalize_symbol_information(
  symbol_information: &mut lsp::SymbolInformation,
  snapshot: &StateSnapshot,
) {
  normalize_location(&mut symbol_information.location, snapshot)
}
