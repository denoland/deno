// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! The collection of APIs to be able to take `deno_graph` module graphs and
//! populate a cache, emit files, and transform a graph into the structures for
//! loading into an isolate.

use crate::args::config_file::IgnoredCompilerOptions;
use crate::args::ConfigFile;
use crate::args::EmitConfigOptions;
use crate::args::TsConfig;
use crate::cache::EmitCache;
use crate::cache::FastInsecureHasher;
use crate::cache::ParsedSourceCache;

use deno_ast::swc::bundler::Hook;
use deno_ast::swc::bundler::ModuleRecord;
use deno_ast::swc::common::Span;
use deno_core::error::AnyError;
use deno_core::serde::Serialize;
use deno_core::serde::Serializer;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::ModuleSpecifier;
use deno_graph::MediaType;
use deno_graph::ModuleGraphError;
use deno_graph::ResolutionError;
use std::fmt;
use std::sync::Arc;

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
  /// Return a configuration to use tsc to type check. This
  /// is independent of either bundling or emitting via swc.
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

/// A hashing function that takes the source code, version and optionally a
/// user provided config and generates a string hash which can be stored to
/// determine if the cached emit is valid or not.
pub fn get_source_hash(source_text: &str, emit_options_hash: u64) -> u64 {
  FastInsecureHasher::new()
    .write_str(source_text)
    .write_u64(emit_options_hash)
    .finish()
}

pub fn emit_parsed_source(
  emit_cache: &EmitCache,
  parsed_source_cache: &ParsedSourceCache,
  specifier: &ModuleSpecifier,
  media_type: MediaType,
  source: &Arc<str>,
  emit_options: &deno_ast::EmitOptions,
  emit_config_hash: u64,
) -> Result<String, AnyError> {
  let source_hash = get_source_hash(source, emit_config_hash);

  if let Some(emit_code) =
    emit_cache.get_emit_code(specifier, Some(source_hash))
  {
    Ok(emit_code)
  } else {
    // this will use a cached version if it exists
    let parsed_source = parsed_source_cache.get_or_parse_module(
      specifier,
      source.clone(),
      media_type,
    )?;
    let transpiled_source = parsed_source.transpile(emit_options)?;
    debug_assert!(transpiled_source.source_map.is_none());
    emit_cache.set_emit_code(specifier, source_hash, &transpiled_source.text);
    Ok(transpiled_source.text)
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
