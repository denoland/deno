// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::diagnostics::Diagnostics;
use crate::media_type::MediaType;
use crate::module_graph2::Graph2;
use crate::module_graph2::Stats;
use crate::tsc_config::TsConfig;

use deno_core::error::anyhow;
use deno_core::error::bail;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::json_op_sync;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpFn;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use serde::Deserialize;
use serde::Serialize;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct EmittedFile {
  pub data: String,
  pub maybe_specifiers: Option<Vec<ModuleSpecifier>>,
  pub media_type: MediaType,
}

/// A structure representing a request to be sent to the tsc runtime.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
  /// The TypeScript compiler options which will be serialized and sent to
  /// tsc.
  pub config: TsConfig,
  /// Indicates to the tsc runtime if debug logging should occur.
  pub debug: bool,
  #[serde(skip_serializing)]
  pub graph: Rc<RefCell<Graph2>>,
  #[serde(skip_serializing)]
  pub hash_data: Vec<Vec<u8>>,
  #[serde(skip_serializing)]
  pub maybe_tsbuildinfo: Option<String>,
  /// A vector of strings that represent the root/entry point modules for the
  /// program.
  pub root_names: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Response {
  /// Any diagnostics that have been returned from the checker.
  pub diagnostics: Diagnostics,
  /// Any files that were emitted during the check.
  pub emitted_files: Vec<EmittedFile>,
  /// If there was any build info associated with the exec request.
  pub maybe_tsbuildinfo: Option<String>,
  /// Statistics from the check.
  pub stats: Stats,
}

struct State {
  hash_data: Vec<Vec<u8>>,
  emitted_files: Vec<EmittedFile>,
  graph: Rc<RefCell<Graph2>>,
  maybe_tsbuildinfo: Option<String>,
  maybe_response: Option<RespondArgs>,
}

