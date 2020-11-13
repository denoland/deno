// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::ast::parse;
use crate::ast::transpile_module;
use crate::ast::BundleHook;
use crate::ast::Location;
use crate::ast::ParsedModule;
use crate::colors;
use crate::diagnostics::Diagnostics;
use crate::import_map::ImportMap;
use crate::info::ModuleGraphInfo;
use crate::info::ModuleInfo;
use crate::info::ModuleInfoMap;
use crate::info::ModuleInfoMapItem;
use crate::lockfile::Lockfile;
use crate::media_type::MediaType;
use crate::program_state::ProgramState;
use crate::specifier_handler::CachedModule;
use crate::specifier_handler::Dependency;
use crate::specifier_handler::DependencyMap;
use crate::specifier_handler::Emit;
use crate::specifier_handler::FetchFuture;
use crate::specifier_handler::FetchHandler;
use crate::specifier_handler::SpecifierHandler;
use crate::tsc;
use crate::tsc_config::IgnoredCompilerOptions;
use crate::tsc_config::TsConfig;
use crate::version;
use deno_core::error::AnyError;

use deno_core::error::anyhow;
use deno_core::error::custom_error;
use deno_core::error::generic_error;
use deno_core::error::get_custom_error_class;
use deno_core::error::Context;
use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::stream::StreamExt;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleResolutionError;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_runtime::permissions::Permissions;
use regex::Regex;
use std::collections::HashSet;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use std::result;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

lazy_static! {
  /// Matched the `@deno-types` pragma.
  static ref DENO_TYPES_RE: Regex =
    Regex::new(r#"(?i)^\s*@deno-types\s*=\s*(?:["']([^"']+)["']|(\S+))"#)
      .unwrap();
  /// Matches a `/// <reference ... />` comment reference.
  static ref TRIPLE_SLASH_REFERENCE_RE: Regex =
    Regex::new(r"(?i)^/\s*<reference\s.*?/>").unwrap();
  /// Matches a path reference, which adds a dependency to a module
  static ref PATH_REFERENCE_RE: Regex =
    Regex::new(r#"(?i)\spath\s*=\s*["']([^"']*)["']"#).unwrap();
  /// Matches a types reference, which for JavaScript files indicates the
  /// location of types to use when type checking a program that includes it as
  /// a dependency.
  static ref TYPES_REFERENCE_RE: Regex =
    Regex::new(r#"(?i)\stypes\s*=\s*["']([^"']*)["']"#).unwrap();
}

/// A group of errors that represent errors that can occur when interacting with
/// a module graph.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum GraphError {
  /// A module using the HTTPS protocol is trying to import a module with an
  /// HTTP schema.
  InvalidDowngrade(ModuleSpecifier, Location),
  /// A remote module is trying to import a local module.
  InvalidLocalImport(ModuleSpecifier, Location),
  /// The source code is invalid, as it does not match the expected hash in the
  /// lockfile.
  InvalidSource(ModuleSpecifier, PathBuf),
  /// An unexpected dependency was requested for a module.
  MissingDependency(ModuleSpecifier, String),
  /// An unexpected specifier was requested.
  MissingSpecifier(ModuleSpecifier),
  /// The current feature is not supported.
  NotSupported(String),
  /// A unsupported media type was attempted to be imported as a module.
  UnsupportedImportType(ModuleSpecifier, MediaType),
}

impl fmt::Display for GraphError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      GraphError::InvalidDowngrade(ref specifier, ref location) => write!(f, "Modules imported via https are not allowed to import http modules.\n  Importing: {}\n    at {}", specifier, location),
      GraphError::InvalidLocalImport(ref specifier, ref location) => write!(f, "Remote modules are not allowed to import local modules.  Consider using a dynamic import instead.\n  Importing: {}\n    at {}", specifier, location),
      GraphError::InvalidSource(ref specifier, ref lockfile) => write!(f, "The source code is invalid, as it does not match the expected hash in the lock file.\n  Specifier: {}\n  Lock file: {}", specifier, lockfile.to_str().unwrap()),
      GraphError::MissingDependency(ref referrer, specifier) => write!(
        f,
        "The graph is missing a dependency.\n  Specifier: {} from {}",
        specifier, referrer
      ),
      GraphError::MissingSpecifier(ref specifier) => write!(
        f,
        "The graph is missing a specifier.\n  Specifier: {}",
        specifier
      ),
      GraphError::NotSupported(ref msg) => write!(f, "{}", msg),
      GraphError::UnsupportedImportType(ref specifier, ref media_type) => write!(f, "An unsupported media type was attempted to be imported as a module.\n  Specifier: {}\n  MediaType: {}", specifier, media_type),
    }
  }
}

impl Error for GraphError {}

/// A structure for handling bundle loading, which is implemented here, to
/// avoid a circular dependency with `ast`.
struct BundleLoader<'a> {
  cm: Rc<swc_common::SourceMap>,
  emit_options: &'a ast::EmitOptions,
  globals: &'a swc_common::Globals,
  graph: &'a Graph,
}

impl<'a> BundleLoader<'a> {
  pub fn new(
    graph: &'a Graph,
    emit_options: &'a ast::EmitOptions,
    globals: &'a swc_common::Globals,
    cm: Rc<swc_common::SourceMap>,
  ) -> Self {
    BundleLoader {
      cm,
      emit_options,
      globals,
      graph,
    }
  }
}

impl swc_bundler::Load for BundleLoader<'_> {
  fn load(
    &self,
    file: &swc_common::FileName,
  ) -> Result<swc_bundler::ModuleData, AnyError> {
    match file {
      swc_common::FileName::Custom(filename) => {
        let specifier = ModuleSpecifier::resolve_url_or_path(filename)
          .context("Failed to convert swc FileName to ModuleSpecifier.")?;
        if let Some(src) = self.graph.get_source(&specifier) {
          let media_type = self
            .graph
            .get_media_type(&specifier)
            .context("Looking up media type during bundling.")?;
          let (source_file, module) = transpile_module(
            filename,
            &src,
            &media_type,
            self.emit_options,
            self.globals,
            self.cm.clone(),
          )?;
          Ok(swc_bundler::ModuleData {
            fm: source_file,
            module,
            helpers: Default::default(),
          })
        } else {
          Err(
            GraphError::MissingDependency(specifier, "<bundle>".to_string())
              .into(),
          )
        }
      }
      _ => unreachable!("Received request for unsupported filename {:?}", file),
    }
  }
}

/// An enum which represents the parsed out values of references in source code.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TypeScriptReference {
  Path(String),
  Types(String),
}

/// Determine if a comment contains a triple slash reference and optionally
/// return its kind and value.
pub fn parse_ts_reference(comment: &str) -> Option<TypeScriptReference> {
  if !TRIPLE_SLASH_REFERENCE_RE.is_match(comment) {
    None
  } else if let Some(captures) = PATH_REFERENCE_RE.captures(comment) {
    Some(TypeScriptReference::Path(
      captures.get(1).unwrap().as_str().to_string(),
    ))
  } else if let Some(captures) = TYPES_REFERENCE_RE.captures(comment) {
    Some(TypeScriptReference::Types(
      captures.get(1).unwrap().as_str().to_string(),
    ))
  } else {
    None
  }
}

/// Determine if a comment contains a `@deno-types` pragma and optionally return
/// its value.
pub fn parse_deno_types(comment: &str) -> Option<String> {
  if let Some(captures) = DENO_TYPES_RE.captures(comment) {
    if let Some(m) = captures.get(1) {
      Some(m.as_str().to_string())
    } else if let Some(m) = captures.get(2) {
      Some(m.as_str().to_string())
    } else {
      panic!("unreachable");
    }
  } else {
    None
  }
}

/// A hashing function that takes the source code, version and optionally a
/// user provided config and generates a string hash which can be stored to
/// determine if the cached emit is valid or not.
fn get_version(source: &str, version: &str, config: &[u8]) -> String {
  crate::checksum::gen(&[source.as_bytes(), version.as_bytes(), config])
}

/// A logical representation of a module within a graph.
#[derive(Debug, Clone)]
pub struct Module {
  pub dependencies: DependencyMap,
  is_dirty: bool,
  is_parsed: bool,
  maybe_emit: Option<Emit>,
  maybe_emit_path: Option<(PathBuf, Option<PathBuf>)>,
  maybe_import_map: Option<Arc<Mutex<ImportMap>>>,
  maybe_types: Option<(String, ModuleSpecifier)>,
  maybe_version: Option<String>,
  media_type: MediaType,
  specifier: ModuleSpecifier,
  source: String,
  source_path: PathBuf,
}

impl Default for Module {
  fn default() -> Self {
    Module {
      dependencies: HashMap::new(),
      is_dirty: false,
      is_parsed: false,
      maybe_emit: None,
      maybe_emit_path: None,
      maybe_import_map: None,
      maybe_types: None,
      maybe_version: None,
      media_type: MediaType::Unknown,
      specifier: ModuleSpecifier::resolve_url("file:///example.js").unwrap(),
      source: "".to_string(),
      source_path: PathBuf::new(),
    }
  }
}

impl Module {
  pub fn new(
    cached_module: CachedModule,
    is_root: bool,
    maybe_import_map: Option<Arc<Mutex<ImportMap>>>,
  ) -> Self {
    // If this is a local root file, and its media type is unknown, set the
    // media type to JavaScript.  This allows easier ability to create "shell"
    // scripts with Deno.
    let media_type = if is_root
      && !cached_module.is_remote
      && cached_module.media_type == MediaType::Unknown
    {
      MediaType::JavaScript
    } else {
      cached_module.media_type
    };
    let mut module = Module {
      specifier: cached_module.specifier,
      maybe_import_map,
      media_type,
      source: cached_module.source,
      source_path: cached_module.source_path,
      maybe_emit: cached_module.maybe_emit,
      maybe_emit_path: cached_module.maybe_emit_path,
      maybe_version: cached_module.maybe_version,
      is_dirty: false,
      ..Self::default()
    };
    if module.maybe_import_map.is_none() {
      if let Some(dependencies) = cached_module.maybe_dependencies {
        module.dependencies = dependencies;
        module.is_parsed = true;
      }
    }
    module.maybe_types = if let Some(ref specifier) = cached_module.maybe_types
    {
      Some((
        specifier.clone(),
        module
          .resolve_import(&specifier, None)
          .expect("could not resolve module"),
      ))
    } else {
      None
    };
    module
  }

