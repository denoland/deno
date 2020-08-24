// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast::parse;
use crate::ast::EmitTranspileOptions;
use crate::ast::Location;
use crate::ast::ParsedModule;
use crate::bundler;
use crate::compiler::CompilerIsolate;
use crate::compiler::InternalCompileOptions;
use crate::compiler::InternalTranspileOptions;
use crate::config::json_merge;
use crate::config::parse_config;
use crate::import_map::ImportMap;
use crate::msg::as_ts_filename;
use crate::msg::common_path_reduce;
use crate::msg::CompilerStats;
use crate::msg::EmittedFile;
use crate::msg::IgnoredCompilerOptions;
use crate::msg::MediaType;
use crate::msg::TranspileSourceFile;
use crate::BundleOptions;
use crate::CompileOptions;
use crate::Result;
use crate::TranspileOptions;

use deno_core::ModuleSpecifier;
use deno_core::Snapshot;
use deno_core::StartupData;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures::Future;
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::pin::Pin;
use std::rc::Rc;

pub type FetchFuture =
  Pin<Box<(dyn Future<Output = Result<ModuleSpecifier>> + 'static)>>;

/// A trait which provides the methods required for a module graph to be built
/// and any changes to be notified back to the handler.
pub trait SpecifierHandler {
  /// Instructs the handler to fetch a specifier, resolving when finished.
  fn fetch(&mut self, specifier: &ModuleSpecifier) -> FetchFuture;

  /// Get a module from cache, which has been previously fetched. It is expected
  /// that source for the module had been invalidated that the related emitted
  /// code would not be returned either.
  fn get_cache(&self, specifier: &ModuleSpecifier) -> Result<CachedModule>;

  /// Get the optional build info from the cache for a given module specifier.
  /// Because build infos are only associated with the "root" modules, they are
  /// not expected to be cached for each module, but are "lazily" checked when
  /// a root module is identified.  The `cache_type` also indicates what form
  /// of the module the build info is valid for.
  fn get_build_info(
    &self,
    specifier: &ModuleSpecifier,
    cache_type: &CacheType,
  ) -> Result<Option<String>>;

  /// Set the emitted code (and maybe map) for a given module specifier.  The
  /// cache type indicates what form the emit is related to.
  fn set_cache(
    &mut self,
    specifier: &ModuleSpecifier,
    cache_type: &CacheType,
    code: String,
    maybe_map: Option<String>,
  ) -> Result<()>;

  /// When parsed out of a JavaScript module source, the triple slash reference
  /// to the types should be stored in the cache.
  fn set_types(
    &mut self,
    specifier: &ModuleSpecifier,
    types: String,
  ) -> Result<()>;

  /// Set the build info for a module specifier, also providing the cache type.
  fn set_build_info(
    &mut self,
    specifier: &ModuleSpecifier,
    cache_type: &CacheType,
    build_info: String,
  ) -> Result<()>;

  /// Set the graph dependencies for a given module specifier.
  fn set_deps(
    &mut self,
    specifier: &ModuleSpecifier,
    dependencies: CachedDependencies,
  ) -> Result<()>;
}

impl std::fmt::Debug for dyn SpecifierHandler {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(f, "SpecifierHandler {{ }}")
  }
}

/// A trait, implemented by `Graph` that provides the interfaces that the
/// compiler ops require to be able to retrieve and set information about a
/// compilation.
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

#[derive(Clone, Debug)]
pub struct CachedDependency {
  pub specifier: String,
  pub maybe_code: Option<ModuleSpecifier>,
  pub maybe_type: Option<ModuleSpecifier>,
}

#[derive(Clone, Debug)]
pub struct CachedDependencies(pub Vec<CachedDependency>);

impl From<&HashMap<String, ModuleDependency>> for CachedDependencies {
  fn from(dependencies: &HashMap<String, ModuleDependency>) -> Self {
    let mut cached_deps = Vec::new();
    for (specifier, dep) in dependencies.iter() {
      let cached_dep = CachedDependency {
        specifier: specifier.clone(),
        maybe_code: dep.maybe_code.clone(),
        maybe_type: dep.maybe_type.clone(),
      };
      cached_deps.push(cached_dep);
    }
    CachedDependencies(cached_deps)
  }
}

