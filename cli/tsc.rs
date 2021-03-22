// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::diagnostics::Diagnostics;
use crate::media_type::MediaType;
use crate::module_graph::Graph;
use crate::module_graph::Stats;
use crate::tsc_config::TsConfig;

use deno_core::error::anyhow;
use deno_core::error::bail;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::json_op_sync;
use deno_core::resolve_url_or_path;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpFn;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

// Declaration files

pub static DENO_NS_LIB: &str = include_str!("dts/lib.deno.ns.d.ts");
pub static DENO_CONSOLE_LIB: &str = include_str!(env!("DENO_CONSOLE_LIB_PATH"));
pub static DENO_URL_LIB: &str = include_str!(env!("DENO_URL_LIB_PATH"));
pub static DENO_WEB_LIB: &str = include_str!(env!("DENO_WEB_LIB_PATH"));
pub static DENO_FETCH_LIB: &str = include_str!(env!("DENO_FETCH_LIB_PATH"));
pub static DENO_WEBGPU_LIB: &str = include_str!(env!("DENO_WEBGPU_LIB_PATH"));
pub static DENO_WEBSOCKET_LIB: &str =
  include_str!(env!("DENO_WEBSOCKET_LIB_PATH"));
pub static DENO_CRYPTO_LIB: &str =
  include_str!(env!("DENO_CRYPTO_LIB_PATH"));
pub static SHARED_GLOBALS_LIB: &str =
  include_str!("dts/lib.deno.shared_globals.d.ts");
pub static WINDOW_LIB: &str = include_str!("dts/lib.deno.window.d.ts");
pub static UNSTABLE_NS_LIB: &str = include_str!("dts/lib.deno.unstable.d.ts");

pub static COMPILER_SNAPSHOT: &[u8] =
  include_bytes!(concat!(env!("OUT_DIR"), "/COMPILER_SNAPSHOT.bin"));

pub fn compiler_snapshot() -> Snapshot {
  Snapshot::Static(COMPILER_SNAPSHOT)
}

macro_rules! inc {
  ($e:expr) => {
    include_str!(concat!("dts/", $e))
  };
}

lazy_static! {
  /// Contains static assets that are not preloaded in the compiler snapshot.
  pub(crate) static ref STATIC_ASSETS: HashMap<&'static str, &'static str> = (&[
    ("lib.dom.asynciterable.d.ts", inc!("lib.dom.asynciterable.d.ts")),
    ("lib.dom.d.ts", inc!("lib.dom.d.ts")),
    ("lib.dom.iterable.d.ts", inc!("lib.dom.iterable.d.ts")),
    ("lib.es6.d.ts", inc!("lib.es6.d.ts")),
    ("lib.es2016.full.d.ts", inc!("lib.es2016.full.d.ts")),
    ("lib.es2017.full.d.ts", inc!("lib.es2017.full.d.ts")),
    ("lib.es2018.full.d.ts", inc!("lib.es2018.full.d.ts")),
    ("lib.es2019.full.d.ts", inc!("lib.es2019.full.d.ts")),
    ("lib.es2020.full.d.ts", inc!("lib.es2020.full.d.ts")),
    ("lib.esnext.full.d.ts", inc!("lib.esnext.full.d.ts")),
    ("lib.scripthost.d.ts", inc!("lib.scripthost.d.ts")),
    ("lib.webworker.d.ts", inc!("lib.webworker.d.ts")),
    ("lib.webworker.importscripts.d.ts", inc!("lib.webworker.importscripts.d.ts")),
    ("lib.webworker.iterable.d.ts", inc!("lib.webworker.iterable.d.ts")),
  ])
  .iter()
  .cloned()
  .collect();
}

/// Retrieve a static asset that are included in the binary.
pub fn get_asset(asset: &str) -> Option<&'static str> {
  STATIC_ASSETS.get(asset).map(|s| s.to_owned())
}

fn get_maybe_hash(
  maybe_source: &Option<String>,
  hash_data: &[Vec<u8>],
) -> Option<String> {
  if let Some(source) = maybe_source {
    let mut data = vec![source.as_bytes().to_owned()];
    data.extend_from_slice(hash_data);
    Some(crate::checksum::gen(&data))
  } else {
    None
  }
}

fn hash_data_url(
  specifier: &ModuleSpecifier,
  media_type: &MediaType,
) -> String {
  assert_eq!(
    specifier.scheme(),
    "data",
    "Specifier must be a data: specifier."
  );
  let hash = crate::checksum::gen(&[specifier.path().as_bytes()]);
  format!("data:///{}{}", hash, media_type.as_ts_extension())
}