  /// Return `true` if the current hash of the module matches the stored
  /// version.
  pub fn is_emit_valid(&self, config: &[u8]) -> bool {
    if let Some(version) = self.maybe_version.clone() {
      version == get_version(&self.source, &version::deno(), config)
    } else {
      false
    }
  }

  /// Parse a module, populating the structure with data retrieved from the
  /// source of the module.
  pub fn parse(&mut self) -> Result<ParsedModule, AnyError> {
    let parsed_module =
      parse(self.specifier.as_str(), &self.source, &self.media_type)?;

    // parse out any triple slash references
    for comment in parsed_module.get_leading_comments().iter() {
      if let Some(ts_reference) = parse_ts_reference(&comment.text) {
        let location = parsed_module.get_location(&comment.span);
        match ts_reference {
          TypeScriptReference::Path(import) => {
            let specifier =
              self.resolve_import(&import, Some(location.clone()))?;
            let dep = self
              .dependencies
              .entry(import)
              .or_insert_with(|| Dependency::new(location));
            dep.maybe_code = Some(specifier);
          }
          TypeScriptReference::Types(import) => {
            let specifier =
              self.resolve_import(&import, Some(location.clone()))?;
            if self.media_type == MediaType::JavaScript
              || self.media_type == MediaType::JSX
            {
              // TODO(kitsonk) we need to specifically update the cache when
              // this value changes
              self.maybe_types = Some((import.clone(), specifier));
            } else {
              let dep = self
                .dependencies
                .entry(import)
                .or_insert_with(|| Dependency::new(location));
              dep.maybe_type = Some(specifier);
            }
          }
        }
      }
    }

    // Parse out all the syntactical dependencies for a module
    let dependencies = parsed_module.analyze_dependencies();
    for desc in dependencies.iter().filter(|desc| {
      desc.kind != swc_ecmascript::dep_graph::DependencyKind::Require
    }) {
      let location = Location {
        filename: self.specifier.to_string(),
        col: desc.col,
        line: desc.line,
      };

      // In situations where there is a potential issue with resolving the
      // import specifier, that ends up being a module resolution error for a
      // code dependency, we should not throw in the `ModuleGraph` but instead
      // wait until runtime and throw there, as with dynamic imports they need
      // to be catchable, which means they need to be resolved at runtime.
      let maybe_specifier =
        match self.resolve_import(&desc.specifier, Some(location.clone())) {
          Ok(specifier) => Some(specifier),
          Err(any_error) => {
            match any_error.downcast_ref::<ModuleResolutionError>() {
              Some(ModuleResolutionError::ImportPrefixMissing(_, _)) => None,
              _ => {
                return Err(any_error);
              }
            }
          }
        };

      // Parse out any `@deno-types` pragmas and modify dependency
      let maybe_type = if !desc.leading_comments.is_empty() {
        let comment = desc.leading_comments.last().unwrap();
        if let Some(deno_types) = parse_deno_types(&comment.text).as_ref() {
          Some(self.resolve_import(deno_types, Some(location.clone()))?)
        } else {
          None
        }
      } else {
        None
      };

      let dep = self
        .dependencies
        .entry(desc.specifier.to_string())
        .or_insert_with(|| Dependency::new(location));
      dep.is_dynamic = desc.is_dynamic;
      if let Some(specifier) = maybe_specifier {
        if desc.kind == swc_ecmascript::dep_graph::DependencyKind::ExportType
          || desc.kind == swc_ecmascript::dep_graph::DependencyKind::ImportType
        {
          dep.maybe_type = Some(specifier);
        } else {
          dep.maybe_code = Some(specifier);
        }
      }
      // If the dependency wasn't a type only dependency already, and there is
      // a `@deno-types` comment, then we will set the `maybe_type` dependency.
      if maybe_type.is_some() && dep.maybe_type.is_none() {
        dep.maybe_type = maybe_type;
      }
    }
    Ok(parsed_module)
  }

  fn resolve_import(
    &self,
    specifier: &str,
    maybe_location: Option<Location>,
  ) -> Result<ModuleSpecifier, AnyError> {
    let maybe_resolve = if let Some(import_map) = self.maybe_import_map.clone()
    {
      import_map
        .lock()
        .unwrap()
        .resolve(specifier, self.specifier.as_str())?
    } else {
      None
    };
    let mut remapped_import = false;
    let specifier = if let Some(module_specifier) = maybe_resolve {
      remapped_import = true;
      module_specifier
    } else {
      ModuleSpecifier::resolve_import(specifier, self.specifier.as_str())?
    };

    let referrer_scheme = self.specifier.as_url().scheme();
    let specifier_scheme = specifier.as_url().scheme();
    let location = maybe_location.unwrap_or(Location {
      filename: self.specifier.to_string(),
      line: 0,
      col: 0,
    });

    // Disallow downgrades from HTTPS to HTTP
    if referrer_scheme == "https" && specifier_scheme == "http" {
      return Err(
        GraphError::InvalidDowngrade(specifier.clone(), location).into(),
      );
    }

    // Disallow a remote URL from trying to import a local URL, unless it is a
    // remapped import via the import map
    if (referrer_scheme == "https" || referrer_scheme == "http")
      && !(specifier_scheme == "https" || specifier_scheme == "http")
      && !remapped_import
    {
      return Err(
        GraphError::InvalidLocalImport(specifier.clone(), location).into(),
      );
    }

    Ok(specifier)
  }

  pub fn set_emit(&mut self, code: String, maybe_map: Option<String>) {
    self.maybe_emit = Some(Emit::Cli((code, maybe_map)));
  }

  /// Calculate the hashed version of the module and update the `maybe_version`.
  pub fn set_version(&mut self, config: &[u8]) {
    self.maybe_version =
      Some(get_version(&self.source, &version::deno(), config))
  }

  pub fn size(&self) -> usize {
    self.source.as_bytes().len()
  }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Stats(pub Vec<(String, u32)>);

impl<'de> Deserialize<'de> for Stats {
  fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
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
      writeln!(f, "  {}: {}", key, value)?;
    }

    Ok(())
  }
}

/// A structure that provides information about a module graph result.
#[derive(Debug, Default)]
pub struct ResultInfo {
  /// A structure which provides diagnostic information (usually from `tsc`)
  /// about the code in the module graph.
  pub diagnostics: Diagnostics,
  /// A map of specifiers to the result of their resolution in the module graph.
  pub loadable_modules:
    HashMap<ModuleSpecifier, Result<ModuleSource, AnyError>>,
  /// Optionally ignored compiler options that represent any options that were
  /// ignored if there was a user provided configuration.
  pub maybe_ignored_options: Option<IgnoredCompilerOptions>,
  /// A structure providing key metrics around the operation performed, in
  /// milliseconds.
  pub stats: Stats,
}

/// Represents the "default" type library that should be used when type
/// checking the code in the module graph.  Note that a user provided config
/// of `"lib"` would override this value.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TypeLib {
  DenoWindow,
  DenoWorker,
  UnstableDenoWindow,
  UnstableDenoWorker,
}

impl Default for TypeLib {
  fn default() -> Self {
    TypeLib::DenoWindow
  }
}

impl Serialize for TypeLib {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let value = match self {
      TypeLib::DenoWindow => vec!["deno.window".to_string()],
      TypeLib::DenoWorker => vec!["deno.worker".to_string()],
      TypeLib::UnstableDenoWindow => {
        vec!["deno.window".to_string(), "deno.unstable".to_string()]
      }
      TypeLib::UnstableDenoWorker => {
        vec!["deno.worker".to_string(), "deno.unstable".to_string()]
      }
    };
    Serialize::serialize(&value, serializer)
  }
}

#[derive(Debug, Default)]
pub struct BundleOptions {
  /// If `true` then debug logging will be output from the isolate.
  pub debug: bool,
  /// An optional string that points to a user supplied TypeScript configuration
  /// file that augments the the default configuration passed to the TypeScript
  /// compiler.
  pub maybe_config_path: Option<String>,
}