/// The logical representation of a cached module that is returned from a
/// specifier handler.
#[derive(Clone, Debug)]
pub struct CachedModule {
  pub bundle_code: Option<String>,
  pub bundle_map: Option<String>,
  pub code: Option<String>,
  pub map: Option<String>,
  pub maybe_dependencies: Option<CachedDependencies>,
  pub maybe_types: Option<String>,
  pub media_type: MediaType,
  pub source: String,
}

#[cfg(test)]
impl Default for CachedModule {
  fn default() -> Self {
    CachedModule {
      bundle_code: None,
      bundle_map: None,
      code: None,
      map: None,
      maybe_dependencies: None,
      maybe_types: None,
      media_type: MediaType::Unknown,
      source: "".to_string(),
    }
  }
}

/// Represents different types of cached code which can be generated for a
/// specifier.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum CacheType {
  /// A cache version which is a valid JavaScript ES Module
  Cli,
  /// A cache version which is a SystemJS Module
  Bundle,
}

impl Default for CacheType {
  fn default() -> Self {
    CacheType::Cli
  }
}

/// A group of errors that represent errors that can occur when interacting with
/// a module graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphError {
  /// A module using the HTTPS protocol is trying to import a module with an
  /// HTTP schema.
  InvalidDowngrade(ModuleSpecifier, Location),
  /// A remote module is trying to import a local module.
  InvalidLocalImport(ModuleSpecifier, Location),
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

/// Represents the type of dependencies for a specifier, as code and type
/// dependencies are treated differently when checking a program.
pub enum DependencyType {
  // These are the code dependencies, which their code output will be needed
  // at runtime.
  Code,
  // These are type dependencies, which are needed to type check the the
  // source code.
  Type,
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

enum TypeScriptReference {
  Path(String),
  Types(String),
}

/// Parse out any `path` or `types` triple-slash references in a comment string
/// and return it.
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

#[derive(Debug, Clone, Default)]
pub struct ModuleDependency {
  maybe_code: Option<ModuleSpecifier>,
  maybe_type: Option<ModuleSpecifier>,
}

/// A logical representation of a module within a module graph.
#[derive(Debug, Clone)]
pub struct Module {
  /// The module specifier for the module
  specifier: ModuleSpecifier,
  /// The media type for the module
  media_type: MediaType,
  /// A hash map of dependencies, where the key is the string of the dependency
  /// from the module, and the value is a descriptor of the dependency
  dependencies: HashMap<String, ModuleDependency>,
  /// The source code of the dependency
  pub source: String,
  /// An optional import map which will be used when resolving imported modules.
  maybe_import_map: Option<Rc<RefCell<ImportMap>>>,
  maybe_parsed_module: Option<ParsedModule>,
  /// If there is an `X-TypeScript-Types` header or the source contains a
  /// triple slash reference to some types, this will be populated with the
  /// specifier that should be used instead.
  maybe_types: Option<(String, ModuleSpecifier)>,
  /// If the module has been emitted, for the CLI, the emitted code is here.
  code: Option<String>,
  /// If the module has been emitted, if it has a separate source map, it is
  /// here.
  map: Option<String>,
  /// If the module has been emitted, for a bundle, the emitted code is here.
  bundle_code: Option<String>,
  /// If the module has been emitted for a bundle, and has a separate source
  /// map, it is here.
  bundle_map: Option<String>,
  /// A flag that indicates that the emit of the module is dirty and needs to
  /// have its cache updated.
  is_dirty: bool,
  /// A flag to indicated if the module has had its dependencies analysed.
  is_parsed: bool,
  /// A flag to indicate if the file has been hydrated based on its cached
  /// value from the specifier handler.
  is_hydrated: bool,
}

impl Module {
  pub fn new(
    specifier: ModuleSpecifier,
    maybe_import_map: Option<Rc<RefCell<ImportMap>>>,
  ) -> Self {
    Module {
      specifier,
      maybe_import_map,
      media_type: MediaType::Unknown,
      ..Module::default()
    }
  }

