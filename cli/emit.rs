// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

//! The collection of APIs to be able to take `deno_graph` module graphs and
//! populate a cache, emit files, and transform a graph into the structures for
//! loading into an isolate.

use crate::ast;
use crate::cache::Cacher;
use crate::config_file::ConfigFile;
use crate::config_file::IgnoredCompilerOptions;
use crate::config_file::TsConfig;
use crate::diagnostics::Diagnostics;
use crate::tsc;
use crate::version;

use deno_ast::swc;
use deno_core::error::anyhow;
use deno_core::error::AnyError;
use deno_core::error::Context;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json::json;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_graph::MediaType;
use deno_graph::ModuleGraph;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

/// Represents the "default" type library that should be used when type
/// checking the code in the module graph.  Note that a user provided config
/// of `"lib"` would override this value.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum TypeLib {
  DenoWindow,
  DenoWorker,
  UnstableDenoWindow,
  UnstableDenoWorker,
}

impl Default for TypeLib {
  fn default() -> Self {
    Self::DenoWindow
  }
}

impl Serialize for TypeLib {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let value = match self {
      Self::DenoWindow => vec!["deno.window".to_string()],
      Self::DenoWorker => vec!["deno.worker".to_string()],
      Self::UnstableDenoWindow => {
        vec!["deno.window".to_string(), "deno.unstable".to_string()]
      }
      Self::UnstableDenoWorker => {
        vec!["deno.worker".to_string(), "deno.unstable".to_string()]
      }
    };
    Serialize::serialize(&value, serializer)
  }
}

type Modules = HashMap<ModuleSpecifier, Result<ModuleSource, AnyError>>;

pub(crate) struct CheckOptions {
  /// Set the debug flag on the TypeScript type checker.
  pub debug: bool,
  pub maybe_config_specifier: Option<ModuleSpecifier>,
  pub ts_config: TsConfig,
}

#[derive(Debug, Default)]
pub(crate) struct CheckEmitResult {
  pub diagnostics: Diagnostics,
  pub stats: crate::module_graph::Stats,
}

pub(crate) enum ConfigType {
  Bundle,
  Check { emit: bool, lib: TypeLib },
  Emit,
}

/// For a given configuration type and optionally a configuration file, return a
/// tuple of the resulting `TsConfig` struct and optionally any user
/// configuration options that were ignored.
pub(crate) fn get_ts_config(
  config_type: ConfigType,
  maybe_config_file: &Option<ConfigFile>,
) -> Result<(TsConfig, Option<IgnoredCompilerOptions>), AnyError> {
  let mut ts_config = match config_type {
    ConfigType::Bundle => TsConfig::new(json!({
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": false,
      "inlineSources": false,
      "sourceMap": false,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
    })),
    ConfigType::Check { emit, lib } => {
      let mut ts_config = TsConfig::new(json!({
        "allowJs": true,
        "experimentalDecorators": true,
        "incremental": true,
        "jsx": "react",
        "isolatedModules": true,
        "lib": lib,
        "module": "esnext",
        "strict": true,
        "target": "esnext",
        "tsBuildInfoFile": "deno:///.tsbuildinfo",
        "useDefineForClassFields": true,
        // TODO(@kitsonk) remove for Deno 2.0
        "useUnknownInCatchVariables": false,
      }));
      if emit {
        ts_config.merge(&json!({
          "emitDecoratorMetadata": false,
          "importsNotUsedAsValues": "remove",
          "inlineSourceMap": true,
          "inlineSources": true,
          "outDir": "deno://",
          "removeComments": true,
        }));
      } else {
        ts_config.merge(&json!({
          "noEmit": true,
        }));
      }
      ts_config
    }
    ConfigType::Emit => TsConfig::new(json!({
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": true,
      // TODO(@kitsonk) make this actually work when https://github.com/swc-project/swc/issues/2218 addressed.
      "inlineSources": true,
      "sourceMap": false,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
    })),
  };
  let maybe_ignored_options =
    ts_config.merge_tsconfig_from_config_file(maybe_config_file)?;
  Ok((ts_config, maybe_ignored_options))
}

/// Transform the graph into root specifiers that we can feed `tsc`. We have to
/// provide the media type for root modules because `tsc` does not "resolve" the
/// media type like other modules, as well as a root specifier needs any
/// redirects resolved. If we aren't checking JavaScript, we need to include all
/// the emittable files in the roots, so they get type checked and optionally
/// emitted, otherwise they would be ignored if only imported into JavaScript.
fn get_root_names(
  graph: &ModuleGraph,
  check_js: bool,
) -> Vec<(ModuleSpecifier, MediaType)> {
  if !check_js {
    graph
      .specifiers()
      .into_iter()
      .filter_map(|(_, r)| match r {
        Ok((s, mt)) => match &mt {
          MediaType::TypeScript | MediaType::Tsx | MediaType::Jsx => {
            Some((s, mt))
          }
          _ => None,
        },
        _ => None,
      })
      .collect()
  } else {
    graph
      .roots
      .iter()
      .filter_map(|s| {
        graph
          .get(s)
          .map(|m| (m.specifier.clone(), m.media_type.clone()))
      })
      .collect()
  }
}

