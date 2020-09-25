// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::ast::parse;
use crate::ast::Location;
use crate::ast::ParsedModule;
use crate::file_fetcher::TextDocument;
use crate::import_map::ImportMap;
use crate::lockfile::Lockfile;
use crate::media_type::MediaType;
use crate::specifier_handler::CachedModule;
use crate::specifier_handler::DependencyMap;
use crate::specifier_handler::EmitMap;
use crate::specifier_handler::EmitType;
use crate::specifier_handler::FetchFuture;
use crate::specifier_handler::SpecifierHandler;
use crate::tsc_config::json_merge;
use crate::tsc_config::parse_config;
use crate::tsc_config::IgnoredCompilerOptions;
use crate::AnyError;

use deno_core::futures::stream::FuturesUnordered;
use deno_core::futures::stream::StreamExt;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use regex::Regex;
use serde::Deserialize;
use serde::Deserializer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::rc::Rc;
use std::result;
use std::sync::Mutex;
use std::time::Instant;
use swc_ecmascript::dep_graph::DependencyKind;

type Result<V> = result::Result<V, AnyError>;

pub type BuildInfoMap = HashMap<EmitType, TextDocument>;

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
#[allow(unused)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum GraphError {
  /// A module using the HTTPS protocol is trying to import a module with an
  /// HTTP schema.
  InvalidDowngrade(ModuleSpecifier, Location),
  /// A remote module is trying to import a local module.
  InvalidLocalImport(ModuleSpecifier, Location),
  /// A remote module is trying to import a local module.
  InvalidSource(ModuleSpecifier, String),
  /// A module specifier could not be resolved for a given import.
  InvalidSpecifier(String, Location),
  /// An unexpected dependency was requested for a module.
  MissingDependency(ModuleSpecifier, String),
  /// An unexpected specifier was requested.
  MissingSpecifier(ModuleSpecifier),
  /// Snapshot data was not present in a situation where it was required.
  MissingSnapshotData,
  /// The current feature is not supported.
  NotSupported(String),
}
use GraphError::*;

impl fmt::Display for GraphError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      InvalidDowngrade(ref specifier, ref location) => write!(f, "Modules imported via https are not allowed to import http modules.\n  Importing: {}\n    at {}:{}:{}", specifier, location.filename, location.line, location.col),
      InvalidLocalImport(ref specifier, ref location) => write!(f, "Remote modules are not allowed to import local modules.\n  Importing: {}\n    at {}:{}:{}", specifier, location.filename, location.line, location.col),
      InvalidSource(ref specifier, ref lockfile) => write!(f, "The source code is invalid, as it does not match the expected hash in the lock file.\n  Specifier: {}\n  Lock file: {}", specifier, lockfile),
      InvalidSpecifier(ref specifier, ref location) => write!(f, "Unable to resolve dependency specifier.\n  Specifier: {}\n    at {}:{}:{}", specifier, location.filename, location.line, location.col),
      MissingDependency(ref referrer, specifier) => write!(
        f,
        "The graph is missing a dependency.\n  Specifier: {} from {}",
        specifier, referrer
      ),
      MissingSpecifier(ref specifier) => write!(
        f,
        "The graph is missing a specifier.\n  Specifier: {}",
        specifier
      ),
      MissingSnapshotData => write!(f, "Snapshot data was not supplied, but required."),
      NotSupported(ref msg) => write!(f, "{}", msg),
    }
  }
}

impl Error for GraphError {}

/// A trait, implemented by `Graph` that provides the interfaces that the
/// compiler ops require to be able to retrieve information about the graph.
pub trait ModuleProvider {
  /// Get the source for a given module specifier.  If the module is not part
  /// of the graph, the result will be `None`.
  fn get_source(&self, specifier: &ModuleSpecifier) -> Option<String>;
  /// Given a string specifier and a referring module specifier, provide the
  /// resulting module specifier and media type for the module that is part of
  /// the graph.
  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<(ModuleSpecifier, MediaType)>;
}

/// An enum which represents the parsed out values of references in source code.
#[derive(Debug, Clone, Eq, PartialEq)]
enum TypeScriptReference {
  Path(String),
  Types(String),
}

