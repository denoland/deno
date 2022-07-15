// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! The collection of APIs to be able to take `deno_graph` module graphs and
//! populate a cache, emit files, and transform a graph into the structures for
//! loading into an isolate.

use crate::args::config_file::IgnoredCompilerOptions;
use crate::args::ConfigFile;
use crate::args::EmitConfigOptions;
use crate::args::TsConfig;
use crate::args::TypeCheckMode;
use crate::cache::EmitCache;
use crate::cache::SpecifierEmitCacheData;
use crate::cache::TypeCheckCache;
use crate::colors;
use crate::diagnostics::Diagnostics;
use crate::graph_util::GraphData;
use crate::graph_util::ModuleEntry;
use crate::tsc;
use crate::version;

use deno_ast::swc::bundler::Hook;
use deno_ast::swc::bundler::ModuleRecord;
use deno_ast::swc::common::Span;
use deno_core::error::AnyError;
use deno_core::parking_lot::RwLock;
use deno_core::serde::Deserialize;
use deno_core::serde::Deserializer;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_graph::MediaType;
use deno_graph::ModuleGraph;
use deno_graph::ModuleGraphError;
use deno_graph::ModuleKind;
use deno_graph::ResolutionError;
use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

/// A structure representing stats from an emit operation for a graph.
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
      writeln!(f, "  {}: {}", key, value)?;
    }

    Ok(())
  }
}

/// Represents the "default" type library that should be used when type
/// checking the code in the module graph.  Note that a user provided config
/// of `"lib"` would override this value.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum TsTypeLib {
  DenoWindow,
  DenoWorker,
  UnstableDenoWindow,
  UnstableDenoWorker,
}

impl Default for TsTypeLib {
  fn default() -> Self {
    Self::DenoWindow
  }
}

