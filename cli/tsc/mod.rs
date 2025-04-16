// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;

use deno_ast::MediaType;
use deno_core::anyhow::Context;
use deno_core::located_script_name;
use deno_core::op2;
use deno_core::resolve_url_or_path;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_core::FastString;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::RuntimeOptions;
use deno_graph::GraphKind;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_graph::ResolutionResolved;
use deno_lib::util::checksum;
use deno_lib::util::hash::FastInsecureHasher;
use deno_lib::worker::create_isolate_create_params;
use deno_resolver::npm::managed::ResolvePkgFolderFromDenoModuleError;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_semver::npm::NpmPackageReqReference;
use indexmap::IndexMap;
use node_resolver::errors::NodeJsErrorCode;
use node_resolver::errors::NodeJsErrorCoded;
use node_resolver::errors::PackageSubpathResolveError;
use node_resolver::resolve_specifier_into_node_modules;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use once_cell::sync::Lazy;
use thiserror::Error;

use crate::args::TsConfig;
use crate::args::TypeCheckMode;
use crate::cache::ModuleInfoCache;
use crate::node::CliNodeResolver;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::sys::CliSys;
use crate::util::path::mapped_specifier_for_tsc;

mod diagnostics;

pub use self::diagnostics::Diagnostic;
pub use self::diagnostics::DiagnosticCategory;
pub use self::diagnostics::Diagnostics;
pub use self::diagnostics::Position;

pub fn get_types_declaration_file_text() -> String {
  let lib_names = vec![
    "deno.ns",
    "deno.console",
    "deno.url",
    "deno.web",
    "deno.fetch",
    "deno.webgpu",
    "deno.websocket",
    "deno.webstorage",
    "deno.canvas",
    "deno.crypto",
    "deno.broadcast_channel",
    "deno.net",
    "deno.shared_globals",
    "deno.cache",
    "deno.window",
    "deno.unstable",
  ];

  lib_names
    .into_iter()
    .map(|name| {
      let lib_name = format!("lib.{name}.d.ts");
      LAZILY_LOADED_STATIC_ASSETS
        .get(lib_name.as_str())
        .unwrap()
        .as_str()
    })
    .collect::<Vec<_>>()
    .join("\n")
}

macro_rules! maybe_compressed_source {
  ($file: expr) => {{
    maybe_compressed_source!(compressed = $file, uncompressed = $file)
  }};
  (compressed = $comp: expr, uncompressed = $uncomp: expr) => {{
    #[cfg(feature = "hmr")]
    {
      StaticAssetSource::Owned(
        concat!(env!("CARGO_MANIFEST_DIR"), "/", $uncomp),
        std::sync::OnceLock::new(),
      )
    }
    #[cfg(not(feature = "hmr"))]
    {
      #[cfg(debug_assertions)]
      {
        StaticAssetSource::Uncompressed(include_str!(concat!(
          env!("CARGO_MANIFEST_DIR"),
          "/",
          $uncomp
        )))
      }
      #[cfg(not(debug_assertions))]
      {
        StaticAssetSource::Compressed(CompressedSource::new(include_bytes!(
          concat!(env!("OUT_DIR"), "/", $comp, ".zstd")
        )))
      }
    }
  }};
}

macro_rules! maybe_compressed_lib {
  ($name: expr, $file: expr) => {
    ($name, maybe_compressed_source!(concat!("tsc/dts/", $file)))
  };
  ($e: expr) => {
    maybe_compressed_lib!($e, $e)
  };
}