/// Determine if a comment contains a triple slash reference and optionally
/// return its kind and value.
fn parse_ts_reference(comment: &str) -> Option<TypeScriptReference> {
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
fn parse_deno_types(comment: &str) -> Option<String> {
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

/// A logical representation of a module within a graph.
#[derive(Debug, Clone)]
struct Module {
  dependencies: DependencyMap,
  emits: EmitMap,
  is_dirty: bool,
  is_hydrated: bool,
  is_parsed: bool,
  maybe_import_map: Option<Rc<RefCell<ImportMap>>>,
  maybe_parsed_module: Option<ParsedModule>,
  maybe_types: Option<(String, ModuleSpecifier)>,
  media_type: MediaType,
  specifier: ModuleSpecifier,
  source: TextDocument,
}

impl Default for Module {
  fn default() -> Self {
    Module {
      dependencies: HashMap::new(),
      emits: HashMap::new(),
      is_dirty: false,
      is_hydrated: false,
      is_parsed: false,
      maybe_import_map: None,
      maybe_parsed_module: None,
      maybe_types: None,
      media_type: MediaType::Unknown,
      specifier: ModuleSpecifier::resolve_url("https://deno.land/x/").unwrap(),
      source: TextDocument::new(Vec::new(), Option::<&str>::None),
    }
  }
}

impl Module {
  pub fn new(
    specifier: ModuleSpecifier,
    maybe_import_map: Option<Rc<RefCell<ImportMap>>>,
  ) -> Self {
    Module {
      specifier,
      maybe_import_map,
      ..Module::default()
    }
  }

  pub fn hydrate(&mut self, cached_module: CachedModule) {
    self.media_type = cached_module.media_type;
    self.source = cached_module.source;
    if self.maybe_import_map.is_none() {
      if let Some(dependencies) = cached_module.maybe_dependencies {
        self.dependencies = dependencies;
        self.is_parsed = true;
      }
    }
    self.maybe_types = if let Some(ref specifier) = cached_module.maybe_types {
      Some((
        specifier.clone(),
        self
          .resolve_import(&specifier, None)
          .expect("could not resolve module"),
      ))
    } else {
      None
    };
    self.is_dirty = false;
    self.emits = cached_module.emits;
    self.is_hydrated = true;
  }

  pub fn parse(&mut self) -> Result<()> {
    let parsed_module =
      parse(&self.specifier, &self.source.to_str()?, &self.media_type)?;

    // parse out any triple slash references
    for comment in parsed_module.get_leading_comments().iter() {
      if let Some(ts_reference) = parse_ts_reference(&comment.text) {
        let location: Location = parsed_module.get_location(&comment.span);
        match ts_reference {
          TypeScriptReference::Path(import) => {
            let specifier = self.resolve_import(&import, Some(location))?;
            let dep = self.dependencies.entry(import).or_default();
            dep.maybe_code = Some(specifier);
          }
          TypeScriptReference::Types(import) => {
            let specifier = self.resolve_import(&import, Some(location))?;
            if self.media_type == MediaType::JavaScript
              || self.media_type == MediaType::JSX
            {
              // TODO(kitsonk) we need to specifically update the cache when
              // this value changes
              self.maybe_types = Some((import.clone(), specifier));
            } else {
              let dep = self.dependencies.entry(import).or_default();
              dep.maybe_type = Some(specifier);
            }
          }
        }
      }
    }

    // Parse out all the syntactical dependencies for a module
    let dependencies = parsed_module.analyze_dependencies();
    for desc in dependencies.iter() {
      let location = Location {
        filename: self.specifier.to_string(),
        col: desc.col,
        line: desc.line,
      };
      let specifier =
        self.resolve_import(&desc.specifier, Some(location.clone()))?;

      // Parse out any `@deno-types` pragmas and modify dependency
      let maybe_types_specifier = if !desc.leading_comments.is_empty() {
        let comment = desc.leading_comments.last().unwrap();
        if let Some(deno_types) = parse_deno_types(&comment.text).as_ref() {
          Some(self.resolve_import(deno_types, Some(location))?)
        } else {
          None
        }
      } else {
        None
      };

      let dep = self
        .dependencies
        .entry(desc.specifier.to_string())
        .or_default();
      if desc.kind == DependencyKind::ExportType
        || desc.kind == DependencyKind::ImportType
      {
        dep.maybe_type = Some(specifier);
      } else {
        dep.maybe_code = Some(specifier);
      }
      if let Some(types_specifier) = maybe_types_specifier {
        dep.maybe_type = Some(types_specifier);
      }
    }

    self.maybe_parsed_module = Some(parsed_module);
    Ok(())
  }

  fn resolve_import(
    &self,
    specifier: &str,
    maybe_location: Option<Location>,
  ) -> Result<ModuleSpecifier> {
    let maybe_resolve = if let Some(import_map) = self.maybe_import_map.clone()
    {
      import_map
        .borrow()
        .resolve(specifier, self.specifier.as_str())?
    } else {
      None
    };
    let specifier = if let Some(module_specifier) = maybe_resolve {
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
      return Err(InvalidDowngrade(specifier.clone(), location).into());
    }

    // Disallow a remote URL from trying to import a local URL
    if (referrer_scheme == "https" || referrer_scheme == "http")
      && !(specifier_scheme == "https" || specifier_scheme == "http")
    {
      return Err(InvalidLocalImport(specifier.clone(), location).into());
    }

    Ok(specifier)
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stats(Vec<(String, u128)>);

impl<'de> Deserialize<'de> for Stats {
  fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let items: Vec<(String, u128)> = Deserialize::deserialize(deserializer)?;
    Ok(Stats(items))
  }
}

impl fmt::Display for Stats {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    for (key, value) in self.0.clone() {
      write!(f, "{}: {}", key, value)?;
    }

    Ok(())
  }
}

/// A structure which provides options when transpiling modules.
#[derive(Debug, Default)]
pub struct TranspileOptions {
  /// If `true` then debug logging will be output from the isolate.
  pub debug: bool,
  /// A string of configuration data that augments the the default configuration
  /// passed to the TypeScript compiler.  This is typically the contents of a
  /// user supplied `tsconfig.json`.
  pub maybe_config: Option<String>,
}

/// The transpile options that are significant out of a user provided tsconfig
/// file, that we want to deserialize out of the final config for a transpile.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranspileConfigOptions {
  pub check_js: bool,
  pub emit_decorator_metadata: bool,
  pub jsx: String,
  pub jsx_factory: String,
  pub jsx_fragment_factory: String,
}

/// A dependency graph of modules, were the modules that have been inserted via
/// the builder will be loaded into the graph.  Also provides an interface to
/// be able to manipulate and handle the graph.

#[derive(Debug)]
pub struct Graph {
  build_info: BuildInfoMap,
  handler: Rc<RefCell<dyn SpecifierHandler>>,
  modules: HashMap<ModuleSpecifier, Module>,
  roots: Vec<ModuleSpecifier>,
}

impl Graph {
  /// Create a new instance of a graph, ready to have modules loaded it.
  ///
  /// The argument `handler` is an instance of a structure that implements the
  /// `SpecifierHandler` trait.
  ///
  pub fn new(handler: Rc<RefCell<dyn SpecifierHandler>>) -> Self {
    Graph {
      build_info: HashMap::new(),
      handler,
      modules: HashMap::new(),
      roots: Vec::new(),
    }
  }

  /// Update the handler with any modules that are marked as _dirty_ and update
  /// any build info if present.
  fn flush(&mut self, emit_type: &EmitType) -> Result<()> {
    let mut handler = self.handler.borrow_mut();
    for (_, module) in self.modules.iter_mut() {
      if module.is_dirty {
        let (code, maybe_map) = module.emits.get(emit_type).unwrap();
        handler.set_cache(
          &module.specifier,
          &emit_type,
          code.clone(),
          maybe_map.clone(),
        )?;
        module.is_dirty = false;
      }
    }
    for root_specifier in self.roots.iter() {
      if let Some(build_info) = self.build_info.get(&emit_type) {
        handler.set_build_info(
          root_specifier,
          &emit_type,
          build_info.to_owned(),
        )?;
      }
    }

    Ok(())
  }

  /// Verify the subresource integrity of the graph based upon the optional
  /// lockfile, updating the lockfile with any missing resources.  This will
  /// error if any of the resources do not match their lock status.
  pub fn lock(&self, maybe_lockfile: &Option<Mutex<Lockfile>>) -> Result<()> {
    if let Some(lf) = maybe_lockfile {
      let mut lockfile = lf.lock().unwrap();
      for (ms, module) in self.modules.iter() {
        let specifier = module.specifier.to_string();
        let code = module.source.to_string()?;
        let valid = lockfile.check_or_insert(&specifier, &code);
        if !valid {
          return Err(
            InvalidSource(ms.clone(), lockfile.filename.clone()).into(),
          );
        }
      }
    }

    Ok(())
  }

  /// Transpile (only transform) the graph, updating any emitted modules
  /// with the specifier handler.  The result contains any performance stats
  /// from the compiler and optionally any user provided configuration compiler
  /// options that were ignored.
  ///
  /// # Arguments
  ///
  /// - `options` - A structure of options which impact how the code is
  ///   transpiled.
  ///
  pub fn transpile(
    &mut self,
    options: TranspileOptions,
  ) -> Result<(Stats, Option<IgnoredCompilerOptions>)> {
    let start = Instant::now();
    let emit_type = EmitType::Cli;
    let mut compiler_options = json!({
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
    });

    let maybe_ignored_options = if let Some(config_text) = options.maybe_config
    {
      let (user_config, ignored_options) = parse_config(&config_text)?;
      json_merge(&mut compiler_options, &user_config);
      ignored_options
    } else {
      None
    };

    let compiler_options: TranspileConfigOptions =
      serde_json::from_value(compiler_options)?;
    let check_js = compiler_options.check_js;
    let transform_jsx = compiler_options.jsx == "react";
    let emit_options = ast::TranspileOptions {
      emit_metadata: compiler_options.emit_decorator_metadata,
      inline_source_map: true,
      jsx_factory: compiler_options.jsx_factory,
      jsx_fragment_factory: compiler_options.jsx_fragment_factory,
      transform_jsx,
    };

    let mut emit_count: u128 = 0;
    for (_, module) in self.modules.iter_mut() {
      // if the module is a Dts file we should skip it
      if module.media_type == MediaType::Dts {
        continue;
      }
      // skip modules that already have a valid emit
      if module.emits.contains_key(&emit_type) {
        continue;
      }
      // if we don't have check_js enabled, we won't touch non TypeScript
      // modules
      if !(check_js
        || module.media_type == MediaType::TSX
        || module.media_type == MediaType::TypeScript)
      {
        continue;
      }
      if module.maybe_parsed_module.is_none() {
        module.parse()?;
      }
      let parsed_module = module.maybe_parsed_module.clone().unwrap();
      let emit = parsed_module.transpile(&emit_options)?;
      emit_count += 1;
      module.emits.insert(emit_type.clone(), emit);
      module.is_dirty = true;
    }
    self.flush(&emit_type)?;

    let stats = Stats(vec![
      ("Files".to_string(), self.modules.len() as u128),
      ("Emitted".to_string(), emit_count),
      ("Total time".to_string(), start.elapsed().as_millis()),
    ]);

    Ok((stats, maybe_ignored_options))
  }
}

impl<'a> ModuleProvider for Graph {
  fn get_source(&self, specifier: &ModuleSpecifier) -> Option<String> {
    if let Some(module) = self.modules.get(specifier) {
      if let Ok(source) = module.source.to_string() {
        Some(source)
      } else {
        None
      }
    } else {
      None
    }
  }

  fn resolve(
    &self,
    specifier: &str,
    referrer: &ModuleSpecifier,
  ) -> Result<(ModuleSpecifier, MediaType)> {
    if !self.modules.contains_key(referrer) {
      return Err(MissingSpecifier(referrer.to_owned()).into());
    }
    let module = self.modules.get(referrer).unwrap();
    if !module.dependencies.contains_key(specifier) {
      return Err(
        MissingDependency(referrer.to_owned(), specifier.to_owned()).into(),
      );
    }
    let dependency = module.dependencies.get(specifier).unwrap();
    // If there is a @deno-types pragma that impacts the dependency, then the
    // maybe_type property will be set with that specifier, otherwise we use the
    // specifier that point to the runtime code.
    let resolved_specifier =
      if let Some(type_specifier) = dependency.maybe_type.clone() {
        type_specifier
      } else if let Some(code_specifier) = dependency.maybe_code.clone() {
        code_specifier
      } else {
        return Err(
          MissingDependency(referrer.to_owned(), specifier.to_owned()).into(),
        );
      };
    if !self.modules.contains_key(&resolved_specifier) {
      return Err(
        MissingDependency(referrer.to_owned(), resolved_specifier.to_string())
          .into(),
      );
    }
    let dep_module = self.modules.get(&resolved_specifier).unwrap();
    // In the case that there is a X-TypeScript-Types or a triple-slash types,
    // then the `maybe_types` specifier will be populated and we should use that
    // instead.
    let result = if let Some((_, types)) = dep_module.maybe_types.clone() {
      if let Some(types_module) = self.modules.get(&types) {
        (types, types_module.media_type)
      } else {
        return Err(
          MissingDependency(referrer.to_owned(), types.to_string()).into(),
        );
      }
    } else {
      (resolved_specifier, dep_module.media_type)
    };

    Ok(result)
  }
}