#[derive(Debug, Default)]
pub struct CheckOptions {
  /// If `true` then debug logging will be output from the isolate.
  pub debug: bool,
  /// Utilise the emit from `tsc` to update the emitted code for modules.
  pub emit: bool,
  /// The base type libraries that should be used when type checking.
  pub lib: TypeLib,
  /// An optional string that points to a user supplied TypeScript configuration
  /// file that augments the the default configuration passed to the TypeScript
  /// compiler.
  pub maybe_config_path: Option<String>,
  /// Ignore any previously emits and ensure that all files are emitted from
  /// source.
  pub reload: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum BundleType {
  /// Return the emitted contents of the program as a single "flattened" ES
  /// module.
  Esm,
  // TODO(@kitsonk) once available in swc
  // Iife,
  /// Do not bundle the emit, instead returning each of the modules that are
  /// part of the program as individual files.
  None,
}

impl Default for BundleType {
  fn default() -> Self {
    BundleType::None
  }
}

#[derive(Debug, Default)]
pub struct EmitOptions {
  /// If true, then code will be type checked, otherwise type checking will be
  /// skipped.  If false, then swc will be used for the emit, otherwise tsc will
  /// be used.
  pub check: bool,
  /// Indicate the form the result of the emit should take.
  pub bundle_type: BundleType,
  /// If `true` then debug logging will be output from the isolate.
  pub debug: bool,
  /// An optional map that contains user supplied TypeScript compiler
  /// configuration options that are passed to the TypeScript compiler.
  pub maybe_user_config: Option<HashMap<String, Value>>,
}

/// A structure which provides options when transpiling modules.
#[derive(Debug, Default)]
pub struct TranspileOptions {
  /// If `true` then debug logging will be output from the isolate.
  pub debug: bool,
  /// An optional string that points to a user supplied TypeScript configuration
  /// file that augments the the default configuration passed to the TypeScript
  /// compiler.
  pub maybe_config_path: Option<String>,
  /// Ignore any previously emits and ensure that all files are emitted from
  /// source.
  pub reload: bool,
}

#[derive(Debug, Clone)]
enum ModuleSlot {
  /// The module fetch resulted in a non-recoverable error.
  Err(Arc<AnyError>),
  /// The the fetch resulted in a module.
  Module(Box<Module>),
  /// Used to denote a module that isn't part of the graph.
  None,
  /// The fetch of the module is pending.
  Pending,
}

/// A dependency graph of modules, were the modules that have been inserted via
/// the builder will be loaded into the graph.  Also provides an interface to
/// be able to manipulate and handle the graph.
#[derive(Debug, Clone)]
pub struct Graph {
  /// A reference to the specifier handler that will retrieve and cache modules
  /// for the graph.
  handler: Arc<Mutex<dyn SpecifierHandler>>,
  /// Optional TypeScript build info that will be passed to `tsc` if `tsc` is
  /// invoked.
  maybe_tsbuildinfo: Option<String>,
  /// The modules that are part of the graph.
  modules: HashMap<ModuleSpecifier, ModuleSlot>,
  /// A map of redirects, where a module specifier is redirected to another
  /// module specifier by the handler.  All modules references should be
  /// resolved internally via this, before attempting to access the module via
  /// the handler, to make sure the correct modules is being dealt with.
  redirects: HashMap<ModuleSpecifier, ModuleSpecifier>,
  /// The module specifiers that have been uniquely added to the graph, which
  /// does not include any transient dependencies.
  roots: Vec<ModuleSpecifier>,
  /// If all of the root modules are dynamically imported, then this is true.
  /// This is used to ensure correct `--reload` behavior, where subsequent
  /// calls to a module graph where the emit is already valid do not cause the
  /// graph to re-emit.
  roots_dynamic: bool,
  // A reference to lock file that will be used to check module integrity.
  maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
}

/// Convert a specifier and a module slot in a result to the module source which
/// is needed by Deno core for loading the module.
fn to_module_result(
  (specifier, module_slot): (&ModuleSpecifier, &ModuleSlot),
) -> (ModuleSpecifier, Result<ModuleSource, AnyError>) {
  match module_slot {
    ModuleSlot::Err(err) => (specifier.clone(), Err(anyhow!(err.to_string()))),
    ModuleSlot::Module(module) => (
      specifier.clone(),
      if let Some(emit) = &module.maybe_emit {
        match emit {
          Emit::Cli((code, _)) => Ok(ModuleSource {
            code: code.clone(),
            module_url_found: module.specifier.to_string(),
            module_url_specified: specifier.to_string(),
          }),
        }
      } else {
        match module.media_type {
          MediaType::JavaScript | MediaType::Unknown => Ok(ModuleSource {
            code: module.source.clone(),
            module_url_found: module.specifier.to_string(),
            module_url_specified: specifier.to_string(),
          }),
          _ => Err(custom_error(
            "NotFound",
            format!("Compiled module not found \"{}\"", specifier),
          )),
        }
      },
    ),
    _ => (
      specifier.clone(),
      Err(anyhow!("Module \"{}\" unavailable.", specifier)),
    ),
  }
}

impl Graph {
  /// Create a new instance of a graph, ready to have modules loaded it.
  ///
  /// The argument `handler` is an instance of a structure that implements the
  /// `SpecifierHandler` trait.
  ///
  pub fn new(
    handler: Arc<Mutex<dyn SpecifierHandler>>,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    Graph {
      handler,
      maybe_tsbuildinfo: None,
      modules: HashMap::new(),
      redirects: HashMap::new(),
      roots: Vec::new(),
      roots_dynamic: true,
      maybe_lockfile,
    }
  }

  /// Transform the module graph into a single JavaScript module which is
  /// returned as a `String` in the result.
  pub fn bundle(
    &self,
    options: BundleOptions,
  ) -> Result<(String, Stats, Option<IgnoredCompilerOptions>), AnyError> {
    if self.roots.is_empty() || self.roots.len() > 1 {
      return Err(GraphError::NotSupported(format!("Bundling is only supported when there is a single root module in the graph.  Found: {}", self.roots.len())).into());
    }

    let start = Instant::now();
    let root_specifier = self.roots[0].clone();
    let mut ts_config = TsConfig::new(json!({
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "inlineSourceMap": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
    }));
    let maybe_ignored_options =
      ts_config.merge_tsconfig(options.maybe_config_path)?;

    let s = self.emit_bundle(&root_specifier, &ts_config.into())?;
    let stats = Stats(vec![
      ("Files".to_string(), self.modules.len() as u32),
      ("Total time".to_string(), start.elapsed().as_millis() as u32),
    ]);

    Ok((s, stats, maybe_ignored_options))
  }

  /// Type check the module graph, corresponding to the options provided.
  pub fn check(self, options: CheckOptions) -> Result<ResultInfo, AnyError> {
    let mut config = TsConfig::new(json!({
      "allowJs": true,
      // TODO(@kitsonk) is this really needed?
      "esModuleInterop": true,
      // Enabled by default to align to transpile/swc defaults
      "experimentalDecorators": true,
      "incremental": true,
      "isolatedModules": true,
      "lib": options.lib,
      "module": "esnext",
      "strict": true,
      "target": "esnext",
      "tsBuildInfoFile": "deno:///.tsbuildinfo",
    }));
    if options.emit {
      config.merge(&json!({
        // TODO(@kitsonk) consider enabling this by default
        //   see: https://github.com/denoland/deno/issues/7732
        "emitDecoratorMetadata": false,
        "jsx": "react",
        "inlineSourceMap": true,
        "outDir": "deno://",
        "removeComments": true,
      }));
    } else {
      config.merge(&json!({
        "noEmit": true,
      }));
    }
    let maybe_ignored_options =
      config.merge_tsconfig(options.maybe_config_path)?;

    // Short circuit if none of the modules require an emit, or all of the
    // modules that require an emit have a valid emit.  There is also an edge
    // case where there are multiple imports of a dynamic module during a
    // single invocation, if that is the case, even if there is a reload, we
    // will simply look at if the emit is invalid, to avoid two checks for the
    // same programme.
    if !self.needs_emit(&config)
      || (self.is_emit_valid(&config)
        && (!options.reload || self.roots_dynamic))
    {
      debug!("graph does not need to be checked or emitted.");
      return Ok(ResultInfo {
        maybe_ignored_options,
        loadable_modules: self.get_loadable_modules(),
        ..Default::default()
      });
    }

    // TODO(@kitsonk) not totally happy with this here, but this is the first
    // point where we know we are actually going to check the program.  If we
    // moved it out of here, we wouldn't know until after the check has already
    // happened, which isn't informative to the users.
    for specifier in &self.roots {
      info!("{} {}", colors::green("Check"), specifier);
    }

    let root_names = self.get_root_names(!config.get_check_js())?;
    let maybe_tsbuildinfo = self.maybe_tsbuildinfo.clone();
    let hash_data =
      vec![config.as_bytes(), version::deno().as_bytes().to_owned()];
    let graph = Arc::new(Mutex::new(self));

    let response = tsc::exec(tsc::Request {
      config: config.clone(),
      debug: options.debug,
      graph: graph.clone(),
      hash_data,
      maybe_tsbuildinfo,
      root_names,
    })?;

    let mut graph = graph.lock().unwrap();
    graph.maybe_tsbuildinfo = response.maybe_tsbuildinfo;
    // Only process changes to the graph if there are no diagnostics and there
    // were files emitted.
    if response.diagnostics.is_empty() {
      if !response.emitted_files.is_empty() {
        let mut codes = HashMap::new();
        let mut maps = HashMap::new();
        let check_js = config.get_check_js();
        for emit in &response.emitted_files {
          if let Some(specifiers) = &emit.maybe_specifiers {
            assert!(specifiers.len() == 1, "Unexpected specifier length");
            // The specifier emitted might not be the redirected specifier, and
            // therefore we need to ensure it is the correct one.
            let specifier = graph.resolve_specifier(&specifiers[0]);
            // Sometimes if tsc sees a CommonJS file it will _helpfully_ output it
            // to ESM, which we don't really want unless someone has enabled the
            // check_js option.
            if !check_js
              && graph.get_media_type(&specifier) == Some(MediaType::JavaScript)
            {
              debug!("skipping emit for {}", specifier);
              continue;
            }
            match emit.media_type {
              MediaType::JavaScript => {
                codes.insert(specifier.clone(), emit.data.clone());
              }
              MediaType::SourceMap => {
                maps.insert(specifier.clone(), emit.data.clone());
              }
              _ => unreachable!(),
            }
          }
        }
        let config = config.as_bytes();
        for (specifier, code) in codes.iter() {
          if let ModuleSlot::Module(module) =
            graph.get_module_mut(specifier).unwrap()
          {
            module.set_emit(code.clone(), maps.get(specifier).cloned());
            module.set_version(&config);
            module.is_dirty = true;
          } else {
            return Err(GraphError::MissingSpecifier(specifier.clone()).into());
          }
        }
      }
      graph.flush()?;
    }

    Ok(ResultInfo {
      diagnostics: response.diagnostics,
      loadable_modules: graph.get_loadable_modules(),
      maybe_ignored_options,
      stats: response.stats,
    })
  }

