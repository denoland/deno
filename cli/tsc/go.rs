// Copyright 2018-2025 the Deno authors. MIT license.

mod tsgo_version;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json::CompilerOptions;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_error::JsErrorBox;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_typescript_go_client_rust::CallbackHandler;
use deno_typescript_go_client_rust::SyncRpcChannel;
use deno_typescript_go_client_rust::types::GetImpliedNodeFormatForFilePayload;
use deno_typescript_go_client_rust::types::Project;
use deno_typescript_go_client_rust::types::ResolveModuleNamePayload;
use deno_typescript_go_client_rust::types::ResolveTypeReferenceDirectivePayload;
use sha2::Digest;

use super::Request;
use super::Response;
use crate::cache::DenoDir;
use crate::http_util::HttpClientProvider;
use crate::tsc::RequestNpmState;
use crate::tsc::ResolveNonGraphSpecifierTypesError;
use crate::tsc::get_lazily_loaded_asset;

macro_rules! jsons {
  ($($arg:tt)*) => {
    serde_json::to_string(&json!($($arg)*))
  };
}

fn deser<T: serde::de::DeserializeOwned>(
  payload: impl AsRef<str>,
) -> Result<T, serde_json::Error> {
  serde_json::from_str::<T>(payload.as_ref())
}

fn synthetic_config(
  config: &CompilerOptions,
  root_names: &[String],
) -> Result<String, serde_json::Error> {
  let mut config = serde_json::to_value(config)?;
  config
    .as_object_mut()
    .unwrap()
    .insert("allowImportingTsExtensions".to_string(), json!(true));
  log::debug!("config: {}", serde_json::to_string(&config)?);
  serde_json::to_string(&json!({
    "compilerOptions": config,
    "files": root_names,
  }))
}

pub fn exec_request(
  request: Request,
  root_names: Vec<String>,
  root_map: HashMap<String, ModuleSpecifier>,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  tsgo_path: &Path,
) -> Result<Response, super::ExecError> {
  exec_request_inner(
    request,
    root_names,
    root_map,
    remapped_specifiers,
    tsgo_path,
  )
  .map_err(super::ExecError::Go)
}

fn exec_request_inner(
  request: Request,
  root_names: Vec<String>,
  root_map: HashMap<String, ModuleSpecifier>,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  tsgo_path: &Path,
) -> Result<Response, ExecError> {
  let handler = Handler::new(
    "/virtual/tsconfig.json".to_string(),
    synthetic_config(request.config.as_ref(), &root_names)?,
    remapped_specifiers,
    root_map,
    std::env::current_dir().unwrap(),
    request.graph.clone(),
    request.maybe_npm,
  );

  let callbacks = handler.supported_callbacks();
  let bin_path = tsgo_path;
  let mut channel = SyncRpcChannel::new(bin_path, vec!["--api"], handler)?;

  channel.request_sync(
    "configure",
    jsons!({
      "callbacks": callbacks.into_iter().collect::<Vec<_>>(),
      "logFile": ""
    })?,
  )?;

  let project = channel.request_sync(
    "loadProject",
    jsons!({
      "configFileName": "/virtual/tsconfig.json",
    })?,
  )?;
  let project = deser::<Project>(project)?;

  let diagnostics = channel.request_sync(
    "getDiagnostics",
    jsons!({
      "project": &project.id,
    })?,
  )?;
  let diagnostics = deser::<
    Vec<deno_typescript_go_client_rust::types::Diagnostic>,
  >(diagnostics)?;

  Ok(Response {
    diagnostics: convert_diagnostics(diagnostics),
    maybe_tsbuildinfo: None,
    ambient_modules: vec![],
    stats: super::Stats::default(),
  })
}

fn convert_diagnostic(
  diagnostic: deno_typescript_go_client_rust::types::Diagnostic,
  _diagnostics: &[deno_typescript_go_client_rust::types::Diagnostic],
) -> super::Diagnostic {
  let (start, end) = if diagnostic.start.line == 0
    && diagnostic.start.character == 0
    && diagnostic.end.line == 0
    && diagnostic.end.character == 0
  {
    (None, None)
  } else {
    (Some(diagnostic.start), Some(diagnostic.end))
  };

  super::Diagnostic {
    category: match diagnostic.category.as_str() {
      "error" => super::DiagnosticCategory::Error,
      "warning" => super::DiagnosticCategory::Warning,
      "message" => super::DiagnosticCategory::Message,
      "suggestion" => super::DiagnosticCategory::Suggestion,
      _ => unreachable!(),
    },
    code: diagnostic.code as u64,
    start: start.map(|s| super::Position {
      line: s.line,
      character: s.character,
    }),
    end: end.map(|e| super::Position {
      line: e.line,
      character: e.character,
    }),
    original_source_start: None,
    message_text: Some(diagnostic.message),
    message_chain: None,
    file_name: Some(diagnostic.file_name),
    missing_specifier: None,
    other: Default::default(),
    related_information: None,
    reports_deprecated: Some(diagnostic.reports_deprecated),
    reports_unnecessary: Some(diagnostic.reports_unnecessary),
    source: None,
    source_line: Some(diagnostic.source_line),
  }
}