/// A structure for building a dependency graph of modules.
pub struct GraphBuilder {
  fetched: HashSet<ModuleSpecifier>,
  graph: Graph,
  maybe_import_map: Option<Rc<RefCell<ImportMap>>>,
  pending: FuturesUnordered<FetchFuture>,
}

impl GraphBuilder {
  pub fn new(
    handler: Rc<RefCell<dyn SpecifierHandler>>,
    maybe_import_map: Option<ImportMap>,
  ) -> Self {
    let internal_import_map = if let Some(import_map) = maybe_import_map {
      Some(Rc::new(RefCell::new(import_map)))
    } else {
      None
    };
    GraphBuilder {
      graph: Graph::new(handler),
      fetched: HashSet::new(),
      maybe_import_map: internal_import_map,
      pending: FuturesUnordered::new(),
    }
  }

  /// Request a module to be fetched from the handler and queue up its future
  /// to be awaited to be resolved.
  fn fetch(&mut self, specifier: &ModuleSpecifier) -> Result<()> {
    if self.fetched.contains(&specifier) {
      return Ok(());
    }

    self.fetched.insert(specifier.clone());
    let future = self.graph.handler.borrow_mut().fetch(specifier.clone());
    self.pending.push(future);

    Ok(())
  }

  /// Visit a module that has been fetched, hydrating the module, analyzing its
  /// dependencies if required, fetching those dependencies, and inserting the
  /// module into the graph.
  fn visit(&mut self, cached_module: CachedModule) -> Result<()> {
    let specifier = cached_module.specifier.clone();
    let mut module =
      Module::new(specifier.clone(), self.maybe_import_map.clone());
    module.hydrate(cached_module);
    if !module.is_parsed {
      let has_types = module.maybe_types.is_some();
      module.parse()?;
      if self.maybe_import_map.is_none() {
        let mut handler = self.graph.handler.borrow_mut();
        handler.set_deps(&specifier, module.dependencies.clone())?;
        if !has_types {
          if let Some((types, _)) = module.maybe_types.clone() {
            handler.set_types(&specifier, types)?;
          }
        }
      }
    }
    for (_, dep) in module.dependencies.iter() {
      if let Some(specifier) = dep.maybe_code.as_ref() {
        self.fetch(specifier)?;
      }
      if let Some(specifier) = dep.maybe_type.as_ref() {
        self.fetch(specifier)?;
      }
    }
    if let Some((_, specifier)) = module.maybe_types.as_ref() {
      self.fetch(specifier)?;
    }
    self.graph.modules.insert(specifier, module);

    Ok(())
  }

