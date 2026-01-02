// Copyright 2018-2025 the Deno authors. MIT license.

mod setup;
mod tsgo_version;

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json::CompilerOptions;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_graph::ModuleGraph;
use deno_resolver::deno_json::JsxImportSourceConfigResolver;
use deno_typescript_go_client_rust::CallbackHandler;
use deno_typescript_go_client_rust::SyncRpcChannel;
use deno_typescript_go_client_rust::types::GetImpliedNodeFormatForFilePayload;
use deno_typescript_go_client_rust::types::Project;
use deno_typescript_go_client_rust::types::ResolveModuleNamePayload;
use deno_typescript_go_client_rust::types::ResolveTypeReferenceDirectivePayload;
pub use setup::DownloadError;
pub use setup::ensure_tsgo;

use super::Request;
use super::Response;
use crate::args::TypeCheckMode;

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

// the way tsgo currently works, it really wants an actual tsconfig.json file.
// it also doesn't let you just pass in root file names. instead of making more changes in tsgo,
// work around both by making a fake tsconfig.json file with the `"files"` field set to the root file names.
// it's "synthetic" because it's not actually on disk, we pass a fake path to load it from memory.
fn synthetic_config(
  config: &CompilerOptions,
  root_names: &[String],
  type_check_mode: TypeCheckMode,
) -> Result<String, serde_json::Error> {
  let mut config = serde_json::to_value(config)?;
  let obj = config.as_object_mut().unwrap();
  obj.insert("allowImportingTsExtensions".to_string(), json!(true));
  if type_check_mode != TypeCheckMode::All {
    obj.insert("skipDefaultLibCheck".to_string(), json!(true));
  }
  if let Some(jsx) = obj.get("jsx")
    && jsx.as_str() == Some("precompile")
  {
    obj.insert("jsx".to_string(), json!("react-jsx"));
  }
  obj.insert("allowArbitraryExtensions".to_string(), json!(true));
  let config = serde_json::to_string(&json!({
    "compilerOptions": config,
    "files": root_names,
  }))?;
  log::debug!("synthetic config: {}", config);
  Ok(config)
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
    synthetic_config(request.config.as_ref(), &root_names, request.check_mode)?,
    remapped_specifiers,
    root_map,
    request.initial_cwd,
    request.graph.clone(),
    request.jsx_import_source_config_resolver.clone(),
    request.maybe_npm,
  );

  let callbacks = handler.supported_callbacks();
  let bin_path = tsgo_path;
  let mut channel = SyncRpcChannel::new(bin_path, vec!["--api"], handler)?;

  channel.request_sync(
    "configure",
    jsons!({
      "callbacks": callbacks.iter().collect::<Vec<_>>(),
      "logFile": "",
      "forkContextInfo": {
        "typesNodeIgnorableNames": super::TYPES_NODE_IGNORABLE_NAMES,
        "nodeOnlyGlobalNames": super::NODE_ONLY_GLOBALS,
      },
    })?,
  )?;

  let project = channel.request_sync(
    "loadProject",
    jsons!({
      "configFileName": "/virtual/tsconfig.json",
    })?,
  )?;
  let project = deser::<Project>(project)?;

  let file_names = if request.check_mode != TypeCheckMode::All {
    root_names
  } else {
    Vec::new()
  };
  let diagnostics = channel.request_sync(
    "getDiagnostics",
    jsons!({
      "project": &project.id,
      "fileNames": file_names,
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

fn diagnostic_category(category: &str) -> super::DiagnosticCategory {
  match category {
    "error" => super::DiagnosticCategory::Error,
    "warning" => super::DiagnosticCategory::Warning,
    "message" => super::DiagnosticCategory::Message,
    "suggestion" => super::DiagnosticCategory::Suggestion,
    _ => unreachable!("unexpected diagnostic category: {category}"),
  }
}

fn maybe_rewrite_message(message: String, code: u64) -> String {
  if code == 2304 && message.starts_with("Cannot find name 'Deno'") {
    r#"Cannot find name 'Deno'. Do you need to change your target library? Try changing the 'lib' compiler option to include 'deno.ns' or add a triple-slash directive to the top of your entrypoint (main file): /// <reference lib="deno.ns" />"#.to_string()
  } else if code == 2581 {
    r#"Cannot find name '$'. Did you mean to import jQuery? Try adding `import $ from "npm:jquery";`."#.to_string()
  } else if code == 2580 {
    let regex = lazy_regex::regex!(r#"Cannot find name '([^']+)'"#);
    let captures = regex.captures(&message).unwrap();
    let name = captures.get(1).unwrap().as_str();
    format!("Cannot find name '{}'.", name)
  } else if code == 1203 {
    "Export assignment cannot be used when targeting ECMAScript modules. Consider using 'export default' or another module format instead. This will start erroring in a future version of Deno 2 in order to align with TypeScript.".to_string()
  } else if code == 2339 && message.contains("on type 'typeof Deno'") {
    let regex = lazy_regex::regex!(
      r#"Property '([^']+)' does not exist on type 'typeof Deno'"#
    );
    let captures = regex.captures(&message).unwrap();
    let name = captures.get(1).unwrap().as_str();
    format!(
      "Property '{name}' does not exist on type 'typeof Deno'. 'Deno.{name}' is an unstable API. If not, try changing the 'lib' compiler option to include 'deno.unstable' or add a triple-slash directive to the top of your entrypoint (main file): /// <reference lib=\"deno.unstable\" />",
    )
  } else {
    message
  }
}

fn maybe_remap_category(
  code: u64,
  category: super::DiagnosticCategory,
) -> super::DiagnosticCategory {
  if code == 1203 {
    super::DiagnosticCategory::Warning
  } else {
    category
  }
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
    category: maybe_remap_category(
      diagnostic.code as u64,
      diagnostic_category(diagnostic.category.as_str()),
    ),
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
    message_chain: None,
    message_text: Some(maybe_rewrite_message(
      diagnostic.message,
      diagnostic.code as u64,
    )),
    file_name: Some(diagnostic.file_name),
    missing_specifier: None,
    other: Default::default(),
    related_information: if diagnostic.related_information.is_empty() {
      None
    } else {
      Some(
        diagnostic
          .related_information
          .into_iter()
          .map(|d| convert_diagnostic(d, _diagnostics))
          .collect::<Vec<_>>(),
      )
    },
    reports_deprecated: Some(diagnostic.reports_deprecated),
    reports_unnecessary: Some(diagnostic.reports_unnecessary),
    source: None,
    source_line: Some(diagnostic.source_line),
  }
}

fn should_ignore_diagnostic(diagnostic: &super::Diagnostic) -> bool {
  super::IGNORED_DIAGNOSTIC_CODES.contains(&diagnostic.code)
}

fn convert_diagnostics(
  diagnostics: Vec<deno_typescript_go_client_rust::types::Diagnostic>,
) -> super::Diagnostics {
  super::diagnostics::Diagnostics::from(
    diagnostics
      .iter()
      .map(|diagnostic| convert_diagnostic(diagnostic.clone(), &diagnostics))
      .filter(|diagnostic| !should_ignore_diagnostic(diagnostic))
      .collect::<Vec<_>>(),
  )
}

struct Handler {
  state: RefCell<HandlerState>,
}

impl Handler {
  #[allow(clippy::too_many_arguments)]
  fn new(
    config_path: String,
    synthetic_config: String,
    remapped_specifiers: HashMap<String, ModuleSpecifier>,
    root_map: HashMap<String, ModuleSpecifier>,
    current_dir: PathBuf,
    graph: Arc<ModuleGraph>,
    jsx_import_source_config_resolver: Arc<JsxImportSourceConfigResolver>,
    maybe_npm: Option<super::RequestNpmState>,
  ) -> Self {
    Self {
      state: RefCell::new(HandlerState {
        config_path,
        synthetic_config,
        remapped_specifiers,
        root_map,
        current_dir,
        graph,
        jsx_import_source_config_resolver,
        maybe_npm,
        module_kind_map: HashMap::new(),
        load_result_pending: HashMap::new(),
      }),
    }
  }
}

fn get_package_json_scope_if_applicable(
  state: &mut HandlerState,
  payload: String,
) -> Result<String, deno_typescript_go_client_rust::Error> {
  log::debug!("get_package_json_scope_if_applicable: {}", payload);
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
      .get_closest_package_jsons(&file_path)
      .next()
      .and_then(|r| r.ok())
    {
      let package_directory = package_json.path.parent();
      let contents = serde_json::to_string(&package_json).ok();
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

fn append_raw_import_fragment(specifier: &mut String, raw_kind: &str) {
  let fragment_index = specifier.find('#');
  match fragment_index {
    Some(index) => {
      if specifier[index..].contains(&format!("denoRawImport={}.ts", raw_kind))
      {
        return;
      }
      specifier.push_str(&format!("&denoRawImport={}.ts", raw_kind));
    }
    None => {
      specifier.push_str(&format!("#denoRawImport={}.ts", raw_kind));
    }
  }
}

struct HandlerState {
  config_path: String,
  synthetic_config: String,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  root_map: HashMap<String, ModuleSpecifier>,
  current_dir: PathBuf,
  graph: Arc<ModuleGraph>,
  jsx_import_source_config_resolver: Arc<JsxImportSourceConfigResolver>,
  maybe_npm: Option<super::RequestNpmState>,

  module_kind_map:
    HashMap<String, deno_typescript_go_client_rust::types::ResolutionMode>,

  load_result_pending: HashMap<String, LoadResult>,
}

impl deno_typescript_go_client_rust::CallbackHandler for Handler {
  fn supported_callbacks(&self) -> &'static [&'static str] {
    &[
      "readFile",
      "resolveJsxImportSource",
      "resolveModuleName",
      "getPackageJsonScopeIfApplicable",
      "getPackageScopeForPath",
      "resolveTypeReferenceDirective",
      "getImpliedNodeFormatForFile",
      "isNodeSourceFile",
    ]
  }

  fn handle_callback(
    &self,
    name: &str,
    payload: String,
  ) -> Result<String, deno_typescript_go_client_rust::Error> {
    let mut state = self.state.borrow_mut();
    match name {
      "readFile" => {
        log::debug!("readFile: {}", payload);
        let payload = deser::<String>(payload)?;
        if payload == state.config_path {
          Ok(jsons!(&state.synthetic_config)?)
        } else {
          if let Some(load_result) = state.load_result_pending.remove(&payload)
          {
            return Ok(jsons!(load_result.contents)?);
          }
          let result = load_inner(&mut state, &payload).map_err(adhoc)?;

          if let Some(result) = result {
            let contents = result.contents;
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
      "loadSourceFile" => {
        let payload = deser::<String>(payload)?;
        log::debug!("loadSourceFile: {}", payload);
        if let Some(load_result) = state.load_result_pending.remove(&payload) {
          Ok(jsons!(&load_result)?)
        } else {
          let result = load_inner(&mut state, &payload).map_err(adhoc)?;
          Ok(jsons!(&result)?)
        }
      }
      "resolveModuleName" => {
        let payload = deser::<ResolveModuleNamePayload>(payload)?;
        let import_attribute_type = payload.import_attribute_type.clone();
        let (mut out_name, mut extension) = resolve_name(&mut state, payload)?;
        if let Some(import_attribute_type) = &import_attribute_type
          && matches!(import_attribute_type.as_str(), "text" | "bytes")
        {
          append_raw_import_fragment(
            &mut out_name,
            import_attribute_type.as_str(),
          );
          extension = Some("ts");
        }

        Ok(jsons!({
          "resolvedFileName": out_name,
          "extension": extension,
        })?)
      }
      "getPackageJsonScopeIfApplicable" => {
        log::debug!("getPackageJsonScopeIfApplicable: {}", payload);
        get_package_json_scope_if_applicable(&mut state, payload).inspect(
          |res| log::debug!("getPackageJsonScopeIfApplicable -> {}", res),
        )
      }
      "getPackageScopeForPath" => {
        log::debug!("getPackageScopeForPath: {}", payload);
        get_package_json_scope_if_applicable(&mut state, payload)
          .inspect(|res| log::debug!("getPackageScopeForPath -> {}", res))
      }
      "resolveTypeReferenceDirective" => {
        log::debug!("resolveTypeReferenceDirective: {}", payload);
        let payload = deser::<ResolveTypeReferenceDirectivePayload>(payload)?;
        let payload = ResolveModuleNamePayload {
          module_name: payload.type_reference_directive_name,
          containing_file: payload.containing_file,
          resolution_mode: payload.resolution_mode,
          import_attribute_type: None,
        };
        let (out_name, extension) = resolve_name(&mut state, payload)?;
        log::debug!(
          "resolveTypeReferenceDirective: {:?}",
          (&out_name, &extension)
        );
        Ok(jsons!({
          "resolvedFileName": out_name,
          "extension": extension,
          "primary": true,
        })?)
      }
      "getImpliedNodeFormatForFile" => {
        let payload = deser::<GetImpliedNodeFormatForFilePayload>(payload)?;
        log::debug!("getImpliedNodeFormatForFile: {:?}", payload);
        // check if we already determined the module kind from a previous load
        if let Some(module_kind) = state.module_kind_map.get(&payload.file_name)
        {
          log::debug!("getImpliedNodeFormatForFile -> {:?}", module_kind);
          Ok(jsons!(&module_kind)?)
        } else {
          // if not, load the file and determine the module kind
          let load_result =
            load_inner(&mut state, &payload.file_name).map_err(adhoc)?;
          if let Some(load_result) = load_result {
            // store the load result in the pending map to avoid loading the file again
            state
              .load_result_pending
              .insert(payload.file_name.clone(), load_result);
            let module_kind = state
              .module_kind_map
              .get(&payload.file_name)
              .copied()
              .unwrap_or(
                deno_typescript_go_client_rust::types::ResolutionMode::ESM,
              );
            Ok(jsons!(&module_kind)?)
          } else {
            Ok(jsons!(
              &deno_typescript_go_client_rust::types::ResolutionMode::ESM
            )?)
          }
        }
      }
      "isNodeSourceFile" => {
        let path = deser::<String>(payload)?;
        let state = &*state;
        let result = path.starts_with("asset:///node/")
          || ModuleSpecifier::parse(&path)
            .ok()
            .or_else(|| {
              deno_path_util::resolve_url_or_path(&path, &state.current_dir)
                .ok()
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
      "resolveJsxImportSource" => {
        let referrer = deser::<String>(payload)?;
        let state = &*state;
        let referrer = if let Some(remapped_specifier) =
          state.maybe_remapped_specifier(&referrer)
        {
          Some(Cow::Borrowed(remapped_specifier))
        } else {
          deno_path_util::resolve_url_or_path(&referrer, &state.current_dir)
            .ok()
            .map(Cow::Owned)
        };
        let result = referrer.and_then(|referrer| {
          state
            .jsx_import_source_config_resolver
            .for_specifier(&referrer)
            .and_then(|config| config.specifier())
        });
        Ok(jsons!(result.unwrap_or_default())?)
      }
      _ => unreachable!("unknown callback: {name}"),
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
  log::debug!("resolve_name({payload:?})");
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
  let result = super::resolve_specifier_for_tsc(
    specifier,
    &referrer,
    graph,
    match payload.resolution_mode {
      deno_typescript_go_client_rust::types::ResolutionMode::None => {
        super::ResolutionMode::Import
      }
      deno_typescript_go_client_rust::types::ResolutionMode::CommonJS => {
        super::ResolutionMode::Require
      }
      deno_typescript_go_client_rust::types::ResolutionMode::ESM => {
        super::ResolutionMode::Import
      }
    },
    maybe_npm,
    referrer_module,
    &mut handler.remapped_specifiers,
  )
  .map_err(adhoc)?;

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
  #[error(transparent)]
  PackageJsonLoad(#[from] deno_package_json::PackageJsonLoadError),

  #[class(generic)]
  #[error(transparent)]
  PackageJsonLoadError(#[from] node_resolver::errors::PackageJsonLoadError),

  #[class(generic)]
  #[error(transparent)]
  DownloadError(#[from] DownloadError),

  #[class(generic)]
  #[error(transparent)]
  LoadError(#[from] super::LoadError),
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadResult {
  contents: String,
  script_kind: i32,
}

impl super::LoadContent for String {
  fn from_static(source: &'static str) -> Self {
    source.to_string()
  }
  fn from_string(source: String) -> Self {
    source
  }
  fn from_arc_str(source: Arc<str>) -> Self {
    source.to_string()
  }
}

impl super::Mapper for HandlerState {
  fn maybe_remapped_specifier(
    &self,
    specifier: &str,
  ) -> Option<&ModuleSpecifier> {
    self.maybe_remapped_specifier(specifier)
  }
}

fn load_inner(
  state: &mut HandlerState,
  load_specifier: &str,
) -> Result<Option<LoadResult>, ExecError> {
  log::debug!("load_inner: {}", load_specifier);
  let result = super::load_for_tsc(
    load_specifier,
    state.maybe_npm.as_ref(),
    &state.current_dir,
    &state.graph,
    None,
    0,
    state,
  )?;
  let Some(result) = result else {
    log::debug!("load_inner {load_specifier} -> None");
    return Ok(None);
  };
  let is_cjs = result.is_cjs;
  let media_type = result.media_type;

  match media_type {
    MediaType::JavaScript
    | MediaType::Jsx
    | MediaType::Mjs
    | MediaType::Cjs
    | MediaType::TypeScript
    | MediaType::Mts
    | MediaType::Cts
    | MediaType::Dts
    | MediaType::Dmts
    | MediaType::Dcts
    | MediaType::Json
    | MediaType::Tsx => {}

    // anything you return from here will be treated as a js/ts
    // source file and attempt to be parsed by typescript. so
    // if it's not a js/ts source file, return None.
    MediaType::SourceMap
    | MediaType::Css
    | MediaType::Jsonc
    | MediaType::Json5
    | MediaType::Html
    | MediaType::Sql
    | MediaType::Wasm
    | MediaType::Unknown => return Ok(None),
  }

  let module_kind = get_resolution_mode(is_cjs, media_type);
  let script_kind = super::as_ts_script_kind(media_type);
  log::debug!("load_inner {load_specifier} -> {:?}", module_kind);
  log::trace!("loaded contents ({load_specifier}) -> {:?}", result.data);
  state
    .module_kind_map
    .insert(load_specifier.to_string(), module_kind);
  Ok(Some(LoadResult {
    contents: result.data,
    script_kind,
  }))
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