fn convert_diagnostics(
  diagnostics: Vec<deno_typescript_go_client_rust::types::Diagnostic>,
) -> super::Diagnostics {
  super::diagnostics::Diagnostics::from(
    diagnostics
      .iter()
      .map(|diagnostic| convert_diagnostic(diagnostic.clone(), &diagnostics))
      .collect::<Vec<_>>(),
  )
}

struct Handler {
  state: RefCell<HandlerState>,
}

impl Handler {
  fn new(
    config_path: String,
    synthetic_config: String,
    remapped_specifiers: HashMap<String, ModuleSpecifier>,
    root_map: HashMap<String, ModuleSpecifier>,
    current_dir: PathBuf,
    graph: Arc<ModuleGraph>,
    maybe_npm: Option<super::RequestNpmState>,
  ) -> Self {
    if maybe_npm.is_none() {
      panic!("no npm state");
    }
    Self {
      state: RefCell::new(HandlerState {
        config_path,
        synthetic_config,
        remapped_specifiers,
        root_map,
        current_dir,
        graph,
        maybe_npm,
        module_kind_map: HashMap::new(),
      }),
    }
  }
}

fn get_package_json_scope_if_applicable(
  state: &mut HandlerState,
  payload: String,
) -> Result<String, deno_typescript_go_client_rust::Error> {
  if let Some(maybe_npm) = state.maybe_npm.as_ref() {
    let file_path = deser::<String>(&payload)?;
    let file_path = if let Ok(specifier) = ModuleSpecifier::parse(&file_path) {
      deno_path_util::url_to_file_path(&specifier).ok()
    } else {
      Some(PathBuf::from(file_path))
    };
    let Some(file_path) = file_path else {
      return Ok(jsons!(None::<String>)?);
    };
    if let Some(package_json) = maybe_npm
      .package_json_resolver
      .get_closest_package_json_path(&file_path)
    {
      let package_directory = package_json.parent();
      let contents = std::fs::read_to_string(&package_json).ok();
      if let Some(contents) = contents {
        return Ok(jsons!({
          "packageDirectory": package_directory,
          "directoryExists": true,
          "contents": contents,
        })?);
      }
    }
  }

  Ok(jsons!(None::<String>)?)
}

struct HandlerState {
  config_path: String,
  synthetic_config: String,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  root_map: HashMap<String, ModuleSpecifier>,
  current_dir: PathBuf,
  graph: Arc<ModuleGraph>,
  maybe_npm: Option<super::RequestNpmState>,

  module_kind_map:
    HashMap<String, deno_typescript_go_client_rust::types::ResolutionMode>,
}

impl deno_typescript_go_client_rust::CallbackHandler for Handler {
  fn supported_callbacks(
    &self,
  ) -> std::collections::HashSet<std::string::String> {
    [
      "readFile",
      "resolveModuleName",
      "getPackageJsonScopeIfApplicable",
      "getPackageScopeForPath",
      "resolveTypeReferenceDirective",
      "getImpliedNodeFormatForFile",
      "isNodeSourceFile",
    ]
    .into_iter()
    .map(|s| s.to_owned())
    .collect()
  }

