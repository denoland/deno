use super::Request;
use super::Response;
use deno_ast::ModuleSpecifier;
use deno_config::deno_json::CompilerOptions;
use deno_core::serde_json;
use deno_core::serde_json::json;
use std::collections::HashMap;
use typescript_go_client_rust::CallbackHandler;
use typescript_go_client_rust::SyncRpcChannel;
use typescript_go_client_rust::types::Project;

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
  let handler = Handler {
    config_path: "virtual:///tsconfig.json".to_string(),
    synthetic_config: synthetic_config(request.config.as_ref(), &root_names)?,
  };

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
      "configFileName": "virtual:///tsconfig.json",
    })?,
  )?;
  let project = deser::<Project>(project)?;

  let diagnostics = channel.request_sync(
    "getDiagnostics",
    jsons!({
      "project": &project.id,
    })?,
  )?;
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
  config_path: String,
  synthetic_config: String,
}

impl typescript_go_client_rust::CallbackHandler for Handler {
  fn supported_callbacks(
    &self,
  ) -> std::collections::HashSet<std::string::String> {
    ["readFile", "getAccessibleEntries", "resolveModuleName"]
      .into_iter()
      .map(|s| s.to_owned())
      .collect()
  }

  fn handle_callback(
    &self,
    name: &str,
    payload: String,
  ) -> Result<String, typescript_go_client_rust::Error> {
    match name {
      "readFile" => {
        todo!()
      }
      "getAccessibleEntries" => {
        todo!()
      }
      "resolveModuleName" => {
        todo!()
      }
      _ => unreachable!(),
    }
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