  pub fn default() -> Self {
    Module {
      specifier: ModuleSpecifier::resolve_url("https://deno.land/x/").unwrap(),
      media_type: MediaType::Unknown,
      dependencies: HashMap::new(),
      source: "".to_string(),
      maybe_import_map: None,
      maybe_parsed_module: None,
      maybe_types: None,
      is_dirty: false,
      code: None,
      map: None,
      bundle_code: None,
      bundle_map: None,
      is_parsed: false,
      is_hydrated: false,
    }
  }

  /// Take the cached representation of a module and _hydrate_ its structure.
  pub fn hydrate(&mut self, cached_module: CachedModule) {
    self.media_type = cached_module.media_type;
    self.source = cached_module.source;
    if self.maybe_import_map.is_none() {
      if let Some(ref dependencies) = cached_module.maybe_dependencies {
        for cached_dep in dependencies.0.iter() {
          self.dependencies.insert(
            cached_dep.specifier.clone(),
            ModuleDependency {
              maybe_code: cached_dep.maybe_code.clone(),
              maybe_type: cached_dep.maybe_type.clone(),
            },
          );
        }
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
    self.code = cached_module.code;
    self.map = cached_module.map;
    self.bundle_code = cached_module.bundle_code;
    self.bundle_map = cached_module.bundle_map;
    self.is_hydrated = true;
  }

  /// Perform a parse of the source of the module, identifying any dependencies.
  ///
  /// This has specific logic that parses out triple slash references supported
  /// by Deno as well as `@deno-types` pragmas in addition to identifying any
  /// _standard_ ES module dependencies.
  pub fn parse(&mut self) -> Result<()> {
    let parsed_module = parse(&self.specifier, &self.source, self.media_type)?;

    // Parse out any triple slash references
    for comment in parsed_module.leading_comments.iter() {
      if let Some(ts_reference) = parse_ts_reference(&comment.text) {
        let location: Location = parsed_module
          .source_map
          .lookup_char_pos(comment.span.lo)
          .into();
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
    let dependencies = parsed_module.get_dependencies();
    for desc in dependencies.iter() {
      let specifier =
        self.resolve_import(&desc.specifier, Some(desc.location.clone()))?;

      // Parse out any `@deno-types` pragmas and modify dependency
      let maybe_types_specifier = if !desc.leading_comments.is_empty() {
        let comment = desc.leading_comments.last().unwrap();
        if let Some(deno_types) = parse_deno_types(&comment.text).as_ref() {
          Some(self.resolve_import(deno_types, Some(desc.location.clone()))?)
        } else {
          None
        }
      } else {
        None
      };

      let dep = self.dependencies.entry(desc.specifier.clone()).or_default();
      if desc.is_type {
        dep.maybe_type = Some(specifier);
      } else {
        dep.maybe_code = Some(specifier);
      }
      if let Some(types_specifier) = maybe_types_specifier {
        dep.maybe_type = Some(types_specifier);
      }
    }

    self.is_parsed = true;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranspileTsOptions {
  check_js: bool,
  emit_decorator_metadata: bool,
  jsx: String,
  jsx_factory: String,
  jsx_fragment_factory: String,
}

/// A structure which represents a dependency graph of a set of modules, which
/// can be used to compile or transpile source code into code that can be run
/// within an isolate.
#[derive(Debug, Clone)]
pub struct Graph {
  build_info: HashMap<CacheType, String>,
  handler: Rc<RefCell<dyn SpecifierHandler>>,
  modules: HashMap<ModuleSpecifier, Module>,
  roots: Vec<ModuleSpecifier>,
}

impl Graph {
  pub fn new(handler: Rc<RefCell<dyn SpecifierHandler>>) -> Self {
    Graph {
      build_info: HashMap::new(),
      handler,
      modules: HashMap::new(),
      roots: Vec::new(),
    }
  }

  /// Create a single file output for the module graph, updating any emitted
  /// modules and build info with the specifier handler.
  ///
  /// # Arguments
  ///
  /// - `options` - a structure of options that effect the generation of the
  ///   bundle.
  /// - `maybe_snapshot_data` - A snapshot of a TypeScript compiler isolate,
  ///   which will be used to compile the modules.  Currently, this is required
  ///   but maybe optional in the future.  If not present, the result will
  ///   error.
  ///
  pub fn bundle(
    self,
    options: BundleOptions,
    maybe_snapshot_data: Option<Box<[u8]>>,
  ) -> Result<(
    String,
    Option<String>,
    CompilerStats,
    Option<IgnoredCompilerOptions>,
  )> {
    if self.roots.len() != 1 {
      return Err(
        NotSupported(format!(
          "Bundling only supports a single root file.  Graph has {} root(s).",
          self.roots.len()
        ))
        .into(),
      );
    }
    if let Some(snapshot_data) = maybe_snapshot_data {
      let roots = self.roots.clone();
      let maybe_shared_path = self.get_shared_path();
      let main_specifier = as_ts_filename(&roots[0], &maybe_shared_path);
      let maybe_named_exports = self.get_root_export_names()?;
      let startup_data = StartupData::Snapshot(Snapshot::Boxed(snapshot_data));
      let mut compiler = CompilerIsolate::new(startup_data);
      let cache_type = CacheType::Bundle;
      let provider = Rc::new(RefCell::new(self));
      let emit_result: Result<(CompilerStats, Option<IgnoredCompilerOptions>)> =
        if options.check {
          let p = provider.borrow();
          let maybe_build_info = p.build_info.get(&cache_type).cloned();
          let root_names = p.roots.iter().clone().collect();

          let emit = compiler.compile(InternalCompileOptions {
            bundle: true,
            check_only: false,
            compile_options: CompileOptions {
              debug: options.debug,
              incremental: options.incremental,
              lib: options.lib.clone(),
              maybe_config: options.maybe_config.clone(),
            },
            maybe_build_info,
            maybe_shared_path,
            provider: provider.clone(),
            root_names,
          })?;
          drop(p);
          let mut p = provider.borrow_mut();
          p.update(emit.written_files, &cache_type, true)?;
          p.flush(&cache_type)?;

          Ok((emit.stats, emit.maybe_ignored_options))
        } else {
          let p = provider.borrow();
          let sources = p.get_sources(true, &cache_type);
          drop(p);

          let emit = compiler.transpile(InternalTranspileOptions {
            bundle: true,
            sources,
            transpile_options: TranspileOptions {
              debug: options.debug,
              maybe_config: options.maybe_config.clone(),
            },
          })?;
          let mut p = provider.borrow_mut();
          p.update(emit.written_files, &cache_type, true)?;
          p.flush(&cache_type)?;

          Ok((emit.stats, emit.maybe_ignored_options))
        };
      let (stats, maybe_ignored_options) = emit_result?;
      let p = provider.borrow();
      let files: Vec<bundler::BundleFile> = p
        .modules
        .iter()
        .map(|(_, m)| bundler::BundleFile {
          code: m.bundle_code.clone().unwrap(),
          map: m.bundle_map.clone().unwrap(),
        })
        .collect();

      let (bundle_code, maybe_bundle_map) = bundler::bundle(
        files,
        bundler::BundleOptions {
          inline_source_map: options.inline_source_map,
          main_specifier,
          maybe_named_exports,
          target_es5: false,
        },
      )?;

      Ok((bundle_code, maybe_bundle_map, stats, maybe_ignored_options))
    } else {
      Err(
        NotSupported(
          "Compiling without snapshot data currently not supported.".into(),
        )
        .into(),
      )
    }
  }

  /// Compile (type check and transform) the graph, updating any emitted modules
  /// and build info with the specifier handler.  The result contains any
  /// performance stats from the compiler and optionally any user provided
  /// configuration compiler options that were ignored.
  ///
  /// # Arguments
  ///
  /// - `options` - A structure of compiler options which effect how the modules
  ///   in the graph are compiled.
  /// - `check_only` - Do a compilation of the graph, but do not emit any files
  ///   and update the graph.
  /// - `maybe_snapshot_data` - A snapshot of a TypeScript compiler isolate,
  ///   which will be used to compile the modules.  Currently, this is required
  ///   but maybe optional in the future.  If not present, the result will
  ///   error.
  ///
  pub fn compile(
    self,
    compile_options: CompileOptions,
    check_only: bool,
    maybe_snapshot_data: Option<Box<[u8]>>,
  ) -> Result<(CompilerStats, Option<IgnoredCompilerOptions>)> {
    if let Some(snapshot_data) = maybe_snapshot_data {
      let cache_type = CacheType::Cli;
      let startup_data = StartupData::Snapshot(Snapshot::Boxed(snapshot_data));
      let mut compiler = CompilerIsolate::new(startup_data);
      let provider = Rc::new(RefCell::new(self));
      let p = provider.borrow();
      let maybe_build_info = p.build_info.get(&cache_type).cloned();
      let root_names = p.roots.iter().clone().collect();

      let emit = compiler.compile(InternalCompileOptions {
        bundle: false,
        check_only,
        compile_options,
        maybe_build_info,
        maybe_shared_path: None,
        provider: provider.clone(),
        root_names,
      })?;
      drop(p);
      if !check_only {
        let mut p = provider.borrow_mut();
        p.update(emit.written_files, &cache_type, emit.cache_js)?;
        p.flush(&cache_type)?;
      }

      Ok((emit.stats, emit.maybe_ignored_options))
    } else {
      Err(
        NotSupported(
          "Compiling without snapshot data currently not supported.".into(),
        )
        .into(),
      )
    }
  }

  /// Update the handler with any modules that are marked as _dirty_ and update
  /// any build info if present.
  fn flush(&mut self, cache_type: &CacheType) -> Result<()> {
    let mut handler = self.handler.borrow_mut();
    for (_, module) in self.modules.iter_mut() {
      if module.is_dirty {
        match cache_type {
          CacheType::Cli => handler.set_cache(
            &module.specifier,
            &cache_type,
            module.code.clone().unwrap(),
            module.map.clone(),
          )?,
          CacheType::Bundle => handler.set_cache(
            &module.specifier,
            &cache_type,
            module.bundle_code.clone().unwrap(),
            module.bundle_map.clone(),
          )?,
        }
        module.is_dirty = false;
      }
    }
    for root_specifier in self.roots.iter() {
      if let Some(build_info) = self.build_info.get(&cache_type) {
        handler.set_build_info(
          root_specifier,
          &cache_type,
          build_info.to_owned(),
        )?;
      }
    }

    Ok(())
  }

  /// Recursively get the exported names of a module.  This handles situations
  /// where the `export * from 'spec'` exports.
  fn get_export_names(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Vec<String>> {
    if let Some(module) = self.modules.get(specifier) {
      let mut names: Vec<String> = Vec::new();
      if let Some(parsed_module) = module.maybe_parsed_module.as_ref() {
        let (mut parsed_names, export_all_specifiers) =
          parsed_module.get_export_names();
        names.append(&mut parsed_names);
        for spec in export_all_specifiers.iter() {
          let (exp_specifier, _) = self.resolve(spec, specifier)?;
          let mut parsed_names = self.get_export_names(&exp_specifier)?;
          names.append(&mut parsed_names);
        }
      }

      Ok(names)
    } else {
      Err(MissingSpecifier(specifier.clone()).into())
    }
  }

  /// For the root module, optionally return a vector of the strings of the
  /// names of those exports.  This is used when bundling to "re-export" the
  /// surface area of the root module from the bundle.
  fn get_root_export_names(&self) -> Result<Option<Vec<String>>> {
    let root_specifier = &self.roots[0];
    let names = self.get_export_names(root_specifier)?;

    if !names.is_empty() {
      Ok(Some(names))
    } else {
      Ok(None)
    }
  }

  fn get_shared_path(&self) -> Option<String> {
    let specifiers: Vec<&ModuleSpecifier> = self
      .modules
      .iter()
      .filter(|(k, _)| k.as_url().scheme() == "file")
      .map(|(k, _)| k)
      .collect();
    let maybe_shared_path = common_path_reduce(specifiers);
    if let Some(specifier) = maybe_shared_path.as_ref() {
      Some(as_ts_filename(specifier, &None))
    } else {
      None
    }
  }

  /// Retrieve the graph of dependencies as a hash map, typically used for
  /// transpiling/type stripping.
  ///
  /// Note, that when the `cache_type` is `CacheType::Bundle`, the source files
  /// will contain a remapping of the module specifiers, to those provided in
  /// the bundle.
  fn get_sources(
    &self,
    include_js: bool,
    cache_type: &CacheType,
  ) -> HashMap<String, TranspileSourceFile> {
    let mut sources: HashMap<String, TranspileSourceFile> = HashMap::new();
    for (specifier, module) in self.modules.iter() {
      // we will skip any files where we have valid source, as it is expected
      // that if the cache is invalid, the handler will not provide the code
      if match cache_type {
        CacheType::Cli => module.code.is_some(),
        CacheType::Bundle => module.bundle_code.is_some(),
      } {
        continue;
      }
      if !(include_js
        || module.media_type == MediaType::TypeScript
        || module.media_type == MediaType::TSX)
      {
        continue;
      }
      if *cache_type == CacheType::Bundle {
        let shared_path = self.get_shared_path();
        let key = as_ts_filename(specifier, &shared_path);
        let mut renamed_dependencies = HashMap::new();
        for (spec, dep) in module.dependencies.iter() {
          if let Some(dep_code) = &dep.maybe_code {
            renamed_dependencies
              .insert(spec.clone(), as_ts_filename(dep_code, &shared_path));
          }
        }
        sources.insert(
          key,
          TranspileSourceFile {
            data: module.source.clone(),
            renamed_dependencies: Some(renamed_dependencies),
          },
        );
      } else {
        sources.insert(
          specifier.to_string(),
          TranspileSourceFile {
            data: module.source.clone(),
            renamed_dependencies: None,
          },
        );
      }
    }

    sources
  }

  /// Checks the modules in the graph of any that are lacking an emit, and if
  /// so return `true` otherwise return `false`.  The argument `include_js` will
  /// determine if JavaScript in the graph are considered in this.
  pub fn needs_emit(&self, include_js: bool) -> bool {
    self.modules.iter().fold(false, |acc, (_, m)| {
      if !include_js && m.media_type == MediaType::JavaScript {
        acc
      } else if m.code.is_none() {
        true
      } else {
        acc
      }
    })
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
  /// - `maybe_shanpshot_data` - If provided, it is expected that the data is
  ///   a snapshot of the TypeScript compiler, which will in turn be used to
  ///   perform the transpilation.  If none, then the transpilation will be
  ///   performed by swc.
  ///
  pub fn transpile(
    mut self,
    options: TranspileOptions,
    maybe_snapshot_data: Option<Box<[u8]>>,
  ) -> Result<(CompilerStats, Option<IgnoredCompilerOptions>)> {
    let cache_type = CacheType::Cli;
    if let Some(snapshot_data) = maybe_snapshot_data {
      let startup_data = StartupData::Snapshot(Snapshot::Boxed(snapshot_data));
      let mut compiler = CompilerIsolate::new(startup_data);
      let sources = self.get_sources(false, &cache_type);

      let emit = compiler.transpile(InternalTranspileOptions {
        bundle: false,
        sources,
        transpile_options: options,
      })?;
      self.update(emit.written_files, &cache_type, true)?;
      self.flush(&cache_type)?;

      Ok((emit.stats, emit.maybe_ignored_options))
    } else {
      let mut compiler_options = json!({
        "checkJs": false,
        "emitDecoratorMetadata": false,
        "jsx": "react",
        "jsxFactory": "React.createElement",
        "jsxFragmentFactory": "React.Fragment",
      });

      let maybe_ignored_options =
        if let Some(config_text) = options.maybe_config {
          let (user_config, ignored_options) = parse_config(config_text)?;
          json_merge(&mut compiler_options, &user_config);
          ignored_options
        } else {
          None
        };

      let compiler_options: TranspileTsOptions =
        serde_json::from_value(compiler_options)?;
      let check_js = compiler_options.check_js;
      let transform_jsx = compiler_options.jsx == "react";
      let emit_options = EmitTranspileOptions {
        emit_metadata: compiler_options.emit_decorator_metadata,
        inline_source_map: true,
        jsx_factory: compiler_options.jsx_factory,
        jsx_fragment_factory: compiler_options.jsx_fragment_factory,
        transform_jsx,
      };

      for (_, module) in self.modules.iter_mut() {
        // skip modules that already have a valid emit
        if module.code.is_some() {
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
        // if the module looks like it is a dts module, we should skip it as well
        if module.specifier.as_url().path().ends_with(".d.ts") {
          continue;
        }
        if module.maybe_parsed_module.is_none() {
          module.parse()?;
        }
        let parsed_module = module.maybe_parsed_module.clone().unwrap();
        let (code, maybe_map) = parsed_module.transpile(&emit_options)?;
        module.code = Some(code);
        module.map = maybe_map;
        module.is_dirty = true;
      }
      self.flush(&cache_type)?;

      // TODO @kitsonk - provide some useful stats

      Ok((CompilerStats { items: vec![] }, maybe_ignored_options))
    }
  }

  /// Given a set of emitted files, update the modules that are part of the
  /// graph, marking updated modules as _dirty_ so they will be updated when
  /// flushed.
  fn update(
    &mut self,
    files: Vec<EmittedFile>,
    cache_type: &CacheType,
    cache_js: bool,
  ) -> Result<()> {
    for file in files.iter() {
      if let Some(module_name) = file.maybe_module_name.as_ref() {
        let specifier = ModuleSpecifier::resolve_url_or_path(module_name)?;
        if let Some(module) = self.modules.get_mut(&specifier) {
          if !cache_js && module.media_type == MediaType::JavaScript {
            continue;
          }
          let is_map = file.url.ends_with(".map");
          let data = file.data.clone();
          match cache_type {
            CacheType::Cli => {
              if is_map {
                module.map = Some(data);
              } else {
                module.code = Some(data);
              }
            }
            CacheType::Bundle => {
              if is_map {
                module.bundle_map = Some(data);
              } else {
                module.bundle_code = Some(data);
              }
            }
          };
          module.is_dirty = true;
        } else {
          return Err(MissingSpecifier(specifier).into());
        }
      }
    }

    Ok(())
  }
}

impl ModuleProvider for Graph {
  fn get_source(&self, specifier: &ModuleSpecifier) -> Option<String> {
    if let Some(module) = self.modules.get(specifier) {
      Some(module.source.clone())
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
    let future = self.graph.handler.borrow_mut().fetch(specifier);
    self.pending.push(future);

    Ok(())
  }

  /// Visit a module that has been fetched, hydrating the module, analyzing its
  /// dependencies if required, fetching those dependencies, and inserting the
  /// module into the graph.
  fn visit(&mut self, specifier: &ModuleSpecifier) -> Result<()> {
    let cached_module = self.graph.handler.borrow().get_cache(specifier)?;
    let mut module =
      Module::new(specifier.clone(), self.maybe_import_map.clone());
    module.hydrate(cached_module);
    if !module.is_parsed {
      let has_types = module.maybe_types.is_some();
      module.parse()?;
      if self.maybe_import_map.is_none() {
        let mut handler = self.graph.handler.borrow_mut();
        handler.set_deps(
          specifier,
          CachedDependencies::from(&module.dependencies),
        )?;
        if !has_types {
          if let Some((types, _)) = module.maybe_types.clone() {
            handler.set_types(specifier, types)?;
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
    self.graph.modules.insert(specifier.clone(), module);

    Ok(())
  }

  /// Insert a module into the graph based on a module specifier.  The module
  /// and any dependencies will be fetched from the handler.  The module will
  /// also be treated as a _root_ module in the graph.
  pub async fn insert(&mut self, specifier: &ModuleSpecifier) -> Result<()> {
    self.fetch(specifier)?;

    loop {
      let next_specifier = self.pending.next().await.unwrap()?;
      self.visit(&next_specifier)?;
      if self.pending.is_empty() {
        break;
      }
    }

    if !self.graph.roots.contains(specifier) {
      let handler = self.graph.handler.borrow();
      // any specifier that is inserted becomes a root specifier
      self.graph.roots.push(specifier.clone());
      // if there is currently not any build info, check the specifier handler
      // to see if there is any and populate it
      if self.graph.build_info.is_empty() {
        if let Some(build_info) =
          handler.get_build_info(specifier, &CacheType::Cli)?
        {
          self.graph.build_info.insert(CacheType::Cli, build_info);
        }
        if let Some(build_info) =
          handler.get_build_info(specifier, &CacheType::Bundle)?
        {
          self.graph.build_info.insert(CacheType::Bundle, build_info);
        }
      }
    }

    Ok(())
  }

  /// Move out the graph from the builder to be utilized further.
  pub fn get_graph(self) -> Graph {
    self.graph
  }
}

#[cfg(test)]
pub mod tests {
  use super::*;

  use crate::tests::MockSpecifierHandler;
  use std::env;
  use std::path::PathBuf;

  #[tokio::test]
  async fn test_graph_builder() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("fixtures");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/test/mod.ts")
        .expect("could not resolve module");
    let mut builder = GraphBuilder::new(handler, None);
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    let graph = builder.get_graph();
    let actual = graph
      .resolve("./a.ts", &specifier)
      .expect("module to resolve");
    let expected = (
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/test/a.ts")
        .expect("unable to resolve"),
      MediaType::TypeScript,
    );
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn test_graph_builder_types() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("fixtures");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/mod.ts")
        .expect("could not resolve module");
    let mut builder = GraphBuilder::new(handler, None);
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    let graph = builder.get_graph();
    let actual = graph
      .resolve("./type_reference.js", &specifier)
      .expect("module to resolve");
    let expected = (
      ModuleSpecifier::resolve_url_or_path("file:///tests/type_reference.d.ts")
        .expect("unable to resolve"),
      MediaType::TypeScript,
    );
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn test_graph_builder_deno_types() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("fixtures");
    let handler = Rc::new(RefCell::new(MockSpecifierHandler {
      fixtures,
      ..MockSpecifierHandler::default()
    }));
    let specifier =
      ModuleSpecifier::resolve_url_or_path("file:///tests/deno_types.ts")
        .expect("could not resolve module");
    let mut builder = GraphBuilder::new(handler, None);
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    let graph = builder.get_graph();
    let actual = graph
      .resolve("https://deno.land/x/jquery.js", &specifier)
      .expect("module to resolve");
    let expected = (
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/jquery.d.ts")
        .expect("unable to resolve"),
      MediaType::TypeScript,
    );
    assert_eq!(actual, expected);
  }

  #[tokio::test]
  async fn test_graph_builder_import_map() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let fixtures = c.join("fixtures");
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
    let specifier =
      ModuleSpecifier::resolve_url_or_path("https://deno.land/x/import_map.ts")
        .expect("could not resolve module");
    let mut builder = GraphBuilder::new(handler, Some(import_map));
    builder
      .insert(&specifier)
      .await
      .expect("module not inserted");
    let graph = builder.get_graph();
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
}