  /// Emit the module graph in a specific format.  This is specifically designed
  /// to be an "all-in-one" API for access by the runtime, allowing both
  /// emitting single modules as well as bundles, using Deno module resolution
  /// or supplied sources.
  pub fn emit(
    mut self,
    options: EmitOptions,
  ) -> Result<(HashMap<String, String>, ResultInfo), AnyError> {
    let mut config = TsConfig::new(json!({
      "allowJs": true,
      "checkJs": false,
      // TODO(@kitsonk) consider enabling this by default
      //   see: https://github.com/denoland/deno/issues/7732
      "emitDecoratorMetadata": false,
      "esModuleInterop": true,
      "experimentalDecorators": true,
      "inlineSourceMap": false,
      "isolatedModules": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "lib": TypeLib::DenoWindow,
      "module": "esnext",
      "strict": true,
      "target": "esnext",
    }));
    let opts = match options.bundle_type {
      BundleType::Esm => json!({
        "noEmit": true,
      }),
      BundleType::None => json!({
        "outDir": "deno://",
        "removeComments": true,
        "sourceMap": true,
      }),
    };
    config.merge(&opts);
    let maybe_ignored_options =
      if let Some(user_options) = &options.maybe_user_config {
        config.merge_user_config(user_options)?
      } else {
        None
      };

    if !options.check && config.get_declaration() {
      return Err(anyhow!("The option of `check` is false, but the compiler option of `declaration` is true which is not currently supported."));
    }
    if options.bundle_type != BundleType::None && config.get_declaration() {
      return Err(anyhow!("The bundle option is set, but the compiler option of `declaration` is true which is not currently supported."));
    }

    let mut emitted_files = HashMap::new();
    if options.check {
      let root_names = self.get_root_names(!config.get_check_js())?;
      let hash_data =
        vec![config.as_bytes(), version::deno().as_bytes().to_owned()];
      let graph = Arc::new(Mutex::new(self));
      let response = tsc::exec(tsc::Request {
        config: config.clone(),
        debug: options.debug,
        graph: graph.clone(),
        hash_data,
        maybe_tsbuildinfo: None,
        root_names,
      })?;

      let graph = graph.lock().unwrap();
      match options.bundle_type {
        BundleType::Esm => {
          assert!(
            response.emitted_files.is_empty(),
            "No files should have been emitted from tsc."
          );
          assert_eq!(
            graph.roots.len(),
            1,
            "Only a single root module supported."
          );
          let specifier = &graph.roots[0];
          let s = graph.emit_bundle(specifier, &config.into())?;
          emitted_files.insert("deno:///bundle.js".to_string(), s);
        }
        BundleType::None => {
          for emitted_file in &response.emitted_files {
            assert!(
              emitted_file.maybe_specifiers.is_some(),
              "Orphaned file emitted."
            );
            let specifiers = emitted_file.maybe_specifiers.clone().unwrap();
            assert_eq!(
              specifiers.len(),
              1,
              "An unexpected number of specifiers associated with emitted file."
            );
            let specifier = specifiers[0].clone();
            let extension = match emitted_file.media_type {
              MediaType::JavaScript => ".js",
              MediaType::SourceMap => ".js.map",
              MediaType::Dts => ".d.ts",
              _ => unreachable!(),
            };
            let key = format!("{}{}", specifier, extension);
            emitted_files.insert(key, emitted_file.data.clone());
          }
        }
      };

      Ok((
        emitted_files,
        ResultInfo {
          diagnostics: response.diagnostics,
          loadable_modules: graph.get_loadable_modules(),
          maybe_ignored_options,
          stats: response.stats,
        },
      ))
    } else {
      let start = Instant::now();
      let mut emit_count = 0_u32;
      match options.bundle_type {
        BundleType::Esm => {
          assert_eq!(
            self.roots.len(),
            1,
            "Only a single root module supported."
          );
          let specifier = &self.roots[0];
          let s = self.emit_bundle(specifier, &config.into())?;
          emit_count += 1;
          emitted_files.insert("deno:///bundle.js".to_string(), s);
        }
        BundleType::None => {
          let emit_options: ast::EmitOptions = config.into();
          for (_, module_slot) in self.modules.iter_mut() {
            if let ModuleSlot::Module(module) = module_slot {
              if !(emit_options.check_js
                || module.media_type == MediaType::JSX
                || module.media_type == MediaType::TSX
                || module.media_type == MediaType::TypeScript)
              {
                emitted_files
                  .insert(module.specifier.to_string(), module.source.clone());
              }
              let parsed_module = module.parse()?;
              let (code, maybe_map) = parsed_module.transpile(&emit_options)?;
              emit_count += 1;
              emitted_files.insert(format!("{}.js", module.specifier), code);
              if let Some(map) = maybe_map {
                emitted_files
                  .insert(format!("{}.js.map", module.specifier), map);
              }
            }
          }
          self.flush()?;
        }
      }

      let stats = Stats(vec![
        ("Files".to_string(), self.modules.len() as u32),
        ("Emitted".to_string(), emit_count),
        ("Total time".to_string(), start.elapsed().as_millis() as u32),
      ]);

      Ok((
        emitted_files,
        ResultInfo {
          diagnostics: Default::default(),
          loadable_modules: self.get_loadable_modules(),
          maybe_ignored_options,
          stats,
        },
      ))
    }
  }

  /// Shared between `bundle()` and `emit()`.
  fn emit_bundle(
    &self,
    specifier: &ModuleSpecifier,
    emit_options: &ast::EmitOptions,
  ) -> Result<String, AnyError> {
    let cm = Rc::new(swc_common::SourceMap::new(
      swc_common::FilePathMapping::empty(),
    ));
    let globals = swc_common::Globals::new();
    let loader = BundleLoader::new(self, emit_options, &globals, cm.clone());
    let hook = Box::new(BundleHook);
    let bundler = swc_bundler::Bundler::new(
      &globals,
      cm.clone(),
      loader,
      self,
      swc_bundler::Config::default(),
      hook,
    );
    let mut entries = HashMap::new();
    entries.insert(
      "bundle".to_string(),
      swc_common::FileName::Custom(specifier.to_string()),
    );
    let output = bundler
      .bundle(entries)
      .context("Unable to output bundle during Graph::bundle().")?;
    let mut buf = Vec::new();
    {
      let mut emitter = swc_ecmascript::codegen::Emitter {
        cfg: swc_ecmascript::codegen::Config { minify: false },
        cm: cm.clone(),
        comments: None,
        wr: Box::new(swc_ecmascript::codegen::text_writer::JsWriter::new(
          cm, "\n", &mut buf, None,
        )),
      };

      emitter
        .emit_module(&output[0].module)
        .context("Unable to emit bundle during Graph::bundle().")?;
    }

    String::from_utf8(buf).context("Emitted bundle is an invalid utf-8 string.")
  }

  /// Update the handler with any modules that are marked as _dirty_ and update
  /// any build info if present.
  fn flush(&mut self) -> Result<(), AnyError> {
    let mut handler = self.handler.lock().unwrap();
    for (_, module_slot) in self.modules.iter_mut() {
      if let ModuleSlot::Module(module) = module_slot {
        if module.is_dirty {
          if let Some(emit) = &module.maybe_emit {
            handler.set_cache(&module.specifier, emit)?;
          }
          if let Some(version) = &module.maybe_version {
            handler.set_version(&module.specifier, version.clone())?;
          }
          module.is_dirty = false;
        }
      }
    }
    for root_specifier in self.roots.iter() {
      if let Some(tsbuildinfo) = &self.maybe_tsbuildinfo {
        handler.set_tsbuildinfo(root_specifier, tsbuildinfo.to_owned())?;
      }
    }

    Ok(())
  }

  fn get_info(
    &self,
    specifier: &ModuleSpecifier,
    seen: &mut HashSet<ModuleSpecifier>,
    totals: &mut HashMap<ModuleSpecifier, usize>,
  ) -> ModuleInfo {
    let not_seen = seen.insert(specifier.clone());
    let module = match self.get_module(specifier) {
      ModuleSlot::Module(module) => module,
      ModuleSlot::Err(err) => {
        error!("{}: {}", colors::red_bold("error"), err.to_string());
        std::process::exit(1);
      }
      _ => unreachable!(),
    };
    let mut deps = Vec::new();
    let mut total_size = None;

    if not_seen {
      let mut seen_deps = HashSet::new();
      // TODO(@kitsonk) https://github.com/denoland/deno/issues/7927
      for (_, dep) in module.dependencies.iter() {
        // Check the runtime code dependency
        if let Some(code_dep) = &dep.maybe_code {
          if seen_deps.insert(code_dep.clone()) {
            deps.push(self.get_info(code_dep, seen, totals));
          }
        }
      }
      deps.sort();
      total_size = if let Some(total) = totals.get(specifier) {
        Some(total.to_owned())
      } else {
        let mut total = deps
          .iter()
          .map(|d| {
            if let Some(total_size) = d.total_size {
              total_size
            } else {
              0
            }
          })
          .sum();
        total += module.size();
        totals.insert(specifier.clone(), total);
        Some(total)
      };
    }

    ModuleInfo {
      deps,
      name: specifier.clone(),
      size: module.size(),
      total_size,
    }
  }

  fn get_info_map(&self) -> ModuleInfoMap {
    let map = self
      .modules
      .iter()
      .filter_map(|(specifier, module_slot)| {
        if let ModuleSlot::Module(module) = module_slot {
          let mut deps = BTreeSet::new();
          for (_, dep) in module.dependencies.iter() {
            if let Some(code_dep) = &dep.maybe_code {
              deps.insert(code_dep.clone());
            }
            if let Some(type_dep) = &dep.maybe_type {
              deps.insert(type_dep.clone());
            }
          }
          if let Some((_, types_dep)) = &module.maybe_types {
            deps.insert(types_dep.clone());
          }
          let item = ModuleInfoMapItem {
            deps: deps.into_iter().collect(),
            size: module.size(),
          };
          Some((specifier.clone(), item))
        } else {
          None
        }
      })
      .collect();

    ModuleInfoMap::new(map)
  }

  /// Retrieve a map that contains a representation of each module in the graph
  /// which can be used to provide code to a module loader without holding all
  /// the state to be able to operate on the graph.
  pub fn get_loadable_modules(
    &self,
  ) -> HashMap<ModuleSpecifier, Result<ModuleSource, AnyError>> {
    let mut loadable_modules: HashMap<
      ModuleSpecifier,
      Result<ModuleSource, AnyError>,
    > = self.modules.iter().map(to_module_result).collect();
    for (specifier, _) in self.redirects.iter() {
      if let Some(module_slot) =
        self.modules.get(self.resolve_specifier(specifier))
      {
        let (_, result) = to_module_result((specifier, module_slot));
        loadable_modules.insert(specifier.clone(), result);
      }
    }
    loadable_modules
  }