#[derive(Clone)]
pub enum StaticAssetSource {
  #[cfg_attr(any(debug_assertions, feature = "hmr"), allow(dead_code))]
  Compressed(CompressedSource),
  Uncompressed(&'static str),
  #[cfg_attr(not(feature = "hmr"), allow(dead_code))]
  Owned(&'static str, std::sync::OnceLock<Arc<str>>),
}

impl StaticAssetSource {
  pub fn as_str(&'static self) -> &'static str {
    match self {
      StaticAssetSource::Compressed(compressed_source) => {
        compressed_source.get()
      }
      StaticAssetSource::Uncompressed(src) => src,
      StaticAssetSource::Owned(path, cell) => {
        let str =
          cell.get_or_init(|| std::fs::read_to_string(path).unwrap().into());
        str.as_ref()
      }
    }
  }
}

/// Contains static assets that are not preloaded in the compiler snapshot.
///
/// We lazily load these because putting them in the compiler snapshot will
/// increase memory usage when not used (last time checked by about 0.5MB).
pub static LAZILY_LOADED_STATIC_ASSETS: Lazy<
  IndexMap<&'static str, StaticAssetSource>,
> = Lazy::new(|| {
  IndexMap::from([
    // compressed in build.rs
    maybe_compressed_lib!("lib.deno.console.d.ts", "lib.deno_console.d.ts"),
    maybe_compressed_lib!("lib.deno.url.d.ts", "lib.deno_url.d.ts"),
    maybe_compressed_lib!("lib.deno.web.d.ts", "lib.deno_web.d.ts"),
    maybe_compressed_lib!("lib.deno.fetch.d.ts", "lib.deno_fetch.d.ts"),
    maybe_compressed_lib!("lib.deno.websocket.d.ts", "lib.deno_websocket.d.ts"),
    maybe_compressed_lib!(
      "lib.deno.webstorage.d.ts",
      "lib.deno_webstorage.d.ts"
    ),
    maybe_compressed_lib!("lib.deno.canvas.d.ts", "lib.deno_canvas.d.ts"),
    maybe_compressed_lib!("lib.deno.crypto.d.ts", "lib.deno_crypto.d.ts"),
    maybe_compressed_lib!(
      "lib.deno.broadcast_channel.d.ts",
      "lib.deno_broadcast_channel.d.ts"
    ),
    maybe_compressed_lib!("lib.deno.net.d.ts", "lib.deno_net.d.ts"),
    maybe_compressed_lib!("lib.deno.cache.d.ts", "lib.deno_cache.d.ts"),
    maybe_compressed_lib!("lib.deno.webgpu.d.ts", "lib.deno_webgpu.d.ts"),
    maybe_compressed_lib!("lib.deno.window.d.ts"),
    maybe_compressed_lib!("lib.deno.worker.d.ts"),
    maybe_compressed_lib!("lib.deno.shared_globals.d.ts"),
    maybe_compressed_lib!("lib.deno.ns.d.ts"),
    maybe_compressed_lib!("lib.deno.unstable.d.ts"),
    maybe_compressed_lib!("lib.decorators.d.ts"),
    maybe_compressed_lib!("lib.decorators.legacy.d.ts"),
    maybe_compressed_lib!("lib.dom.asynciterable.d.ts"),
    maybe_compressed_lib!("lib.dom.d.ts"),
    maybe_compressed_lib!("lib.dom.extras.d.ts"),
    maybe_compressed_lib!("lib.dom.iterable.d.ts"),
    maybe_compressed_lib!("lib.es2015.collection.d.ts"),
    maybe_compressed_lib!("lib.es2015.core.d.ts"),
    maybe_compressed_lib!("lib.es2015.d.ts"),
    maybe_compressed_lib!("lib.es2015.generator.d.ts"),
    maybe_compressed_lib!("lib.es2015.iterable.d.ts"),
    maybe_compressed_lib!("lib.es2015.promise.d.ts"),
    maybe_compressed_lib!("lib.es2015.proxy.d.ts"),
    maybe_compressed_lib!("lib.es2015.reflect.d.ts"),
    maybe_compressed_lib!("lib.es2015.symbol.d.ts"),
    maybe_compressed_lib!("lib.es2015.symbol.wellknown.d.ts"),
    maybe_compressed_lib!("lib.es2016.array.include.d.ts"),
    maybe_compressed_lib!("lib.es2016.d.ts"),
    maybe_compressed_lib!("lib.es2016.full.d.ts"),
    maybe_compressed_lib!("lib.es2016.intl.d.ts"),
    maybe_compressed_lib!("lib.es2017.arraybuffer.d.ts"),
    maybe_compressed_lib!("lib.es2017.d.ts"),
    maybe_compressed_lib!("lib.es2017.date.d.ts"),
    maybe_compressed_lib!("lib.es2017.full.d.ts"),
    maybe_compressed_lib!("lib.es2017.intl.d.ts"),
    maybe_compressed_lib!("lib.es2017.object.d.ts"),
    maybe_compressed_lib!("lib.es2017.sharedmemory.d.ts"),
    maybe_compressed_lib!("lib.es2017.string.d.ts"),
    maybe_compressed_lib!("lib.es2017.typedarrays.d.ts"),
    maybe_compressed_lib!("lib.es2018.asyncgenerator.d.ts"),
    maybe_compressed_lib!("lib.es2018.asynciterable.d.ts"),
    maybe_compressed_lib!("lib.es2018.d.ts"),
    maybe_compressed_lib!("lib.es2018.full.d.ts"),
    maybe_compressed_lib!("lib.es2018.intl.d.ts"),
    maybe_compressed_lib!("lib.es2018.promise.d.ts"),
    maybe_compressed_lib!("lib.es2018.regexp.d.ts"),
    maybe_compressed_lib!("lib.es2019.array.d.ts"),
    maybe_compressed_lib!("lib.es2019.d.ts"),
    maybe_compressed_lib!("lib.es2019.full.d.ts"),
    maybe_compressed_lib!("lib.es2019.intl.d.ts"),
    maybe_compressed_lib!("lib.es2019.object.d.ts"),
    maybe_compressed_lib!("lib.es2019.string.d.ts"),
    maybe_compressed_lib!("lib.es2019.symbol.d.ts"),
    maybe_compressed_lib!("lib.es2020.bigint.d.ts"),
    maybe_compressed_lib!("lib.es2020.d.ts"),
    maybe_compressed_lib!("lib.es2020.date.d.ts"),
    maybe_compressed_lib!("lib.es2020.full.d.ts"),
    maybe_compressed_lib!("lib.es2020.intl.d.ts"),
    maybe_compressed_lib!("lib.es2020.number.d.ts"),
    maybe_compressed_lib!("lib.es2020.promise.d.ts"),
    maybe_compressed_lib!("lib.es2020.sharedmemory.d.ts"),
    maybe_compressed_lib!("lib.es2020.string.d.ts"),
    maybe_compressed_lib!("lib.es2020.symbol.wellknown.d.ts"),
    maybe_compressed_lib!("lib.es2021.d.ts"),
    maybe_compressed_lib!("lib.es2021.full.d.ts"),
    maybe_compressed_lib!("lib.es2021.intl.d.ts"),
    maybe_compressed_lib!("lib.es2021.promise.d.ts"),
    maybe_compressed_lib!("lib.es2021.string.d.ts"),
    maybe_compressed_lib!("lib.es2021.weakref.d.ts"),
    maybe_compressed_lib!("lib.es2022.array.d.ts"),
    maybe_compressed_lib!("lib.es2022.d.ts"),
    maybe_compressed_lib!("lib.es2022.error.d.ts"),
    maybe_compressed_lib!("lib.es2022.full.d.ts"),
    maybe_compressed_lib!("lib.es2022.intl.d.ts"),
    maybe_compressed_lib!("lib.es2022.object.d.ts"),
    maybe_compressed_lib!("lib.es2022.regexp.d.ts"),
    maybe_compressed_lib!("lib.es2022.string.d.ts"),
    maybe_compressed_lib!("lib.es2023.array.d.ts"),
    maybe_compressed_lib!("lib.es2023.collection.d.ts"),
    maybe_compressed_lib!("lib.es2023.d.ts"),
    maybe_compressed_lib!("lib.es2023.full.d.ts"),
    maybe_compressed_lib!("lib.es2023.intl.d.ts"),
    maybe_compressed_lib!("lib.es2024.arraybuffer.d.ts"),
    maybe_compressed_lib!("lib.es2024.collection.d.ts"),
    maybe_compressed_lib!("lib.es2024.d.ts"),
    maybe_compressed_lib!("lib.es2024.full.d.ts"),
    maybe_compressed_lib!("lib.es2024.object.d.ts"),
    maybe_compressed_lib!("lib.es2024.promise.d.ts"),
    maybe_compressed_lib!("lib.es2024.regexp.d.ts"),
    maybe_compressed_lib!("lib.es2024.sharedmemory.d.ts"),
    maybe_compressed_lib!("lib.es2024.string.d.ts"),
    maybe_compressed_lib!("lib.es5.d.ts"),
    maybe_compressed_lib!("lib.es6.d.ts"),
    maybe_compressed_lib!("lib.esnext.array.d.ts"),
    maybe_compressed_lib!("lib.esnext.collection.d.ts"),
    maybe_compressed_lib!("lib.esnext.d.ts"),
    maybe_compressed_lib!("lib.esnext.decorators.d.ts"),
    maybe_compressed_lib!("lib.esnext.disposable.d.ts"),
    maybe_compressed_lib!("lib.esnext.full.d.ts"),
    maybe_compressed_lib!("lib.esnext.intl.d.ts"),
    maybe_compressed_lib!("lib.esnext.iterator.d.ts"),
    maybe_compressed_lib!("lib.scripthost.d.ts"),
    maybe_compressed_lib!("lib.webworker.asynciterable.d.ts"),
    maybe_compressed_lib!("lib.webworker.d.ts"),
    maybe_compressed_lib!("lib.webworker.importscripts.d.ts"),
    maybe_compressed_lib!("lib.webworker.iterable.d.ts"),
    (
      // Special file that can be used to inject the @types/node package.
      // This is used for `node:` specifiers.
      "node_types.d.ts",
      StaticAssetSource::Uncompressed(
        "/// <reference types=\"npm:@types/node\" />\n",
      ),
    ),
  ])
});

/// A structure representing stats from a type check operation for a graph.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Stats(pub Vec<(String, u32)>);

impl<'de> Deserialize<'de> for Stats {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let items: Vec<(String, u32)> = Deserialize::deserialize(deserializer)?;
    Ok(Stats(items))
  }
}

impl Serialize for Stats {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    Serialize::serialize(&self.0, serializer)
  }
}

impl fmt::Display for Stats {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    writeln!(f, "Compilation statistics:")?;
    for (key, value) in self.0.clone() {
      writeln!(f, "  {key}: {value}")?;
    }

    Ok(())
  }
}

/// Retrieve a static asset that are included in the binary.
fn get_lazily_loaded_asset(asset: &str) -> Option<&'static str> {
  LAZILY_LOADED_STATIC_ASSETS.get(asset).map(|s| s.as_str())
}

fn get_maybe_hash(
  maybe_source: Option<&str>,
  hash_data: u64,
) -> Option<String> {
  maybe_source.map(|source| get_hash(source, hash_data))
}

fn get_hash(source: &str, hash_data: u64) -> String {
  FastInsecureHasher::new_without_deno_version()
    .write_str(source)
    .write_u64(hash_data)
    .finish()
    .to_string()
}

