// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::state::ServerStateSnapshot;
use super::text;

use crate::media_type::MediaType;
use crate::tsc::ResolveArgs;
use crate::tsc_config::TsConfig;

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::json_op_sync;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpFn;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
struct Response {
  id: usize,
  data: Value,
}

struct State<'a> {
  last_id: usize,
  response: Option<Response>,
  server_state: ServerStateSnapshot,
  snapshots: HashMap<(Cow<'a, str>, Cow<'a, str>), String>,
}

impl<'a> State<'a> {
  fn new(server_state: ServerStateSnapshot) -> Self {
    Self {
      last_id: 1,
      response: None,
      server_state,
      snapshots: Default::default(),
    }
  }
}

/// If a snapshot is missing from the state cache, add it.
fn cache_snapshot(
  state: &mut State,
  specifier: String,
  version: String,
) -> Result<(), AnyError> {
  if !state
    .snapshots
    .contains_key(&(specifier.clone().into(), version.clone().into()))
  {
    let s = ModuleSpecifier::resolve_url(&specifier)?;
    let file_cache = state.server_state.file_cache.read().unwrap();
    let file_id = file_cache.lookup(&s).unwrap();
    let content = file_cache.get_contents(file_id)?;
    state
      .snapshots
      .insert((specifier.into(), version.into()), content);
  }
  Ok(())
}

