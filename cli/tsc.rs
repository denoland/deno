// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::config_file::TsConfig;
use crate::diagnostics::Diagnostics;
use crate::emit;
use crate::graph_util::GraphData;
use crate::graph_util::ModuleEntry;

use deno_ast::MediaType;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::op_sync;
use deno_core::parking_lot::RwLock;
use deno_core::resolve_url_or_path;
use deno_core::serde::de;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpFn;
use deno_core::RuntimeOptions;
use deno_core::Snapshot;
use deno_graph::Resolved;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

// Declaration files

pub static DENO_NS_LIB: &str = include_str!("dts/lib.deno.ns.d.ts");
pub static DENO_CONSOLE_LIB: &str = include_str!(env!("DENO_CONSOLE_LIB_PATH"));
pub static DENO_URL_LIB: &str = include_str!(env!("DENO_URL_LIB_PATH"));
pub static DENO_WEB_LIB: &str = include_str!(env!("DENO_WEB_LIB_PATH"));
pub static DENO_FETCH_LIB: &str = include_str!(env!("DENO_FETCH_LIB_PATH"));
pub static DENO_WEBGPU_LIB: &str = include_str!(env!("DENO_WEBGPU_LIB_PATH"));
pub static DENO_WEBSOCKET_LIB: &str =
  include_str!(env!("DENO_WEBSOCKET_LIB_PATH"));
pub static DENO_WEBSTORAGE_LIB: &str =
  include_str!(env!("DENO_WEBSTORAGE_LIB_PATH"));
pub static DENO_CRYPTO_LIB: &str = include_str!(env!("DENO_CRYPTO_LIB_PATH"));
pub static DENO_BROADCAST_CHANNEL_LIB: &str =
  include_str!(env!("DENO_BROADCAST_CHANNEL_LIB_PATH"));
pub static DENO_NET_LIB: &str = include_str!(env!("DENO_NET_LIB_PATH"));
pub static SHARED_GLOBALS_LIB: &str =
  include_str!("dts/lib.deno.shared_globals.d.ts");
pub static WINDOW_LIB: &str = include_str!("dts/lib.deno.window.d.ts");
pub static UNSTABLE_NS_LIB: &str = include_str!("dts/lib.deno.unstable.d.ts");

pub static COMPILER_SNAPSHOT: Lazy<Box<[u8]>> = Lazy::new(
  #[cold]
  #[inline(never)]
  || {
    static COMPRESSED_COMPILER_SNAPSHOT: &[u8] =
      include_bytes!(concat!(env!("OUT_DIR"), "/COMPILER_SNAPSHOT.bin"));

    zstd::block::decompress(
      &COMPRESSED_COMPILER_SNAPSHOT[4..],
      u32::from_le_bytes(COMPRESSED_COMPILER_SNAPSHOT[0..4].try_into().unwrap())
        as usize,
    )
    .unwrap()
    .into_boxed_slice()
  },
);

pub fn compiler_snapshot() -> Snapshot {
  Snapshot::Static(&*COMPILER_SNAPSHOT)
}

macro_rules! inc {
  ($e:expr) => {
    include_str!(concat!("dts/", $e))
  };
}

/// Contains static assets that are not preloaded in the compiler snapshot.
pub(crate) static STATIC_ASSETS: Lazy<HashMap<&'static str, &'static str>> =
  Lazy::new(|| {
    (&[
      (
        "lib.dom.asynciterable.d.ts",
        inc!("lib.dom.asynciterable.d.ts"),
      ),
      ("lib.dom.d.ts", inc!("lib.dom.d.ts")),
      ("lib.dom.iterable.d.ts", inc!("lib.dom.iterable.d.ts")),
      ("lib.es6.d.ts", inc!("lib.es6.d.ts")),
      ("lib.es2016.full.d.ts", inc!("lib.es2016.full.d.ts")),
      ("lib.es2017.full.d.ts", inc!("lib.es2017.full.d.ts")),
      ("lib.es2018.full.d.ts", inc!("lib.es2018.full.d.ts")),
      ("lib.es2019.full.d.ts", inc!("lib.es2019.full.d.ts")),
      ("lib.es2020.full.d.ts", inc!("lib.es2020.full.d.ts")),
      ("lib.es2021.full.d.ts", inc!("lib.es2021.full.d.ts")),
      ("lib.esnext.full.d.ts", inc!("lib.esnext.full.d.ts")),
      ("lib.scripthost.d.ts", inc!("lib.scripthost.d.ts")),
      ("lib.webworker.d.ts", inc!("lib.webworker.d.ts")),
      (
        "lib.webworker.importscripts.d.ts",
        inc!("lib.webworker.importscripts.d.ts"),
      ),
      (
        "lib.webworker.iterable.d.ts",
        inc!("lib.webworker.iterable.d.ts"),
      ),
    ])
      .iter()
      .cloned()
      .collect()
  });

