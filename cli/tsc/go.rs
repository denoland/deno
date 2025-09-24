use crate::tsc::ResolveNonGraphSpecifierTypesError;

use super::Request;
use super::Response;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json::CompilerOptions;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use typescript_go_client_rust::CallbackHandler;
use typescript_go_client_rust::SyncRpcChannel;
use typescript_go_client_rust::types::Project;
use typescript_go_client_rust::types::ResolveModuleNamePayload;

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
) -> Result<Response, super::ExecError> {
  exec_request_inner(request, root_names, root_map, remapped_specifiers)
    .map_err(super::ExecError::Go)
}

fn exec_request_inner(
  request: Request,
  root_names: Vec<String>,
  root_map: HashMap<String, ModuleSpecifier>,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
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
  let mut channel = SyncRpcChannel::new(
    "/Users/nathanwhit/Documents/Code/typescript-go/built/local/tsgo",
    vec!["--api"],
    handler,
  )?;

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
  eprintln!("diagnostics: {}", diagnostics);
  let diagnostics =
    deser::<Vec<typescript_go_client_rust::types::Diagnostic>>(diagnostics)?;

  Ok(Response {
    diagnostics: convert_diagnostics(diagnostics),
    maybe_tsbuildinfo: None,
    ambient_modules: vec![],
    stats: super::Stats::default(),
  })
}

fn convert_diagnostic(
  diagnostic: typescript_go_client_rust::types::Diagnostic,
  diagnostics: &[typescript_go_client_rust::types::Diagnostic],
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
    source_line: None,
  }
}

fn convert_diagnostics(
  diagnostics: Vec<typescript_go_client_rust::types::Diagnostic>,
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
    Self {
      state: RefCell::new(HandlerState {
        config_path,
        synthetic_config,
        remapped_specifiers,
        root_map,
        current_dir,
        graph,
        maybe_npm,
      }),
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
  maybe_npm: Option<super::RequestNpmState>,
}

impl typescript_go_client_rust::CallbackHandler for Handler {
  fn supported_callbacks(
    &self,
  ) -> std::collections::HashSet<std::string::String> {
    [
      "readFile",
      "resolveModuleName",
      "getPackageJsonScopeIfApplicable",
    ]
    .into_iter()
    .map(|s| s.to_owned())
    .collect()
  }

  fn handle_callback(
    &self,
    name: &str,
    payload: String,
  ) -> Result<String, typescript_go_client_rust::Error> {
    eprintln!("handle_callback: {}", name);
    let mut state = self.state.borrow_mut();
    match name {
      "readFile" => {
        let payload = deser::<String>(payload)?;
        if payload == state.config_path {
          Ok(jsons!(&state.synthetic_config)?)
        } else {
          eprintln!("readFile: {}", payload);
          let contents = match std::fs::read_to_string(payload) {
            Ok(contents) => contents,
            Err(e) => match e.kind() {
              std::io::ErrorKind::NotFound => {
                return Ok(jsons!(None::<String>)?);
              }
              _ => {
                return Err(typescript_go_client_rust::Error::AdHoc(
                  e.to_string(),
                ));
              }
            },
          };
          Ok(jsons!(&contents)?)
        }
      }
      "resolveModuleName" => {
        let payload = deser::<ResolveModuleNamePayload>(payload)?;
        let (out_name, extension) = resolve_name(&mut *state, payload)?;

        Ok(jsons!({
          "resolvedFileName": out_name,
          "extension": extension,
        })?)
      }
      "getPackageJsonScopeIfApplicable" => Ok(String::new()),

      _ => unreachable!(),
    }
  }
}

fn adhoc(err: impl std::error::Error) -> typescript_go_client_rust::Error {
  typescript_go_client_rust::Error::AdHoc(err.to_string())
}

fn resolve_name(
  handler: &mut HandlerState,
  payload: ResolveModuleNamePayload,
) -> Result<(String, Option<&'static str>), typescript_go_client_rust::Error> {
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
    typescript_go_client_rust::types::ResolutionMode::CommonJS
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
        &graph,
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
  TsgoClient(#[from] typescript_go_client_rust::Error),
}