/// Hash the URL so it can be sent to `tsc` in a supportable way
fn hash_url(specifier: &ModuleSpecifier, media_type: MediaType) -> String {
  let hash = checksum::gen(&[specifier.path().as_bytes()]);
  format!(
    "{}:///{}{}",
    specifier.scheme(),
    hash,
    media_type.as_ts_extension()
  )
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct EmittedFile {
  pub data: String,
  pub maybe_specifiers: Option<Vec<ModuleSpecifier>>,
  pub media_type: MediaType,
}

pub fn into_specifier_and_media_type(
  specifier: Option<ModuleSpecifier>,
) -> (ModuleSpecifier, MediaType) {
  match specifier {
    Some(specifier) => {
      let media_type = MediaType::from_specifier(&specifier);

      (specifier, media_type)
    }
    None => (
      Url::parse(MISSING_DEPENDENCY_SPECIFIER).unwrap(),
      MediaType::Dts,
    ),
  }
}

#[derive(Debug)]
pub struct TypeCheckingCjsTracker {
  cjs_tracker: Arc<CliCjsTracker>,
  module_info_cache: Arc<ModuleInfoCache>,
}

impl TypeCheckingCjsTracker {
  pub fn new(
    cjs_tracker: Arc<CliCjsTracker>,
    module_info_cache: Arc<ModuleInfoCache>,
  ) -> Self {
    Self {
      cjs_tracker,
      module_info_cache,
    }
  }

  pub fn is_cjs(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    code: &Arc<str>,
  ) -> bool {
    let maybe_is_script = self
      .module_info_cache
      .as_module_analyzer()
      .analyze_sync(specifier, media_type, code)
      .ok()
      .map(|info| info.is_script);
    maybe_is_script
      .and_then(|is_script| {
        self
          .cjs_tracker
          .is_cjs_with_known_is_script(specifier, media_type, is_script)
          .ok()
      })
      .unwrap_or_else(|| {
        self
          .cjs_tracker
          .is_maybe_cjs(specifier, media_type)
          .unwrap_or(false)
      })
  }

  pub fn is_cjs_with_known_is_script(
    &self,
    specifier: &ModuleSpecifier,
    media_type: MediaType,
    is_script: bool,
  ) -> Result<bool, node_resolver::errors::ClosestPkgJsonError> {
    self
      .cjs_tracker
      .is_cjs_with_known_is_script(specifier, media_type, is_script)
  }
}

#[derive(Debug)]
pub struct RequestNpmState {
  pub cjs_tracker: Arc<TypeCheckingCjsTracker>,
  pub node_resolver: Arc<CliNodeResolver>,
  pub npm_resolver: CliNpmResolver,
}

/// A structure representing a request to be sent to the tsc runtime.
#[derive(Debug)]
pub struct Request {
  /// The TypeScript compiler options which will be serialized and sent to
  /// tsc.
  pub config: Arc<TsConfig>,
  /// Indicates to the tsc runtime if debug logging should occur.
  pub debug: bool,
  pub graph: Arc<ModuleGraph>,
  pub hash_data: u64,
  pub maybe_npm: Option<RequestNpmState>,
  pub maybe_tsbuildinfo: Option<String>,
  /// A vector of strings that represent the root/entry point modules for the
  /// program.
  pub root_names: Vec<(ModuleSpecifier, MediaType)>,
  pub check_mode: TypeCheckMode,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Response {
  /// Any diagnostics that have been returned from the checker.
  pub diagnostics: Diagnostics,
  /// If there was any build info associated with the exec request.
  pub maybe_tsbuildinfo: Option<String>,
  /// Statistics from the check.
  pub stats: Stats,
}

// TODO(bartlomieju): we have similar struct in `tsc.rs` - maybe at least change
// the name of the struct to avoid confusion?
#[derive(Debug)]
struct State {
  hash_data: u64,
  graph: Arc<ModuleGraph>,
  maybe_tsbuildinfo: Option<String>,
  maybe_response: Option<RespondArgs>,
  maybe_npm: Option<RequestNpmState>,
  // todo(dsherret): it looks like the remapped_specifiers and
  // root_map could be combined... what is the point of the separation?
  remapped_specifiers: HashMap<String, ModuleSpecifier>,
  root_map: HashMap<String, ModuleSpecifier>,
  current_dir: PathBuf,
}

impl Default for State {
  fn default() -> Self {
    Self {
      hash_data: Default::default(),
      graph: Arc::new(ModuleGraph::new(GraphKind::All)),
      maybe_tsbuildinfo: Default::default(),
      maybe_response: Default::default(),
      maybe_npm: Default::default(),
      remapped_specifiers: Default::default(),
      root_map: Default::default(),
      current_dir: Default::default(),
    }
  }
}

impl State {
  pub fn new(
    graph: Arc<ModuleGraph>,
    hash_data: u64,
    maybe_npm: Option<RequestNpmState>,
    maybe_tsbuildinfo: Option<String>,
    root_map: HashMap<String, ModuleSpecifier>,
    remapped_specifiers: HashMap<String, ModuleSpecifier>,
    current_dir: PathBuf,
  ) -> Self {
    State {
      hash_data,
      graph,
      maybe_npm,
      maybe_tsbuildinfo,
      maybe_response: None,
      remapped_specifiers,
      root_map,
      current_dir,
    }
  }

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

#[op2]
#[string]
fn op_create_hash(s: &mut OpState, #[string] text: &str) -> String {
  op_create_hash_inner(s, text)
}

#[inline]
fn op_create_hash_inner(s: &mut OpState, text: &str) -> String {
  let state = s.borrow_mut::<State>();
  get_hash(text, state.hash_data)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmitArgs {
  /// The text data/contents of the file.
  data: String,
  /// The _internal_ filename for the file.  This will be used to determine how
  /// the file is cached and stored.
  file_name: String,
}

#[op2(fast)]
fn op_emit(
  state: &mut OpState,
  #[string] data: String,
  #[string] file_name: String,
) -> bool {
  op_emit_inner(state, EmitArgs { data, file_name })
}

#[inline]
fn op_emit_inner(state: &mut OpState, args: EmitArgs) -> bool {
  let state = state.borrow_mut::<State>();
  match args.file_name.as_ref() {
    "internal:///.tsbuildinfo" => state.maybe_tsbuildinfo = Some(args.data),
    _ => {
      if cfg!(debug_assertions) {
        panic!("Unhandled emit write: {}", args.file_name);
      }
    }
  }

  true
}

pub fn as_ts_script_kind(media_type: MediaType) -> i32 {
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
    MediaType::SourceMap
    | MediaType::Css
    | MediaType::Html
    | MediaType::Sql
    | MediaType::Wasm
    | MediaType::Unknown => 0,
  }
}

pub const MISSING_DEPENDENCY_SPECIFIER: &str =
  "internal:///missing_dependency.d.ts";

#[derive(Debug, Error, deno_error::JsError)]
pub enum LoadError {
  #[class(generic)]
  #[error("Unable to load {path}: {error}")]
  LoadFromNodeModule { path: String, error: std::io::Error },
  #[class(inherit)]
  #[error("{0}")]
  ResolveUrlOrPathError(#[from] deno_path_util::ResolveUrlOrPathError),
  #[class(inherit)]
  #[error(
    "Error converting a string module specifier for \"op_resolve\": {0}"
  )]
  ModuleResolution(#[from] deno_core::ModuleResolutionError),
  #[class(inherit)]
  #[error("{0}")]
  ClosestPkgJson(#[from] node_resolver::errors::ClosestPkgJsonError),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LoadResponse {
  data: FastString,
  version: Option<String>,
  script_kind: i32,
  is_cjs: bool,
}

#[op2]
#[serde]
fn op_load(
  state: &mut OpState,
  #[string] load_specifier: &str,
) -> Result<Option<LoadResponse>, LoadError> {
  op_load_inner(state, load_specifier)
}

fn op_load_inner(
  state: &mut OpState,
  load_specifier: &str,
) -> Result<Option<LoadResponse>, LoadError> {
  fn load_from_node_modules(
    specifier: &ModuleSpecifier,
    npm_state: Option<&RequestNpmState>,
    media_type: &mut MediaType,
    is_cjs: &mut bool,
  ) -> Result<Option<FastString>, LoadError> {
    *media_type = MediaType::from_specifier(specifier);
    let file_path = specifier.to_file_path().unwrap();
    let code = match std::fs::read_to_string(&file_path) {
      Ok(code) => code,
      Err(err) if err.kind() == ErrorKind::NotFound => {
        return Ok(None);
      }
      Err(err) => {
        return Err(LoadError::LoadFromNodeModule {
          path: file_path.display().to_string(),
          error: err,
        })
      }
    };
    let code: Arc<str> = code.into();
    *is_cjs = npm_state
      .map(|npm_state| {
        npm_state.cjs_tracker.is_cjs(specifier, *media_type, &code)
      })
      .unwrap_or(false);
    Ok(Some(code.into()))
  }

  let state = state.borrow_mut::<State>();

  let specifier = resolve_url_or_path(load_specifier, &state.current_dir)?;

  let mut hash: Option<String> = None;
  let mut media_type = MediaType::Unknown;
  let graph = &state.graph;
  let mut is_cjs = false;

  let data = if load_specifier == "internal:///.tsbuildinfo" {
    state
      .maybe_tsbuildinfo
      .as_deref()
      .map(|s| s.to_string().into())
  // in certain situations we return a "blank" module to tsc and we need to
  // handle the request for that module here.
  } else if load_specifier == MISSING_DEPENDENCY_SPECIFIER {
    None
  } else if let Some(name) = load_specifier.strip_prefix("asset:///") {
    let maybe_source = get_lazily_loaded_asset(name);
    hash = get_maybe_hash(maybe_source, state.hash_data);
    media_type = MediaType::from_str(load_specifier);
    maybe_source.map(FastString::from_static)
  } else {
    let specifier = if let Some(remapped_specifier) =
      state.maybe_remapped_specifier(load_specifier)
    {
      remapped_specifier
    } else {
      &specifier
    };
    let maybe_module = match graph.try_get(specifier) {
      Ok(maybe_module) => maybe_module,
      Err(err) => match err {
        deno_graph::ModuleError::UnsupportedMediaType(_, media_type, _) => {
          return Ok(Some(LoadResponse {
            data: FastString::from_static(""),
            version: Some("1".to_string()),
            script_kind: as_ts_script_kind(*media_type),
            is_cjs: false,
          }))
        }
        _ => None,
      },
    };
    let maybe_source = if let Some(module) = maybe_module {
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
              .map(|m| FastString::from(m.source.clone()))
              .unwrap_or(module.source.clone().into()),
          )
        }
        Module::Json(module) => {
          media_type = MediaType::Json;
          Some(FastString::from(module.source.clone()))
        }
        Module::Wasm(module) => {
          media_type = MediaType::Dts;
          Some(FastString::from(module.source_dts.clone()))
        }
        Module::Npm(_) | Module::Node(_) => None,
        Module::External(module) => {
          if module.specifier.scheme() != "file" {
            None
          } else {
            // means it's Deno code importing an npm module
            let specifier = resolve_specifier_into_node_modules(
              &CliSys::default(),
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
    };
    hash = get_maybe_hash(maybe_source.as_deref(), state.hash_data);
    maybe_source
  };
  let Some(data) = data else {
    return Ok(None);
  };
  Ok(Some(LoadResponse {
    data,
    version: hash,
    script_kind: as_ts_script_kind(media_type),
    is_cjs,
  }))
}

#[derive(Debug, Error, deno_error::JsError)]
pub enum ResolveError {
  #[class(inherit)]
  #[error(
    "Error converting a string module specifier for \"op_resolve\": {0}"
  )]
  ModuleResolution(#[from] deno_core::ModuleResolutionError),
  #[class(inherit)]
  #[error(transparent)]
  FilePathToUrl(#[from] deno_path_util::PathToUrlError),
  #[class(inherit)]
  #[error("{0}")]
  PackageSubpathResolve(PackageSubpathResolveError),
  #[class(inherit)]
  #[error("{0}")]
  ResolveUrlOrPathError(#[from] deno_path_util::ResolveUrlOrPathError),
  #[class(inherit)]
  #[error("{0}")]
  ResolvePkgFolderFromDenoModule(#[from] ResolvePkgFolderFromDenoModuleError),
  #[class(inherit)]
  #[error("{0}")]
  ResolveNonGraphSpecifierTypes(#[from] ResolveNonGraphSpecifierTypesError),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveArgs {
  /// The base specifier that the supplied specifier strings should be resolved
  /// relative to.
  pub base: String,
  /// A list of specifiers that should be resolved.
  /// (is_cjs: bool, raw_specifier: String)
  pub specifiers: Vec<(bool, String)>,
}

#[op2]
#[string]
fn op_remap_specifier(
  state: &mut OpState,
  #[string] specifier: &str,
) -> Option<String> {
  let state = state.borrow::<State>();
  state
    .maybe_remapped_specifier(specifier)
    .map(|url| url.to_string())
}

#[op2]
#[serde]
fn op_libs() -> Vec<String> {
  let mut out = Vec::with_capacity(LAZILY_LOADED_STATIC_ASSETS.len());
  for key in LAZILY_LOADED_STATIC_ASSETS.keys() {
    let lib = key
      .replace("lib.", "")
      .replace(".d.ts", "")
      .replace("deno_", "deno.");
    out.push(lib);
  }
  out
}

#[op2]
#[serde]
fn op_resolve(
  state: &mut OpState,
  #[string] base: String,
  #[serde] specifiers: Vec<(bool, String)>,
) -> Result<Vec<(String, Option<&'static str>)>, ResolveError> {
  op_resolve_inner(state, ResolveArgs { base, specifiers })
}

#[inline]
fn op_resolve_inner(
  state: &mut OpState,
  args: ResolveArgs,
) -> Result<Vec<(String, Option<&'static str>)>, ResolveError> {
  let state = state.borrow_mut::<State>();
  let mut resolved: Vec<(String, Option<&'static str>)> =
    Vec::with_capacity(args.specifiers.len());
  let referrer = if let Some(remapped_specifier) =
    state.maybe_remapped_specifier(&args.base)
  {
    remapped_specifier.clone()
  } else {
    resolve_url_or_path(&args.base, &state.current_dir)?
  };
  let referrer_module = state.graph.get(&referrer);
  for (is_cjs, specifier) in args.specifiers {
    if specifier.starts_with("node:") {
      resolved.push((
        MISSING_DEPENDENCY_SPECIFIER.to_string(),
        Some(MediaType::Dts.as_ts_extension()),
      ));
      continue;
    }

    if specifier.starts_with("asset:///") {
      let ext = MediaType::from_str(&specifier).as_ts_extension();
      resolved.push((specifier, Some(ext)));
      continue;
    }

    let resolved_dep = referrer_module
      .and_then(|m| match m {
        Module::Js(m) => m.dependencies_prefer_fast_check().get(&specifier),
        Module::Json(_) => None,
        Module::Wasm(m) => m.dependencies.get(&specifier),
        Module::Npm(_) | Module::Node(_) | Module::External(_) => None,
      })
      .and_then(|d| d.maybe_type.ok().or_else(|| d.maybe_code.ok()));
    let resolution_mode = if is_cjs {
      ResolutionMode::Require
    } else {
      ResolutionMode::Import
    };

    let maybe_result = match resolved_dep {
      Some(ResolutionResolved { specifier, .. }) => {
        resolve_graph_specifier_types(
          specifier,
          &referrer,
          // we could get this from the resolved dep, but for now assume
          // the value resolved in TypeScript is better
          resolution_mode,
          state,
        )?
      }
      _ => {
        match resolve_non_graph_specifier_types(
          &specifier,
          &referrer,
          resolution_mode,
          state,
        ) {
          Ok(maybe_result) => maybe_result,
          Err(
            err @ ResolveNonGraphSpecifierTypesError::ResolvePkgFolderFromDenoReq(
              ResolvePkgFolderFromDenoReqError::Managed(_),
            ),
          ) => {
            // it's most likely requesting the jsxImportSource, which isn't loaded
            // into the graph when not using jsx, so just ignore this error
            if specifier.ends_with("/jsx-runtime") {
              None
            } else {
              return Err(err.into());
            }
          }
          Err(err) => return Err(err.into()),
        }
      }
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
              mapped_specifier_for_tsc(&specifier, media_type)
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
        MISSING_DEPENDENCY_SPECIFIER.to_string(),
        Some(MediaType::Dts.as_ts_extension()),
      ),
    };
    log::debug!("Resolved {} from {} to {:?}", specifier, referrer, result);
    resolved.push(result);
  }

  Ok(resolved)
}

fn resolve_graph_specifier_types(
  specifier: &ModuleSpecifier,
  referrer: &ModuleSpecifier,
  resolution_mode: ResolutionMode,
  state: &State,
) -> Result<Option<(ModuleSpecifier, MediaType)>, ResolveError> {
  let graph = &state.graph;
  let maybe_module = match graph.try_get(specifier) {
    Ok(Some(module)) => Some(module),
    Ok(None) => None,
    Err(err) => match err {
      deno_graph::ModuleError::UnsupportedMediaType(
        specifier,
        media_type,
        _,
      ) => {
        return Ok(Some((specifier.clone(), *media_type)));
      }
      _ => None,
    },
  };
  // follow the types reference directive, which may be pointing at an npm package
  let maybe_module = match maybe_module {
    Some(Module::Js(module)) => {
      let maybe_types_dep = module
        .maybe_types_dependency
        .as_ref()
        .map(|d| &d.dependency);
      match maybe_types_dep.and_then(|d| d.maybe_specifier()) {
        Some(specifier) => graph.get(specifier),
        _ => maybe_module,
      }
    }
    maybe_module => maybe_module,
  };

  // now get the types from the resolved module
  match maybe_module {
    Some(Module::Js(module)) => {
      Ok(Some((module.specifier.clone(), module.media_type)))
    }
    Some(Module::Json(module)) => {
      Ok(Some((module.specifier.clone(), module.media_type)))
    }
    Some(Module::Wasm(module)) => {
      Ok(Some((module.specifier.clone(), MediaType::Dmts)))
    }
    Some(Module::Npm(module)) => {
      if let Some(npm) = &state.maybe_npm.as_ref() {
        let package_folder = npm
          .npm_resolver
          .as_managed()
          .unwrap() // should never be byonm because it won't create Module::Npm
          .resolve_pkg_folder_from_deno_module(module.nv_reference.nv())?;
        let res_result =
          npm.node_resolver.resolve_package_subpath_from_deno_module(
            &package_folder,
            module.nv_reference.sub_path(),
            Some(referrer),
            resolution_mode,
            NodeResolutionKind::Types,
          );
        let maybe_url = match res_result {
          Ok(path_or_url) => Some(path_or_url.into_url()?),
          Err(err) => match err.code() {
            NodeJsErrorCode::ERR_TYPES_NOT_FOUND => {
              let reqs = npm
                .npm_resolver
                .as_managed()
                .unwrap()
                .resolution()
                .package_reqs();
              if let Some((_, types_nv)) =
                deno_resolver::npm::find_definitely_typed_package(
                  module.nv_reference.nv(),
                  reqs.iter().map(|tup| (&tup.0, &tup.1)),
                )
              {
                let package_folder = npm
                  .npm_resolver
                  .as_managed()
                  .unwrap() // should never be byonm because it won't create Module::Npm
                  .resolve_pkg_folder_from_deno_module(types_nv)?;
                let res_result =
                  npm.node_resolver.resolve_package_subpath_from_deno_module(
                    &package_folder,
                    module.nv_reference.sub_path(),
                    Some(referrer),
                    resolution_mode,
                    NodeResolutionKind::Types,
                  );
                if let Ok(res_result) = res_result {
                  Some(res_result.into_url()?)
                } else {
                  None
                }
              } else {
                None
              }
            }
            NodeJsErrorCode::ERR_MODULE_NOT_FOUND => None,
            _ => return Err(ResolveError::PackageSubpathResolve(err)),
          },
        };
        Ok(Some(into_specifier_and_media_type(maybe_url)))
      } else {
        Ok(None)
      }
    }
    Some(Module::External(module)) => {
      // we currently only use "External" for when the module is in an npm package
      Ok(state.maybe_npm.as_ref().map(|_| {
        let specifier = resolve_specifier_into_node_modules(
          &CliSys::default(),
          &module.specifier,
        );
        into_specifier_and_media_type(Some(specifier))
      }))
    }
    Some(Module::Node(_)) | None => Ok(None),
  }
}

#[derive(Debug, Error, deno_error::JsError)]
pub enum ResolveNonGraphSpecifierTypesError {
  #[class(inherit)]
  #[error(transparent)]
  FilePathToUrl(#[from] deno_path_util::PathToUrlError),
  #[class(inherit)]
  #[error(transparent)]
  ResolvePkgFolderFromDenoReq(#[from] ResolvePkgFolderFromDenoReqError),
  #[class(inherit)]
  #[error(transparent)]
  PackageSubpathResolve(#[from] PackageSubpathResolveError),
}

fn resolve_non_graph_specifier_types(
  raw_specifier: &str,
  referrer: &ModuleSpecifier,
  resolution_mode: ResolutionMode,
  state: &State,
) -> Result<
  Option<(ModuleSpecifier, MediaType)>,
  ResolveNonGraphSpecifierTypesError,
> {
  let npm = match state.maybe_npm.as_ref() {
    Some(npm) => npm,
    None => return Ok(None), // we only support non-graph types for npm packages
  };
  let node_resolver = &npm.node_resolver;
  if node_resolver.in_npm_package(referrer) {
    // we're in an npm package, so use node resolution
    Ok(Some(into_specifier_and_media_type(
      node_resolver
        .resolve(
          raw_specifier,
          referrer,
          resolution_mode,
          NodeResolutionKind::Types,
        )
        .and_then(|res| res.into_url())
        .ok(),
    )))
  } else if let Ok(npm_req_ref) =
    NpmPackageReqReference::from_str(raw_specifier)
  {
    debug_assert_eq!(resolution_mode, ResolutionMode::Import);
    // todo(dsherret): add support for injecting this in the graph so
    // we don't need this special code here.
    // This could occur when resolving npm:@types/node when it is
    // injected and not part of the graph
    let package_folder = npm
      .npm_resolver
      .resolve_pkg_folder_from_deno_module_req(npm_req_ref.req(), referrer)?;
    let res_result = node_resolver.resolve_package_subpath_from_deno_module(
      &package_folder,
      npm_req_ref.sub_path(),
      Some(referrer),
      resolution_mode,
      NodeResolutionKind::Types,
    );
    let maybe_url = match res_result {
      Ok(url_or_path) => Some(url_or_path.into_url()?),
      Err(err) => match err.code() {
        NodeJsErrorCode::ERR_TYPES_NOT_FOUND
        | NodeJsErrorCode::ERR_MODULE_NOT_FOUND => None,
        _ => return Err(err.into()),
      },
    };
    Ok(Some(into_specifier_and_media_type(maybe_url)))
  } else {
    Ok(None)
  }
}

#[op2(fast)]
fn op_is_node_file(state: &mut OpState, #[string] path: &str) -> bool {
  let state = state.borrow::<State>();
  ModuleSpecifier::parse(path)
    .ok()
    .and_then(|specifier| {
      state
        .maybe_npm
        .as_ref()
        .map(|n| n.node_resolver.in_npm_package(&specifier))
    })
    .unwrap_or(false)
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct RespondArgs {
  pub diagnostics: Diagnostics,
  pub stats: Stats,
}

// TODO(bartlomieju): this mechanism is questionable.
// Can't we use something more efficient here?
#[op2]
fn op_respond(state: &mut OpState, #[serde] args: RespondArgs) {
  op_respond_inner(state, args)
}

#[inline]
fn op_respond_inner(state: &mut OpState, args: RespondArgs) {
  let state = state.borrow_mut::<State>();
  state.maybe_response = Some(args);
}

#[derive(Debug, Error, deno_error::JsError)]
pub enum ExecError {
  #[class(generic)]
  #[error("The response for the exec request was not set.")]
  ResponseNotSet,
  #[class(inherit)]
  #[error(transparent)]
  Core(deno_core::error::CoreError),
}

#[derive(Clone)]
pub(crate) struct CompressedSource {
  bytes: &'static [u8],
  uncompressed: OnceLock<Arc<str>>,
}

impl CompressedSource {
  #[cfg_attr(any(debug_assertions, feature = "hmr"), allow(dead_code))]
  pub(crate) const fn new(bytes: &'static [u8]) -> Self {
    Self {
      bytes,
      uncompressed: OnceLock::new(),
    }
  }
  pub(crate) fn get(&self) -> &str {
    self
      .uncompressed
      .get_or_init(|| decompress_source(self.bytes))
      .as_ref()
  }
}

pub(crate) static MAIN_COMPILER_SOURCE: StaticAssetSource =
  maybe_compressed_source!("tsc/99_main_compiler.js");
pub(crate) static LSP_SOURCE: StaticAssetSource =
  maybe_compressed_source!("tsc/98_lsp.js");
pub(crate) static TS_HOST_SOURCE: StaticAssetSource =
  maybe_compressed_source!("tsc/97_ts_host.js");
pub(crate) static TYPESCRIPT_SOURCE: StaticAssetSource =
  maybe_compressed_source!("tsc/00_typescript.js");

pub(crate) fn decompress_source(contents: &[u8]) -> Arc<str> {
  let len_bytes = contents[0..4].try_into().unwrap();
  let len = u32::from_le_bytes(len_bytes);
  let uncompressed =
    zstd::bulk::decompress(&contents[4..], len as usize).unwrap();
  String::from_utf8(uncompressed).unwrap().into()
}

deno_core::extension!(deno_cli_tsc,
  ops = [
    op_create_hash,
    op_emit,
    op_is_node_file,
    op_load,
    op_remap_specifier,
    op_resolve,
    op_respond,
    op_libs,
  ],
  options = {
    request: Request,
    root_map: HashMap<String, Url>,
    remapped_specifiers: HashMap<String, Url>,
  },
  state = |state, options| {
    state.put(State::new(
      options.request.graph,
      options.request.hash_data,
      options.request.maybe_npm,
      options.request.maybe_tsbuildinfo,
      options.root_map,
      options.remapped_specifiers,
      std::env::current_dir()
        .context("Unable to get CWD")
        .unwrap(),
    ));
  },
  customizer = |ext: &mut deno_core::Extension| {
    use deno_core::ExtensionFileSource;
    ext.esm_files.to_mut().push(ExtensionFileSource::new_computed("ext:deno_cli_tsc/99_main_compiler.js", crate::tsc::MAIN_COMPILER_SOURCE.as_str().into()));
    ext.esm_files.to_mut().push(ExtensionFileSource::new_computed("ext:deno_cli_tsc/97_ts_host.js", crate::tsc::TS_HOST_SOURCE.as_str().into()));
    ext.esm_files.to_mut().push(ExtensionFileSource::new_computed("ext:deno_cli_tsc/98_lsp.js", crate::tsc::LSP_SOURCE.as_str().into()));
    ext.js_files.to_mut().push(ExtensionFileSource::new_computed("ext:deno_cli_tsc/00_typescript.js", crate::tsc::TYPESCRIPT_SOURCE.as_str().into()));
    ext.esm_entry_point = Some("ext:deno_cli_tsc/99_main_compiler.js");
  }
);

pub struct TscExtCodeCache {
  cache: Arc<dyn deno_runtime::code_cache::CodeCache>,
}

impl TscExtCodeCache {
  pub fn new(cache: Arc<dyn deno_runtime::code_cache::CodeCache>) -> Self {
    Self { cache }
  }
}

impl deno_core::ExtCodeCache for TscExtCodeCache {
  fn get_code_cache_info(
    &self,
    specifier: &ModuleSpecifier,
    code: &deno_core::ModuleSourceCode,
    esm: bool,
  ) -> deno_core::SourceCodeCacheInfo {
    use deno_runtime::code_cache::CodeCacheType;
    let code_hash = FastInsecureHasher::new_deno_versioned()
      .write_hashable(code)
      .finish();
    let data = self
      .cache
      .get_sync(
        specifier,
        if esm {
          CodeCacheType::EsModule
        } else {
          CodeCacheType::Script
        },
        code_hash,
      )
      .map(Cow::from)
      .inspect(|_| {
        log::debug!(
          "V8 code cache hit for Extension module: {specifier}, [{code_hash:?}]"
        );
      });
    deno_core::SourceCodeCacheInfo {
      hash: code_hash,
      data,
    }
  }

  fn code_cache_ready(
    &self,
    specifier: ModuleSpecifier,
    source_hash: u64,
    code_cache: &[u8],
    esm: bool,
  ) {
    use deno_runtime::code_cache::CodeCacheType;

    log::debug!(
      "Updating V8 code cache for Extension module: {specifier}, [{source_hash:?}]"
    );
    self.cache.set_sync(
      specifier,
      if esm {
        CodeCacheType::EsModule
      } else {
        CodeCacheType::Script
      },
      source_hash,
      code_cache,
    );
  }
}

/// Execute a request on the supplied snapshot, returning a response which
/// contains information, like any emitted files, diagnostics, statistics and
/// optionally an updated TypeScript build info.
pub fn exec(
  request: Request,
  code_cache: Option<Arc<dyn deno_runtime::code_cache::CodeCache>>,
) -> Result<Response, ExecError> {
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
        let specifier_str = hash_url(s, *mt);
        remapped_specifiers.insert(specifier_str.clone(), s.clone());
        specifier_str
      }
      _ => {
        if let Some(new_specifier) = mapped_specifier_for_tsc(s, *mt) {
          root_map.insert(new_specifier.clone(), s.clone());
          new_specifier
        } else {
          s.to_string()
        }
      }
    })
    .collect();

  let request_value = json!({
    "config": request.config,
    "debug": request.debug,
    "rootNames": root_names,
    "localOnly": request.check_mode == TypeCheckMode::Local,
  });
  let exec_source = format!("globalThis.exec({request_value})");

  let mut extensions =
    deno_runtime::snapshot_info::get_extensions_in_snapshot();
  extensions.push(deno_cli_tsc::init_ops_and_esm(
    request,
    root_map,
    remapped_specifiers,
  ));
  let extension_code_cache = code_cache.map(|cache| {
    Rc::new(TscExtCodeCache::new(cache)) as Rc<dyn deno_core::ExtCodeCache>
  });
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions,
    create_params: create_isolate_create_params(),
    startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
    extension_code_cache,
    ..Default::default()
  });

  runtime
    .execute_script(located_script_name!(), exec_source)
    .map_err(ExecError::Core)?;

  let op_state = runtime.op_state();
  let mut op_state = op_state.borrow_mut();
  let state = op_state.take::<State>();

  if let Some(response) = state.maybe_response {
    let diagnostics = response.diagnostics;
    let maybe_tsbuildinfo = state.maybe_tsbuildinfo;
    let stats = response.stats;

    Ok(Response {
      diagnostics,
      maybe_tsbuildinfo,
      stats,
    })
  } else {
    Err(ExecError::ResponseNotSet)
  }
}

#[cfg(test)]
mod tests {
  use deno_core::futures::future;
  use deno_core::parking_lot::Mutex;
  use deno_core::serde_json;
  use deno_core::OpState;
  use deno_error::JsErrorBox;
  use deno_graph::GraphKind;
  use deno_graph::ModuleGraph;
  use deno_runtime::code_cache::CodeCacheType;
  use test_util::PathRef;

  use super::Diagnostic;
  use super::DiagnosticCategory;
  use super::*;
  use crate::args::TsConfig;

  #[derive(Debug, Default)]
  pub struct MockLoader {
    pub fixtures: PathRef,
  }

  impl deno_graph::source::Loader for MockLoader {
    fn load(
      &self,
      specifier: &ModuleSpecifier,
      _options: deno_graph::source::LoadOptions,
    ) -> deno_graph::source::LoadFuture {
      let specifier_text = specifier
        .to_string()
        .replace(":///", "_")
        .replace("://", "_")
        .replace('/', "-");
      let source_path = self.fixtures.join(specifier_text);
      let response = source_path
        .read_to_bytes_if_exists()
        .map(|c| {
          Some(deno_graph::source::LoadResponse::Module {
            specifier: specifier.clone(),
            maybe_headers: None,
            content: c.into(),
          })
        })
        .map_err(|e| {
          deno_graph::source::LoadError::Other(Arc::new(JsErrorBox::generic(
            e.to_string(),
          )))
        });
      Box::pin(future::ready(response))
    }
  }

  async fn setup(
    maybe_specifier: Option<ModuleSpecifier>,
    maybe_hash_data: Option<u64>,
    maybe_tsbuildinfo: Option<String>,
  ) -> OpState {
    let specifier = maybe_specifier
      .unwrap_or_else(|| ModuleSpecifier::parse("file:///main.ts").unwrap());
    let hash_data = maybe_hash_data.unwrap_or(0);
    let fixtures = test_util::testdata_path().join("tsc2");
    let loader = MockLoader { fixtures };
    let mut graph = ModuleGraph::new(GraphKind::TypesOnly);
    graph
      .build(vec![specifier], &loader, Default::default())
      .await;
    let state = State::new(
      Arc::new(graph),
      hash_data,
      None,
      maybe_tsbuildinfo,
      HashMap::new(),
      HashMap::new(),
      std::env::current_dir()
        .context("Unable to get CWD")
        .unwrap(),
    );
    let mut op_state = OpState::new(None, None);
    op_state.put(state);
    op_state
  }

  async fn test_exec(
    specifier: &ModuleSpecifier,
  ) -> Result<Response, ExecError> {
    test_exec_with_cache(specifier, None).await
  }
  async fn test_exec_with_cache(
    specifier: &ModuleSpecifier,
    code_cache: Option<Arc<dyn deno_runtime::code_cache::CodeCache>>,
  ) -> Result<Response, ExecError> {
    let hash_data = 123; // something random
    let fixtures = test_util::testdata_path().join("tsc2");
    let loader = MockLoader { fixtures };
    let mut graph = ModuleGraph::new(GraphKind::TypesOnly);
    graph
      .build(vec![specifier.clone()], &loader, Default::default())
      .await;
    let config = Arc::new(TsConfig::new(json!({
      "allowJs": true,
      "checkJs": false,
      "esModuleInterop": true,
      "emitDecoratorMetadata": false,
      "incremental": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "lib": ["deno.window"],
      "noEmit": true,
      "outDir": "internal:///",
      "strict": true,
      "target": "esnext",
      "tsBuildInfoFile": "internal:///.tsbuildinfo",
    })));
    let request = Request {
      config,
      debug: false,
      graph: Arc::new(graph),
      hash_data,
      maybe_npm: None,
      maybe_tsbuildinfo: None,
      root_names: vec![(specifier.clone(), MediaType::TypeScript)],
      check_mode: TypeCheckMode::All,
    };
    exec(request, code_cache)
  }

  #[tokio::test]
  async fn test_create_hash() {
    let mut state = setup(None, Some(123), None).await;
    let actual = op_create_hash_inner(&mut state, "some sort of content");
    assert_eq!(actual, "11905938177474799758");
  }

  #[tokio::test]
  async fn test_hash_url() {
    let specifier = deno_core::resolve_url(
      "data:application/javascript,console.log(\"Hello%20Deno\");",
    )
    .unwrap();
    assert_eq!(hash_url(&specifier, MediaType::JavaScript), "data:///d300ea0796bd72b08df10348e0b70514c021f2e45bfe59cec24e12e97cd79c58.js");
  }

  #[tokio::test]
  async fn test_emit_tsbuildinfo() {
    let mut state = setup(None, None, None).await;
    let actual = op_emit_inner(
      &mut state,
      EmitArgs {
        data: "some file content".to_string(),
        file_name: "internal:///.tsbuildinfo".to_string(),
      },
    );
    assert!(actual);
    let state = state.borrow::<State>();
    assert_eq!(
      state.maybe_tsbuildinfo,
      Some("some file content".to_string())
    );
  }

  #[tokio::test]
  async fn test_load() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/mod.ts").unwrap()),
      None,
      Some("some content".to_string()),
    )
    .await;
    let actual =
      op_load_inner(&mut state, "https://deno.land/x/mod.ts").unwrap();
    assert_eq!(
      serde_json::to_value(actual).unwrap(),
      json!({
        "data": "console.log(\"hello deno\");\n",
        "version": "7821807483407828376",
        "scriptKind": 3,
        "isCjs": false,
      })
    );
  }

  #[tokio::test]
  async fn test_load_asset() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/mod.ts").unwrap()),
      None,
      Some("some content".to_string()),
    )
    .await;
    let actual = op_load_inner(&mut state, "asset:///lib.dom.d.ts")
      .expect("should have invoked op")
      .expect("load should have succeeded");
    let expected = get_lazily_loaded_asset("lib.dom.d.ts").unwrap();
    assert_eq!(actual.data.to_string(), expected.to_string());
    assert!(actual.version.is_some());
    assert_eq!(actual.script_kind, 3);
  }

  #[tokio::test]
  async fn test_load_tsbuildinfo() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/mod.ts").unwrap()),
      None,
      Some("some content".to_string()),
    )
    .await;
    let actual = op_load_inner(&mut state, "internal:///.tsbuildinfo")
      .expect("should have invoked op")
      .expect("load should have succeeded");
    assert_eq!(
      serde_json::to_value(actual).unwrap(),
      json!({
        "data": "some content",
        "version": null,
        "scriptKind": 0,
        "isCjs": false,
      })
    );
  }

  #[tokio::test]
  async fn test_load_missing_specifier() {
    let mut state = setup(None, None, None).await;
    let actual = op_load_inner(&mut state, "https://deno.land/x/mod.ts")
      .expect("should have invoked op");
    assert_eq!(serde_json::to_value(actual).unwrap(), json!(null));
  }

  #[tokio::test]
  async fn test_resolve() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/a.ts").unwrap()),
      None,
      None,
    )
    .await;
    let actual = op_resolve_inner(
      &mut state,
      ResolveArgs {
        base: "https://deno.land/x/a.ts".to_string(),
        specifiers: vec![(false, "./b.ts".to_string())],
      },
    )
    .expect("should have invoked op");
    assert_eq!(
      actual,
      vec![("https://deno.land/x/b.ts".into(), Some(".ts"))]
    );
  }

  #[tokio::test]
  async fn test_resolve_empty() {
    let mut state = setup(
      Some(ModuleSpecifier::parse("https://deno.land/x/a.ts").unwrap()),
      None,
      None,
    )
    .await;
    let actual = op_resolve_inner(
      &mut state,
      ResolveArgs {
        base: "https://deno.land/x/a.ts".to_string(),
        specifiers: vec![(false, "./bad.ts".to_string())],
      },
    )
    .expect("should have not errored");
    assert_eq!(
      actual,
      vec![(MISSING_DEPENDENCY_SPECIFIER.into(), Some(".d.ts"))]
    );
  }

  #[tokio::test]
  async fn test_respond() {
    let mut state = setup(None, None, None).await;
    let args = serde_json::from_value(json!({
      "diagnostics": [
        {
          "messageText": "Unknown compiler option 'invalid'.",
          "category": 1,
          "code": 5023
        }
      ],
      "stats": [["a", 12]]
    }))
    .unwrap();
    op_respond_inner(&mut state, args);
    let state = state.borrow::<State>();
    assert_eq!(
      state.maybe_response,
      Some(RespondArgs {
        diagnostics: Diagnostics::new(vec![Diagnostic {
          category: DiagnosticCategory::Error,
          code: 5023,
          start: None,
          end: None,
          original_source_start: None,
          message_text: Some(
            "Unknown compiler option \'invalid\'.".to_string()
          ),
          message_chain: None,
          source: None,
          source_line: None,
          file_name: None,
          related_information: None,
          reports_deprecated: None,
          reports_unnecessary: None,
          other: Default::default(),
        }]),
        stats: Stats(vec![("a".to_string(), 12)])
      })
    );
  }

  #[tokio::test]
  async fn test_exec_basic() {
    let specifier = ModuleSpecifier::parse("https://deno.land/x/a.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(!actual.diagnostics.has_diagnostic());
    assert!(actual.maybe_tsbuildinfo.is_some());
    assert_eq!(actual.stats.0.len(), 12);
  }

  #[tokio::test]
  async fn test_exec_reexport_dts() {
    let specifier = ModuleSpecifier::parse("file:///reexports.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(!actual.diagnostics.has_diagnostic());
    assert!(actual.maybe_tsbuildinfo.is_some());
    assert_eq!(actual.stats.0.len(), 12);
  }

  #[tokio::test]
  async fn fix_lib_ref() {
    let specifier = ModuleSpecifier::parse("file:///libref.ts").unwrap();
    let actual = test_exec(&specifier)
      .await
      .expect("exec should not have errored");
    assert!(!actual.diagnostics.has_diagnostic());
  }

  pub type SpecifierWithType = (ModuleSpecifier, CodeCacheType);

  #[derive(Default)]
  struct TestExtCodeCache {
    cache: Mutex<HashMap<(SpecifierWithType, u64), Vec<u8>>>,

    hits: Mutex<HashMap<SpecifierWithType, usize>>,
    misses: Mutex<HashMap<SpecifierWithType, usize>>,
  }

  impl deno_runtime::code_cache::CodeCache for TestExtCodeCache {
    fn get_sync(
      &self,
      specifier: &ModuleSpecifier,
      code_cache_type: CodeCacheType,
      source_hash: u64,
    ) -> Option<Vec<u8>> {
      let result = self
        .cache
        .lock()
        .get(&((specifier.clone(), code_cache_type), source_hash))
        .cloned();
      if result.is_some() {
        *self
          .hits
          .lock()
          .entry((specifier.clone(), code_cache_type))
          .or_default() += 1;
      } else {
        *self
          .misses
          .lock()
          .entry((specifier.clone(), code_cache_type))
          .or_default() += 1;
      }
      result
    }

    fn set_sync(
      &self,
      specifier: ModuleSpecifier,
      code_cache_type: CodeCacheType,
      source_hash: u64,
      data: &[u8],
    ) {
      self
        .cache
        .lock()
        .insert(((specifier, code_cache_type), source_hash), data.to_vec());
    }
  }

  #[tokio::test]
  async fn test_exec_code_cache() {
    let code_cache = Arc::new(TestExtCodeCache::default());
    let specifier = ModuleSpecifier::parse("https://deno.land/x/a.ts").unwrap();
    let actual = test_exec_with_cache(&specifier, Some(code_cache.clone()))
      .await
      .expect("exec should not have errored");
    assert!(!actual.diagnostics.has_diagnostic());

    let expect = [
      (
        "ext:deno_cli_tsc/99_main_compiler.js",
        CodeCacheType::EsModule,
      ),
      ("ext:deno_cli_tsc/98_lsp.js", CodeCacheType::EsModule),
      ("ext:deno_cli_tsc/97_ts_host.js", CodeCacheType::EsModule),
      ("ext:deno_cli_tsc/00_typescript.js", CodeCacheType::Script),
    ];

    {
      let mut files = HashMap::new();

      for (((specifier, ty), _), _) in code_cache.cache.lock().iter() {
        let specifier = specifier.to_string();
        if files.contains_key(&specifier) {
          panic!("should have only 1 entry per specifier");
        }
        files.insert(specifier, *ty);
      }

      // 99_main_compiler, 98_lsp, 97_ts_host, 00_typescript
      assert_eq!(files.len(), 4);
      assert_eq!(code_cache.hits.lock().len(), 0);
      assert_eq!(code_cache.misses.lock().len(), 4);

      for (specifier, ty) in &expect {
        assert_eq!(files.get(*specifier), Some(ty));
      }

      code_cache.hits.lock().clear();
      code_cache.misses.lock().clear();
    }

    {
      let _ = test_exec_with_cache(&specifier, Some(code_cache.clone()))
        .await
        .expect("exec should not have errored");

      // 99_main_compiler, 98_lsp, 97_ts_host, 00_typescript
      assert_eq!(code_cache.hits.lock().len(), 4);
      assert_eq!(code_cache.misses.lock().len(), 0);

      for (specifier, ty) in expect {
        let url = ModuleSpecifier::parse(specifier).unwrap();
        assert_eq!(code_cache.hits.lock().get(&(url, ty)), Some(&1));
      }
    }
  }
}
