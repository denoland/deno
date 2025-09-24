// Copyright 2018-2025 the Deno authors. MIT license.
//
mod go;
mod js;

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
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

macro_rules! maybe_compressed_lib {
  ($name: expr, $file: expr) => {
    (
      $name,
      StaticAsset {
        is_lib: true,
        source: maybe_compressed_source!(concat!("tsc/dts/", $file)),
      },
    )
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
    maybe_compressed_lib!("lib.esnext.error.d.ts"),
    maybe_compressed_lib!("lib.esnext.float16.d.ts"),
    maybe_compressed_lib!("lib.esnext.full.d.ts"),
    maybe_compressed_lib!("lib.esnext.intl.d.ts"),
    maybe_compressed_lib!("lib.esnext.iterator.d.ts"),
    maybe_compressed_lib!("lib.esnext.promise.d.ts"),
    maybe_compressed_lib!("lib.esnext.sharedmemory.d.ts"),
    maybe_compressed_lib!("lib.scripthost.d.ts"),
    maybe_compressed_lib!("lib.webworker.asynciterable.d.ts"),
    maybe_compressed_lib!("lib.webworker.d.ts"),
    maybe_compressed_lib!("lib.webworker.importscripts.d.ts"),
    maybe_compressed_lib!("lib.webworker.iterable.d.ts"),
    (
      // Special file that can be used to inject the @types/node package.
      // This is used for `node:` specifiers.
      "node_types.d.ts",
      StaticAsset {
        is_lib: false,
        source: StaticAssetSource::Uncompressed(
          "/// <reference types=\"npm:@types/node\" />\n",
        ),
      },
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
    Some(Module::Npm(module)) => {
      if let Some(npm) = maybe_npm {
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
        // todo(dsherret): add support for injecting this in the graph so
        // we don't need this special code here.
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
            NodeJsErrorCode::ERR_TYPES_NOT_FOUND
            | NodeJsErrorCode::ERR_MODULE_NOT_FOUND => None,
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
  tsgo: bool,
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

  if tsgo {
    go::exec_request(request, root_names, root_map, remapped_specifiers)
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
