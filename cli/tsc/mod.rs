// Copyright 2018-2025 the Deno authors. MIT license.
//
mod go;
mod js;

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::OnceLock;

use deno_ast::MediaType;
use deno_core::ModuleSpecifier;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::url::Url;
use deno_graph::Module;
use deno_graph::ModuleGraph;
use deno_lib::util::checksum;
use deno_lib::util::hash::FastInsecureHasher;
use deno_resolver::deno_json::JsxImportSourceConfigResolver;
use deno_resolver::npm::ResolvePkgFolderFromDenoReqError;
use deno_resolver::npm::managed::ResolvePkgFolderFromDenoModuleError;
use deno_semver::npm::NpmPackageReqReference;
use indexmap::IndexMap;
use node_resolver::NodeResolutionKind;
use node_resolver::ResolutionMode;
use node_resolver::errors::NodeJsErrorCode;
use node_resolver::errors::NodeJsErrorCoded;
use node_resolver::errors::PackageSubpathFromDenoModuleResolveError;
use node_resolver::resolve_specifier_into_node_modules;
use once_cell::sync::Lazy;
use thiserror::Error;

use crate::args::CompilerOptions;
use crate::args::TypeCheckMode;
use crate::cache::ModuleInfoCache;
use crate::node::CliNodeResolver;
use crate::node::CliPackageJsonResolver;
use crate::npm::CliNpmResolver;
use crate::resolver::CliCjsTracker;
use crate::sys::CliSys;
use crate::util::path::mapped_specifier_for_tsc;

mod diagnostics;

pub use self::diagnostics::Diagnostic;
pub use self::diagnostics::DiagnosticCategory;
pub use self::diagnostics::Diagnostics;
pub use self::diagnostics::Position;
pub use self::go::ensure_tsgo;
pub use self::js::TscConstants;

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
        .source
        .as_str()
    })
    .collect::<Vec<_>>()
    .join("\n")
}