  pub fn get_media_type(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<MediaType> {
    if let ModuleSlot::Module(module) = self.get_module(specifier) {
      Some(module.media_type)
    } else {
      None
    }
  }

  fn get_module(&self, specifier: &ModuleSpecifier) -> &ModuleSlot {
    let s = self.resolve_specifier(specifier);
    if let Some(module_slot) = self.modules.get(s) {
      module_slot
    } else {
      &ModuleSlot::None
    }
  }

  fn get_module_mut(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> Option<&mut ModuleSlot> {
    // this is duplicated code because `.resolve_specifier` requires an
    // immutable borrow, but if `.resolve_specifier` is mut, then everything
    // that calls it is is mut
    let mut s = specifier;
    while let Some(redirect) = self.redirects.get(s) {
      s = redirect;
    }
    self.modules.get_mut(s)
  }

  /// Consume graph and return list of all module specifiers contained in the
  /// graph.
  pub fn get_modules(&self) -> Vec<ModuleSpecifier> {
    self.modules.keys().map(|s| s.to_owned()).collect()
  }

  /// Transform `self.roots` into something that works for `tsc`, because `tsc`
  /// doesn't like root names without extensions that match its expectations,
  /// nor does it have any concept of redirection, so we have to resolve all
  /// that upfront before feeding it to `tsc`. In addition, if checkJs is not
  /// true, we should pass all emittable files in as the roots, so that `tsc`
  /// type checks them and potentially emits them.
  fn get_root_names(
    &self,
    include_emittable: bool,
  ) -> Result<Vec<(ModuleSpecifier, MediaType)>, AnyError> {
    let root_names: Vec<ModuleSpecifier> = if include_emittable {
      // in situations where there is `allowJs` with tsc, but not `checkJs`,
      // then tsc will not parse the whole module graph, meaning that any
      // JavaScript importing TypeScript will get ignored, meaning that those
      // files will not get emitted.  To counter act that behavior, we will
      // include all modules that are emittable.
      let mut specifiers = HashSet::<&ModuleSpecifier>::new();
      for (_, module_slot) in self.modules.iter() {
        if let ModuleSlot::Module(module) = module_slot {
          if module.media_type == MediaType::JSX
            || module.media_type == MediaType::TypeScript
            || module.media_type == MediaType::TSX
          {
            specifiers.insert(&module.specifier);
          }
        }
      }
      // We should include all the original roots as well.
      for specifier in self.roots.iter() {
        specifiers.insert(specifier);
      }
      specifiers.into_iter().cloned().collect()
    } else {
      self.roots.clone()
    };
    let mut root_types = vec![];
    for ms in root_names {
      // if the root module has a types specifier, we should be sending that
      // to tsc instead of the original specifier
      let specifier = self.resolve_specifier(&ms);
      let module = match self.get_module(specifier) {
        ModuleSlot::Module(module) => module,
        ModuleSlot::Err(error) => {
          // It would be great if we could just clone the error here...
          if let Some(class) = get_custom_error_class(error) {
            return Err(custom_error(class, error.to_string()));
          } else {
            panic!("unsupported ModuleSlot error");
          }
        }
        _ => {
          panic!("missing module");
        }
      };
      let specifier = if let Some((_, types_specifier)) = &module.maybe_types {
        self.resolve_specifier(types_specifier)
      } else {
        specifier
      };
      root_types.push((
        // root modules can be redirects, so before we pass it to tsc we need
        // to resolve the redirect
        specifier.clone(),
        self.get_media_type(specifier).unwrap(),
      ));
    }
    Ok(root_types)
  }

  /// Get the source for a given module specifier.  If the module is not part
  /// of the graph, the result will be `None`.
  pub fn get_source(&self, specifier: &ModuleSpecifier) -> Option<String> {
    if let ModuleSlot::Module(module) = self.get_module(specifier) {
      Some(module.source.clone())
    } else {
      None
    }
  }

  /// Return a structure which provides information about the module graph and
  /// the relationship of the modules in the graph.  This structure is used to
  /// provide information for the `info` subcommand.
  pub fn info(&self) -> Result<ModuleGraphInfo, AnyError> {
    if self.roots.is_empty() || self.roots.len() > 1 {
      return Err(GraphError::NotSupported(format!("Info is only supported when there is a single root module in the graph.  Found: {}", self.roots.len())).into());
    }

    let module = self.roots[0].clone();
    let m = if let ModuleSlot::Module(module) = self.get_module(&module) {
      module
    } else {
      return Err(GraphError::MissingSpecifier(module.clone()).into());
    };

    let mut seen = HashSet::new();
    let mut totals = HashMap::new();
    let info = self.get_info(&module, &mut seen, &mut totals);

    let files = self.get_info_map();
    let total_size = totals.get(&module).unwrap_or(&m.size()).to_owned();
    let (compiled, map) =
      if let Some((emit_path, maybe_map_path)) = &m.maybe_emit_path {
        (Some(emit_path.clone()), maybe_map_path.clone())
      } else {
        (None, None)
      };

    let dep_count = self
      .modules
      .iter()
      .filter_map(|(_, m)| match m {
        ModuleSlot::Module(_) => Some(1),
        _ => None,
      })
      .count()
      - 1;

    Ok(ModuleGraphInfo {
      compiled,
      dep_count,
      file_type: m.media_type,
      files,
      info,
      local: m.source_path.clone(),
      map,
      module,
      total_size,
    })
  }

  /// Determines if all of the modules in the graph that require an emit have
  /// a valid emit.  Returns `true` if all the modules have a valid emit,
  /// otherwise false.
  fn is_emit_valid(&self, config: &TsConfig) -> bool {
    let check_js = config.get_check_js();
    let config = config.as_bytes();
    self.modules.iter().all(|(_, m)| {
      if let ModuleSlot::Module(m) = m {
        let needs_emit = match m.media_type {
          MediaType::TypeScript | MediaType::TSX | MediaType::JSX => true,
          MediaType::JavaScript => check_js,
          _ => false,
        };
        if needs_emit {
          m.is_emit_valid(&config)
        } else {
          true
        }
      } else {
        false
      }
    })
  }

  /// Verify the subresource integrity of the graph based upon the optional
  /// lockfile, updating the lockfile with any missing resources.  This will
  /// error if any of the resources do not match their lock status.
  pub fn lock(&self) {
    if let Some(lf) = self.maybe_lockfile.as_ref() {
      let mut lockfile = lf.lock().unwrap();
      for (ms, module_slot) in self.modules.iter() {
        if let ModuleSlot::Module(module) = module_slot {
          let specifier = module.specifier.to_string();
          let valid = lockfile.check_or_insert(&specifier, &module.source);
          if !valid {
            eprintln!(
              "{}",
              GraphError::InvalidSource(ms.clone(), lockfile.filename.clone())
            );
            std::process::exit(10);
          }
        }
      }
    }
  }

  /// Determines if any of the modules in the graph are required to be emitted.
  /// This is similar to `emit_valid()` except that the actual emit isn't
  /// checked to determine if it is valid.
  fn needs_emit(&self, config: &TsConfig) -> bool {
    let check_js = config.get_check_js();
    self.modules.iter().any(|(_, m)| match m {
      ModuleSlot::Module(m) => match m.media_type {
        MediaType::TypeScript | MediaType::TSX | MediaType::JSX => true,
        MediaType::JavaScript => check_js,
        _ => false,
      },
      _ => false,
    })
  }

  /// Given a string specifier and a referring module specifier, provide the
  /// resulting module specifier and media type for the module that is part of
  /// the graph.
  ///
  /// # Arguments
  ///
  /// * `specifier` - The string form of the module specifier that needs to be
  ///   resolved.
  /// * `referrer` - The referring `ModuleSpecifier`.
  /// * `prefer_types` - When resolving to a module specifier, determine if a
  ///   type dependency is preferred over a code dependency.  This is set to
  ///   `true` when resolving module names for `tsc` as it needs the type
  ///   dependency over the code, while other consumers do not handle type only
  ///   dependencies.
  pub fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
    prefer_types: bool,
  ) -> Result<ModuleSpecifier, AnyError> {
    let module = if let ModuleSlot::Module(module) = self.get_module(referrer) {
      module
    } else {
      return Err(GraphError::MissingSpecifier(referrer.clone()).into());
    };
    if !module.dependencies.contains_key(specifier) {
      return Err(
        GraphError::MissingDependency(
          referrer.to_owned(),
          specifier.to_owned(),
        )
        .into(),
      );
    }
    let dependency = module.dependencies.get(specifier).unwrap();
    // If there is a @deno-types pragma that impacts the dependency, then the
    // maybe_type property will be set with that specifier, otherwise we use the
    // specifier that point to the runtime code.
    let resolved_specifier = if prefer_types && dependency.maybe_type.is_some()
    {
      dependency.maybe_type.clone().unwrap()
    } else if let Some(code_specifier) = dependency.maybe_code.clone() {
      code_specifier
    } else {
      return Err(
        GraphError::MissingDependency(
          referrer.to_owned(),
          specifier.to_owned(),
        )
        .into(),
      );
    };
    let dep_module = if let ModuleSlot::Module(dep_module) =
      self.get_module(&resolved_specifier)
    {
      dep_module
    } else {
      return Err(
        GraphError::MissingDependency(
          referrer.to_owned(),
          resolved_specifier.to_string(),
        )
        .into(),
      );
    };
    // In the case that there is a X-TypeScript-Types or a triple-slash types,
    // then the `maybe_types` specifier will be populated and we should use that
    // instead.
    let result = if prefer_types && dep_module.maybe_types.is_some() {
      let (_, types) = dep_module.maybe_types.clone().unwrap();
      // It is possible that `types` points to a redirected specifier, so we
      // need to ensure it resolves to the final specifier in the graph.
      self.resolve_specifier(&types).clone()
    } else {
      dep_module.specifier.clone()
    };

    Ok(result)
  }

  /// Takes a module specifier and returns the "final" specifier, accounting for
  /// any redirects that may have occurred.
  fn resolve_specifier<'a>(
    &'a self,
    specifier: &'a ModuleSpecifier,
  ) -> &'a ModuleSpecifier {
    let mut s = specifier;
    let mut seen = HashSet::new();
    seen.insert(s.clone());
    while let Some(redirect) = self.redirects.get(s) {
      if !seen.insert(redirect.clone()) {
        eprintln!("An infinite loop of module redirections detected.\n  Original specifier: {}", specifier);
        break;
      }
      s = redirect;
      if seen.len() > 5 {
        eprintln!("An excessive number of module redirections detected.\n  Original specifier: {}", specifier);
        break;
      }
    }
    s
  }

  /// Transpile (only transform) the graph, updating any emitted modules
  /// with the specifier handler.  The result contains any performance stats
  /// from the compiler and optionally any user provided configuration compiler
  /// options that were ignored.
  ///
  /// # Arguments
  ///
  /// * `options` - A structure of options which impact how the code is
  ///   transpiled.
  ///
  pub fn transpile(
    &mut self,
    options: TranspileOptions,
  ) -> Result<ResultInfo, AnyError> {
    let start = Instant::now();

    let mut ts_config = TsConfig::new(json!({
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "inlineSourceMap": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
    }));

    let maybe_ignored_options =
      ts_config.merge_tsconfig(options.maybe_config_path)?;

    let config = ts_config.as_bytes();
    let emit_options: ast::EmitOptions = ts_config.into();
    let mut emit_count = 0_u32;
    for (_, module_slot) in self.modules.iter_mut() {
      if let ModuleSlot::Module(module) = module_slot {
        // TODO(kitsonk) a lot of this logic should be refactored into `Module` as
        // we start to support other methods on the graph.  Especially managing
        // the dirty state is something the module itself should "own".

        // if the module is a Dts file we should skip it
        if module.media_type == MediaType::Dts {
          continue;
        }
        // if we don't have check_js enabled, we won't touch non TypeScript or JSX
        // modules
        if !(emit_options.check_js
          || module.media_type == MediaType::JSX
          || module.media_type == MediaType::TSX
          || module.media_type == MediaType::TypeScript)
        {
          continue;
        }
        // skip modules that already have a valid emit
        if !options.reload && module.is_emit_valid(&config) {
          continue;
        }
        let parsed_module = module.parse()?;
        let emit = parsed_module.transpile(&emit_options)?;
        emit_count += 1;
        module.maybe_emit = Some(Emit::Cli(emit));
        module.set_version(&config);
        module.is_dirty = true;
      }
    }
    self.flush()?;

    let stats = Stats(vec![
      ("Files".to_string(), self.modules.len() as u32),
      ("Emitted".to_string(), emit_count),
      ("Total time".to_string(), start.elapsed().as_millis() as u32),
    ]);

    Ok(ResultInfo {
      diagnostics: Default::default(),
      loadable_modules: self.get_loadable_modules(),
      maybe_ignored_options,
      stats,
    })
  }
}