impl Serialize for TsTypeLib {
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

/// An enum that represents the base tsc configuration to return.
pub enum TsConfigType {
  /// Return a configuration for bundling, using swc to emit the bundle. This is
  /// independent of type checking.
  Bundle,
  /// Return a configuration to use tsc to type check and optionally emit. This
  /// is independent of either bundling or just emitting via swc
  Check { lib: TsTypeLib },
  /// Return a configuration to use swc to emit single module files.
  Emit,
}

pub struct TsConfigWithIgnoredOptions {
  pub ts_config: TsConfig,
  pub maybe_ignored_options: Option<IgnoredCompilerOptions>,
}

/// For a given configuration type and optionally a configuration file,
/// return a `TsConfig` struct and optionally any user configuration
/// options that were ignored.
pub fn get_ts_config_for_emit(
  config_type: TsConfigType,
  maybe_config_file: Option<&ConfigFile>,
) -> Result<TsConfigWithIgnoredOptions, AnyError> {
  let mut ts_config = match config_type {
    TsConfigType::Bundle => TsConfig::new(json!({
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
    TsConfigType::Check { lib } => TsConfig::new(json!({
      "allowJs": true,
      "allowSyntheticDefaultImports": true,
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "experimentalDecorators": true,
      "incremental": true,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": true,
      "inlineSources": true,
      "isolatedModules": true,
      "lib": lib,
      "module": "esnext",
      "moduleDetection": "force",
      "noEmit": true,
      "resolveJsonModule": true,
      "sourceMap": false,
      "strict": true,
      "target": "esnext",
      "tsBuildInfoFile": "deno:///.tsbuildinfo",
      "useDefineForClassFields": true,
      // TODO(@kitsonk) remove for Deno 2.0
      "useUnknownInCatchVariables": false,
    })),
    TsConfigType::Emit => TsConfig::new(json!({
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
  };
  let maybe_ignored_options =
    ts_config.merge_tsconfig_from_config_file(maybe_config_file)?;
  Ok(TsConfigWithIgnoredOptions {
    ts_config,
    maybe_ignored_options,
  })
}

/// Transform the graph into root specifiers that we can feed `tsc`. We have to
/// provide the media type for root modules because `tsc` does not "resolve" the
/// media type like other modules, as well as a root specifier needs any
/// redirects resolved. We need to include all the emittable files in
/// the roots, so they get type checked and optionally emitted,
/// otherwise they would be ignored if only imported into JavaScript.
fn get_tsc_roots(
  graph_data: &GraphData,
  check_js: bool,
) -> Vec<(ModuleSpecifier, MediaType)> {
  graph_data
    .entries()
    .into_iter()
    .filter_map(|(specifier, module_entry)| match module_entry {
      ModuleEntry::Module {
        media_type,
        ts_check,
        ..
      } => match &media_type {
        MediaType::TypeScript
        | MediaType::Tsx
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Jsx => Some((specifier.clone(), *media_type)),
        MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs
          if check_js || *ts_check =>
        {
          Some((specifier.clone(), *media_type))
        }
        _ => None,
      },
      _ => None,
    })
    .collect()
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
pub fn is_emittable(
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
pub struct CheckOptions {
  /// The check flag from the option which can effect the filtering of
  /// diagnostics in the emit result.
  pub type_check_mode: TypeCheckMode,
  /// Set the debug flag on the TypeScript type checker.
  pub debug: bool,
  /// The module specifier to the configuration file, passed to tsc so that
  /// configuration related diagnostics are properly formed.
  pub maybe_config_specifier: Option<ModuleSpecifier>,
  /// The derived tsconfig that should be used when checking.
  pub ts_config: TsConfig,
  /// If true, `Check <specifier>` will be written to stdout for each root.
  pub log_checks: bool,
  /// If true, valid `.tsbuildinfo` files will be ignored and type checking
  /// will always occur.
  pub reload: bool,
}

/// The result of a check of a module graph.
#[derive(Debug, Default)]
pub struct CheckResult {
  pub diagnostics: Diagnostics,
  pub stats: Stats,
}

/// Given a set of roots and graph data, type check the module graph.
///
/// It is expected that it is determined if a check and/or emit is validated
/// before the function is called.
pub fn check(
  roots: &[(ModuleSpecifier, ModuleKind)],
  graph_data: Arc<RwLock<GraphData>>,
  cache: &TypeCheckCache,
  options: CheckOptions,
) -> Result<CheckResult, AnyError> {
  let check_js = options.ts_config.get_check_js();
  let segment_graph_data = {
    let graph_data = graph_data.read();
    graph_data.graph_segment(roots).unwrap()
  };
  let check_hash = match get_check_hash(&segment_graph_data, &options) {
    CheckHashResult::NoFiles => return Ok(Default::default()),
    CheckHashResult::Hash(hash) => hash,
  };

  // do not type check if we know this is type checked
  if !options.reload && cache.has_check_hash(check_hash) {
    return Ok(Default::default());
  }

  let root_names = get_tsc_roots(&segment_graph_data, check_js);
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
    cache.get_tsbuildinfo(&roots[0].0)
  };
  // to make tsc build info work, we need to consistently hash modules, so that
  // tsc can better determine if an emit is still valid or not, so we provide
  // that data here.
  let hash_data = vec![
    options.ts_config.as_bytes(),
    version::deno().as_bytes().to_owned(),
  ];

  let response = tsc::exec(tsc::Request {
    config: options.ts_config,
    debug: options.debug,
    graph_data,
    hash_data,
    maybe_config_specifier: options.maybe_config_specifier,
    maybe_tsbuildinfo,
    root_names,
  })?;

  let diagnostics = if options.type_check_mode == TypeCheckMode::Local {
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

  if let Some(tsbuildinfo) = response.maybe_tsbuildinfo {
    cache.set_tsbuildinfo(&roots[0].0, &tsbuildinfo);
  }

  if diagnostics.is_empty() {
    cache.add_check_hash(check_hash);
  }

  Ok(CheckResult {
    diagnostics,
    stats: response.stats,
  })
}

pub struct EmitOptions {
  pub ts_config: TsConfig,
  pub reload: bool,
  pub reload_exclusions: HashSet<ModuleSpecifier>,
}

/// Given a module graph, emit any appropriate modules and cache them.
// TODO(nayeemrmn): This would ideally take `GraphData` like
// `check()`, but the AST isn't stored in that. Cleanup.
pub fn emit(
  graph: &ModuleGraph,
  cache: &dyn EmitCache,
  options: EmitOptions,
) -> Result<CheckResult, AnyError> {
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
    let is_valid = cache
      .get_source_hash(&module.specifier)
      .map_or(false, |v| v == version);
    if is_valid && !needs_reload {
      continue;
    }
    let transpiled_source = module
      .maybe_parsed_source
      .as_ref()
      .map(|source| source.transpile(&emit_options))
      .unwrap()?;
    emit_count += 1;
    cache.set_emit_data(
      module.specifier.clone(),
      SpecifierEmitCacheData {
        source_hash: version,
        text: transpiled_source.text,
        map: transpiled_source.source_map,
      },
    )?;
  }

  let stats = Stats(vec![
    ("Files".to_string(), file_count),
    ("Emitted".to_string(), emit_count),
    ("Total time".to_string(), start.elapsed().as_millis() as u32),
  ]);

  Ok(CheckResult {
    diagnostics: Diagnostics::default(),
    stats,
  })
}

enum CheckHashResult {
  Hash(u64),
  NoFiles,
}

/// Gets a hash of the inputs for type checking. This can then
/// be used to tell
fn get_check_hash(
  graph_data: &GraphData,
  options: &CheckOptions,
) -> CheckHashResult {
  // twox hash is insecure, but fast so it works for our purposes
  use std::hash::Hasher;
  use twox_hash::XxHash64;

  let mut hasher = XxHash64::default();
  hasher.write_u8(match options.type_check_mode {
    TypeCheckMode::All => 0,
    TypeCheckMode::Local => 1,
    TypeCheckMode::None => 2,
  });
  hasher.write(&options.ts_config.as_bytes());

  let check_js = options.ts_config.get_check_js();
  let mut sorted_entries = graph_data.entries().collect::<Vec<_>>();
  sorted_entries.sort_by_key(|(s, _)| s.as_str()); // make it deterministic
  let mut has_file = false;
  let mut has_file_to_type_check = false;
  for (specifier, module_entry) in sorted_entries {
    if let ModuleEntry::Module {
      code,
      media_type,
      ts_check,
      ..
    } = module_entry
    {
      if *ts_check {
        has_file_to_type_check = true;
      }

      match media_type {
        MediaType::TypeScript
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Tsx => {
          has_file = true;
          has_file_to_type_check = true;
        }
        MediaType::JavaScript
        | MediaType::Mjs
        | MediaType::Cjs
        | MediaType::Jsx => {
          has_file = true;
          if !check_js && !ts_check {
            continue;
          }
        }
        MediaType::Json
        | MediaType::TsBuildInfo
        | MediaType::SourceMap
        | MediaType::Wasm
        | MediaType::Unknown => continue,
      }
      hasher.write(specifier.as_str().as_bytes());
      hasher.write(code.as_bytes());
    }
  }

  if !has_file || !check_js && !has_file_to_type_check {
    // no files to type check
    CheckHashResult::NoFiles
  } else {
    CheckHashResult::Hash(hasher.finish())
  }
}

/// An adapter struct to make a deno_graph::ModuleGraphError display as expected
/// in the Deno CLI.
#[derive(Debug)]
pub struct GraphError(pub ModuleGraphError);

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
          raw: None,
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

impl From<TsConfig> for deno_ast::EmitOptions {
  fn from(config: TsConfig) -> Self {
    let options: EmitConfigOptions = serde_json::from_value(config.0).unwrap();
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