  fn handle_callback(
    &self,
    name: &str,
    payload: String,
  ) -> Result<String, deno_typescript_go_client_rust::Error> {
    let mut state = self.state.borrow_mut();
    match name {
      "readFile" => {
        let payload = deser::<String>(payload)?;
        if payload == state.config_path {
          Ok(jsons!(&state.synthetic_config)?)
        } else {
          let contents = load_inner(&mut state, &payload).map_err(adhoc)?;
          if let Some(contents) = contents {
            Ok(jsons!(&contents)?)
          } else {
            let path = Path::new(&payload);
            if let Ok(contents) = std::fs::read_to_string(path) {
              Ok(jsons!(&contents)?)
            } else {
              Ok(jsons!(None::<String>)?)
            }
          }
        }
      }
      "resolveModuleName" => {
        let payload = deser::<ResolveModuleNamePayload>(payload)?;
        let (out_name, extension) = resolve_name(&mut state, payload)?;

        Ok(jsons!({
          "resolvedFileName": out_name,
          "extension": extension,
        })?)
      }
      "getPackageJsonScopeIfApplicable" => {
        get_package_json_scope_if_applicable(&mut state, payload)
      }
      "getPackageScopeForPath" => {
        get_package_json_scope_if_applicable(&mut state, payload)
      }
      "resolveTypeReferenceDirective" => {
        log::debug!("resolveTypeReferenceDirective: {}", payload);
        let payload = deser::<ResolveTypeReferenceDirectivePayload>(payload)?;
        let payload = ResolveModuleNamePayload {
          module_name: payload.type_reference_directive_name,
          containing_file: payload.containing_file,
          resolution_mode: payload.resolution_mode,
        };
        let (out_name, extension) = resolve_name(&mut state, payload)?;
        log::debug!(
          "resolveTypeReferenceDirective: {:?}",
          (&out_name, &extension)
        );
        Ok(jsons!({
          "resolvedFileName": out_name,
          "primary": true,
        })?)
      }
      "getImpliedNodeFormatForFile" => {
        let payload = deser::<GetImpliedNodeFormatForFilePayload>(payload)?;
        let module_kind =
          state.module_kind_map.get(&payload.file_name).unwrap_or(
            &deno_typescript_go_client_rust::types::ResolutionMode::ESM,
          );
        Ok(jsons!(&module_kind)?)
      }
      "isNodeSourceFile" => {
        let path = deser::<String>(payload)?;
        let state = &*state;
        let result = ModuleSpecifier::parse(&path)
          .ok()
          .or_else(|| {
            deno_path_util::resolve_url_or_path(&path, &state.current_dir).ok()
          })
          .and_then(|specifier| {
            state
              .maybe_npm
              .as_ref()
              .map(|n| n.node_resolver.in_npm_package(&specifier))
          })
          .unwrap_or(false);
        Ok(jsons!(result)?)
      }
      _ => unreachable!(),
    }
  }
}

fn adhoc(err: impl std::error::Error) -> deno_typescript_go_client_rust::Error {
  deno_typescript_go_client_rust::Error::AdHoc(err.to_string())
}