/// Retrieve a static asset that are included in the binary.
pub fn get_asset(asset: &str) -> Option<&'static str> {
  STATIC_ASSETS.get(asset).map(|s| s.to_owned())
}

fn get_maybe_hash(
  maybe_source: Option<&String>,
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

/// Hash the URL so it can be sent to `tsc` in a supportable way
fn hash_url(specifier: &ModuleSpecifier, media_type: &MediaType) -> String {
  let hash = crate::checksum::gen(&[specifier.path().as_bytes()]);
  format!(
    "{}:///{}{}",
    specifier.scheme(),
    hash,
    media_type.as_ts_extension()
  )
}

/// If the provided URLs derivable tsc media type doesn't match the media type,
/// we will add an extension to the output.  This is to avoid issues with
/// specifiers that don't have extensions, that tsc refuses to emit because they
/// think a `.js` version exists, when it doesn't.
fn maybe_remap_specifier(
  specifier: &ModuleSpecifier,
  media_type: &MediaType,
) -> Option<String> {
  let path = if specifier.scheme() == "file" {
    if let Ok(path) = specifier.to_file_path() {
      path
    } else {
      PathBuf::from(specifier.path())
    }
  } else {
    PathBuf::from(specifier.path())
  };
  if path.extension().is_none() {
    Some(format!("{}{}", specifier, media_type.as_ts_extension()))
  } else {
    None
  }
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
      Some("mts") => {
        if let Some(os_str) = path.file_stem() {
          if let Some(file_name) = os_str.to_str() {
            if file_name.ends_with(".d") {
              return MediaType::Dmts;
            }
          }
        }
        MediaType::Mts
      }
      Some("cts") => {
        if let Some(os_str) = path.file_stem() {
          if let Some(file_name) = os_str.to_str() {
            if file_name.ends_with(".d") {
              return MediaType::Dcts;
            }
          }
        }
        MediaType::Cts
      }
      Some("tsx") => MediaType::Tsx,
      Some("js") => MediaType::JavaScript,
      Some("mjs") => MediaType::Mjs,
      Some("cjs") => MediaType::Cjs,
      Some("jsx") => MediaType::Jsx,
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
  pub(crate) graph_data: Arc<RwLock<GraphData>>,
  pub hash_data: Vec<Vec<u8>>,
  pub maybe_config_specifier: Option<ModuleSpecifier>,
  pub maybe_tsbuildinfo: Option<String>,
  /// A vector of strings that represent the root/entry point modules for the
  /// program.
  pub root_names: Vec<(ModuleSpecifier, MediaType)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Response {
  /// Any diagnostics that have been returned from the checker.
  pub diagnostics: Diagnostics,
  /// Any files that were emitted during the check.
  pub emitted_files: Vec<EmittedFile>,
  /// If there was any build info associated with the exec request.
  pub maybe_tsbuildinfo: Option<String>,
  /// Statistics from the check.
  pub stats: emit::Stats,
}

#[derive(Debug)]
struct State {
  hash_data: Vec<Vec<u8>>,
  emitted_files: Vec<EmittedFile>,
  graph_data: Arc<RwLock<GraphData>>,
  maybe_config_specifier: Option<ModuleSpecifier>,
  maybe_tsbuildinfo: Option<String>,
  maybe_response: Option<RespondArgs>,
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  root_map: HashMap<String, ModuleSpecifier>,
}

impl State {
  pub fn new(
    graph_data: Arc<RwLock<GraphData>>,
    hash_data: Vec<Vec<u8>>,
    maybe_config_specifier: Option<ModuleSpecifier>,
    maybe_tsbuildinfo: Option<String>,
    root_map: HashMap<String, ModuleSpecifier>,
    remapped_specifiers: HashMap<String, ModuleSpecifier>,
  ) -> Self {
    State {
      hash_data,
      emitted_files: Default::default(),
      graph_data,
      maybe_config_specifier,
      maybe_tsbuildinfo,
      maybe_response: None,
      remapped_specifiers,
      root_map,
    }
  }
}

fn normalize_specifier(specifier: &str) -> Result<ModuleSpecifier, AnyError> {
  resolve_url_or_path(&specifier.replace(".d.ts.d.ts", ".d.ts"))
    .map_err(|err| err.into())
}

fn op<F, V, R>(op_fn: F) -> Box<OpFn>
where
  F: Fn(&mut State, V) -> Result<R, AnyError> + 'static,
  V: de::DeserializeOwned,
  R: Serialize + 'static,
{
  op_sync(move |s, args, _: ()| {
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

fn op_create_hash(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: CreateHashArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_create_hash\".")?;
  let mut data = vec![v.data.as_bytes().to_owned()];
  data.extend_from_slice(&state.hash_data);
  let hash = crate::checksum::gen(&data);
  Ok(json!({ "hash": hash }))
}

fn op_cwd(state: &mut State, _args: Value) -> Result<String, AnyError> {
  if let Some(config_specifier) = &state.maybe_config_specifier {
    let cwd = config_specifier.join("./")?;
    Ok(cwd.to_string())
  } else {
    Ok("cache:///".to_string())
  }
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

fn op_emit(state: &mut State, args: EmitArgs) -> Result<Value, AnyError> {
  match args.file_name.as_ref() {
    "deno:///.tsbuildinfo" => state.maybe_tsbuildinfo = Some(args.data),
    _ => {
      let media_type = MediaType::from(&args.file_name);
      let media_type = if matches!(
        media_type,
        MediaType::JavaScript
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::Dts
          | MediaType::Dmts
          | MediaType::Dcts
          | MediaType::SourceMap
          | MediaType::TsBuildInfo
      ) {
        media_type
      } else {
        MediaType::JavaScript
      };
      state.emitted_files.push(EmittedFile {
        data: args.data,
        maybe_specifiers: if let Some(specifiers) = &args.maybe_specifiers {
          let specifiers = specifiers
            .iter()
            .map(|s| {
              if let Some(data_specifier) = state.remapped_specifiers.get(s) {
                data_specifier.clone()
              } else if let Some(remapped_specifier) = state.root_map.get(s) {
                remapped_specifier.clone()
              } else {
                normalize_specifier(s).unwrap()
              }
            })
            .collect();
          Some(specifiers)
        } else {
          None
        },
        media_type,
      })
    }
  }

  Ok(json!(true))
}

#[derive(Debug, Deserialize)]
struct ExistsArgs {
  /// The fully qualified specifier that should be loaded.
  specifier: String,
}

fn op_exists(state: &mut State, args: ExistsArgs) -> Result<bool, AnyError> {
  let graph_data = state.graph_data.read();
  if let Ok(specifier) = normalize_specifier(&args.specifier) {
    if specifier.scheme() == "asset" || specifier.scheme() == "data" {
      Ok(true)
    } else {
      Ok(matches!(
        graph_data.get(&graph_data.follow_redirect(&specifier)),
        Some(ModuleEntry::Module { .. })
      ))
    }
  } else {
    Ok(false)
  }
}

#[derive(Debug, Deserialize)]
struct LoadArgs {
  /// The fully qualified specifier that should be loaded.
  specifier: String,
}

fn as_ts_script_kind(media_type: &MediaType) -> i32 {
  match media_type {
    MediaType::JavaScript => 1,
    MediaType::Jsx => 2,
    MediaType::Mjs => 1,
    MediaType::Cjs => 1,
    MediaType::TypeScript => 3,
    MediaType::Mts => 3,
    MediaType::Cts => 3,
    MediaType::Dts => 3,
    MediaType::Dmts => 3,
    MediaType::Dcts => 3,
    MediaType::Tsx => 4,
    MediaType::Json => 6,
    _ => 0,
  }
}

fn op_load(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: LoadArgs = serde_json::from_value(args)
    .context("Invalid request from JavaScript for \"op_load\".")?;
  let specifier = normalize_specifier(&v.specifier)
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
    hash = get_maybe_hash(maybe_source.as_ref(), &state.hash_data);
    media_type = MediaType::from(&v.specifier);
    maybe_source
  } else {
    let specifier = if let Some(remapped_specifier) =
      state.remapped_specifiers.get(&v.specifier)
    {
      remapped_specifier.clone()
    } else if let Some(remapped_specifier) = state.root_map.get(&v.specifier) {
      remapped_specifier.clone()
    } else {
      specifier
    };
    let graph_data = state.graph_data.read();
    let maybe_source = if let Some(ModuleEntry::Module {
      code,
      media_type: mt,
      ..
    }) =
      graph_data.get(&graph_data.follow_redirect(&specifier))
    {
      media_type = *mt;
      Some(code.as_ref().clone())
    } else {
      media_type = MediaType::Unknown;
      None
    };
    hash = get_maybe_hash(maybe_source.as_ref(), &state.hash_data);
    maybe_source
  };

  Ok(
    json!({ "data": data, "hash": hash, "scriptKind": as_ts_script_kind(&media_type) }),
  )
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveArgs {
  /// The base specifier that the supplied specifier strings should be resolved
  /// relative to.
  pub base: String,
  /// A list of specifiers that should be resolved.
  pub specifiers: Vec<String>,
}

fn op_resolve(state: &mut State, args: ResolveArgs) -> Result<Value, AnyError> {
  let mut resolved: Vec<(String, String)> = Vec::new();
  let referrer = if let Some(remapped_specifier) =
    state.remapped_specifiers.get(&args.base)
  {
    remapped_specifier.clone()
  } else if let Some(remapped_base) = state.root_map.get(&args.base) {
    remapped_base.clone()
  } else {
    normalize_specifier(&args.base).context(
      "Error converting a string module specifier for \"op_resolve\".",
    )?
  };
  for specifier in &args.specifiers {
    if specifier.starts_with("asset:///") {
      resolved.push((
        specifier.clone(),
        MediaType::from(specifier).as_ts_extension().to_string(),
      ));
    } else {
      let graph_data = state.graph_data.read();
      let referrer = graph_data.follow_redirect(&referrer);
      let resolved_dep = match graph_data.get(&referrer) {
        Some(ModuleEntry::Module { dependencies, .. }) => {
          dependencies.get(specifier).map(|d| {
            if matches!(d.maybe_type, Resolved::Ok { .. }) {
              &d.maybe_type
            } else {
              &d.maybe_code
            }
          })
        }
        Some(ModuleEntry::Configuration { dependencies }) => {
          dependencies.get(specifier)
        }
        _ => None,
      };
      let maybe_result = match resolved_dep {
        Some(Resolved::Ok { specifier, .. }) => {
          let specifier = graph_data.follow_redirect(specifier);
          match graph_data.get(&specifier) {
            Some(ModuleEntry::Module {
              media_type,
              maybe_types,
              ..
            }) => match maybe_types {
              Some(Resolved::Ok { specifier, .. }) => {
                let types = graph_data.follow_redirect(specifier);
                match graph_data.get(&types) {
                  Some(ModuleEntry::Module { media_type, .. }) => {
                    Some((types, media_type))
                  }
                  _ => None,
                }
              }
              _ => Some((specifier, media_type)),
            },
            _ => None,
          }
        }
        _ => None,
      };
      let result = match maybe_result {
        Some((specifier, media_type)) => {
          let specifier_str = match specifier.scheme() {
            "data" | "blob" => {
              let specifier_str = hash_url(&specifier, media_type);
              state
                .remapped_specifiers
                .insert(specifier_str.clone(), specifier);
              specifier_str
            }
            _ => {
              if let Some(specifier_str) =
                maybe_remap_specifier(&specifier, media_type)
              {
                state
                  .remapped_specifiers
                  .insert(specifier_str.clone(), specifier);
                specifier_str
              } else {
                specifier.to_string()
              }
            }
          };
          (specifier_str, media_type.as_ts_extension().into())
        }
        None => (
          "deno:///missing_dependency.d.ts".to_string(),
          ".d.ts".to_string(),
        ),
      };
      resolved.push(result);
    }
  }

  Ok(json!(resolved))
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct RespondArgs {
  pub diagnostics: Diagnostics,
  pub stats: emit::Stats,
}

fn op_respond(state: &mut State, args: Value) -> Result<Value, AnyError> {
  let v: RespondArgs = serde_json::from_value(args)
    .context("Error converting the result for \"op_respond\".")?;
  state.maybe_response = Some(v);
  Ok(json!(true))
}

/// Execute a request on the supplied snapshot, returning a response which
/// contains information, like any emitted files, diagnostics, statistics and
/// optionally an updated TypeScript build info.
pub(crate) fn exec(request: Request) -> Result<Response, AnyError> {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    startup_snapshot: Some(compiler_snapshot()),
    ..Default::default()
  });
  // tsc cannot handle root specifiers that don't have one of the "acceptable"
  // extensions.  Therefore, we have to check the root modules against their
  // extensions and remap any that are unacceptable to tsc and add them to the
  // op state so when requested, we can remap to the original specifier.
  let mut root_map = HashMap::new();
  let mut remapped_specifiers = HashMap::new();
  let root_names: Vec<String> = request
    .root_names
    .iter()
    .map(|(s, mt)| match s.scheme() {
      "data" | "blob" => {
        let specifier_str = hash_url(s, mt);
        remapped_specifiers.insert(specifier_str.clone(), s.clone());
        specifier_str
      }
      _ => {
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
      request.graph_data,
      request.hash_data.clone(),
      request.maybe_config_specifier.clone(),
      request.maybe_tsbuildinfo.clone(),
      root_map,
      remapped_specifiers,
    ));
  }

  runtime.register_op("op_cwd", op(op_cwd));
  runtime.register_op("op_create_hash", op(op_create_hash));
  runtime.register_op("op_emit", op(op_emit));
  runtime.register_op("op_exists", op(op_exists));
  runtime.register_op("op_load", op(op_load));
  runtime.register_op("op_resolve", op(op_resolve));
  runtime.register_op("op_respond", op(op_respond));
  runtime.sync_ops_cache();

  let startup_source = "globalThis.startup({ legacyFlag: false })";
  let request_value = json!({
    "config": request.config,
    "debug": request.debug,
    "rootNames": root_names,
  });
  let request_str = request_value.to_string();
  let exec_source = format!("globalThis.exec({})", request_str);

  runtime
    .execute_script(&located_script_name!(), startup_source)
    .context("Could not properly start the compiler runtime.")?;
  runtime.execute_script(&located_script_name!(), &exec_source)?;

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
  use crate::config_file::TsConfig;
  use crate::diagnostics::Diagnostic;
  use crate::diagnostics::DiagnosticCategory;
  use crate::emit::Stats;
  use deno_core::futures::future;
  use deno_graph::ModuleKind;
  use std::fs;

  #[derive(Debug, Default)]
  pub(crate) struct MockLoader {
    pub fixtures: PathBuf,
  }

  impl deno_graph::source::Loader for MockLoader {
    fn load(
      &mut self,
      specifier: &ModuleSpecifier,
      _is_dynamic: bool,
    ) -> deno_graph::source::LoadFuture {
      let specifier_text = specifier
        .to_string()
        .replace(":///", "_")
        .replace("://", "_")
        .replace('/', "-");
      let source_path = self.fixtures.join(specifier_text);
      let response = fs::read_to_string(&source_path)
        .map(|c| {
          Some(deno_graph::source::LoadResponse::Module {
            specifier: specifier.clone(),
            maybe_headers: None,
            content: Arc::new(c),
          })
        })
        .map_err(|err| err.into());
      Box::pin(future::ready(response))
    }
  }

  async fn setup(
    maybe_specifier: Option<ModuleSpecifier>,
    maybe_hash_data: Option<Vec<Vec<u8>>>,
    maybe_tsbuildinfo: Option<String>,
  ) -> State {
    let specifier = maybe_specifier
      .unwrap_or_else(|| resolve_url_or_path("file:///main.ts").unwrap());
    let hash_data = maybe_hash_data.unwrap_or_else(|| vec![b"".to_vec()]);
    let fixtures = test_util::testdata_path().join("tsc2");
    let mut loader = MockLoader { fixtures };
    let graph = deno_graph::create_graph(
      vec![(specifier, ModuleKind::Esm)],
      false,
      None,
      &mut loader,
      None,
      None,
      None,
      None,
    )
    .await;
    State::new(
      Arc::new(RwLock::new((&graph).into())),
      hash_data,
      None,
      maybe_tsbuildinfo,
      HashMap::new(),
      HashMap::new(),
    )
  }

  async fn test_exec(
    specifier: &ModuleSpecifier,
  ) -> Result<Response, AnyError> {
    let hash_data = vec![b"something".to_vec()];
    let fixtures = test_util::testdata_path().join("tsc2");
    let mut loader = MockLoader { fixtures };
    let graph = deno_graph::create_graph(
      vec![(specifier.clone(), ModuleKind::Esm)],
      false,
      None,
      &mut loader,
      None,
      None,
      None,
      None,
    )
    .await;
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
      graph_data: Arc::new(RwLock::new((&graph).into())),
      hash_data,
      maybe_config_specifier: None,
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
      .execute_script(
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
      op_create_hash(&mut state, json!({ "data": "some sort of content" }))
        .expect("could not invoke op");
    assert_eq!(
      actual,
      json!({"hash": "ae92df8f104748768838916857a1623b6a3c593110131b0a00f81ad9dac16511"})
    );
  }

  #[test]
  fn test_hash_url() {
    let specifier = deno_core::resolve_url(
      "data:application/javascript,console.log(\"Hello%20Deno\");",
    )
    .unwrap();
    assert_eq!(hash_url(&specifier, &MediaType::JavaScript), "data:///d300ea0796bd72b08df10348e0b70514c021f2e45bfe59cec24e12e97cd79c58.js");
  }

  #[test]
  fn test_get_tsc_media_type() {
    let fixtures = vec![
      ("file:///a.ts", MediaType::TypeScript),
      ("file:///a.cts", MediaType::Cts),
      ("file:///a.mts", MediaType::Mts),
      ("file:///a.tsx", MediaType::Tsx),
      ("file:///a.d.ts", MediaType::Dts),
      ("file:///a.d.cts", MediaType::Dcts),
      ("file:///a.d.mts", MediaType::Dmts),
      ("file:///a.js", MediaType::JavaScript),
      ("file:///a.jsx", MediaType::Jsx),
      ("file:///a.cjs", MediaType::Cjs),
      ("file:///a.mjs", MediaType::Mjs),
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
    let actual = op_emit(
      &mut state,
      EmitArgs {
        data: "some file content".to_string(),
        file_name: "cache:///some/file.js".to_string(),
        maybe_specifiers: Some(vec!["file:///some/file.ts".to_string()]),
      },
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
  async fn test_emit_strange_specifier() {
    let mut state = setup(None, None, None).await;
    let actual = op_emit(
      &mut state,
      EmitArgs {
        data: "some file content".to_string(),
        file_name: "deno:///some.file.ts?q=.json".to_string(),
        maybe_specifiers: Some(
          vec!["file:///some/file.ts?q=.json".to_string()],
        ),
      },
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
          "file:///some/file.ts?q=.json"
        )
        .unwrap()]),
        media_type: MediaType::JavaScript,
      }
    );
  }

  #[tokio::test]
  async fn test_emit_tsbuildinfo() {
    let mut state = setup(None, None, None).await;
    let actual = op_emit(
      &mut state,
      EmitArgs {
        data: "some file content".to_string(),
        file_name: "deno:///.tsbuildinfo".to_string(),
        maybe_specifiers: None,
      },
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
    let actual = op_load(
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
      op_load(&mut state, json!({ "specifier": "asset:///lib.dom.d.ts" }))
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
      op_load(&mut state, json!({ "specifier": "deno:///.tsbuildinfo"}))
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
    let actual = op_load(
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
    let actual = op_resolve(
      &mut state,
      ResolveArgs {
        base: "https://deno.land/x/a.ts".to_string(),
        specifiers: vec!["./b.ts".to_string()],
      },
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
    let actual = op_resolve(
      &mut state,
      ResolveArgs {
        base: "https://deno.land/x/a.ts".to_string(),
        specifiers: vec!["./bad.ts".to_string()],
      },
    )
    .expect("should have not errored");
    assert_eq!(
      actual,
      json!([["deno:///missing_dependency.d.ts", ".d.ts"]])
    );
  }

  #[tokio::test]
  async fn test_respond() {
    let mut state = setup(None, None, None).await;
    let actual = op_respond(
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
    eprintln!("diagnostics {:#?}", actual.diagnostics);
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
    eprintln!("diagnostics {:#?}", actual.diagnostics);
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
    eprintln!("diagnostics {:#?}", actual.diagnostics);
    assert!(actual.diagnostics.is_empty());
  }
}