/// Given a module graph, type check the module graph and optionally emit
/// modules, updating the cache as appropriate. Emitting is determined by the
/// tsconfig supplied in the options.
pub(crate) fn check_and_maybe_emit(
  graph: Arc<ModuleGraph>,
  cache: &mut dyn Cacher,
  options: CheckOptions,
) -> Result<CheckEmitResult, AnyError> {
  let check_js = options.ts_config.get_check_js();
  let root_names = get_root_names(&graph, check_js);
  // while there might be multiple roots, we can't "merge" the build info, so we
  // try to retrieve the build info for first root, which is the most common use
  // case.
  let maybe_tsbuildinfo = cache.get_tsbuildinfo(&graph.roots[0]);
  let hash_data = vec![
    options.ts_config.as_bytes(),
    version::deno().as_bytes().to_owned(),
  ];
  let config_bytes = options.ts_config.as_bytes();

  let response = tsc::exec(tsc::Request {
    config: options.ts_config,
    debug: options.debug,
    graph: tsc::GraphOrModuleGraph::ModuleGraph(graph.clone()),
    hash_data,
    maybe_config_specifier: options.maybe_config_specifier,
    maybe_tsbuildinfo,
    root_names,
  })?;

  if let Some(info) = &response.maybe_tsbuildinfo {
    // while we retrieve the build info for just the first module, it can be
    // used for all the roots in the graph, so we will cache it for all roots
    for root in &graph.roots {
      cache.set_tsbuildinfo(root, info.clone())?;
    }
  }
  if response.diagnostics.is_empty() && !response.emitted_files.is_empty() {
    for emit in &response.emitted_files {
      if let Some(specifiers) = &emit.maybe_specifiers {
        assert!(specifiers.len() == 1);
        // The emitted specifier might not be the file specifier we want, so we
        // resolve it via the graph.
        let specifier = graph.resolve(&specifiers[0]);
        let (media_type, source) = if let Some(module) = graph.get(&specifier) {
          (&module.media_type, module.source.clone())
        } else {
          log::debug!("module missing, skipping emit for {}", specifier);
          continue;
        };
        // Sometimes if `tsc` sees a CommonJS file it will _helpfully_ output it
        // to ESM, which we don't really want to do unless someone has enabled
        // check_js.
        if !check_js && *media_type == MediaType::JavaScript {
          log::debug!("skipping emit for {}", specifier);
          continue;
        }
        match emit.media_type {
          MediaType::JavaScript => {
            let version = get_version(source.as_bytes(), &config_bytes);
            cache.set_version(&specifier, version)?;
            cache.set_emit(&specifier, emit.data.clone())?;
          }
          MediaType::SourceMap => {
            cache.set_map(&specifier, emit.data.clone())?
          }
          _ => unreachable!(),
        }
      }
    }
  }

  Ok(CheckEmitResult {
    diagnostics: response.diagnostics,
    stats: response.stats,
  })
}

pub(crate) enum BundleType {
  /// Return the emitted contents of the program as a single "flattened" ES
  /// module.
  Module,
  /// Return the emitted contents of the program as a single script that
  /// executes the program using an immediately invoked function execution
  /// (IIFE).
  Classic,
  /// Do not bundle the emit, instead returning each of the modules that are
  /// part of the program as individual files.
  None,
}

impl From<BundleType> for swc::bundler::ModuleType {
  fn from(bundle_type: BundleType) -> Self {
    match bundle_type {
      BundleType::Classic => Self::Iife,
      BundleType::Module => Self::Es,
      _ => unreachable!("invalid bundle type"),
    }
  }
}

pub(crate) struct BundleOptions {
  pub bundle_type: BundleType,
  pub ts_config: TsConfig,
}

struct BundleLoader<'a> {
  cm: Rc<swc::common::SourceMap>,
  emit_options: &'a ast::EmitOptions,
  globals: &'a deno_ast::swc::common::Globals,
  graph: &'a ModuleGraph,
}