impl State {
  pub fn new(
    graph: Rc<RefCell<Graph2>>,
    hash_data: Vec<Vec<u8>>,
    maybe_tsbuildinfo: Option<String>,
  ) -> Self {
    State {
      hash_data,
      emitted_files: Vec::new(),
      graph,
      maybe_tsbuildinfo,
      maybe_response: None,
    }
  }
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
struct CreateHashArgs {
  /// The string data to be used to generate the hash.  This will be mixed with
  /// other state data in Deno to derive the final hash.
  data: String,
}

fn create_hash(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: CreateHashArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_create_hash\".")?;
  let mut data = vec![v.data.as_bytes().to_owned()];
  data.extend_from_slice(&state.hash_data);
  let hash = crate::checksum::gen(&data);
  Ok(json!({ "hash": hash }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmitArgs {
  /// The text data/contents of the file.
  data: String,
  /// The _internal_ filename for the file.  This will be used to determine how
  /// the file is cached and stored.
  file_name: String,
  /// A string representation of the specifier that was associated with a
  /// module.  This should be present on every module that represents a module
  /// that was requested to be transformed.
  maybe_specifiers: Option<Vec<String>>,
}

fn emit(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: EmitArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_emit\".")?;
  match v.file_name.as_ref() {
    "deno:///.tsbuildinfo" => state.maybe_tsbuildinfo = Some(v.data),
    _ => state.emitted_files.push(EmittedFile {
      data: v.data,
      maybe_specifiers: if let Some(specifiers) = &v.maybe_specifiers {
        let specifiers = specifiers
          .iter()
          .map(|s| ModuleSpecifier::resolve_url_or_path(s).unwrap())
          .collect();
        Some(specifiers)
      } else {
        None
      },
      media_type: MediaType::from(&v.file_name),
    }),
  }

  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
struct LoadArgs {
  /// The fully qualified specifier that should be loaded.
  specifier: String,
}

fn load(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: LoadArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_load\".")?;
  let specifier = ModuleSpecifier::resolve_url_or_path(&v.specifier)
    .context("Error converting a string module specifier for \"op_load\".")?;
  let mut hash: Option<String> = None;
  let mut media_type = MediaType::Unknown;
  let data = if &v.specifier == "deno:///.tsbuildinfo" {
    state.maybe_tsbuildinfo.clone()
  // in certain situations we return a "blank" module to tsc and we need to
  // handle the request for that module here.
  } else if &v.specifier == "deno:///none.d.ts" {
    hash = Some("1".to_string());
    media_type = MediaType::TypeScript;
    Some("declare var a: any;\nexport = a;\n".to_string())
  } else {
    let graph = state.graph.borrow();
    let maybe_source = graph.get_source(&specifier);
    media_type = if let Some(media_type) = graph.get_media_type(&specifier) {
      media_type
    } else {
      MediaType::Unknown
    };
    if let Some(source) = &maybe_source {
      let mut data = vec![source.as_bytes().to_owned()];
      data.extend_from_slice(&state.hash_data);
      hash = Some(crate::checksum::gen(&data));
    }
    maybe_source
  };

  Ok(
    json!({ "data": data, "hash": hash, "scriptKind": media_type.as_ts_script_kind() }),
  )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveArgs {
  /// The base specifier that the supplied specifier strings should be resolved
  /// relative to.
  base: String,
  /// A list of specifiers that should be resolved.
  specifiers: Vec<String>,
}

fn resolve(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: ResolveArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_resolve\".")?;
  let mut resolved: Vec<(String, String)> = Vec::new();
  let referrer = ModuleSpecifier::resolve_url_or_path(&v.base).context(
    "Error converting a string module specifier for \"op_resolve\".",
  )?;
  for specifier in &v.specifiers {
    if specifier.starts_with("asset:///") {
      resolved.push((
        specifier.clone(),
        MediaType::from(specifier).as_ts_extension().to_string(),
      ));
    } else {
      let graph = state.graph.borrow();
      match graph.resolve(specifier, &referrer, true) {
        Ok(resolved_specifier) => {
          let media_type = if let Some(media_type) =
            graph.get_media_type(&resolved_specifier)
          {
            media_type
          } else {
            bail!(
              "Unable to resolve media type for specifier: \"{}\"",
              resolved_specifier
            )
          };
          resolved.push((
            resolved_specifier.to_string(),
            media_type.as_ts_extension(),
          ));
        }
        // in certain situations, like certain dynamic imports, we won't have
        // the source file in the graph, so we will return a fake module to
        // make tsc happy.
        Err(_) => {
          resolved.push(("deno:///none.d.ts".to_string(), ".d.ts".to_string()));
        }
      }
    }
  }

  Ok(json!(resolved))
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct RespondArgs {
  pub diagnostics: Diagnostics,
  pub stats: Stats,
}

fn respond(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: RespondArgs = serde_json::from_value(args)
    .context("Error converting the result for \"op_respond\".")?;
  state.maybe_response = Some(v);
  Ok(json!(true))
}

/// Execute a request on the supplied snapshot, returning a response which
/// contains information, like any emitted files, diagnostics, statistics and
/// optionally an updated TypeScript build info.
pub fn exec(
  snapshot: Snapshot,
  request: Request,
) -> Result<Response, AnyError> {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(snapshot),
    ..Default::default()
  });

  {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    op_state.put(State::new(
      request.graph.clone(),
      request.hash_data.clone(),
      request.maybe_tsbuildinfo.clone(),
    ));
  }

  runtime.register_op("op_create_hash", op(create_hash));
  runtime.register_op("op_emit", op(emit));
  runtime.register_op("op_load", op(load));
  runtime.register_op("op_resolve", op(resolve));
  runtime.register_op("op_respond", op(respond));

  let startup_source = "globalThis.startup({ legacyFlag: false })";
  let request_str =
    serde_json::to_string(&request).context("Could not serialize request.")?;
  let exec_source = format!("globalThis.exec({})", request_str);

  runtime
    .execute("[native code]", startup_source)
    .context("Could not properly start the compiler runtime.")?;
  runtime.execute("[native_code]", &exec_source)?;

  let op_state = runtime.op_state();
  let mut op_state = op_state.borrow_mut();
  let state = op_state.take::<State>();

  if let Some(response) = state.maybe_response {
    let diagnostics = response.diagnostics;
    let emitted_files = state.emitted_files;
    let maybe_tsbuildinfo = state.maybe_tsbuildinfo;
    let stats = response.stats;

    Ok(Response {
      diagnostics,
      emitted_files,
      maybe_tsbuildinfo,
      stats,
    })
  } else {
    Err(anyhow!("The response for the exec request was not set."))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::diagnostics::Diagnostic;
  use crate::diagnostics::DiagnosticCategory;
  use crate::js;
  use crate::module_graph2::tests::MockSpecifierHandler;
  use crate::module_graph2::GraphBuilder2;
  use crate::tsc_config::TsConfig;
  use std::cell::RefCell;
  use std::env;
  use std::path::PathBuf;

  async fn setup(
    maybe_specifier: Option<ModuleSpecifier>,
    maybe_hash_data: Option<Vec<Vec<u8>>>,
    maybe_tsbuildinfo: Option<String>,
  ) -> State {
    let specifier = maybe_specifier.unwrap_or_else(|| {
      ModuleSpecifier::resolve_url_or_path("file:///main.ts").unwrap()
    });
    let hash_data = maybe_hash_data.unwrap_or_else(|| vec![b"".to_vec()]);
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/tsc2");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder2::new(handler.clone(), None, None);
    builder
      .add(&specifier, false)
      .await
      .expect("module not inserted");
    let graph = Rc::new(RefCell::new(builder.get_graph()));
    State::new(graph, hash_data, maybe_tsbuildinfo)
  }

  #[tokio::test]
  async fn test_create_hash() {
    let mut state = setup(None, Some(vec![b"something".to_vec()]), None).await;
    let actual =
      create_hash(&mut state, json!({ "data": "some sort of content" }))
        .expect("could not invoke op");
    assert_eq!(
      actual,
      json!({"hash": "ae92df8f104748768838916857a1623b6a3c593110131b0a00f81ad9dac16511"})
    );
  }

  #[tokio::test]
  async fn test_emit() {
    let mut state = setup(None, None, None).await;
    let actual = emit(
      &mut state,
      json!({
        "data": "some file content",
        "fileName": "cache:///some/file.js",
        "maybeSpecifiers": ["file:///some/file.ts"]
      }),
    )
    .expect("should have invoked op");
    assert_eq!(actual, json!(true));
    assert_eq!(state.emitted_files.len(), 1);
    assert!(state.maybe_tsbuildinfo.is_none());
    assert_eq!(
      state.emitted_files[0],
      EmittedFile {
        data: "some file content".to_string(),
        maybe_specifiers: Some(vec![ModuleSpecifier::resolve_url_or_path(
          "file:///some/file.ts"
        )
        .unwrap()]),
        media_type: MediaType::JavaScript,
      }
    );
  }

  #[tokio::test]
  async fn test_emit_tsbuildinfo() {
    let mut state = setup(None, None, None).await;
    let actual = emit(
      &mut state,
      json!({
        "data": "some file content",
        "fileName": "deno:///.tsbuildinfo",
      }),
    )
    .expect("should have invoked op");
    assert_eq!(actual, json!(true));
    assert_eq!(state.emitted_files.len(), 0);
    assert_eq!(
      state.maybe_tsbuildinfo,
      Some("some file content".to_string())
    );
  }

  #[tokio::test]
  async fn test_load() {
    let mut state = setup(
      Some(
        ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
          .unwrap(),
      ),
      None,
      Some("some content".to_string()),
    )
    .await;
    let actual = load(
      &mut state,
      json!({ "specifier": "https://deno.land/x/mod.ts"}),
    )
    .expect("should have invoked op");
    assert_eq!(
      actual,
      json!({
        "data": "console.log(\"hello deno\");\n",
        "hash": "149c777056afcc973d5fcbe11421b6d5ddc57b81786765302030d7fc893bf729",
        "scriptKind": 3,
      })
    );
  }

  #[tokio::test]
  async fn test_load_tsbuildinfo() {
    let mut state = setup(
      Some(
        ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
          .unwrap(),
      ),
      None,
      Some("some content".to_string()),
    )
    .await;
    let actual =
      load(&mut state, json!({ "specifier": "deno:///.tsbuildinfo"}))
        .expect("should have invoked op");
    assert_eq!(
      actual,
      json!({
        "data": "some content",
        "hash": null,
        "scriptKind": 0,
      })
    );
  }

  #[tokio::test]
  async fn test_load_missing_specifier() {
    let mut state = setup(None, None, None).await;
    let actual = load(
      &mut state,
      json!({ "specifier": "https://deno.land/x/mod.ts"}),
    )
    .expect("should have invoked op");
    assert_eq!(
      actual,
      json!({
        "data": null,
        "hash": null,
        "scriptKind": 0,
      })
    )
  }

  #[tokio::test]
  async fn test_resolve() {
    let mut state = setup(
      Some(
        ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts")
          .unwrap(),
      ),
      None,
      None,
    )
    .await;
    let actual = resolve(
      &mut state,
      json!({ "base": "https://deno.land/x/a.ts", "specifiers": [ "./b.ts" ]}),
    )
    .expect("should have invoked op");
    assert_eq!(actual, json!([["https://deno.land/x/b.ts", ".ts"]]));
  }

  #[tokio::test]
  async fn test_resolve_empty() {
    let mut state = setup(
      Some(
        ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts")
          .unwrap(),
      ),
      None,
      None,
    )
    .await;
    let actual = resolve(
      &mut state,
      json!({ "base": "https://deno.land/x/a.ts", "specifiers": [ "./bad.ts" ]}),
    ).expect("should have not errored");
    assert_eq!(actual, json!([["deno:///none.d.ts", ".d.ts"]]));
  }

  #[tokio::test]
  async fn test_respond() {
    let mut state = setup(None, None, None).await;
    let actual = respond(
      &mut state,
      json!({
        "diagnostics": [
          {
            "messageText": "Unknown compiler option 'invalid'.",
            "category": 1,
            "code": 5023
          }
        ],
        "stats": [["a", 12]]
      }),
    )
    .expect("should have invoked op");
    assert_eq!(actual, json!(true));
    assert_eq!(
      state.maybe_response,
      Some(RespondArgs {
        diagnostics: Diagnostics(vec![Diagnostic {
          category: DiagnosticCategory::Error,
          code: 5023,
          start: None,
          end: None,
          message_text: Some(
            "Unknown compiler option \'invalid\'.".to_string()
          ),
          message_chain: None,
          source: None,
          source_line: None,
          file_name: None,
          related_information: None,
        }]),
        stats: Stats(vec![("a".to_string(), 12)])
      })
    );
  }

  #[tokio::test]
  async fn test_exec() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts").unwrap();
    let hash_data = vec![b"something".to_vec()];
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/tsc2");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder2::new(handler.clone(), None, None);
    builder
      .add(&specifier, false)
      .await
      .expect("module not inserted");
    let graph = Rc::new(RefCell::new(builder.get_graph()));
    let config = TsConfig::new(json!({
      "allowJs": true,
      "checkJs": false,
      "esModuleInterop": true,
      "emitDecoratorMetadata": false,
      "incremental": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "lib": ["deno.window"],
      "module": "esnext",
      "noEmit": true,
      "outDir": "deno:///",
      "strict": true,
      "target": "esnext",
      "tsBuildInfoFile": "deno:///.tsbuildinfo",
    }));
    let request = Request {
      config,
      debug: false,
      graph,
      hash_data,
      maybe_tsbuildinfo: None,
      root_names: vec!["https://deno.land/x/a.ts".to_string()],
    };
    let actual = exec(js::compiler_isolate_init(), request)
      .expect("exec should have not errored");
    assert!(actual.diagnostics.0.is_empty());
    assert!(actual.emitted_files.is_empty());
    assert!(actual.maybe_tsbuildinfo.is_some());
    assert_eq!(actual.stats.0.len(), 12);
  }
}