macro_rules! maybe_compressed_source {
  ($file: expr) => {{ maybe_compressed_source!(compressed = $file, uncompressed = $file) }};
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

macro_rules! maybe_compressed_static_asset {
  ($name: expr, $file: expr, $is_lib: literal) => {
    (
      $name,
      StaticAsset {
        is_lib: $is_lib,
        source: maybe_compressed_source!(concat!("tsc/dts/", $file)),
      },
    )
  };
  ($e: expr, $is_lib: literal) => {
    maybe_compressed_static_asset!($e, $e, $is_lib)
  };
}

macro_rules! maybe_compressed_lib {
  ($name: expr, $file: expr) => {
    maybe_compressed_static_asset!($name, $file, true)
  };
  ($e: expr) => {
    maybe_compressed_lib!($e, $e)
  };
}

// Include the auto-generated node type libs macro
include!(concat!(env!("OUT_DIR"), "/node_types.rs"));

#[derive(Clone)]
pub enum StaticAssetSource {
  #[cfg_attr(any(debug_assertions, feature = "hmr"), allow(dead_code))]
  Compressed(CompressedSource),
  #[allow(dead_code)]
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

pub struct StaticAsset {
  pub is_lib: bool,
  pub source: StaticAssetSource,
}

/// Contains static assets that are not preloaded in the compiler snapshot.
///
/// We lazily load these because putting them in the compiler snapshot will
/// increase memory usage when not used (last time checked by about 0.5MB).
pub static LAZILY_LOADED_STATIC_ASSETS: Lazy<
  IndexMap<&'static str, StaticAsset>,
> = Lazy::new(|| {
  Vec::from([
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
    maybe_compressed_lib!("lib.esnext.error.d.ts"),
    maybe_compressed_lib!("lib.esnext.float16.d.ts"),
    maybe_compressed_lib!("lib.esnext.full.d.ts"),
    maybe_compressed_lib!("lib.esnext.intl.d.ts"),
    maybe_compressed_lib!("lib.esnext.iterator.d.ts"),
    maybe_compressed_lib!("lib.esnext.promise.d.ts"),
    maybe_compressed_lib!("lib.esnext.sharedmemory.d.ts"),
    maybe_compressed_lib!("lib.node.d.ts"),
    maybe_compressed_lib!("lib.scripthost.d.ts"),
    maybe_compressed_lib!("lib.webworker.asynciterable.d.ts"),
    maybe_compressed_lib!("lib.webworker.d.ts"),
    maybe_compressed_lib!("lib.webworker.importscripts.d.ts"),
    maybe_compressed_lib!("lib.webworker.iterable.d.ts"),
    (
      // Special file that can be used to inject the @types/node package.
      // This is used for `node:` specifiers.
      "reference_types_node.d.ts",
      StaticAsset {
        is_lib: false,
        source: StaticAssetSource::Uncompressed(
          // causes either the built-in node types to be used or it
          // prefers the @types/node if it exists
          "/// <reference lib=\"node\" />\n/// <reference types=\"npm:@types/node\" />\n",
        ),
      },
    ),
  ])
  .into_iter()
  .chain(node_type_libs!())
  .collect()
});

pub fn lib_names() -> Vec<String> {
  let mut out =
    Vec::with_capacity(crate::tsc::LAZILY_LOADED_STATIC_ASSETS.len());
  for (key, value) in crate::tsc::LAZILY_LOADED_STATIC_ASSETS.iter() {
    if !value.is_lib {
      continue;
    }
    let lib = key
      .replace("lib.", "")
      .replace(".d.ts", "")
      .replace("deno_", "deno.");
    out.push(lib);
  }
  out
}

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
  LAZILY_LOADED_STATIC_ASSETS
    .get(asset)
    .map(|s| s.source.as_str())
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
  let hash = checksum::r#gen(&[specifier.path().as_bytes()]);
  format!(
    "{}:///{}{}",
    specifier.scheme(),
    hash,
    media_type.as_ts_extension()
  )
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
#[allow(dead_code)]
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
  ) -> Result<bool, node_resolver::errors::PackageJsonLoadError> {
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
  pub package_json_resolver: Arc<CliPackageJsonResolver>,
}

/// A structure representing a request to be sent to the tsc runtime.
#[derive(Debug)]
pub struct Request {
  /// The TypeScript compiler options which will be serialized and sent to
  /// tsc.
  pub config: Arc<CompilerOptions>,
  /// Indicates to the tsc runtime if debug logging should occur.
  pub debug: bool,
  pub graph: Arc<ModuleGraph>,
  pub jsx_import_source_config_resolver: Arc<JsxImportSourceConfigResolver>,
  pub hash_data: u64,
  pub maybe_npm: Option<RequestNpmState>,
  pub maybe_tsbuildinfo: Option<String>,
  /// A vector of strings that represent the root/entry point modules for the
  /// program.
  pub root_names: Vec<(ModuleSpecifier, MediaType)>,
  pub check_mode: TypeCheckMode,

  pub initial_cwd: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Response {
  /// Any diagnostics that have been returned from the checker.
  pub diagnostics: Diagnostics,
  /// If there was any build info associated with the exec request.
  pub maybe_tsbuildinfo: Option<String>,
  pub ambient_modules: Vec<String>,
  /// Statistics from the check.
  pub stats: Stats,
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
    | MediaType::Jsonc
    | MediaType::Json5
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
  #[error("Error converting a string module specifier for \"op_resolve\": {0}")]
  ModuleResolution(#[from] deno_core::ModuleResolutionError),
  #[class(inherit)]
  #[error("{0}")]
  ClosestPkgJson(#[from] node_resolver::errors::PackageJsonLoadError),
}
pub fn load_raw_import_source(specifier: &Url) -> Option<&'static str> {
  let raw_import = get_specifier_raw_import(specifier)?;
  let source = match raw_import {
    RawImportKind::Bytes => {
      "const data: Uint8Array<ArrayBuffer>;\nexport default data;\n"
    }
    RawImportKind::Text => "export const data: string;\nexport default data;\n",
  };
  Some(source)
}

enum RawImportKind {
  Bytes,
  Text,
}

/// We store the raw import kind in the fragment of the Url
/// like `#denoRawImport=text`. This is necessary because
/// TypeScript can't handle different modules at the same
/// specifier.
fn get_specifier_raw_import(specifier: &Url) -> Option<RawImportKind> {
  // this is purposefully relaxed about matching in order to keep the
  // code less complex. If someone is doing something to cause this to
  // incorrectly match then they most likely deserve the bug they sought.
  let fragment = specifier.fragment()?;
  let key_text = "denoRawImport=";
  let raw_import_index = fragment.find(key_text)?;
  let remaining = &fragment[raw_import_index + key_text.len()..];
  if remaining.starts_with("text") {
    Some(RawImportKind::Text)
  } else if remaining.starts_with("bytes") {
    Some(RawImportKind::Bytes)
  } else {
    None
  }
}

#[derive(Debug, Error, deno_error::JsError)]
pub enum ResolveError {
  #[class(inherit)]
  #[error("Error converting a string module specifier for \"op_resolve\": {0}")]
  ModuleResolution(#[from] deno_core::ModuleResolutionError),
  #[class(inherit)]
  #[error(transparent)]
  FilePathToUrl(#[from] deno_path_util::PathToUrlError),
  #[class(inherit)]
  #[error("{0}")]
  PackageSubpathResolve(PackageSubpathFromDenoModuleResolveError),
  #[class(inherit)]
  #[error("{0}")]
  ResolveUrlOrPathError(#[from] deno_path_util::ResolveUrlOrPathError),
  #[class(inherit)]
  #[error("{0}")]
  ResolvePkgFolderFromDenoModule(#[from] ResolvePkgFolderFromDenoModuleError),
  #[class(inherit)]
  #[error("{0}")]
  ResolveNonGraphSpecifierTypes(#[from] ResolveNonGraphSpecifierTypesError),
  #[class(inherit)]
  #[error("{0}")]
  ResolvePkgFolderFromDenoReq(#[from] ResolvePkgFolderFromDenoReqError),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveArgs<'a> {
  /// The base specifier that the supplied specifier strings should be resolved
  /// relative to.
  pub base: &'a str,
  /// A list of specifiers that should be resolved.
  /// (is_cjs: bool, raw_specifier: String)
  pub specifiers: Vec<(bool, String)>,
}

fn resolve_graph_specifier_types(
  specifier: &ModuleSpecifier,
  referrer: &ModuleSpecifier,
  resolution_mode: ResolutionMode,
  graph: &ModuleGraph,
  maybe_npm: Option<&RequestNpmState>,
) -> Result<Option<(ModuleSpecifier, MediaType)>, ResolveError> {
  let maybe_module = match graph.try_get(specifier) {
    Ok(Some(module)) => Some(module),
    Ok(None) => None,
    Err(err) => match err.as_kind() {
      deno_graph::ModuleErrorKind::UnsupportedMediaType {
        specifier,
        media_type,
        ..
      } => {
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
    Some(Module::Npm(_)) => {
      if let Some(npm) = maybe_npm
        && let Ok(req_ref) = NpmPackageReqReference::from_specifier(specifier)
      {
        let package_folder = npm
          .npm_resolver
          .resolve_pkg_folder_from_deno_module_req(req_ref.req(), referrer)?;
        let res_result =
          npm.node_resolver.resolve_package_subpath_from_deno_module(
            &package_folder,
            req_ref.sub_path(),
            Some(referrer),
            resolution_mode,
            NodeResolutionKind::Types,
          );
        let maybe_url = match res_result {
          Ok(path_or_url) => Some(path_or_url.into_url()?),
          Err(err) => match err.code() {
            NodeJsErrorCode::ERR_MODULE_NOT_FOUND
            | NodeJsErrorCode::ERR_TYPES_NOT_FOUND => None,
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
      Ok(maybe_npm.map(|_| {
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
  PackageSubpathResolve(#[from] PackageSubpathFromDenoModuleResolveError),
}

fn resolve_non_graph_specifier_types(
  raw_specifier: &str,
  referrer: &ModuleSpecifier,
  resolution_mode: ResolutionMode,
  maybe_npm: Option<&RequestNpmState>,
) -> Result<
  Option<(ModuleSpecifier, MediaType)>,
  ResolveNonGraphSpecifierTypesError,
> {
  let npm = match maybe_npm {
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
  } else {
    match NpmPackageReqReference::from_str(raw_specifier) {
      Ok(npm_req_ref) => {
        debug_assert_eq!(resolution_mode, ResolutionMode::Import);
        // This could occur when resolving npm:@types/node when it is
        // injected and not part of the graph
        let package_folder =
          npm.npm_resolver.resolve_pkg_folder_from_deno_module_req(
            npm_req_ref.req(),
            referrer,
          )?;
        let res_result = node_resolver
          .resolve_package_subpath_from_deno_module(
            &package_folder,
            npm_req_ref.sub_path(),
            Some(referrer),
            resolution_mode,
            NodeResolutionKind::Types,
          );
        let maybe_url = match res_result {
          Ok(url_or_path) => Some(url_or_path.into_url()?),
          Err(err) => match err.code() {
            NodeJsErrorCode::ERR_MODULE_NOT_FOUND
            | NodeJsErrorCode::ERR_TYPES_NOT_FOUND => None,
            _ => return Err(err.into()),
          },
        };
        Ok(Some(into_specifier_and_media_type(maybe_url)))
      }
      _ => Ok(None),
    }
  }
}

#[derive(Debug, Error, deno_error::JsError)]
pub enum ExecError {
  #[class(generic)]
  #[error("The response for the exec request was not set.")]
  ResponseNotSet,
  #[class(inherit)]
  #[error(transparent)]
  Js(Box<deno_core::error::JsError>),

  #[class(inherit)]
  #[error(transparent)]
  Go(#[from] go::ExecError),
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

/// Execute a request on the supplied snapshot, returning a response which
/// contains information, like any emitted files, diagnostics, statistics and
/// optionally an updated TypeScript build info.
#[allow(clippy::result_large_err)]
pub fn exec(
  request: Request,
  code_cache: Option<Arc<dyn deno_runtime::code_cache::CodeCache>>,
  maybe_tsgo_path: Option<&Path>,
) -> Result<Response, ExecError> {
  // tsc cannot handle root specifiers that don't have one of the "acceptable"
  // extensions.  Therefore, we have to check the root modules against their
  // extensions and remap any that are unacceptable to tsc and add them to the
  // op state so when requested, we can remap to the original specifier.
  let mut root_map = HashMap::new();
  let mut remapped_specifiers = HashMap::new();
  log::debug!("exec request, root_names: {:?}", request.root_names);
  let root_names: Vec<String> = request
    .root_names
    .iter()
    .map(|(s, mt)| match s.scheme() {
      "data" | "blob" => {
        let specifier_str = hash_url(s, *mt);
        remapped_specifiers.insert(specifier_str.clone(), s.clone());
        specifier_str
      }
      // "file" if tsgo => {
      //   let specifier_str = s.to_string();
      //   let out = specifier_str.strip_prefix("file://").unwrap().to_string();
      //   remapped_specifiers.insert(out.to_string(), s.clone());
      //   out
      // }
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

  if let Some(tsgo_path) = maybe_tsgo_path {
    go::exec_request(
      request,
      root_names,
      root_map,
      remapped_specifiers,
      tsgo_path,
    )
  } else {
    js::exec_request(
      request,
      root_names,
      root_map,
      remapped_specifiers,
      code_cache,
    )
  }
}

pub fn resolve_specifier_for_tsc(
  specifier: String,
  referrer: &ModuleSpecifier,
  graph: &ModuleGraph,
  resolution_mode: ResolutionMode,
  maybe_npm: Option<&RequestNpmState>,
  referrer_module: Option<&Module>,
  remapped_specifiers: &mut HashMap<String, ModuleSpecifier>,
) -> Result<(String, Option<&'static str>), ResolveError> {
  if specifier.starts_with("node:") {
    return Ok((
      MISSING_DEPENDENCY_SPECIFIER.to_string(),
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

  let maybe_result = match resolved_dep {
    Some(deno_graph::ResolutionResolved { specifier, .. }) => {
      resolve_graph_specifier_types(
        specifier,
        referrer,
        // we could get this from the resolved dep, but for now assume
        // the value resolved in TypeScript is better
        resolution_mode,
        graph,
        maybe_npm,
      )?
    }
    _ => {
      match resolve_non_graph_specifier_types(
        &specifier,
        referrer,
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
          if specifier.ends_with("/jsx-runtime")
            // ignore in order to support attempt to load when it doesn't exist
            || specifier == "npm:@types/node"
          {
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

          remapped_specifiers.insert(specifier_str.clone(), specifier);
          specifier_str
        }
        _ => {
          if let Some(specifier_str) =
            mapped_specifier_for_tsc(&specifier, media_type)
          {
            remapped_specifiers.insert(specifier_str.clone(), specifier);
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
  Ok(result)
}

pub trait LoadContent: AsRef<str> {
  fn from_static(source: &'static str) -> Self;
  fn from_string(source: String) -> Self;
  fn from_arc_str(source: Arc<str>) -> Self;
}

#[derive(Debug)]
pub struct LoadResponse<T: LoadContent> {
  data: T,
  version: Option<String>,
  is_cjs: bool,
  media_type: MediaType,
}

pub trait Mapper {
  fn maybe_remapped_specifier(
    &self,
    specifier: &str,
  ) -> Option<&ModuleSpecifier>;
}

pub fn load_for_tsc<T: LoadContent, M: Mapper>(
  load_specifier: &str,
  maybe_npm: Option<&RequestNpmState>,
  current_dir: &Path,
  graph: &ModuleGraph,
  maybe_tsbuildinfo: Option<&str>,
  hash_data: u64,
  remapper: &M,
) -> Result<Option<LoadResponse<T>>, LoadError> {
  fn load_from_node_modules<T: LoadContent>(
    specifier: &ModuleSpecifier,
    npm_state: Option<&RequestNpmState>,
    media_type: &mut MediaType,
    is_cjs: &mut bool,
  ) -> Result<Option<T>, LoadError> {
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
        });
      }
    };
    let code: Arc<str> = code.into();
    *is_cjs = npm_state
      .map(|npm_state| {
        npm_state.cjs_tracker.is_cjs(specifier, *media_type, &code)
      })
      .unwrap_or(false);
    Ok(Some(T::from_arc_str(code)))
  }

  let specifier =
    deno_path_util::resolve_url_or_path(load_specifier, current_dir)?;

  let mut hash: Option<String> = None;
  let mut media_type = MediaType::Unknown;
  let mut is_cjs = false;

  let data = if load_specifier == "internal:///.tsbuildinfo" {
    maybe_tsbuildinfo.map(|s| T::from_string(s.to_string()))
  // in certain situations we return a "blank" module to tsc and we need to
  // handle the request for that module here.
  } else if load_specifier == MISSING_DEPENDENCY_SPECIFIER {
    None
  } else if let Some(name) = load_specifier.strip_prefix("asset:///") {
    let maybe_source = get_lazily_loaded_asset(name);
    hash = get_maybe_hash(maybe_source, hash_data);
    media_type = MediaType::from_str(load_specifier);
    is_cjs = media_type == MediaType::Dcts;
    maybe_source.map(T::from_static)
  } else if let Some(source) = load_raw_import_source(&specifier) {
    return Ok(Some(LoadResponse {
      data: T::from_static(source),
      version: Some("1".to_string()),
      is_cjs: false,
      media_type: MediaType::TypeScript,
    }));
  } else {
    let specifier = if let Some(remapped_specifier) =
      remapper.maybe_remapped_specifier(load_specifier)
    {
      remapped_specifier
    } else {
      &specifier
    };
    let maybe_module = graph.try_get(specifier).ok().flatten();
    let maybe_source = if let Some(module) = maybe_module {
      match module {
        Module::Js(module) => {
          media_type = module.media_type;
          if let Some(npm_state) = &maybe_npm {
            is_cjs = npm_state.cjs_tracker.is_cjs_with_known_is_script(
              specifier,
              module.media_type,
              module.is_script,
            )?;
          }
          Some(
            module
              .fast_check_module()
              .map(|m| T::from_arc_str(m.source.clone()))
              .unwrap_or(T::from_arc_str(module.source.text.clone())),
          )
        }
        Module::Json(module) => {
          media_type = MediaType::Json;
          Some(T::from_arc_str(module.source.text.clone()))
        }
        Module::Wasm(module) => {
          media_type = MediaType::Dts;
          Some(T::from_arc_str(module.source_dts.clone()))
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
              maybe_npm,
              &mut media_type,
              &mut is_cjs,
            )?
          }
        }
      }
    } else if let Some(npm) = maybe_npm
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
    hash = get_maybe_hash(maybe_source.as_ref().map(|s| s.as_ref()), hash_data);
    maybe_source
  };
  let Some(data) = data else {
    return Ok(None);
  };
  Ok(Some(LoadResponse {
    data,
    version: hash,
    is_cjs,
    media_type,
  }))
}

pub static IGNORED_DIAGNOSTIC_CODES: LazyLock<HashSet<u64>> =
  LazyLock::new(|| {
    [
      // TS1452: 'resolution-mode' assertions are only supported when `moduleResolution` is `node16` or `nodenext`.
      // We specify the resolution mode to be CommonJS for some npm files and this
      // diagnostic gets generated even though we're using custom module resolution.
      1452,
      // Module '...' cannot be imported using this construct. The specifier only resolves to an
      // ES module, which cannot be imported with 'require'.
      1471,
      // TS1479: The current file is a CommonJS module whose imports will produce 'require' calls;
      // however, the referenced file is an ECMAScript module and cannot be imported with 'require'.
      1479,
      // TS1543: Importing a JSON file into an ECMAScript module requires a 'type: \"json\"' import
      // attribute when 'module' is set to 'NodeNext'.
      1543,
      // TS2306: File '.../index.d.ts' is not a module.
      // We get this for `x-typescript-types` declaration files which don't export
      // anything. We prefer to treat these as modules with no exports.
      2306,
      // TS2688: Cannot find type definition file for '...'.
      // We ignore because type definition files can end with '.ts'.
      2688,
      // TS2792: Cannot find module. Did you mean to set the 'moduleResolution'
      // option to 'node', or to add aliases to the 'paths' option?
      2792,
      // TS2307: Cannot find module '{0}' or its corresponding type declarations.
      2307, // Relative import errors to add an extension
      2834, 2835,
      // TS5009: Cannot find the common subdirectory path for the input files.
      5009,
      // TS5055: Cannot write file
      // 'http://localhost:4545/subdir/mt_application_x_javascript.j4.js'
      // because it would overwrite input file.
      5055,
      // TypeScript is overly opinionated that only CommonJS modules kinds can
      // support JSON imports.  Allegedly this was fixed in
      // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
      // so we will ignore complaints about this compiler setting.
      5070,
      // TS7016: Could not find a declaration file for module '...'. '...'
      // implicitly has an 'any' type.  This is due to `allowJs` being off by
      // default but importing of a JavaScript module.
      7016,
    ]
    .into_iter()
    .collect()
  });

pub static TYPES_NODE_IGNORABLE_NAMES: &[&str] = &[
  "AbortController",
  "AbortSignal",
  "AsyncIteratorObject",
  "atob",
  "Blob",
  "BroadcastChannel",
  "btoa",
  "ByteLengthQueuingStrategy",
  "CloseEvent",
  "CompressionStream",
  "CountQueuingStrategy",
  "CustomEvent",
  "DecompressionStream",
  "Disposable",
  "DOMException",
  "Event",
  "EventSource",
  "EventTarget",
  "fetch",
  "File",
  "Float32Array",
  "Float64Array",
  "FormData",
  "Headers",
  "ImportMeta",
  "MessageChannel",
  "MessageEvent",
  "MessagePort",
  "Navigator",
  "performance",
  "PerformanceEntry",
  "PerformanceMark",
  "PerformanceMeasure",
  "QueuingStrategy",
  "QueuingStrategySize",
  "ReadableByteStreamController",
  "ReadableStream",
  "ReadableStreamBYOBReader",
  "ReadableStreamBYOBRequest",
  "ReadableStreamDefaultController",
  "ReadableStreamDefaultReader",
  "ReadonlyArray",
  "Request",
  "Response",
  "Storage",
  "TextDecoder",
  "TextDecoderStream",
  "TextEncoder",
  "TextEncoderStream",
  "TransformStream",
  "TransformStreamDefaultController",
  "URL",
  "URLPattern",
  "URLSearchParams",
  "WebSocket",
  "WritableStream",
  "WritableStreamDefaultController",
  "WritableStreamDefaultWriter",
];

pub static NODE_ONLY_GLOBALS: &[&str] = &[
  "__dirname",
  "__filename",
  "\"buffer\"",
  "Buffer",
  "BufferConstructor",
  "BufferEncoding",
  "clearImmediate",
  "clearInterval",
  "clearTimeout",
  "console",
  "Console",
  "crypto",
  "ErrorConstructor",
  "gc",
  "Global",
  "localStorage",
  "queueMicrotask",
  "RequestInit",
  "ResponseInit",
  "sessionStorage",
  "setImmediate",
  "setInterval",
  "setTimeout",
];