impl swc::bundler::Load for BundleLoader<'_> {
  fn load(
    &self,
    file_name: &swc::common::FileName,
  ) -> Result<swc::bundler::ModuleData, AnyError> {
    match file_name {
      swc::common::FileName::Url(specifier) => {
        if let Some(m) = self.graph.get(specifier) {
          let (fm, module) = ast::transpile_module(
            specifier,
            &m.source,
            m.media_type.clone(),
            self.emit_options,
            self.globals,
            self.cm.clone(),
          )?;
          Ok(swc::bundler::ModuleData {
            fm,
            module,
            helpers: Default::default(),
          })
        } else {
          Err(anyhow!(
            "Module \"{}\" unexpectedly missing when bundling.",
            specifier
          ))
        }
      }
      _ => unreachable!(
        "Received a request for unsupported filename {:?}",
        file_name
      ),
    }
  }
}

struct BundleResolver<'a>(&'a ModuleGraph);

impl swc::bundler::Resolve for BundleResolver<'_> {
  fn resolve(
    &self,
    referrer: &swc::common::FileName,
    specifier: &str,
  ) -> Result<swc::common::FileName, AnyError> {
    let referrer = if let swc::common::FileName::Url(referrer) = referrer {
      referrer
    } else {
      unreachable!(
        "An unexpected referrer was passed when bundling: {:?}",
        referrer
      );
    };
    if let Some(specifier) =
      self.0.resolve_dependency(specifier, referrer, false)
    {
      Ok(deno_ast::swc::common::FileName::Url(specifier.clone()))
    } else {
      Err(anyhow!(
        "Cannot resolve \"{}\" from \"{}\".",
        specifier,
        referrer
      ))
    }
  }
}

/// Given a module graph, generate and return a bundle of the graph.
pub(crate) fn bundle(
  graph: &ModuleGraph,
  options: BundleOptions,
) -> Result<(String, Option<String>), AnyError> {
  let start = Instant::now();

  let emit_options: ast::EmitOptions = options.ts_config.into();

  let cm = Rc::new(swc::common::SourceMap::new(
    swc::common::FilePathMapping::empty(),
  ));
  let globals = swc::common::Globals::new();
  let loader = BundleLoader {
    graph,
    emit_options: &emit_options,
    globals: &globals,
    cm: cm.clone(),
  };
  let resolver = BundleResolver(graph);
  let config = swc::bundler::Config {
    module: options.bundle_type.into(),
    ..Default::default()
  };
  let hook = Box::new(ast::BundleHook);
  let bundler = swc::bundler::Bundler::new(
    &globals,
    cm.clone(),
    loader,
    resolver,
    config,
    hook,
  );
  let mut entries = HashMap::new();
  entries.insert(
    "bundle".to_string(),
    swc::common::FileName::Url(graph.roots[0].clone()),
  );
  let output = bundler
    .bundle(entries)
    .context("Unable to output during bundling.")?;
  let mut buf = Vec::new();
  let mut srcmap = Vec::new();
  {
    let cfg = swc::codegen::Config { minify: false };
    let wr = Box::new(swc::codegen::text_writer::JsWriter::new(
      cm.clone(),
      "\n",
      &mut buf,
      Some(&mut srcmap),
    ));
    let mut emitter = swc::codegen::Emitter {
      cfg,
      cm: cm.clone(),
      comments: None,
      wr,
    };
    emitter
      .emit_module(&output[0].module)
      .context("Unable to emit during bundling.")?;
  }
  let mut code =
    String::from_utf8(buf).context("Emitted code is an invalid string.")?;
  let mut maybe_map: Option<String> = None;
  {
    let mut buf = Vec::new();
    cm.build_source_map_from(&mut srcmap, None)
      .to_writer(&mut buf)?;
    if emit_options.inline_source_map {
      let encoded_map = format!(
        "//# sourceMappingURL=data:application/json;base64,{}\n",
        base64::encode(buf)
      );
      code.push_str(&encoded_map);
    } else if emit_options.source_map {
      maybe_map = Some(String::from_utf8(buf)?);
    }
  }

  Ok((code, maybe_map))
}

// pub(crate) fn deno_emit(graph: &ModuleGraph, cache: &mut dyn Cacher, options: DenoEmitOptions) -> Result<DenoEmitResult, AnyError> {}

pub(crate) struct EmitOptions {
  pub ts_config: TsConfig,
  pub reload_exclusions: HashSet<ModuleSpecifier>,
  pub reload: bool,
}

/// Determine if a given media type is emittable or not.
fn is_emittable(media_type: &MediaType, include_js: bool) -> bool {
  match &media_type {
    MediaType::TypeScript | MediaType::Tsx | MediaType::Jsx => true,
    MediaType::JavaScript => include_js,
    _ => false,
  }
}