fn resolve_name(
  handler: &mut HandlerState,
  payload: ResolveModuleNamePayload,
) -> Result<(String, Option<&'static str>), deno_typescript_go_client_rust::Error>
{
  let graph = &handler.graph;
  let maybe_npm = handler.maybe_npm.as_ref();
  let referrer = if let Some(remapped_specifier) =
    handler.maybe_remapped_specifier(&payload.containing_file)
  {
    remapped_specifier.clone()
  } else {
    deno_path_util::resolve_url_or_path(
      &payload.containing_file,
      &handler.current_dir,
    )
    .map_err(adhoc)?
  };
  let referrer_module = graph.get(&referrer);
  let specifier = payload.module_name;
  if specifier.starts_with("node:") {
    return Ok((
      super::MISSING_DEPENDENCY_SPECIFIER.to_string(),
      Some(MediaType::Dts.as_ts_extension()),
    ));
  }

  if specifier.starts_with("asset:///") {
    let ext = MediaType::from_str(&specifier).as_ts_extension();
    return Ok((specifier, Some(ext)));
  }

  let resolved_dep = referrer_module
    .and_then(|m| match m {
      Module::Js(m) => m.dependencies_prefer_fast_check().get(&specifier),
      Module::Json(_) => None,
      Module::Wasm(m) => m.dependencies.get(&specifier),
      Module::Npm(_) | Module::Node(_) | Module::External(_) => None,
    })
    .and_then(|d| d.maybe_type.ok().or_else(|| d.maybe_code.ok()));
  let resolution_mode = if matches!(
    payload.resolution_mode,
    deno_typescript_go_client_rust::types::ResolutionMode::CommonJS
  ) {
    super::ResolutionMode::Require
  } else {
    super::ResolutionMode::Import
  };

  let maybe_result = match resolved_dep {
    Some(deno_graph::ResolutionResolved { specifier, .. }) => {
      super::resolve_graph_specifier_types(
        specifier,
        &referrer,
        // we could get this from the resolved dep, but for now assume
        // the value resolved in TypeScript is better
        resolution_mode,
        graph,
        maybe_npm,
      )
      .map_err(adhoc)?
    }
    _ => {
      match super::resolve_non_graph_specifier_types(
        &specifier,
        &referrer,
        resolution_mode,
        maybe_npm,
      ) {
        Ok(maybe_result) => maybe_result,
        Err(
          err
          @ ResolveNonGraphSpecifierTypesError::ResolvePkgFolderFromDenoReq(
            ResolvePkgFolderFromDenoReqError::Managed(_),
          ),
        ) => {
          // it's most likely requesting the jsxImportSource, which isn't loaded
          // into the graph when not using jsx, so just ignore this error
          if specifier.ends_with("/jsx-runtime") {
            None
          } else {
            return Err(adhoc(err));
          }
        }
        Err(err) => return Err(adhoc(err)),
      }
    }
  };
  let result = match maybe_result {
    Some((specifier, media_type)) => {
      let specifier_str = match specifier.scheme() {
        "data" | "blob" => {
          let specifier_str = super::hash_url(&specifier, media_type);
          handler
            .remapped_specifiers
            .insert(specifier_str.clone(), specifier);
          specifier_str
        }
        _ => {
          if let Some(specifier_str) =
            super::mapped_specifier_for_tsc(&specifier, media_type)
          {
            handler
              .remapped_specifiers
              .insert(specifier_str.clone(), specifier);
            specifier_str
          } else {
            specifier.to_string()
          }
        }
      };
      (
        specifier_str,
        match media_type {
          MediaType::Css => Some(".js"), // surface these as .js for typescript
          MediaType::Unknown => None,
          media_type => Some(media_type.as_ts_extension()),
        },
      )
    }
    None => (
      super::MISSING_DEPENDENCY_SPECIFIER.to_string(),
      Some(MediaType::Dts.as_ts_extension()),
    ),
  };
  log::debug!("Resolved {} from {} to {:?}", specifier, referrer, result);

  Ok(result)
}

impl HandlerState {
  pub fn maybe_remapped_specifier(
    &self,
    specifier: &str,
  ) -> Option<&ModuleSpecifier> {
    self
      .remapped_specifiers
      .get(specifier)
      .or_else(|| self.root_map.get(specifier))
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ExecError {
  #[class(generic)]
  #[error(transparent)]
  SerdeJson(#[from] serde_json::Error),
  #[class(generic)]
  #[error(transparent)]
  TsgoClient(#[from] deno_typescript_go_client_rust::Error),

  #[class(generic)]
  #[error("failed to load from node module: {path}: {error}")]
  LoadFromNodeModule { path: String, error: std::io::Error },

  #[class(generic)]
  #[error(transparent)]
  PackageJsonLoad(#[from] deno_package_json::PackageJsonLoadError),

  #[class(generic)]
  #[error(transparent)]
  PackageJsonLoadError(#[from] node_resolver::errors::PackageJsonLoadError),

  #[class(generic)]
  #[error(transparent)]
  DownloadError(#[from] DownloadError),
}

fn load_inner(
  state: &mut HandlerState,
  load_specifier: &str,
) -> Result<Option<String>, ExecError> {
  fn load_from_node_modules(
    specifier: &ModuleSpecifier,
    npm_state: Option<&RequestNpmState>,
    media_type: &mut MediaType,
    is_cjs: &mut bool,
  ) -> Result<Option<String>, ExecError> {
    *media_type = MediaType::from_specifier(specifier);
    let file_path = specifier.to_file_path().unwrap();
    let code = match std::fs::read_to_string(&file_path) {
      Ok(code) => code,
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        return Ok(None);
      }
      Err(err) => {
        return Err(ExecError::LoadFromNodeModule {
          path: file_path.display().to_string(),
          error: err,
        });
      }
    };
    let code_arc = code.clone().into();
    *is_cjs = npm_state
      .map(|npm_state| {
        npm_state
          .cjs_tracker
          .is_cjs(specifier, *media_type, &code_arc)
      })
      .unwrap_or(false);
    Ok(Some(code))
  }

  let specifier =
    deno_path_util::resolve_url_or_path(load_specifier, &state.current_dir)
      .map_err(adhoc)?;

  let mut media_type = MediaType::Unknown;
  let graph = &state.graph;
  let mut is_cjs = false;

  let data = if load_specifier == "internal:///.tsbuildinfo" {
    // state
    //   .maybe_tsbuildinfo
    //   .as_deref()
    //   .map(|s| s.to_string().into())
    None
    // in certain situations we return a "blank" module to tsc and we need to
    // handle the request for that module here.
  } else if load_specifier == super::MISSING_DEPENDENCY_SPECIFIER {
    None
  } else if let Some(name) = load_specifier.strip_prefix("asset:///") {
    let maybe_source = get_lazily_loaded_asset(name);
    media_type = MediaType::from_str(load_specifier);
    maybe_source.map(String::from)
  } else if let Some(source) = super::load_raw_import_source(&specifier) {
    return Ok(Some(source.to_string()));
  } else {
    let specifier = if let Some(remapped_specifier) =
      state.maybe_remapped_specifier(load_specifier)
    {
      remapped_specifier
    } else {
      &specifier
    };
    let maybe_module = graph.try_get(specifier).ok().flatten();

    if let Some(module) = maybe_module {
      match module {
        Module::Js(module) => {
          media_type = module.media_type;
          if let Some(npm_state) = &state.maybe_npm {
            is_cjs = npm_state.cjs_tracker.is_cjs_with_known_is_script(
              specifier,
              module.media_type,
              module.is_script,
            )?;
          }
          Some(
            module
              .fast_check_module()
              .map(|m| m.source.clone().to_string())
              .unwrap_or(module.source.text.clone().to_string()),
          )
        }
        Module::Json(module) => {
          media_type = MediaType::Json;
          Some(module.source.text.clone().to_string())
        }
        Module::Wasm(module) => {
          media_type = MediaType::Dts;
          Some(module.source_dts.clone().to_string())
        }
        Module::Npm(_) | Module::Node(_) => None,
        Module::External(module) => {
          if module.specifier.scheme() != "file" {
            None
          } else {
            // means it's Deno code importing an npm module
            let specifier = super::resolve_specifier_into_node_modules(
              &super::CliSys::default(),
              &module.specifier,
            );
            load_from_node_modules(
              &specifier,
              state.maybe_npm.as_ref(),
              &mut media_type,
              &mut is_cjs,
            )?
          }
        }
      }
    } else if let Some(npm) = state
      .maybe_npm
      .as_ref()
      .filter(|npm| npm.node_resolver.in_npm_package(specifier))
    {
      load_from_node_modules(
        specifier,
        Some(npm),
        &mut media_type,
        &mut is_cjs,
      )?
    } else {
      None
    }
  };
  let module_kind = get_resolution_mode(is_cjs, media_type);
  state
    .module_kind_map
    .insert(load_specifier.to_string(), module_kind);
  let Some(data) = data else {
    return Ok(None);
  };
  Ok(Some(data))
}

fn get_resolution_mode(
  is_cjs: bool,
  media_type: MediaType,
) -> deno_typescript_go_client_rust::types::ResolutionMode {
  if is_cjs {
    deno_typescript_go_client_rust::types::ResolutionMode::CommonJS
  } else {
    match media_type {
      MediaType::Cjs | MediaType::Dcts | MediaType::Cts => {
        deno_typescript_go_client_rust::types::ResolutionMode::CommonJS
      }

      MediaType::Css
      | MediaType::Json
      | MediaType::Html
      | MediaType::Sql
      | MediaType::Wasm
      | MediaType::SourceMap
      | MediaType::Unknown => {
        deno_typescript_go_client_rust::types::ResolutionMode::None
      }
      _ => deno_typescript_go_client_rust::types::ResolutionMode::ESM,
    }
  }
}

fn get_download_url(platform: &str) -> String {
  format!(
    "{}/typescript-go-{}-{}.zip",
    tsgo_version::DOWNLOAD_BASE_URL,
    tsgo_version::VERSION,
    platform
  )
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
pub enum DownloadError {
  #[error("unsupported platform for typescript-go: {0}")]
  UnsupportedPlatform(String),
  #[error("invalid download url: {0}")]
  InvalidDownloadUrl(String, #[source] deno_core::url::ParseError),
  #[error("failed to unpack typescript-go: {0}")]
  UnpackFailed(#[source] AnyError),
  #[error("failed to rename typescript-go: {0}")]
  RenameFailed(#[source] std::io::Error),
  #[error("failed to write zip file to {0}: {1}")]
  WriteZipFailed(String, #[source] std::io::Error),
  #[error("failed to download typescript-go: {0}")]
  DownloadFailed(#[source] crate::http_util::DownloadError),
  #[error("{0}")]
  HttpClient(#[source] JsErrorBox),
  #[error("failed to create temp directory: {0}")]
  CreateTempDirFailed(#[source] std::io::Error),
  #[error("hash mismatch: expected {0}, got {1}")]
  HashMismatch(String, String),
}

fn verify_hash(platform: &str, data: &[u8]) -> Result<(), DownloadError> {
  let expected_hash = match platform {
    "windows-x64" => tsgo_version::HASHES.windows_x64,
    "macos-x64" => tsgo_version::HASHES.macos_x64,
    "macos-arm64" => tsgo_version::HASHES.macos_arm64,
    "linux-x64" => tsgo_version::HASHES.linux_x64,
    "linux-arm64" => tsgo_version::HASHES.linux_arm64,
    _ => unreachable!(),
  };
  let (algorithm, expected_hash) = expected_hash.split_once(':').unwrap();
  if algorithm != "sha256" {
    panic!("Hash algorithm is not sha256");
  }

  let mut hash = sha2::Sha256::new();
  hash.update(data);
  let hash = hash.finalize();

  let hash = faster_hex::hex_string(&hash);
  if hash != expected_hash {
    return Err(DownloadError::HashMismatch(expected_hash.to_string(), hash));
  }

  Ok(())
}

pub async fn ensure_tsgo(
  deno_dir: &DenoDir,
  http_client_provider: Arc<HttpClientProvider>,
) -> Result<&'static PathBuf, DownloadError> {
  static TSGO_PATH: OnceLock<PathBuf> = OnceLock::new();

  if let Some(bin_path) = TSGO_PATH.get() {
    return Ok(bin_path);
  }

  if cfg!(debug_assertions) && std::env::var("TSGO_PATH").is_ok() {
    let tsgo_path = std::env::var("TSGO_PATH").unwrap();
    return Ok(TSGO_PATH.get_or_init(|| PathBuf::from(tsgo_path)));
  }

  let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
    ("windows", "x86_64") => "windows-x64",
    ("macos", "x86_64") => "macos-x64",
    ("macos", "aarch64") => "macos-arm64",
    ("linux", "x86_64") => "linux-x64",
    ("linux", "aarch64") => "linux-arm64",
    _ => {
      return Err(DownloadError::UnsupportedPlatform(format!(
        "{} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
      )));
    }
  };

  let folder_path = deno_dir
    .dl_folder_path()
    .join(format!("tsgo-{}", tsgo_version::VERSION));

  let bin_path = folder_path.join(format!(
    "tsgo-{}{}",
    platform,
    if cfg!(windows) { ".exe" } else { "" }
  ));

  if bin_path.exists() {
    return Ok(TSGO_PATH.get_or_init(|| bin_path));
  }

  std::fs::create_dir_all(&folder_path)
    .map_err(DownloadError::CreateTempDirFailed)?;

  let client = http_client_provider
    .get_or_create()
    .map_err(DownloadError::HttpClient)?;
  let download_url = get_download_url(platform);
  log::debug!("Downloading tsgo from {}", download_url);
  let temp = tempfile::tempdir().map_err(DownloadError::CreateTempDirFailed)?;
  let path = temp.path().join("tsgo.zip");
  log::debug!("Downloading tsgo to {}", path.display());
  let data = client
    .download(
      deno_core::url::Url::parse(&download_url)
        .map_err(|e| DownloadError::InvalidDownloadUrl(download_url, e))?,
    )
    .await
    .map_err(DownloadError::DownloadFailed)?;

  verify_hash(platform, &data)?;

  std::fs::write(&path, &data).map_err(|e| {
    DownloadError::WriteZipFailed(path.display().to_string(), e)
  })?;

  log::debug!(
    "Unpacking tsgo from {} to {}",
    path.display(),
    temp.path().display()
  );
  let unpacked_path =
    crate::util::archive::unpack_into_dir(crate::util::archive::UnpackArgs {
      exe_name: "tsgo",
      archive_name: "tsgo.zip",
      archive_data: &data,
      is_windows: cfg!(windows),
      dest_path: temp.path(),
    })
    .map_err(DownloadError::UnpackFailed)?;
  std::fs::rename(unpacked_path, &bin_path)
    .map_err(DownloadError::RenameFailed)?;

  Ok(TSGO_PATH.get_or_init(|| bin_path))
}
