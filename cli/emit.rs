// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! The collection of APIs to be able to take `deno_graph` module graphs and
//! populate a cache, emit files, and transform a graph into the structures for
//! loading into an isolate.

use crate::cache::CacheType;
use crate::cache::Cacher;
use crate::colors;
use crate::config_file;
use crate::config_file::ConfigFile;
use crate::config_file::IgnoredCompilerOptions;
use crate::config_file::TsConfig;
use crate::diagnostics::Diagnostics;
use crate::flags;
use crate::graph_util::GraphData;
use crate::graph_util::ModuleEntry;
use crate::text_encoding::strip_bom;
use crate::tsc;
use crate::version;

use deno_ast::get_syntax;
use deno_ast::swc;
use deno_ast::swc::bundler::Hook;
use deno_ast::swc::bundler::ModuleRecord;
use deno_ast::swc::common::comments::SingleThreadedComments;
use deno_ast::swc::common::FileName;
use deno_ast::swc::common::Mark;
use deno_ast::swc::common::SourceMap;
use deno_ast::swc::common::Span;
use deno_ast::swc::common::Spanned;
use deno_ast::swc::parser::error::Error as SwcError;
use deno_ast::swc::parser::lexer::Lexer;
use deno_ast::swc::parser::StringInput;
use deno_ast::Diagnostic;
use deno_ast::LineAndColumnDisplay;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ModuleSpecifier;
use deno_graph::MediaType;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleKind;
use deno_graph::ResolutionError;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;
use std::result;
use std::sync::Arc;
use std::time::Instant;

const IGNORE_DIRECTIVES: &[&str] = &[
  "// deno-fmt-ignore-file",
  "// deno-lint-ignore-file",
  "// This code was bundled using `deno bundle` and it's not recommended to edit it manually",
  ""
];

/// Represents the "default" type library that should be used when type
/// checking the code in the module graph.  Note that a user provided config
/// of `"lib"` would override this value.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
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

/// A structure representing stats from an emit operation for a graph.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Stats(pub Vec<(String, u32)>);

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

/// An enum that represents the base tsc configuration to return.
pub(crate) enum ConfigType {
  /// Return a configuration for bundling, using swc to emit the bundle. This is
  /// independent of type checking.
  Bundle,
  /// Return a configuration to use tsc to type check and optionally emit. This
  /// is independent of either bundling or just emitting via swc
  Check { lib: TypeLib, tsc_emit: bool },
  /// Return a configuration to use swc to emit single module files.
  Emit,
  /// Return a configuration as a base for the runtime `Deno.emit()` API.
  RuntimeEmit { tsc_emit: bool },
}

/// For a given configuration type and optionally a configuration file, return a
/// tuple of the resulting `TsConfig` struct and optionally any user
/// configuration options that were ignored.
pub(crate) fn get_ts_config(
  config_type: ConfigType,
  maybe_config_file: Option<&ConfigFile>,
  maybe_user_config: Option<&HashMap<String, Value>>,
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
    ConfigType::Check { tsc_emit, lib } => {
      let mut ts_config = TsConfig::new(json!({
        "allowJs": true,
        "allowSyntheticDefaultImports": true,
        "experimentalDecorators": true,
        "incremental": true,
        "jsx": "react",
        "isolatedModules": true,
        "lib": lib,
        "module": "esnext",
        "resolveJsonModule": true,
        "strict": true,
        "target": "esnext",
        "tsBuildInfoFile": "deno:///.tsbuildinfo",
        "useDefineForClassFields": true,
        // TODO(@kitsonk) remove for Deno 2.0
        "useUnknownInCatchVariables": false,
      }));
      if tsc_emit {
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
      "inlineSources": true,
      "sourceMap": false,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "resolveJsonModule": true,
    })),
    ConfigType::RuntimeEmit { tsc_emit } => {
      let mut ts_config = TsConfig::new(json!({
        "allowJs": true,
        "allowSyntheticDefaultImports": true,
        "checkJs": false,
        "emitDecoratorMetadata": false,
        "experimentalDecorators": true,
        "importsNotUsedAsValues": "remove",
        "incremental": true,
        "isolatedModules": true,
        "jsx": "react",
        "jsxFactory": "React.createElement",
        "jsxFragmentFactory": "React.Fragment",
        "lib": TypeLib::DenoWindow,
        "module": "esnext",
        "removeComments": true,
        "inlineSourceMap": false,
        "inlineSources": false,
        "sourceMap": true,
        "strict": true,
        "target": "esnext",
        "tsBuildInfoFile": "deno:///.tsbuildinfo",
        "useDefineForClassFields": true,
        // TODO(@kitsonk) remove for Deno 2.0
        "useUnknownInCatchVariables": false,
      }));
      if tsc_emit {
        ts_config.merge(&json!({
          "importsNotUsedAsValues": "remove",
          "outDir": "deno://",
        }));
      } else {
        ts_config.merge(&json!({
          "noEmit": true,
        }));
      }
      ts_config
    }
  };
  let maybe_ignored_options = if let Some(user_options) = maybe_user_config {
    ts_config.merge_user_config(user_options)?
  } else {
    ts_config.merge_tsconfig_from_config_file(maybe_config_file)?
  };
  Ok((ts_config, maybe_ignored_options))
}