fn op<F>(op_fn: F) -> Box<OpFn>
where
  F: Fn(&mut State, Value) -> Result<Value, AnyError> + 'static,
{
  json_op_sync(move |s, args, _bufs| {
    let state = s.borrow_mut::<State>();
    op_fn(state, args)
  })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceSnapshotArgs {
  specifier: String,
  version: String,
}

/// The language service is dropping a reference to a source file snapshot, and
/// we can drop our version of that document.
fn dispose(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: SourceSnapshotArgs = serde_json::from_value(args)?;
  state
    .snapshots
    .remove(&(v.specifier.into(), v.version.into()));
  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetChangeRangeArgs {
  specifier: String,
  old_version: String,
  version: String,
}

/// The language service wants to compare an old snapshot with a new snapshot to
/// determine what source hash changed.
fn get_change_range(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: GetChangeRangeArgs = serde_json::from_value(args.clone())?;
  cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
  if let (Some(a), Some(b)) = (
    state
      .snapshots
      .get(&(v.specifier.clone().into(), v.old_version.clone().into())),
    state
      .snapshots
      .get(&(v.specifier.clone().into(), v.version.into())),
  ) {
    Ok(text::get_range_change(a, b))
  } else {
    Err(custom_error(
      "MissingSnapshot",
      format!(
        "One of the snapshotted versions is missing.\n  Args: \"{}\"",
        args
      ),
    ))
  }
}

fn get_length(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: SourceSnapshotArgs = serde_json::from_value(args)?;
  cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
  let content = state
    .snapshots
    .get(&(v.specifier.into(), v.version.into()))
    .unwrap();
  Ok(json!(content.chars().count()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetTextArgs {
  specifier: String,
  version: String,
  start: usize,
  end: usize,
}

fn get_text(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: GetTextArgs = serde_json::from_value(args)?;
  cache_snapshot(state, v.specifier.clone(), v.version.clone())?;
  let content = state
    .snapshots
    .get(&(v.specifier.into(), v.version.into()))
    .unwrap();
  Ok(json!(text::slice(content, v.start..v.end)))
}

fn resolve(_state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: ResolveArgs = serde_json::from_value(args)?;
  let mut resolved = Vec::<(String, String)>::new();
  for specifier in &v.specifiers {
    if specifier.starts_with("asset:///") {
      resolved.push((
        specifier.clone(),
        MediaType::from(specifier).as_ts_extension().to_string(),
      ));
    } else {
      let resolved_specifier =
        ModuleSpecifier::resolve_import(specifier, &v.base)?;
      resolved.push((
        resolved_specifier.to_string(),
        MediaType::from(&resolved_specifier)
          .as_ts_extension()
          .to_string(),
      ));
    }
  }

  Ok(json!(resolved))
}

fn respond(state: &mut State, args: Value) -> Result<Value, AnyError> {
  state.response = Some(serde_json::from_value(args)?);
  Ok(json!(true))
}

#[allow(clippy::unnecessary_wraps)]
fn script_names(state: &mut State, _args: Value) -> Result<Value, AnyError> {
  let script_names: Vec<&ModuleSpecifier> =
    state.server_state.doc_data.keys().collect();
  Ok(json!(script_names))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptVersionArgs {
  specifier: String,
}

fn script_version(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: ScriptVersionArgs = serde_json::from_value(args)?;
  let specifier = ModuleSpecifier::resolve_url(&v.specifier)?;
  let maybe_doc_data = state.server_state.doc_data.get(&specifier);
  if let Some(doc_data) = maybe_doc_data {
    if let Some(version) = doc_data.version {
      return Ok(json!(version.to_string()));
    }
  }

  Ok(json!(None::<String>))
}

pub fn start(snapshot: Snapshot, debug: bool) -> Result<JsRuntime, AnyError> {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });

  {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    op_state.put(State::new(ServerStateSnapshot::default()));
  }

  runtime.register_op("op_dispose", op(dispose));
  runtime.register_op("op_get_change_range", op(get_change_range));
  runtime.register_op("op_get_length", op(get_length));
  runtime.register_op("op_get_text", op(get_text));
  runtime.register_op("op_resolve", op(resolve));
  runtime.register_op("op_respond", op(respond));
  runtime.register_op("op_script_names", op(script_names));
  runtime.register_op("op_script_version", op(script_version));

  let init_config = json!({ "debug": debug });
  let init_src = format!("globalThis.serverInit({});", init_config);

  runtime.execute("[native code]", &init_src)?;
  Ok(runtime)
}

/// Methods that are supported by the Language Service in the compiler isolate.
pub enum RequestMethod {
  /// Configure the compilation settings for the server.
  Configure(TsConfig),
  /// Return semantic diagnostics for given file.
  GetSemanticDiagnostics(ModuleSpecifier),
  /// Returns suggestion diagnostics for given file.
  GetSuggestionDiagnostics(ModuleSpecifier),
  /// Return syntactic diagnostics for a given file.
  GetSyntacticDiagnostics(ModuleSpecifier),
}

impl RequestMethod {
  pub fn to_value(&self, id: usize) -> Value {
    match self {
      RequestMethod::Configure(config) => json!({
        "id": id,
        "method": "configure",
        "compilerOptions": config,
      }),
      RequestMethod::GetSemanticDiagnostics(specifier) => json!({
        "id": id,
        "method": "getSemanticDiagnostics",
        "specifier": specifier,
      }),
      RequestMethod::GetSuggestionDiagnostics(specifier) => json!({
        "id": id,
        "method": "getSuggestionDiagnostics",
        "specifier": specifier,
      }),
      RequestMethod::GetSyntacticDiagnostics(specifier) => json!({
        "id": id,
        "method": "getSyntacticDiagnostics",
        "specifier": specifier,
      }),
    }
  }
}

/// Send a request into a runtime and return the JSON value of the response.
pub fn request(
  runtime: &mut JsRuntime,
  server_state: &ServerStateSnapshot,
  method: RequestMethod,
) -> Result<Value, AnyError> {
  let id = {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    let state = op_state.borrow_mut::<State>();
    state.server_state = server_state.clone();
    state.last_id += 1;
    state.last_id
  };
  let request_params = method.to_value(id);
  let request_src = format!("globalThis.serverRequest({});", request_params);
  runtime.execute("[native_code]", &request_src)?;

  let op_state = runtime.op_state();
  let mut op_state = op_state.borrow_mut();
  let state = op_state.borrow_mut::<State>();

  if let Some(response) = state.response.clone() {
    state.response = None;
    Ok(response.data)
  } else {
    Err(custom_error(
      "RequestError",
      "The response was not received for the request.",
    ))
  }
}

#[cfg(test)]
mod tests {
  use super::super::memory_cache::MemoryCache;
  use super::super::state::DocumentData;
  use super::*;
  use crate::js;
  use std::collections::HashMap;
  use std::sync::Arc;
  use std::sync::RwLock;

  fn mock_server_state(sources: Vec<(&str, &str, i32)>) -> ServerStateSnapshot {
    let mut doc_data = HashMap::new();
    let mut file_cache = MemoryCache::default();
    for (specifier, content, version) in sources {
      let specifier = ModuleSpecifier::resolve_url(specifier)
        .expect("failed to create specifier");
      let data = DocumentData {
        version: Some(version),
      };
      doc_data.insert(specifier.clone(), data);
      file_cache.set_contents(specifier, Some(content.as_bytes().to_vec()));
    }
    let file_cache = Arc::new(RwLock::new(file_cache));
    ServerStateSnapshot {
      config: Default::default(),
      diagnostics: Default::default(),
      doc_data,
      file_cache,
    }
  }

  fn setup(
    debug: bool,
    config: Value,
    sources: Vec<(&str, &str, i32)>,
  ) -> (JsRuntime, ServerStateSnapshot) {
    let server_state = mock_server_state(sources.clone());
    let mut runtime = start(js::compiler_isolate_init(), debug)
      .expect("could not start server");
    let ts_config = TsConfig::new(config);
    assert_eq!(
      request(
        &mut runtime,
        &server_state,
        RequestMethod::Configure(ts_config)
      )
      .expect("failed request"),
      json!(true)
    );
    (runtime, server_state)
  }

  #[test]
  fn test_project_configure() {
    setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      vec![],
    );
  }

  #[test]
  fn test_project_reconfigure() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      vec![],
    );
    let ts_config = TsConfig::new(json!({
      "target": "esnext",
      "module": "esnext",
      "noEmit": true,
      "lib": ["deno.ns", "deno.worker"]
    }));
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::Configure(ts_config),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!(true));
  }

  #[test]
  fn test_get_semantic_diagnostics() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "noEmit": true,
      }),
      vec![("file:///a.ts", r#"console.log("hello deno");"#, 1)],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::GetSemanticDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
      response,
      json!([
        {
          "start": {
            "line": 0,
            "character": 0,
          },
          "end": {
            "line": 0,
            "character": 7
          },
          "fileName": "file:///a.ts",
          "messageText": "Cannot find name 'console'. Do you need to change your target library? Try changing the `lib` compiler option to include 'dom'.",
          "sourceLine": "console.log(\"hello deno\");",
          "category": 1,
          "code": 2584
        }
      ])
    );
  }

  #[test]
  fn test_module_resolution() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      vec![(
        "file:///a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
      )],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::GetSemanticDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!([]));
  }

  #[test]
  fn test_remote_modules() {
    let (mut runtime, server_state) = setup(
      false,
      json!({
        "target": "esnext",
        "module": "esnext",
        "lib": ["deno.ns", "deno.window"],
        "noEmit": true,
      }),
      vec![(
        "file:///a.ts",
        r#"
        import { B } from "https://deno.land/x/b/mod.ts";

        const b = new B();

        console.log(b);
      "#,
        1,
      )],
    );
    let specifier = ModuleSpecifier::resolve_url("file:///a.ts")
      .expect("could not resolve url");
    let result = request(
      &mut runtime,
      &server_state,
      RequestMethod::GetSyntacticDiagnostics(specifier),
    );
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response, json!([]));
  }
}