/// tsc only supports `.ts`, `.tsx`, `.d.ts`, `.js`, or `.jsx` as root modules
/// and so we have to detect the apparent media type based on extensions it
/// supports.
fn get_tsc_media_type(specifier: &ModuleSpecifier) -> MediaType {
  let path = if specifier.scheme() == "file" {
    if let Ok(path) = specifier.to_file_path() {
      path
    } else {
      PathBuf::from(specifier.path())
    }
  } else {
    PathBuf::from(specifier.path())
  };
  match path.extension() {
    None => MediaType::Unknown,
    Some(os_str) => match os_str.to_str() {
      Some("ts") => {
        if let Some(os_str) = path.file_stem() {
          if let Some(file_name) = os_str.to_str() {
            if file_name.ends_with(".d") {
              return MediaType::Dts;
            }
          }
        }
        MediaType::TypeScript
      }
      Some("tsx") => MediaType::TSX,
      Some("js") => MediaType::JavaScript,
      Some("jsx") => MediaType::JSX,
      _ => MediaType::Unknown,
    },
  }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct EmittedFile {
  pub data: String,
  pub maybe_specifiers: Option<Vec<ModuleSpecifier>>,
  pub media_type: MediaType,
}

/// A structure representing a request to be sent to the tsc runtime.
#[derive(Debug)]
pub struct Request {
  /// The TypeScript compiler options which will be serialized and sent to
  /// tsc.
  pub config: TsConfig,
  /// Indicates to the tsc runtime if debug logging should occur.
  pub debug: bool,
  pub graph: Arc<Mutex<Graph>>,
  pub hash_data: Vec<Vec<u8>>,
  pub maybe_tsbuildinfo: Option<String>,
  /// A vector of strings that represent the root/entry point modules for the
  /// program.
  pub root_names: Vec<(ModuleSpecifier, MediaType)>,
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

#[derive(Debug)]
struct State {
  data_url_map: HashMap<String, ModuleSpecifier>,
  hash_data: Vec<Vec<u8>>,
  emitted_files: Vec<EmittedFile>,
  graph: Arc<Mutex<Graph>>,
  maybe_tsbuildinfo: Option<String>,
  maybe_response: Option<RespondArgs>,
  root_map: HashMap<String, ModuleSpecifier>,
}

impl State {
  pub fn new(
    graph: Arc<Mutex<Graph>>,
    hash_data: Vec<Vec<u8>>,
    maybe_tsbuildinfo: Option<String>,
    root_map: HashMap<String, ModuleSpecifier>,
    data_url_map: HashMap<String, ModuleSpecifier>,
  ) -> Self {
    State {
      data_url_map,
      hash_data,
      emitted_files: Default::default(),
      graph,
      maybe_tsbuildinfo,
      maybe_response: None,
      root_map,
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
          .map(|s| {
            if let Some(data_specifier) = state.data_url_map.get(s) {
              data_specifier.clone()
            } else if let Some(remapped_specifier) = state.root_map.get(s) {
              remapped_specifier.clone()
            } else {
              resolve_url_or_path(s).unwrap()
            }
          })
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
  let specifier = resolve_url_or_path(&v.specifier)
    .context("Error converting a string module specifier for \"op_load\".")?;
  let mut hash: Option<String> = None;
  let mut media_type = MediaType::Unknown;
  let data = if &v.specifier == "deno:///.tsbuildinfo" {
    state.maybe_tsbuildinfo.clone()
  // in certain situations we return a "blank" module to tsc and we need to
  // handle the request for that module here.
  } else if &v.specifier == "deno:///missing_dependency.d.ts" {
    hash = Some("1".to_string());
    media_type = MediaType::Dts;
    Some("declare const __: any;\nexport = __;\n".to_string())
  } else if v.specifier.starts_with("asset:///") {
    let name = v.specifier.replace("asset:///", "");
    let maybe_source = get_asset(&name).map(String::from);
    hash = get_maybe_hash(&maybe_source, &state.hash_data);
    media_type = MediaType::from(&v.specifier);
    maybe_source
  } else {
    let graph = state.graph.lock().unwrap();
    let specifier = if let Some(data_specifier) =
      state.data_url_map.get(&v.specifier)
    {
      data_specifier.clone()
    } else if let Some(remapped_specifier) = state.root_map.get(&v.specifier) {
      remapped_specifier.clone()
    } else {
      specifier
    };
    let maybe_source = graph.get_source(&specifier);
    media_type = if let Some(media_type) = graph.get_media_type(&specifier) {
      media_type
    } else {
      MediaType::Unknown
    };
    hash = get_maybe_hash(&maybe_source, &state.hash_data);
    maybe_source
  };

  Ok(
    json!({ "data": data, "hash": hash, "scriptKind": media_type.as_ts_script_kind() }),
  )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveArgs {
  /// The base specifier that the supplied specifier strings should be resolved
  /// relative to.
  pub base: String,
  /// A list of specifiers that should be resolved.
  pub specifiers: Vec<String>,
}

fn resolve(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: ResolveArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_resolve\".")?;
  let mut resolved: Vec<(String, String)> = Vec::new();
  let referrer = if let Some(data_specifier) = state.data_url_map.get(&v.base) {
    data_specifier.clone()
  } else if let Some(remapped_base) = state.root_map.get(&v.base) {
    remapped_base.clone()
  } else {
    resolve_url_or_path(&v.base).context(
      "Error converting a string module specifier for \"op_resolve\".",
    )?
  };
  for specifier in &v.specifiers {
    if specifier.starts_with("asset:///") {
      resolved.push((
        specifier.clone(),
        MediaType::from(specifier).as_ts_extension().to_string(),
      ));
    } else {
      let graph = state.graph.lock().unwrap();
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
          let resolved_specifier_str = if resolved_specifier.scheme() == "data"
          {
            let specifier_str = hash_data_url(&resolved_specifier, &media_type);
            state
              .data_url_map
              .insert(specifier_str.clone(), resolved_specifier);
            specifier_str
          } else {
            resolved_specifier.to_string()
          };
          resolved.push((
            resolved_specifier_str,
            media_type.as_ts_extension().into(),
          ));
        }
        // in certain situations, like certain dynamic imports, we won't have
        // the source file in the graph, so we will return a fake module to
        // make tsc happy.
        Err(_) => {
          resolved.push((
            "deno:///missing_dependency.d.ts".to_string(),
            ".d.ts".to_string(),
          ));
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
pub fn exec(request: Request) -> Result<Response, AnyError> {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(compiler_snapshot()),
    ..Default::default()
  });
  // tsc cannot handle root specifiers that don't have one of the "acceptable"
  // extensions.  Therefore, we have to check the root modules against their
  // extensions and remap any that are unacceptable to tsc and add them to the
  // op state so when requested, we can remap to the original specifier.
  let mut root_map = HashMap::new();
  let mut data_url_map = HashMap::new();
  let root_names: Vec<String> = request
    .root_names
    .iter()
    .map(|(s, mt)| {
      if s.scheme() == "data" {
        let specifier_str = hash_data_url(s, mt);
        data_url_map.insert(specifier_str.clone(), s.clone());
        specifier_str
      } else {
        let ext_media_type = get_tsc_media_type(s);
        if mt != &ext_media_type {
          let new_specifier = format!("{}{}", s, mt.as_ts_extension());
          root_map.insert(new_specifier.clone(), s.clone());
          new_specifier
        } else {
          s.as_str().to_owned()
        }
      }
    })
    .collect();

  {
    let op_state = runtime.op_state();
    let mut op_state = op_state.borrow_mut();
    op_state.put(State::new(
      request.graph.clone(),
      request.hash_data.clone(),
      request.maybe_tsbuildinfo.clone(),
      root_map,
      data_url_map,
    ));
  }

  runtime.register_op("op_create_hash", op(create_hash));
  runtime.register_op("op_emit", op(emit));
  runtime.register_op("op_load", op(load));
  runtime.register_op("op_resolve", op(resolve));
  runtime.register_op("op_respond", op(respond));

  let startup_source = "globalThis.startup({ legacyFlag: false })";
  let request_value = json!({
    "config": request.config,
    "debug": request.debug,
    "rootNames": root_names,
  });
  let request_str = request_value.to_string();
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
  use crate::module_graph::tests::MockSpecifierHandler;
  use crate::module_graph::GraphBuilder;
  use crate::tsc_config::TsConfig;
  use std::env;
  use std::path::PathBuf;
  use std::sync::Mutex;

  async fn setup(
    maybe_specifier: Option<ModuleSpecifier>,
    maybe_hash_data: Option<Vec<Vec<u8>>>,
    maybe_tsbuildinfo: Option<String>,
  ) -> State {
    let specifier = maybe_specifier
      .unwrap_or_else(|| resolve_url_or_path("file:///main.ts").unwrap());
    let hash_data = maybe_hash_data.unwrap_or_else(|| vec![b"".to_vec()]);
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/tsc2");
    let handler = Arc::new(Mutex::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None, None);
    builder
      .add(&specifier, false)
      .await
      .expect("module not inserted");
    let graph = Arc::new(Mutex::new(builder.get_graph()));
    State::new(
      graph,
      hash_data,
      maybe_tsbuildinfo,
      HashMap::new(),
      HashMap::new(),
    )
  }

  async fn test_exec(
    specifier: &ModuleSpecifier,
  ) -> Result<Response, AnyError> {
    let hash_data = vec![b"something".to_vec()];
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/tsc2");
    let handler = Arc::new(Mutex::new(MockSpecifierHandler {
      fixtures,
      ..Default::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None, None);
    builder.add(&specifier, false).await?;
    let graph = Arc::new(Mutex::new(builder.get_graph()));
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
      root_names: vec![(specifier.clone(), MediaType::TypeScript)],
    };
    exec(request)
  }

  #[test]
  fn test_compiler_snapshot() {
    let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
      startup_snapshot: Some(compiler_snapshot()),
      ..Default::default()
    });
    js_runtime
      .execute(
        "<anon>",
        r#"
      if (!(startup)) {
          throw Error("bad");
        }
        console.log(`ts version: ${ts.version}`);
      "#,
      )
      .unwrap();
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

  #[test]
  fn test_hash_data_url() {
    let specifier = deno_core::resolve_url(
      "data:application/javascript,console.log(\"Hello%20Deno\");",
    )
    .unwrap();
    assert_eq!(hash_data_url(&specifier, &MediaType::JavaScript), "data:///d300ea0796bd72b08df10348e0b70514c021f2e45bfe59cec24e12e97cd79c58.js");
  }

  #[test]
  fn test_get_tsc_media_type() {
    let fixtures = vec![
      ("file:///a.ts", MediaType::TypeScript),
      ("file:///a.tsx", MediaType::TSX),
      ("file:///a.d.ts", MediaType::Dts),
      ("file:///a.js", MediaType::JavaScript),
      ("file:///a.jsx", MediaType::JSX),
      ("file:///a.cjs", MediaType::Unknown),
      ("file:///a.mjs", MediaType::Unknown),
      ("file:///a.json", MediaType::Unknown),
      ("file:///a.wasm", MediaType::Unknown),
      ("file:///a.js.map", MediaType::Unknown),
      ("file:///.tsbuildinfo", MediaType::Unknown),
    ];
    for (specifier, media_type) in fixtures {
      let specifier = resolve_url_or_path(specifier).unwrap();
      assert_eq!(get_tsc_media_type(&specifier), media_type);
    }
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
        maybe_specifiers: Some(vec![resolve_url_or_path(
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
      Some(resolve_url_or_path("https://deno.land/x/mod.ts").unwrap()),
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

  #[derive(Debug, Deserialize)]
  #[serde(rename_all = "camelCase")]
  struct LoadResponse {
    data: String,
    hash: Option<String>,
    script_kind: i64,
  }

  #[tokio::test]
  async fn test_load_asset() {
    let mut state = setup(
      Some(resolve_url_or_path("https://deno.land/x/mod.ts").unwrap()),
      None,
      Some("some content".to_string()),
    )
    .await;
    let value =
      load(&mut state, json!({ "specifier": "asset:///lib.dom.d.ts" }))
        .expect("should have invoked op");
    let actual: LoadResponse =
      serde_json::from_value(value).expect("failed to deserialize");
    let expected = get_asset("lib.dom.d.ts").unwrap();
    assert_eq!(actual.data, expected);
    assert!(actual.hash.is_some());
    assert_eq!(actual.script_kind, 3);
  }

  #[tokio::test]
  async fn test_load_tsbuildinfo() {
    let mut state = setup(
      Some(resolve_url_or_path("https://deno.land/x/mod.ts").unwrap()),
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
      Some(resolve_url_or_path("https://deno.land/x/a.ts").unwrap()),
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
      Some(resolve_url_or_path("https://deno.land/x/a.ts").unwrap()),
      None,
      None,
    )
    .await;
    let actual = resolve(
      &mut state,
      json!({ "base": "https://deno.land/x/a.ts", "specifiers": [ "./bad.ts" ]}),
    ).expect("should have not errored");
    assert_eq!(
      actual,
      json!([["deno:///missing_dependency.d.ts", ".d.ts"]])
    );
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
        diagnostics: Diagnostics::new(vec![Diagnostic {
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
  async fn test_exec_basic() {
    let specifier = resolve_url_or_path("https://deno.land/x/a.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(actual.diagnostics.is_empty());
    assert!(actual.emitted_files.is_empty());
    assert!(actual.maybe_tsbuildinfo.is_some());
    assert_eq!(actual.stats.0.len(), 12);
  }

  #[tokio::test]
  async fn test_exec_reexport_dts() {
    let specifier = resolve_url_or_path("file:///reexports.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(actual.diagnostics.is_empty());
    assert!(actual.emitted_files.is_empty());
    assert!(actual.maybe_tsbuildinfo.is_some());
    assert_eq!(actual.stats.0.len(), 12);
  }

  #[tokio::test]
  async fn fix_lib_ref() {
    let specifier = resolve_url_or_path("file:///libref.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(actual.diagnostics.is_empty());
  }
}