/// Transform the graph into root specifiers that we can feed `tsc`. We have to
/// provide the media type for root modules because `tsc` does not "resolve" the
/// media type like other modules, as well as a root specifier needs any
/// redirects resolved. If we aren't checking JavaScript, we need to include all
/// the emittable files in the roots, so they get type checked and optionally
/// emitted, otherwise they would be ignored if only imported into JavaScript.
fn get_tsc_roots(
  roots: &[(ModuleSpecifier, ModuleKind)],
  graph_data: &GraphData,
  check_js: bool,
) -> Vec<(ModuleSpecifier, MediaType)> {
  if !check_js {
    graph_data
      .entries()
      .into_iter()
      .filter_map(|(specifier, module_entry)| match module_entry {
        ModuleEntry::Module { media_type, .. } => match &media_type {
          MediaType::TypeScript
          | MediaType::Tsx
          | MediaType::Mts
          | MediaType::Cts
          | MediaType::Jsx => Some((specifier.clone(), *media_type)),
          _ => None,
        },
        _ => None,
      })
      .collect()
  } else {
    roots
      .iter()
      .filter_map(|(specifier, _)| match graph_data.get(specifier) {
        Some(ModuleEntry::Module { media_type, .. }) => {
          Some((specifier.clone(), *media_type))
        }
        _ => None,
      })
      .collect()
  }
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

/// Determine if a given module kind and media type is emittable or not.
pub(crate) fn is_emittable(
  kind: &ModuleKind,
  media_type: &MediaType,
  include_js: bool,
) -> bool {
  if matches!(kind, ModuleKind::Synthetic) {
    return false;
  }
  match &media_type {
    MediaType::TypeScript
    | MediaType::Mts
    | MediaType::Cts
    | MediaType::Tsx
    | MediaType::Jsx => true,
    MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => include_js,
    _ => false,
  }
}

/// Options for performing a check of a module graph. Note that the decision to
/// emit or not is determined by the `ts_config` settings.
pub(crate) struct CheckOptions {
  /// The check flag from the option which can effect the filtering of
  /// diagnostics in the emit result.
  pub check: flags::CheckFlag,
  /// Set the debug flag on the TypeScript type checker.
  pub debug: bool,
  /// If true, any files emitted will be cached, even if there are diagnostics
  /// produced. If false, if there are diagnostics, caching emitted files will
  /// be skipped.
  pub emit_with_diagnostics: bool,
  /// The module specifier to the configuration file, passed to tsc so that
  /// configuration related diagnostics are properly formed.
  pub maybe_config_specifier: Option<ModuleSpecifier>,
  /// The derived tsconfig that should be used when checking.
  pub ts_config: TsConfig,
  /// If true, `Check <specifier>` will be written to stdout for each root.
  pub log_checks: bool,
  /// If true, valid existing emits and `.tsbuildinfo` files will be ignored.
  pub reload: bool,
  pub reload_exclusions: HashSet<ModuleSpecifier>,
}

/// The result of a check or emit of a module graph. Note that the actual
/// emitted sources are stored in the cache and are not returned in the result.
#[derive(Debug, Default)]
pub(crate) struct CheckEmitResult {
  pub diagnostics: Diagnostics,
  pub stats: Stats,
}

/// Given a set of roots and graph data, type check the module graph and
/// optionally emit modules, updating the cache as appropriate. Emitting is
/// determined by the `ts_config` supplied in the options, and if emitting, the
/// files are stored in the cache.
///
/// It is expected that it is determined if a check and/or emit is validated
/// before the function is called.
pub(crate) fn check_and_maybe_emit(
  roots: &[(ModuleSpecifier, ModuleKind)],
  graph_data: Arc<RwLock<GraphData>>,
  cache: &mut dyn Cacher,
  options: CheckOptions,
) -> Result<CheckEmitResult, AnyError> {
  let check_js = options.ts_config.get_check_js();
  let segment_graph_data = {
    let graph_data = graph_data.read();
    graph_data.graph_segment(roots).unwrap()
  };
  if valid_emit(
    &segment_graph_data,
    cache,
    &options.ts_config,
    options.reload,
    &options.reload_exclusions,
  ) {
    return Ok(Default::default());
  }
  let root_names = get_tsc_roots(roots, &segment_graph_data, check_js);
  if options.log_checks {
    for (root, _) in roots {
      let root_str = root.to_string();
      // `$deno` specifiers are internal, don't print them.
      if !root_str.contains("$deno") {
        log::info!("{} {}", colors::green("Check"), root);
      }
    }
  }
  // while there might be multiple roots, we can't "merge" the build info, so we
  // try to retrieve the build info for first root, which is the most common use
  // case.
  let maybe_tsbuildinfo = if options.reload {
    None
  } else {
    cache.get(CacheType::TypeScriptBuildInfo, &roots[0].0)
  };
  // to make tsc build info work, we need to consistently hash modules, so that
  // tsc can better determine if an emit is still valid or not, so we provide
  // that data here.
  let hash_data = vec![
    options.ts_config.as_bytes(),
    version::deno().as_bytes().to_owned(),
  ];
  let config_bytes = options.ts_config.as_bytes();

  let response = tsc::exec(tsc::Request {
    config: options.ts_config,
    debug: options.debug,
    graph_data: graph_data.clone(),
    hash_data,
    maybe_config_specifier: options.maybe_config_specifier,
    maybe_tsbuildinfo,
    root_names,
  })?;

  let diagnostics = if options.check == flags::CheckFlag::Local {
    response.diagnostics.filter(|d| {
      if let Some(file_name) = &d.file_name {
        !file_name.starts_with("http")
      } else {
        true
      }
    })
  } else {
    response.diagnostics
  };

  // sometimes we want to emit when there are diagnostics, and sometimes we
  // don't. tsc will always return an emit if there are diagnostics
  if (diagnostics.is_empty() || options.emit_with_diagnostics)
    && !response.emitted_files.is_empty()
  {
    if let Some(info) = &response.maybe_tsbuildinfo {
      // while we retrieve the build info for just the first module, it can be
      // used for all the roots in the graph, so we will cache it for all roots
      for (root, _) in roots {
        cache.set(CacheType::TypeScriptBuildInfo, root, info.clone())?;
      }
    }
    for emit in response.emitted_files.into_iter() {
      if let Some(specifiers) = emit.maybe_specifiers {
        assert!(specifiers.len() == 1);
        // The emitted specifier might not be the file specifier we want, so we
        // resolve it via the graph.
        let graph_data = graph_data.read();
        let specifier = graph_data.follow_redirect(&specifiers[0]);
        let (source_bytes, media_type) = match graph_data.get(&specifier) {
          Some(ModuleEntry::Module {
            code, media_type, ..
          }) => (code.as_bytes(), *media_type),
          _ => {
            log::debug!("skipping emit for {}", specifier);
            continue;
          }
        };
        // Sometimes if `tsc` sees a CommonJS file or a JSON module, it will
        // _helpfully_ output it, which we don't really want to do unless
        // someone has enabled check_js.
        if matches!(media_type, MediaType::Json)
          || (!check_js
            && matches!(
              media_type,
              MediaType::JavaScript | MediaType::Cjs | MediaType::Mjs
            ))
        {
          log::debug!("skipping emit for {}", specifier);
          continue;
        }
        match emit.media_type {
          MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
            let version = get_version(source_bytes, &config_bytes);
            cache.set(CacheType::Version, &specifier, version)?;
            cache.set(CacheType::Emit, &specifier, emit.data)?;
          }
          MediaType::SourceMap => {
            cache.set(CacheType::SourceMap, &specifier, emit.data)?;
          }
          // this only occurs with the runtime emit, but we are using the same
          // code paths, so we handle it here.
          MediaType::Dts | MediaType::Dcts | MediaType::Dmts => {
            cache.set(CacheType::Declaration, &specifier, emit.data)?;
          }
          _ => unreachable!(
            "unexpected media_type {} {}",
            emit.media_type, specifier
          ),
        }
      }
    }
  }

  Ok(CheckEmitResult {
    diagnostics,
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
}

impl From<BundleType> for swc::bundler::ModuleType {
  fn from(bundle_type: BundleType) -> Self {
    match bundle_type {
      BundleType::Classic => Self::Iife,
      BundleType::Module => Self::Es,
    }
  }
}

pub(crate) struct BundleOptions {
  pub bundle_type: BundleType,
  pub ts_config: TsConfig,
  pub emit_ignore_directives: bool,
}

/// A module loader for swc which does the appropriate retrieval and transpiling
/// of modules from the graph.
struct BundleLoader<'a> {
  cm: Rc<swc::common::SourceMap>,
  emit_options: &'a deno_ast::EmitOptions,
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
          let (fm, module) = transpile_module(
            specifier,
            m.maybe_source.as_ref().map(|s| s.as_str()).unwrap_or(""),
            m.media_type,
            self.emit_options,
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

/// Transpiles a source module into an swc SourceFile.
fn transpile_module(
  specifier: &ModuleSpecifier,
  source: &str,
  media_type: MediaType,
  options: &deno_ast::EmitOptions,
  cm: Rc<swc::common::SourceMap>,
) -> Result<(Rc<swc::common::SourceFile>, swc::ast::Module), AnyError> {
  let source = strip_bom(source);
  let source = if media_type == MediaType::Json {
    format!(
      "export default JSON.parse(`{}`);",
      source.replace("${", "\\${").replace('`', "\\`")
    )
  } else {
    source.to_string()
  };
  let source_file =
    cm.new_source_file(FileName::Url(specifier.clone()), source);
  let input = StringInput::from(&*source_file);
  let comments = SingleThreadedComments::default();
  let syntax = if media_type == MediaType::Json {
    get_syntax(MediaType::JavaScript)
  } else {
    get_syntax(media_type)
  };
  let lexer = Lexer::new(syntax, deno_ast::ES_VERSION, input, Some(&comments));
  let mut parser = swc::parser::Parser::new_from(lexer);
  let module = parser
    .parse_module()
    .map_err(|e| swc_err_to_diagnostic(&cm, specifier, e))?;
  let diagnostics = parser
    .take_errors()
    .into_iter()
    .map(|e| swc_err_to_diagnostic(&cm, specifier, e))
    .collect::<Vec<_>>();

  let top_level_mark = Mark::fresh(Mark::root());
  let program = deno_ast::fold_program(
    swc::ast::Program::Module(module),
    options,
    cm,
    &comments,
    top_level_mark,
    &diagnostics,
  )?;
  let module = match program {
    swc::ast::Program::Module(module) => module,
    _ => unreachable!(),
  };

  Ok((source_file, module))
}

fn swc_err_to_diagnostic(
  source_map: &SourceMap,
  specifier: &ModuleSpecifier,
  err: SwcError,
) -> Diagnostic {
  let location = source_map.lookup_char_pos(err.span().lo);
  Diagnostic {
    specifier: specifier.to_string(),
    span: err.span(),
    display_position: LineAndColumnDisplay {
      line_number: location.line,
      column_number: location.col_display + 1,
    },
    kind: err.into_kind(),
  }
}

/// A resolver implementation for swc that resolves specifiers from the graph.
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

/// Given a module graph, generate and return a bundle of the graph and
/// optionally its source map. Unlike emitting with `check_and_maybe_emit` and
/// `emit`, which store the emitted modules in the cache, this function simply
/// returns the output.
pub(crate) fn bundle(
  graph: &ModuleGraph,
  options: BundleOptions,
) -> Result<(String, Option<String>), AnyError> {
  let globals = swc::common::Globals::new();
  deno_ast::swc::common::GLOBALS.set(&globals, || {
    let emit_options: deno_ast::EmitOptions = options.ts_config.into();
    let source_map_config = deno_ast::SourceMapConfig {
      inline_sources: emit_options.inline_sources,
    };

    let cm = Rc::new(swc::common::SourceMap::new(
      swc::common::FilePathMapping::empty(),
    ));
    let loader = BundleLoader {
      graph,
      emit_options: &emit_options,
      cm: cm.clone(),
    };
    let resolver = BundleResolver(graph);
    let config = swc::bundler::Config {
      module: options.bundle_type.into(),
      ..Default::default()
    };
    // This hook will rewrite the `import.meta` when bundling to give a consistent
    // behavior between bundled and unbundled code.
    let hook = Box::new(BundleHook);
    let mut bundler = swc::bundler::Bundler::new(
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
      swc::common::FileName::Url(graph.roots[0].0.clone()),
    );
    let output = bundler
      .bundle(entries)
      .context("Unable to output during bundling.")?;
    let mut buf = Vec::new();
    let mut srcmap = Vec::new();
    {
      let cfg = swc::codegen::Config { minify: false };
      let mut wr = Box::new(swc::codegen::text_writer::JsWriter::new(
        cm.clone(),
        "\n",
        &mut buf,
        Some(&mut srcmap),
      ));

      if options.emit_ignore_directives {
        // write leading comments in bundled file
        use swc::codegen::text_writer::WriteJs;
        use swc::common::source_map::DUMMY_SP;
        let cmt = IGNORE_DIRECTIVES.join("\n") + "\n";
        wr.write_comment(DUMMY_SP, &cmt)?;
      }

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
      cm.build_source_map_with_config(&mut srcmap, None, source_map_config)
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
  })
}

pub(crate) struct EmitOptions {
  pub ts_config: TsConfig,
  pub reload: bool,
  pub reload_exclusions: HashSet<ModuleSpecifier>,
}

/// Given a module graph, emit any appropriate modules and cache them.
// TODO(nayeemrmn): This would ideally take `GraphData` like
// `check_and_maybe_emit()`, but the AST isn't stored in that. Cleanup.
pub(crate) fn emit(
  graph: &ModuleGraph,
  cache: &mut dyn Cacher,
  options: EmitOptions,
) -> Result<CheckEmitResult, AnyError> {
  let start = Instant::now();
  let config_bytes = options.ts_config.as_bytes();
  let include_js = options.ts_config.get_check_js();
  let emit_options = options.ts_config.into();

  let mut emit_count = 0_u32;
  let mut file_count = 0_u32;
  for module in graph.modules() {
    file_count += 1;
    if !is_emittable(&module.kind, &module.media_type, include_js) {
      continue;
    }
    let needs_reload =
      options.reload && !options.reload_exclusions.contains(&module.specifier);
    let version = get_version(
      module.maybe_source.as_ref().map(|s| s.as_bytes()).unwrap(),
      &config_bytes,
    );
    let is_valid =
      cache
        .get(CacheType::Version, &module.specifier)
        .map_or(false, |v| {
          v == get_version(
            module.maybe_source.as_ref().map(|s| s.as_bytes()).unwrap(),
            &config_bytes,
          )
        });
    if is_valid && !needs_reload {
      continue;
    }
    let transpiled_source = module
      .maybe_parsed_source
      .as_ref()
      .map(|ps| ps.transpile(&emit_options))
      .unwrap()?;
    emit_count += 1;
    cache.set(CacheType::Emit, &module.specifier, transpiled_source.text)?;
    if let Some(map) = transpiled_source.source_map {
      cache.set(CacheType::SourceMap, &module.specifier, map)?;
    }
    if !is_valid {
      cache.set(CacheType::Version, &module.specifier, version)?;
    }
  }

  let stats = Stats(vec![
    ("Files".to_string(), file_count),
    ("Emitted".to_string(), emit_count),
    ("Total time".to_string(), start.elapsed().as_millis() as u32),
  ]);

  Ok(CheckEmitResult {
    diagnostics: Diagnostics::default(),
    stats,
  })
}

/// Check a module graph to determine if the graph contains anything that
/// is required to be emitted to be valid. It determines what modules in the
/// graph are emittable and for those that are emittable, if there is currently
/// a valid emit in the cache.
fn valid_emit(
  graph_data: &GraphData,
  cache: &dyn Cacher,
  ts_config: &TsConfig,
  reload: bool,
  reload_exclusions: &HashSet<ModuleSpecifier>,
) -> bool {
  let config_bytes = ts_config.as_bytes();
  let emit_js = ts_config.get_check_js();
  for (specifier, module_entry) in graph_data.entries() {
    if let ModuleEntry::Module {
      code, media_type, ..
    } = module_entry
    {
      match media_type {
        MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Tsx
        | MediaType::Jsx => {}
        MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
          if !emit_js {
            continue;
          }
        }
        _ => continue,
      }
      if reload && !reload_exclusions.contains(specifier) {
        return false;
      }
      if let Some(version) = cache.get(CacheType::Version, specifier) {
        if version != get_version(code.as_bytes(), &config_bytes) {
          return false;
        }
      } else {
        return false;
      }
    }
  }
  true
}

/// An adapter struct to make a deno_graph::ModuleGraphError display as expected
/// in the Deno CLI.
#[derive(Debug)]
pub(crate) struct GraphError(pub ModuleGraphError);

impl std::error::Error for GraphError {}

impl From<ModuleGraphError> for GraphError {
  fn from(err: ModuleGraphError) -> Self {
    Self(err)
  }
}

impl fmt::Display for GraphError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.0 {
      ModuleGraphError::ResolutionError(err) => {
        if matches!(
          err,
          ResolutionError::InvalidDowngrade { .. }
            | ResolutionError::InvalidLocalImport { .. }
        ) {
          write!(f, "{}", err.to_string_with_range())
        } else {
          self.0.fmt(f)
        }
      }
      _ => self.0.fmt(f),
    }
  }
}

/// Convert a module graph to a map of "files", which are used by the runtime
/// emit to be passed back to the caller.
pub(crate) fn to_file_map(
  graph: &ModuleGraph,
  cache: &dyn Cacher,
) -> HashMap<String, String> {
  let mut files = HashMap::new();
  for (_, result) in graph.specifiers().into_iter() {
    if let Ok((specifier, _, media_type)) = result {
      if let Some(emit) = cache.get(CacheType::Emit, &specifier) {
        files.insert(format!("{}.js", specifier), emit);
        if let Some(map) = cache.get(CacheType::SourceMap, &specifier) {
          files.insert(format!("{}.js.map", specifier), map);
        }
      } else if matches!(
        media_type,
        MediaType::JavaScript
          | MediaType::Mjs
          | MediaType::Cjs
          | MediaType::Json
          | MediaType::Unknown
      ) {
        if let Some(module) = graph.get(&specifier) {
          files.insert(
            specifier.to_string(),
            module
              .maybe_source
              .as_ref()
              .map(|s| s.to_string())
              .unwrap_or_else(|| "".to_string()),
          );
        }
      }
      if let Some(declaration) = cache.get(CacheType::Declaration, &specifier) {
        files.insert(format!("{}.d.ts", specifier), declaration);
      }
    }
  }
  files
}

/// This contains the logic for Deno to rewrite the `import.meta` when bundling.
pub struct BundleHook;

impl Hook for BundleHook {
  fn get_import_meta_props(
    &self,
    span: Span,
    module_record: &ModuleRecord,
  ) -> Result<Vec<deno_ast::swc::ast::KeyValueProp>, AnyError> {
    use deno_ast::swc::ast;

    Ok(vec![
      ast::KeyValueProp {
        key: ast::PropName::Ident(ast::Ident::new("url".into(), span)),
        value: Box::new(ast::Expr::Lit(ast::Lit::Str(ast::Str {
          span,
          value: module_record.file_name.to_string().into(),
          kind: ast::StrKind::Synthesized,
          has_escape: false,
        }))),
      },
      ast::KeyValueProp {
        key: ast::PropName::Ident(ast::Ident::new("main".into(), span)),
        value: Box::new(if module_record.is_entry {
          ast::Expr::Member(ast::MemberExpr {
            span,
            obj: Box::new(ast::Expr::MetaProp(ast::MetaPropExpr {
              span,
              kind: ast::MetaPropKind::ImportMeta,
            })),
            prop: ast::MemberProp::Ident(ast::Ident::new("main".into(), span)),
          })
        } else {
          ast::Expr::Lit(ast::Lit::Bool(ast::Bool { span, value: false }))
        }),
      },
    ])
  }
}

impl From<config_file::TsConfig> for deno_ast::EmitOptions {
  fn from(config: config_file::TsConfig) -> Self {
    let options: config_file::EmitConfigOptions =
      serde_json::from_value(config.0).unwrap();
    let imports_not_used_as_values =
      match options.imports_not_used_as_values.as_str() {
        "preserve" => deno_ast::ImportsNotUsedAsValues::Preserve,
        "error" => deno_ast::ImportsNotUsedAsValues::Error,
        _ => deno_ast::ImportsNotUsedAsValues::Remove,
      };
    let (transform_jsx, jsx_automatic, jsx_development) =
      match options.jsx.as_str() {
        "react" => (true, false, false),
        "react-jsx" => (true, true, false),
        "react-jsxdev" => (true, true, true),
        _ => (false, false, false),
      };
    deno_ast::EmitOptions {
      emit_metadata: options.emit_decorator_metadata,
      imports_not_used_as_values,
      inline_source_map: options.inline_source_map,
      inline_sources: options.inline_sources,
      source_map: options.source_map,
      jsx_automatic,
      jsx_development,
      jsx_factory: options.jsx_factory,
      jsx_fragment_factory: options.jsx_fragment_factory,
      jsx_import_source: options.jsx_import_source,
      transform_jsx,
      var_decl_imports: false,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_emittable() {
    assert!(is_emittable(
      &ModuleKind::Esm,
      &MediaType::TypeScript,
      false
    ));
    assert!(!is_emittable(
      &ModuleKind::Synthetic,
      &MediaType::TypeScript,
      false
    ));
    assert!(!is_emittable(&ModuleKind::Esm, &MediaType::Dts, false));
    assert!(!is_emittable(&ModuleKind::Esm, &MediaType::Dcts, false));
    assert!(!is_emittable(&ModuleKind::Esm, &MediaType::Dmts, false));
    assert!(is_emittable(&ModuleKind::Esm, &MediaType::Tsx, false));
    assert!(!is_emittable(
      &ModuleKind::Esm,
      &MediaType::JavaScript,
      false
    ));
    assert!(!is_emittable(&ModuleKind::Esm, &MediaType::Cjs, false));
    assert!(!is_emittable(&ModuleKind::Esm, &MediaType::Mjs, false));
    assert!(is_emittable(&ModuleKind::Esm, &MediaType::JavaScript, true));
    assert!(is_emittable(&ModuleKind::Esm, &MediaType::Jsx, false));
    assert!(!is_emittable(&ModuleKind::Esm, &MediaType::Json, false));
  }
}