/// Given a module graph, emit any appropriate modules and cache them.
pub(crate) fn emit(
  graph: Arc<ModuleGraph>,
  cache: &mut dyn Cacher,
  options: EmitOptions,
) -> Result<CheckEmitResult, AnyError> {
  let start = Instant::now();
  let config_bytes = options.ts_config.as_bytes();
  let include_js = options.ts_config.get_check_js();
  let emit_options: ast::EmitOptions = options.ts_config.into();
  let mut emit_count = 0_u32;
  let mut file_count = 0_u32;
  for module in graph.modules() {
    file_count += 1;
    if !is_emittable(&module.media_type, include_js) {
      continue;
    }
    let needs_reload =
      options.reload && !options.reload_exclusions.contains(&module.specifier);
    let version = get_version(module.source.as_bytes(), &config_bytes);
    let is_valid = cache.get_version(&module.specifier).map_or(false, |v| {
      v == get_version(module.source.as_bytes(), &config_bytes)
    });
    if is_valid && !needs_reload {
      continue;
    }
    let (emit, maybe_map) =
      ast::transpile(&module.parsed_source, &emit_options)?;
    emit_count += 1;
    cache.set_emit(&module.specifier, emit)?;
    if let Some(map) = maybe_map {
      cache.set_map(&module.specifier, map)?;
    }
    if !is_valid {
      cache.set_version(&module.specifier, version)?;
    }
  }

  let stats = crate::module_graph::Stats(vec![
    ("Files".to_string(), file_count),
    ("Emitted".to_string(), emit_count),
    ("Total time".to_string(), start.elapsed().as_millis() as u32),
  ]);

  Ok(CheckEmitResult {
    diagnostics: Diagnostics::default(),
    stats,
  })
}

/// A hashing function that takes the source code, version and optionally a
/// user provided config and generates a string hash which can be stored to
/// determine if the cached emit is valid or not.
fn get_version(source_bytes: &[u8], config_bytes: &[u8]) -> String {
  crate::checksum::gen(&[
    source_bytes,
    version::deno().as_bytes(),
    config_bytes,
  ])
}

/// Check a module graph to determine if the graph contains anything that
/// is required to be emitted to be valid. It determines what modules in the
/// graph are emittable and for those that are emittable, if there is currently
/// a valid emit in the cache.
pub(crate) fn valid_emit(
  graph: Arc<ModuleGraph>,
  cache: &dyn Cacher,
  ts_config: &TsConfig,
  reload: bool,
  reload_exclusions: &HashSet<ModuleSpecifier>,
) -> bool {
  if reload && reload_exclusions.is_empty() {
    return false;
  }
  let config_bytes = ts_config.as_bytes();
  let emit_js = ts_config.get_check_js();
  graph
    .specifiers()
    .iter()
    .filter(|(_, r)| match r {
      Ok((_, MediaType::TypeScript))
      | Ok((_, MediaType::Tsx))
      | Ok((_, MediaType::Jsx)) => true,
      Ok((_, MediaType::JavaScript)) => emit_js,
      _ => false,
    })
    .all(|(_, r)| {
      if let Ok((s, _)) = r {
        if reload && !reload_exclusions.contains(s) {
          // we are reloading and the specifier isn't excluded from being
          // reloaded
          false
        } else if let Some(version) = cache.get_version(s) {
          if let Some(module) = graph.get(s) {
            version == get_version(module.source.as_bytes(), &config_bytes)
          } else {
            // We have a source module in the graph we can't find, so the emit is
            // clearly wrong
            false
          }
        } else {
          // A module that requires emitting doesn't have a version, so it doesn't
          // have a valid emit
          false
        }
      } else {
        // Something in the module graph is missing, but that doesn't mean the
        // emit is invalid
        true
      }
    })
}

/// Convert a module graph to a map of module sources, which are used by
/// `deno_core` to load modules into V8.
pub(crate) fn to_module_sources(
  graph: Arc<ModuleGraph>,
  cache: &dyn Cacher,
) -> Modules {
  graph
    .specifiers()
    .into_iter()
    .map(|(rs, r)| {
      let result = r.map_or_else(
        |err| Err(err.into()),
        |(fs, mt)| {
          if let Some(code) = cache.get_emit(&fs) {
            Ok(ModuleSource {
              code,
              module_url_found: fs.to_string(),
              module_url_specified: rs.to_string(),
            })
          } else if mt == MediaType::JavaScript || mt == MediaType::Unknown {
            if let Some(module) = graph.get(&fs) {
              Ok(ModuleSource {
                code: module.source.as_str().to_string(),
                module_url_found: module.specifier.to_string(),
                module_url_specified: rs.to_string(),
              })
            } else {
              Err(anyhow!("Emitted module \"{}\" not found.", rs))
            }
          } else {
            Err(anyhow!("Emmitted module \"{}\" not found.", rs))
          }
        },
      );
      (rs, result)
    })
    .collect()
}