impl swc_bundler::Resolve for Graph {
  fn resolve(
    &self,
    referrer: &swc_common::FileName,
    specifier: &str,
  ) -> Result<swc_common::FileName, AnyError> {
    let referrer = if let swc_common::FileName::Custom(referrer) = referrer {
      ModuleSpecifier::resolve_url_or_path(referrer)
        .context("Cannot resolve swc FileName to a module specifier")?
    } else {
      unreachable!(
        "An unexpected referrer was passed when bundling: {:?}",
        referrer
      )
    };
    let specifier = self.resolve(specifier, &referrer, false)?;

    Ok(swc_common::FileName::Custom(specifier.to_string()))
  }
}

/// A structure for building a dependency graph of modules.
pub struct GraphBuilder {
  graph: Graph,
  maybe_import_map: Option<Arc<Mutex<ImportMap>>>,
  pending: FuturesUnordered<FetchFuture>,
}

impl GraphBuilder {
  pub fn new(
    handler: Arc<Mutex<dyn SpecifierHandler>>,
    maybe_import_map: Option<ImportMap>,
    maybe_lockfile: Option<Arc<Mutex<Lockfile>>>,
  ) -> Self {
    let internal_import_map = if let Some(import_map) = maybe_import_map {
      Some(Arc::new(Mutex::new(import_map)))
    } else {
      None
    };
    GraphBuilder {
      graph: Graph::new(handler, maybe_lockfile),
      maybe_import_map: internal_import_map,
      pending: FuturesUnordered::new(),
    }
  }

  /// Add a module into the graph based on a module specifier.  The module
  /// and any dependencies will be fetched from the handler.  The module will
  /// also be treated as a _root_ module in the graph.
  pub async fn add(
    &mut self,
    specifier: &ModuleSpecifier,
    is_dynamic: bool,
  ) -> Result<(), AnyError> {
    self.fetch(specifier, &None, is_dynamic);

    loop {
      match self.pending.next().await {
        Some(Err((specifier, err))) => {
          self
            .graph
            .modules
            .insert(specifier, ModuleSlot::Err(Arc::new(err)));
        }
        Some(Ok(cached_module)) => {
          let is_root = &cached_module.specifier == specifier;
          self.visit(cached_module, is_root)?;
        }
        _ => {}
      }
      if self.pending.is_empty() {
        break;
      }
    }

    if !self.graph.roots.contains(specifier) {
      self.graph.roots.push(specifier.clone());
      self.graph.roots_dynamic = self.graph.roots_dynamic && is_dynamic;
      if self.graph.maybe_tsbuildinfo.is_none() {
        let handler = self.graph.handler.lock().unwrap();
        self.graph.maybe_tsbuildinfo = handler.get_tsbuildinfo(specifier)?;
      }
    }

    Ok(())
  }

  /// Request a module to be fetched from the handler and queue up its future
  /// to be awaited to be resolved.
  fn fetch(
    &mut self,
    specifier: &ModuleSpecifier,
    maybe_referrer: &Option<Location>,
    is_dynamic: bool,
  ) {
    if !self.graph.modules.contains_key(&specifier) {
      self
        .graph
        .modules
        .insert(specifier.clone(), ModuleSlot::Pending);
      let mut handler = self.graph.handler.lock().unwrap();
      let future =
        handler.fetch(specifier.clone(), maybe_referrer.clone(), is_dynamic);
      self.pending.push(future);
    }
  }

  /// Visit a module that has been fetched, hydrating the module, analyzing its
  /// dependencies if required, fetching those dependencies, and inserting the
  /// module into the graph.
  fn visit(
    &mut self,
    cached_module: CachedModule,
    is_root: bool,
  ) -> Result<(), AnyError> {
    let specifier = cached_module.specifier.clone();
    let requested_specifier = cached_module.requested_specifier.clone();
    let mut module =
      Module::new(cached_module, is_root, self.maybe_import_map.clone());
    match module.media_type {
      MediaType::Json
      | MediaType::SourceMap
      | MediaType::TsBuildInfo
      | MediaType::Unknown => {
        return Err(
          GraphError::UnsupportedImportType(
            module.specifier,
            module.media_type,
          )
          .into(),
        );
      }
      _ => (),
    }
    if !module.is_parsed {
      let has_types = module.maybe_types.is_some();
      module.parse()?;
      if self.maybe_import_map.is_none() {
        let mut handler = self.graph.handler.lock().unwrap();
        handler.set_deps(&specifier, module.dependencies.clone())?;
        if !has_types {
          if let Some((types, _)) = module.maybe_types.clone() {
            handler.set_types(&specifier, types)?;
          }
        }
      }
    }
    for (_, dep) in module.dependencies.iter() {
      let maybe_referrer = Some(dep.location.clone());
      if let Some(specifier) = dep.maybe_code.as_ref() {
        self.fetch(specifier, &maybe_referrer, dep.is_dynamic);
      }
      if let Some(specifier) = dep.maybe_type.as_ref() {
        self.fetch(specifier, &maybe_referrer, dep.is_dynamic);
      }
    }
    if let Some((_, specifier)) = module.maybe_types.as_ref() {
      self.fetch(specifier, &None, false);
    }
    if specifier != requested_specifier {
      self
        .graph
        .redirects
        .insert(requested_specifier, specifier.clone());
    }
    self
      .graph
      .modules
      .insert(specifier, ModuleSlot::Module(Box::new(module)));

    Ok(())
  }

  /// Move out the graph from the builder to be utilized further.  An optional
  /// lockfile can be provided, where if the sources in the graph do not match
  /// the expected lockfile, an error will be logged and the process will exit.
  pub fn get_graph(self) -> Graph {
    self.graph.lock();
    self.graph
  }
}

pub async fn create_module_graph_and_maybe_check(
  module_specifier: ModuleSpecifier,
  program_state: Arc<ProgramState>,
  debug: bool,
) -> Result<Graph, AnyError> {
  let handler = Arc::new(Mutex::new(FetchHandler::new(
    &program_state,
    // when bundling, dynamic imports are only access for their type safety,
    // therefore we will allow the graph to access any module.
    Permissions::allow_all(),
  )?));
  let mut builder = GraphBuilder::new(
    handler,
    program_state.maybe_import_map.clone(),
    program_state.lockfile.clone(),
  );
  builder.add(&module_specifier, false).await?;
  let module_graph = builder.get_graph();

  if !program_state.flags.no_check {
    // TODO(@kitsonk) support bundling for workers
    let lib = if program_state.flags.unstable {
      TypeLib::UnstableDenoWindow
    } else {
      TypeLib::DenoWindow
    };
    let result_info = module_graph.clone().check(CheckOptions {
      debug,
      emit: false,
      lib,
      maybe_config_path: program_state.flags.config_path.clone(),
      reload: program_state.flags.reload,
    })?;

    debug!("{}", result_info.stats);
    if let Some(ignored_options) = result_info.maybe_ignored_options {
      eprintln!("{}", ignored_options);
    }
    if !result_info.diagnostics.is_empty() {
      return Err(generic_error(result_info.diagnostics.to_string()));
    }
  }

  Ok(module_graph)
}

#[cfg(test)]
pub mod tests {
  use super::*;

  use crate::specifier_handler::MemoryHandler;
  use deno_core::futures::future;
  use std::env;
  use std::fs;
  use std::path::PathBuf;
  use std::sync::Mutex;

  macro_rules! map (
    { $($key:expr => $value:expr),+ } => {
      {
        let mut m = ::std::collections::HashMap::new();
        $(
          m.insert($key, $value);
        )+
        m
      }
    };
  );

  /// This is a testing mock for `SpecifierHandler` that uses a special file
  /// system renaming to mock local and remote modules as well as provides
  /// "spies" for the critical methods for testing purposes.
  #[derive(Debug, Default)]
  pub struct MockSpecifierHandler {
    pub fixtures: PathBuf,
    pub maybe_tsbuildinfo: Option<String>,
    pub tsbuildinfo_calls: Vec<(ModuleSpecifier, String)>,
    pub cache_calls: Vec<(ModuleSpecifier, Emit)>,
    pub deps_calls: Vec<(ModuleSpecifier, DependencyMap)>,
    pub types_calls: Vec<(ModuleSpecifier, String)>,
    pub version_calls: Vec<(ModuleSpecifier, String)>,
  }

  impl MockSpecifierHandler {
    fn get_cache(
      &self,
      specifier: ModuleSpecifier,
    ) -> Result<CachedModule, (ModuleSpecifier, AnyError)> {
      let specifier_text = specifier
        .to_string()
        .replace(":///", "_")
        .replace("://", "_")
        .replace("/", "-");
      let source_path = self.fixtures.join(specifier_text);
      let media_type = MediaType::from(&source_path);
      let source = fs::read_to_string(&source_path)
        .map_err(|err| (specifier.clone(), err.into()))?;
      let is_remote = specifier.as_url().scheme() != "file";

      Ok(CachedModule {
        source,
        requested_specifier: specifier.clone(),
        source_path,
        specifier,
        media_type,
        is_remote,
        ..CachedModule::default()
      })
    }
  }