  /// Insert a module into the graph based on a module specifier.  The module
  /// and any dependencies will be fetched from the handler.  The module will
  /// also be treated as a _root_ module in the graph.
  pub async fn insert(&mut self, specifier: &ModuleSpecifier) -> Result<()> {
    self.fetch(specifier)?;

    loop {
      let cached_module = self.pending.next().await.unwrap()?;
      self.visit(cached_module)?;
      if self.pending.is_empty() {
        break;
      }
    }

    if !self.graph.roots.contains(specifier) {
      self.graph.roots.push(specifier.clone());
    }

    Ok(())
  }

  /// Move out the graph from the builder to be utilized further.  An optional
  /// lockfile can be provided, where if the sources in the graph do not match
  /// the expected lockfile, the method with error instead of returning the
  /// graph.
  pub fn get_graph(
    self,
    maybe_lockfile: &Option<Mutex<Lockfile>>,
  ) -> Result<Graph> {
    self.graph.lock(maybe_lockfile)?;
    Ok(self.graph)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::specifier_handler::tests::MockSpecifierHandler;

  use std::env;
  use std::path::PathBuf;
  use std::sync::Mutex;

  #[tokio::test]
  async fn test_graph_builder() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler, None);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/mod.ts")
        .expect("could not resolve module");
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    let graph = builder.get_graph(&None).expect("error getting graph");
    let actual = graph
      .resolve("./a.ts", &specifier)
      .expect("module to resolve");
    let expected = (
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/a.ts")
        .expect("unable to resolve"),
      MediaType::TypeScript,
    );
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn test_graph_builder_import_map() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let import_map = ImportMap::from_json(
      "https://deno.land/x/import_map.ts",
      r#"{
      "imports": {
        "jquery": "./jquery.js",
        "lodash": "https://unpkg.com/lodash/index.js"
      }
    }"#,
    )
    .expect("could not load import map");
    let mut builder = GraphBuilder::new(handler, Some(import_map));
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/import_map.ts")
        .expect("could not resolve module");
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    let graph = builder.get_graph(&None).expect("could not get graph");
    let actual_jquery = graph
      .resolve("jquery", &specifier)
      .expect("module to resolve");
    let expected_jquery = (
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/jquery.js")
        .expect("unable to resolve"),
      MediaType::JavaScript,
    );
    assert_eq!(actual_jquery, expected_jquery);
    let actual_lodash = graph
      .resolve("lodash", &specifier)
      .expect("module to resolve");
    let expected_lodash = (
      ModuleSpecifier::resolve_url_or_path("https://unpkg.com/lodash/index.js")
        .expect("unable to resolve"),
      MediaType::JavaScript,
    );
    assert_eq!(actual_lodash, expected_lodash);
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
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts")
        .expect("could not resolve module");
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    let mut graph = builder.get_graph(&None).expect("could not get graph");
    let (stats, maybe_ignored_options) =
      graph.transpile(TranspileOptions::default()).unwrap();
    assert_eq!(stats.0.len(), 3);
    assert_eq!(maybe_ignored_options, None);
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 2);
    assert_eq!(h.cache_calls[0].1, EmitType::Cli);
    assert!(h.cache_calls[0]
      .2
      .to_string()
      .unwrap()
      .contains("# sourceMappingURL=data:application/json;base64,"));
    assert_eq!(h.cache_calls[0].3, None);
    assert_eq!(h.cache_calls[1].1, EmitType::Cli);
    assert!(h.cache_calls[1]
      .2
      .to_string()
      .unwrap()
      .contains("# sourceMappingURL=data:application/json;base64,"));
    assert_eq!(h.cache_calls[0].3, None);
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
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/transpile.tsx")
        .expect("could not resolve module");
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    let mut graph = builder.get_graph(&None).expect("could not get graph");
    let config = r#"{
        "compilerOptions": {
          "target": "es5",
          "jsx": "preserve"
        }
      }"#;
    let (_, maybe_ignored_options) = graph
      .transpile(TranspileOptions {
        debug: false,
        maybe_config: Some(config.to_string()),
      })
      .unwrap();
    assert_eq!(
      maybe_ignored_options,
      Some(IgnoredCompilerOptions(vec!["target".to_string()])),
      "the 'target' options should have been ignored"
    );
    let h = handler.borrow();
    assert_eq!(h.cache_calls.len(), 1, "only one file should be emitted");
    assert!(
      h.cache_calls[0]
        .2
        .to_string()
        .unwrap()
        .contains("<div>Hello world!</div>"),
      "jsx should have been preserved"
    );
  }

  #[tokio::test]
  async fn test_graph_with_lockfile() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let lockfile_path = fixtures.join("lockfile.json");
    let lockfile =
      Lockfile::new(lockfile_path.to_string_lossy().to_string(), false)
        .expect("could not load lockfile");
    let maybe_lockfile = Some(Mutex::new(lockfile));
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts")
        .expect("could not resolve module");
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    builder
      .get_graph(&maybe_lockfile)
      .expect("could not get graph");
  }

  #[tokio::test]
  async fn test_graph_with_lockfile_fail() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("tests/module_graph");
    let lockfile_path = fixtures.join("lockfile_fail.json");
    let lockfile =
      Lockfile::new(lockfile_path.to_string_lossy().to_string(), false)
        .expect("could not load lockfile");
    let maybe_lockfile = Some(Mutex::new(lockfile));
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let mut builder = GraphBuilder::new(handler.clone(), None);
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/main.ts")
        .expect("could not resolve module");
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    builder
      .get_graph(&maybe_lockfile)
      .expect_err("expected an error");
  }
}