  impl SpecifierHandler for MockSpecifierHandler {
    fn fetch(
      &mut self,
      specifier: ModuleSpecifier,
      _maybe_referrer: Option<Location>,
      _is_dynamic: bool,
    ) -> FetchFuture {
      Box::pin(future::ready(self.get_cache(specifier)))
    }
    fn get_tsbuildinfo(
      &self,
      _specifier: &ModuleSpecifier,
    ) -> Result<Option<String>, AnyError> {
      Ok(self.maybe_tsbuildinfo.clone())
    }
    fn set_cache(
      &mut self,
      specifier: &ModuleSpecifier,
      emit: &Emit,
    ) -> Result<(), AnyError> {
      self.cache_calls.push((specifier.clone(), emit.clone()));
      Ok(())
    }
    fn set_types(
      &mut self,
      specifier: &ModuleSpecifier,
      types: String,
    ) -> Result<(), AnyError> {
      self.types_calls.push((specifier.clone(), types));
      Ok(())
    }
    fn set_tsbuildinfo(
      &mut self,
      specifier: &ModuleSpecifier,
      tsbuildinfo: String,
    ) -> Result<(), AnyError> {
      self.maybe_tsbuildinfo = Some(tsbuildinfo.clone());
      self
        .tsbuildinfo_calls
        .push((specifier.clone(), tsbuildinfo));
      Ok(())
    }
    fn set_deps(
      &mut self,
      specifier: &ModuleSpecifier,
      dependencies: DependencyMap,
    ) -> Result<(), AnyError> {
      self.deps_calls.push((specifier.clone(), dependencies));
      Ok(())
    }
    fn set_version(
      &mut self,
      specifier: &ModuleSpecifier,
      version: String,
    ) -> Result<(), AnyError> {
      self.version_calls.push((specifier.clone(), version));
      Ok(())
    }
  }

  async fn setup(
    specifier: ModuleSpecifier,
  ) -> (Graph, Arc<Mutex<MockSpecifierHandler>>) {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let handler = Arc::new(Mutex::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None, None);
    builder
      .add(&specifier, false)
      .await
      .expect("module not inserted");

    (builder.get_graph(), handler)
  }

  async fn setup_memory(
    specifier: ModuleSpecifier,
    sources: HashMap<&str, &str>,
  ) -> Graph {
    let sources: HashMap<String, String> = sources
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect();
    let handler = Arc::new(Mutex::new(MemoryHandler::new(sources)));
    let mut builder = GraphBuilder::new(handler.clone(), None, None);
    builder
      .add(&specifier, false)
      .await
      .expect("module not inserted");

    builder.get_graph()
  }

  #[test]
  fn test_get_version() {
    let doc_a = "console.log(42);";
    let version_a = get_version(&doc_a, "1.2.3", b"");
    let doc_b = "console.log(42);";
    let version_b = get_version(&doc_b, "1.2.3", b"");
    assert_eq!(version_a, version_b);

    let version_c = get_version(&doc_a, "1.2.3", b"options");
    assert_ne!(version_a, version_c);

    let version_d = get_version(&doc_b, "1.2.3", b"options");
    assert_eq!(version_c, version_d);

    let version_e = get_version(&doc_a, "1.2.4", b"");
    assert_ne!(version_a, version_e);

    let version_f = get_version(&doc_b, "1.2.4", b"");
    assert_eq!(version_e, version_f);
  }

  #[test]
  fn test_module_emit_valid() {
    let source = "console.log(42);".to_string();
    let maybe_version = Some(get_version(&source, &version::deno(), b""));
    let module = Module {
      source,
      maybe_version,
      ..Module::default()
    };
    assert!(module.is_emit_valid(b""));

    let source = "console.log(42);".to_string();
    let old_source = "console.log(43);";
    let maybe_version = Some(get_version(old_source, &version::deno(), b""));
    let module = Module {
      source,
      maybe_version,
      ..Module::default()
    };
    assert!(!module.is_emit_valid(b""));

    let source = "console.log(42);".to_string();
    let maybe_version = Some(get_version(&source, "0.0.0", b""));
    let module = Module {
      source,
      maybe_version,
      ..Module::default()
    };
    assert!(!module.is_emit_valid(b""));

    let source = "console.log(42);".to_string();
    let module = Module {
      source,
      ..Module::default()
    };
    assert!(!module.is_emit_valid(b""));
  }

  #[test]
  fn test_module_set_version() {
    let source = "console.log(42);".to_string();
    let expected = Some(get_version(&source, &version::deno(), b""));
    let mut module = Module {
      source,
      ..Module::default()
    };
    assert!(module.maybe_version.is_none());
    module.set_version(b"");
    assert_eq!(module.maybe_version, expected);
  }

  #[tokio::test]
  async fn test_graph_bundle() {
    let tests = vec![
      ("file:///tests/fixture01.ts", "fixture01.out"),
      ("file:///tests/fixture02.ts", "fixture02.out"),
      ("file:///tests/fixture03.ts", "fixture03.out"),
      ("file:///tests/fixture04.ts", "fixture04.out"),
      ("file:///tests/fixture05.ts", "fixture05.out"),
      ("file:///tests/fixture06.ts", "fixture06.out"),
      ("file:///tests/fixture07.ts", "fixture07.out"),
      ("file:///tests/fixture08.ts", "fixture08.out"),
      ("file:///tests/fixture09.ts", "fixture09.out"),
      ("file:///tests/fixture10.ts", "fixture10.out"),
      ("file:///tests/fixture11.ts", "fixture11.out"),
      ("file:///tests/fixture12.ts", "fixture12.out"),
      ("file:///tests/fixture13.ts", "fixture13.out"),
      ("file:///tests/fixture14.ts", "fixture14.out"),
      ("file:///tests/fixture15.ts", "fixture15.out"),
    ];
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/bundle");

    for (specifier, expected_str) in tests {
      let specifier = ModuleSpecifier::resolve_url_or_path(specifier).unwrap();
      let handler = Arc::new(Mutex::new(MockSpecifierHandler {
        fixtures: fixtures.clone(),
        ..MockSpecifierHandler::default()
      }));
      let mut builder = GraphBuilder::new(handler.clone(), None, None);
      builder
        .add(&specifier, false)
        .await
        .expect("module not inserted");
      let graph = builder.get_graph();
      let (actual, stats, maybe_ignored_options) = graph
        .bundle(BundleOptions::default())
        .expect("could not bundle");
      assert_eq!(stats.0.len(), 2);
      assert_eq!(maybe_ignored_options, None);
      let expected_path = fixtures.join(expected_str);
      let expected = fs::read_to_string(expected_path).unwrap();
      assert_eq!(actual, expected, "fixture: {}", specifier);
    }
  }

  #[tokio::test]
  async fn test_graph_check_emit() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts")
        .expect("could not resolve module");
    let (graph, handler) = setup(specifier).await;
    let result_info = graph
      .check(CheckOptions {
        debug: false,
        emit: true,
        lib: TypeLib::DenoWindow,
        maybe_config_path: None,
        reload: false,
      })
      .expect("should have checked");
    assert!(result_info.maybe_ignored_options.is_none());
    assert_eq!(result_info.stats.0.len(), 12);
    assert!(result_info.diagnostics.is_empty());
    let h = handler.lock().unwrap();
    assert_eq!(h.cache_calls.len(), 2);
    assert_eq!(h.tsbuildinfo_calls.len(), 1);
  }

  #[tokio::test]
  async fn test_graph_check_ignores_dynamic_import_errors() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/dynamicimport.ts")
        .expect("could not resolve module");
    let (graph, _) = setup(specifier).await;
    let result_info = graph
      .check(CheckOptions {
        debug: false,
        emit: false,
        lib: TypeLib::DenoWindow,
        maybe_config_path: None,
        reload: false,
      })
      .expect("should have checked");
    assert!(result_info.diagnostics.is_empty());
  }

  #[tokio::test]
  async fn fix_graph_check_emit_diagnostics() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/diag.ts")
        .expect("could not resolve module");
    let (graph, handler) = setup(specifier).await;
    let result_info = graph
      .check(CheckOptions {
        debug: false,
        emit: true,
        lib: TypeLib::DenoWindow,
        maybe_config_path: None,
        reload: false,
      })
      .expect("should have checked");
    assert!(result_info.maybe_ignored_options.is_none());
    assert_eq!(result_info.stats.0.len(), 12);
    assert!(!result_info.diagnostics.is_empty());
    let h = handler.lock().unwrap();
    // we shouldn't cache any files or write out tsbuildinfo if there are
    // diagnostic errors
    assert_eq!(h.cache_calls.len(), 0);
    assert_eq!(h.tsbuildinfo_calls.len(), 0);
  }

  #[tokio::test]
  async fn test_graph_check_no_emit() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts")
        .expect("could not resolve module");
    let (graph, handler) = setup(specifier).await;
    let result_info = graph
      .check(CheckOptions {
        debug: false,
        emit: false,
        lib: TypeLib::DenoWindow,
        maybe_config_path: None,
        reload: false,
      })
      .expect("should have checked");
    assert!(result_info.maybe_ignored_options.is_none());
    assert_eq!(result_info.stats.0.len(), 12);
    assert!(result_info.diagnostics.is_empty());
    let h = handler.lock().unwrap();
    assert_eq!(h.cache_calls.len(), 0);
    assert_eq!(h.tsbuildinfo_calls.len(), 1);
  }

  #[tokio::test]
  async fn fix_graph_check_mjs_root() {
    let specifier = ModuleSpecifier::resolve_url_or_path("file:///tests/a.mjs")
      .expect("could not resolve module");
    let (graph, handler) = setup(specifier).await;
    let result_info = graph
      .check(CheckOptions {
        debug: false,
        emit: true,
        lib: TypeLib::DenoWindow,
        maybe_config_path: None,
        reload: false,
      })
      .expect("should have checked");
    assert!(result_info.maybe_ignored_options.is_none());
    assert!(result_info.diagnostics.is_empty());
    let h = handler.lock().unwrap();
    assert_eq!(h.cache_calls.len(), 1);
    assert_eq!(h.tsbuildinfo_calls.len(), 1);
  }

  #[tokio::test]
  async fn fix_graph_check_types_root() {
    let specifier = ModuleSpecifier::resolve_url_or_path("file:///typesref.js")
      .expect("could not resolve module");
    let (graph, _) = setup(specifier).await;
    let result_info = graph
      .check(CheckOptions {
        debug: false,
        emit: false,
        lib: TypeLib::DenoWindow,
        maybe_config_path: None,
        reload: false,
      })
      .expect("should have checked");
    assert!(result_info.diagnostics.is_empty());
  }

  #[tokio::test]
  async fn test_graph_check_user_config() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/checkwithconfig.ts")
        .expect("could not resolve module");
    let (graph, handler) = setup(specifier.clone()).await;
    let result_info = graph
      .check(CheckOptions {
        debug: false,
        emit: true,
        lib: TypeLib::DenoWindow,
        maybe_config_path: Some(
          "tests/module_graph/tsconfig_01.json".to_string(),
        ),
        reload: true,
      })
      .expect("should have checked");
    assert!(result_info.maybe_ignored_options.is_none());
    assert!(result_info.diagnostics.is_empty());
    let (ver0, ver1) = {
      let h = handler.lock().unwrap();
      assert_eq!(h.version_calls.len(), 2);
      (h.version_calls[0].1.clone(), h.version_calls[1].1.clone())
    };

    // let's do it all over again to ensure that the versions are determinstic
    let (graph, handler) = setup(specifier).await;
    let result_info = graph
      .check(CheckOptions {
        debug: false,
        emit: true,
        lib: TypeLib::DenoWindow,
        maybe_config_path: Some(
          "tests/module_graph/tsconfig_01.json".to_string(),
        ),
        reload: true,
      })
      .expect("should have checked");
    assert!(result_info.maybe_ignored_options.is_none());
    assert!(result_info.diagnostics.is_empty());
    let h = handler.lock().unwrap();
    assert_eq!(h.version_calls.len(), 2);
    assert!(h.version_calls[0].1 == ver0 || h.version_calls[0].1 == ver1);
    assert!(h.version_calls[1].1 == ver0 || h.version_calls[1].1 == ver1);
  }

  #[tokio::test]
  async fn test_graph_emit() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///a.ts").unwrap();
    let graph = setup_memory(
      specifier,
      map!(
        "/a.ts" => r#"
        import * as b from "./b.ts";

        console.log(b);
      "#,
        "/b.ts" => r#"
        export const b = "b";
      "#
      ),
    )
    .await;
    let (emitted_files, result_info) = graph
      .emit(EmitOptions {
        check: true,
        bundle_type: BundleType::None,
        debug: false,
        maybe_user_config: None,
      })
      .expect("should have emitted");
    assert!(result_info.diagnostics.is_empty());
    assert!(result_info.maybe_ignored_options.is_none());
    assert_eq!(emitted_files.len(), 4);
    let out_a = emitted_files.get("file:///a.ts.js");
    assert!(out_a.is_some());
    let out_a = out_a.unwrap();
    assert!(out_a.starts_with("import * as b from"));
    assert!(emitted_files.contains_key("file:///a.ts.js.map"));
    let out_b = emitted_files.get("file:///b.ts.js");
    assert!(out_b.is_some());
    let out_b = out_b.unwrap();
    assert!(out_b.starts_with("export const b = \"b\";"));
    assert!(emitted_files.contains_key("file:///b.ts.js.map"));
  }

  #[tokio::test]
  async fn test_graph_emit_bundle() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///a.ts").unwrap();
    let graph = setup_memory(
      specifier,
      map!(
        "/a.ts" => r#"
        import * as b from "./b.ts";

        console.log(b);
      "#,
        "/b.ts" => r#"
        export const b = "b";
      "#
      ),
    )
    .await;
    let (emitted_files, result_info) = graph
      .emit(EmitOptions {
        check: true,
        bundle_type: BundleType::Esm,
        debug: false,
        maybe_user_config: None,
      })
      .expect("should have emitted");
    assert!(result_info.diagnostics.is_empty());
    assert!(result_info.maybe_ignored_options.is_none());
    assert_eq!(emitted_files.len(), 1);
    let actual = emitted_files.get("deno:///bundle.js");
    assert!(actual.is_some());
    let actual = actual.unwrap();
    assert!(actual.contains("const b = \"b\";"));
    assert!(actual.contains("console.log(mod);"));
  }

  #[tokio::test]
  async fn fix_graph_emit_declaration() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///a.ts").unwrap();
    let graph = setup_memory(
      specifier,
      map!(
        "/a.ts" => r#"
        import * as b from "./b.ts";

        console.log(b);
      "#,
        "/b.ts" => r#"
        export const b = "b";
      "#
      ),
    )
    .await;
    let mut user_config = HashMap::<String, Value>::new();
    user_config.insert("declaration".to_string(), json!(true));
    let (emitted_files, result_info) = graph
      .emit(EmitOptions {
        check: true,
        bundle_type: BundleType::None,
        debug: false,
        maybe_user_config: Some(user_config),
      })
      .expect("should have emitted");
    assert!(result_info.diagnostics.is_empty());
    assert!(result_info.maybe_ignored_options.is_none());
    assert_eq!(emitted_files.len(), 6);
    let out_a = emitted_files.get("file:///a.ts.js");
    assert!(out_a.is_some());
    let out_a = out_a.unwrap();
    assert!(out_a.starts_with("import * as b from"));
    assert!(emitted_files.contains_key("file:///a.ts.js.map"));
    assert!(emitted_files.contains_key("file:///a.ts.d.ts"));
    let out_b = emitted_files.get("file:///b.ts.js");
    assert!(out_b.is_some());
    let out_b = out_b.unwrap();
    assert!(out_b.starts_with("export const b = \"b\";"));
    assert!(emitted_files.contains_key("file:///b.ts.js.map"));
    assert!(emitted_files.contains_key("file:///b.ts.d.ts"));
  }

  #[tokio::test]
  async fn test_graph_info() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts")
        .expect("could not resolve module");
    let (graph, _) = setup(specifier).await;
    let info = graph.info().expect("could not get info");
    assert!(info.compiled.is_none());
    assert_eq!(info.dep_count, 6);
    assert_eq!(info.file_type, MediaType::TypeScript);
    assert_eq!(info.files.0.len(), 7);
    assert!(info.local.to_string_lossy().ends_with("file_tests-main.ts"));
    assert!(info.map.is_none());
    assert_eq!(
      info.module,
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts").unwrap()
    );
    assert_eq!(info.total_size, 344);
  }

  #[tokio::test]
  async fn test_graph_import_json() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/importjson.ts")
        .expect("could not resolve module");
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let handler = Arc::new(Mutex::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None, None);
    builder
      .add(&specifier, false)
      .await
      .expect_err("should have errored");
  }

  #[tokio::test]
  async fn test_graph_transpile() {
    // This is a complex scenario of transpiling, where we have TypeScript
    // importing a JavaScript file (with type definitions) which imports
    // TypeScript, JavaScript, and JavaScript with type definitions.
    // For scenarios where we transpile, we only want the TypeScript files
    // to be actually emitted.
    //
    // This also exercises "@deno-types" and type references.
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts")
        .expect("could not resolve module");
    let (mut graph, handler) = setup(specifier).await;
    let result_info = graph.transpile(TranspileOptions::default()).unwrap();
    assert_eq!(result_info.stats.0.len(), 3);
    assert_eq!(result_info.maybe_ignored_options, None);
    let h = handler.lock().unwrap();
    assert_eq!(h.cache_calls.len(), 2);
    match &h.cache_calls[0].1 {
      Emit::Cli((code, maybe_map)) => {
        assert!(
          code.contains("# sourceMappingURL=data:application/json;base64,")
        );
        assert!(maybe_map.is_none());
      }
    };
    match &h.cache_calls[1].1 {
      Emit::Cli((code, maybe_map)) => {
        assert!(
          code.contains("# sourceMappingURL=data:application/json;base64,")
        );
        assert!(maybe_map.is_none());
      }
    };
    assert_eq!(h.deps_calls.len(), 7);
    assert_eq!(
      h.deps_calls[0].0,
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts").unwrap()
    );
    assert_eq!(h.deps_calls[0].1.len(), 1);
    assert_eq!(
      h.deps_calls[1].0,
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/lib/mod.js")
        .unwrap()
    );
    assert_eq!(h.deps_calls[1].1.len(), 3);
    assert_eq!(
      h.deps_calls[2].0,
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/lib/mod.d.ts")
        .unwrap()
    );
    assert_eq!(h.deps_calls[2].1.len(), 3, "should have 3 dependencies");
    // sometimes the calls are not deterministic, and so checking the contents
    // can cause some failures
    assert_eq!(h.deps_calls[3].1.len(), 0, "should have no dependencies");
    assert_eq!(h.deps_calls[4].1.len(), 0, "should have no dependencies");
    assert_eq!(h.deps_calls[5].1.len(), 0, "should have no dependencies");
    assert_eq!(h.deps_calls[6].1.len(), 0, "should have no dependencies");
  }

  #[tokio::test]
  async fn test_graph_transpile_user_config() {
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/transpile.tsx")
        .expect("could not resolve module");
    let (mut graph, handler) = setup(specifier).await;
    let result_info = graph
      .transpile(TranspileOptions {
        debug: false,
        maybe_config_path: Some("tests/module_graph/tsconfig.json".to_string()),
        reload: false,
      })
      .unwrap();
    assert_eq!(
      result_info.maybe_ignored_options.unwrap().items,
      vec!["target".to_string()],
      "the 'target' options should have been ignored"
    );
    let h = handler.lock().unwrap();
    assert_eq!(h.cache_calls.len(), 1, "only one file should be emitted");
    // FIXME(bartlomieju): had to add space in `<div>`, probably a quirk in swc_ecma_codegen
    match &h.cache_calls[0].1 {
      Emit::Cli((code, _)) => {
        assert!(
          code.contains("<div >Hello world!</div>"),
          "jsx should have been preserved"
        );
      }
    }
  }

  #[tokio::test]
  async fn test_graph_import_map_remote_to_local() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let maybe_import_map = Some(
      ImportMap::from_json(
        "file:///tests/importmap.json",
        r#"{
      "imports": {
        "https://deno.land/x/b/mod.js": "./b/mod.js"
      }
    }
    "#,
      )
      .expect("could not parse import map"),
    );
    let handler = Arc::new(Mutex::new(MockSpecifierHandler {
      fixtures,
      ..Default::default()
    }));
    let mut builder = GraphBuilder::new(handler, maybe_import_map, None);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/importremap.ts")
        .expect("could not resolve module");
    builder.add(&specifier, false).await.expect("could not add");
    builder.get_graph();
  }

  #[tokio::test]
  async fn test_graph_with_lockfile() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let lockfile_path = fixtures.join("lockfile.json");
    let lockfile =
      Lockfile::new(lockfile_path, false).expect("could not load lockfile");
    let maybe_lockfile = Some(Arc::new(Mutex::new(lockfile)));
    let handler = Arc::new(Mutex::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None, maybe_lockfile);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts")
        .expect("could not resolve module");
    builder
      .add(&specifier, false)
      .await
      .expect("module not inserted");
    builder.get_graph();
  }
}
