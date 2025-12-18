// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_config::deno_json::CompilerOptions;
use deno_config::glob::PathOrPatternSet;
use deno_config::workspace::CompilerOptionsSource;
use deno_config::workspace::TsTypeLib;
use deno_config::workspace::WorkspaceDirectory;
use deno_config::workspace::WorkspaceRc;
use deno_error::JsError;
use deno_maybe_sync::new_rc;
use deno_path_util::normalize_path;
use deno_path_util::url_from_directory_path;
use deno_path_util::url_from_file_path;
use deno_path_util::url_to_file_path;
use deno_terminal::colors;
use deno_unsync::sync::AtomicFlag;
use indexmap::IndexMap;
use indexmap::IndexSet;
use node_resolver::DenoIsBuiltInNodeModuleChecker;
use node_resolver::NodeResolutionKind;
use node_resolver::NodeResolver;
use node_resolver::NpmPackageFolderResolver;
use node_resolver::ResolutionMode;
#[cfg(feature = "sync")]
use once_cell::sync::OnceCell;
#[cfg(not(feature = "sync"))]
use once_cell::unsync::OnceCell;
use serde::Serialize;
use serde::Serializer;
use serde_json::json;
use sys_traits::FsRead;
use thiserror::Error;
use url::Url;

use crate::collections::FolderScopedWithUnscopedMap;
use crate::factory::ConfigDiscoveryOption;
use crate::npm::DenoInNpmPackageChecker;
use crate::npm::NpmResolverSys;

#[allow(clippy::disallowed_types)]
type UrlRc = deno_maybe_sync::MaybeArc<Url>;
#[allow(clippy::disallowed_types)]
type CompilerOptionsRc = deno_maybe_sync::MaybeArc<CompilerOptions>;
#[allow(clippy::disallowed_types)]
pub type CompilerOptionsTypesRc =
  deno_maybe_sync::MaybeArc<Vec<(Url, Vec<String>)>>;

/// A structure that represents a set of options that were ignored and the
/// path those options came from.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IgnoredCompilerOptions {
  pub items: Vec<String>,
  pub maybe_specifier: Option<Url>,
}

impl std::fmt::Display for IgnoredCompilerOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    let mut codes = self.items.clone();
    codes.sort_unstable();
    if let Some(specifier) = &self.maybe_specifier {
      write!(
        f,
        "Unsupported compiler options in \"{}\".\n  The following options were ignored:\n    {}",
        specifier,
        codes.join(", ")
      )
    } else {
      write!(
        f,
        "Unsupported compiler options provided.\n  The following options were ignored:\n    {}",
        codes.join(", ")
      )
    }
  }
}

impl Serialize for IgnoredCompilerOptions {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    Serialize::serialize(&self.items, serializer)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerOptionsWithIgnoredOptions {
  pub compiler_options: CompilerOptions,
  pub ignored_options: Vec<IgnoredCompilerOptions>,
}

#[derive(Debug, Default, Clone)]
pub struct ParsedCompilerOptions {
  pub options: serde_json::Map<String, serde_json::Value>,
  pub maybe_ignored: Option<IgnoredCompilerOptions>,
}

/// A set of all the compiler options that should be allowed;
static ALLOWED_COMPILER_OPTIONS: phf::Set<&'static str> = phf::phf_set! {
  "allowUnreachableCode",
  "allowUnusedLabels",
  "baseUrl",
  "checkJs",
  "erasableSyntaxOnly",
  "emitDecoratorMetadata",
  "exactOptionalPropertyTypes",
  "experimentalDecorators",
  "isolatedDeclarations",
  "jsx",
  "jsxFactory",
  "jsxFragmentFactory",
  "jsxImportSource",
  "jsxPrecompileSkipElements",
  "lib",
  "noErrorTruncation",
  "noFallthroughCasesInSwitch",
  "noImplicitAny",
  "noImplicitOverride",
  "noImplicitReturns",
  "noImplicitThis",
  "noPropertyAccessFromIndexSignature",
  "noUncheckedIndexedAccess",
  "noUnusedLocals",
  "noUnusedParameters",
  "paths",
  "rootDirs",
  "skipLibCheck",
  "strict",
  "strictBindCallApply",
  "strictBuiltinIteratorReturn",
  "strictFunctionTypes",
  "strictNullChecks",
  "strictPropertyInitialization",
  "types",
  "useUnknownInCatchVariables",
  "verbatimModuleSyntax",
};

pub fn parse_compiler_options(
  compiler_options: serde_json::Map<String, serde_json::Value>,
  maybe_specifier: Option<&Url>,
) -> ParsedCompilerOptions {
  let mut allowed: serde_json::Map<String, serde_json::Value> =
    serde_json::Map::with_capacity(compiler_options.len());
  let mut ignored: Vec<String> = Vec::new(); // don't pre-allocate because it's rare

  for (key, value) in compiler_options {
    // We don't pass "types" entries to typescript via the compiler
    // options and instead provide those to tsc as "roots". This is
    // because our "types" behavior is at odds with how TypeScript's
    // "types" works.
    // We also don't pass "jsxImportSourceTypes" to TypeScript as it doesn't
    // know about this option. It will still take this option into account
    // because the graph resolves the JSX import source to the types for TSC.
    if key != "types" && key != "jsxImportSourceTypes" {
      if (key == "module"
        && value
          .as_str()
          .map(|s| {
            matches!(
              s.to_ascii_lowercase().as_str(),
              "nodenext" | "esnext" | "preserve"
            )
          })
          .unwrap_or(false))
        || (key == "moduleResolution"
          && value
            .as_str()
            .map(|s| {
              matches!(s.to_ascii_lowercase().as_str(), "nodenext" | "bundler")
            })
            .unwrap_or(false))
        || ALLOWED_COMPILER_OPTIONS.contains(key.as_str())
      {
        allowed.insert(key, value.to_owned());
      } else {
        ignored.push(key);
      }
    }
  }
  let maybe_ignored = if !ignored.is_empty() {
    Some(IgnoredCompilerOptions {
      items: ignored,
      maybe_specifier: maybe_specifier.cloned(),
    })
  } else {
    None
  };

  ParsedCompilerOptions {
    options: allowed,
    maybe_ignored,
  }
}

#[allow(clippy::disallowed_types)]
pub type SerdeJsonErrorArc = std::sync::Arc<serde_json::Error>;

#[derive(Debug, Clone, Error, JsError)]
#[class(type)]
#[error("compilerOptions should be an object at '{specifier}'")]
pub struct CompilerOptionsParseError {
  pub specifier: Url,
  #[source]
  pub source: SerdeJsonErrorArc,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct JsxImportSourceSpecifierConfig {
  pub specifier: String,
  pub base: Url,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct JsxImportSourceConfig {
  pub module: String,
  pub import_source: Option<JsxImportSourceSpecifierConfig>,
  pub import_source_types: Option<JsxImportSourceSpecifierConfig>,
}

impl JsxImportSourceConfig {
  pub fn specifier(&self) -> Option<&str> {
    self.import_source.as_ref().map(|c| c.specifier.as_str())
  }
}

#[allow(clippy::disallowed_types)]
pub type JsxImportSourceConfigRc =
  deno_maybe_sync::MaybeArc<JsxImportSourceConfig>;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum CompilerOptionsModule {
  NodeNext,
  EsNext,
  Preserve,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum CompilerOptionsModuleResolution {
  NodeNext,
  Bundler,
}

#[derive(Debug, Clone, Error, JsError)]
#[class(type)]
pub enum ToMaybeJsxImportSourceConfigError {
  #[error(
    "'jsxImportSource' is only supported when 'jsx' is set to 'react-jsx' or 'react-jsxdev'.\n  at {0}"
  )]
  InvalidJsxImportSourceValue(Url),
  #[error(
    "'jsxImportSourceTypes' is only supported when 'jsx' is set to 'react-jsx' or 'react-jsxdev'.\n  at {0}"
  )]
  InvalidJsxImportSourceTypesValue(Url),
  #[error(
    "Unsupported 'jsx' compiler option value '{value}'. Supported: 'react-jsx', 'react-jsxdev', 'react', 'precompile'\n  at {specifier}"
  )]
  InvalidJsxCompilerOption { value: String, specifier: Url },
}

/// An enum that represents the base tsc configuration to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompilerOptionsType {
  /// Return a configuration for bundling, using swc to emit the bundle. This is
  /// independent of type checking.
  Bundle,
  /// Return a configuration to use tsc to type check. This
  /// is independent of either bundling or emitting via swc.
  Check { lib: TsTypeLib },
  /// Return a configuration to use swc to emit single module files.
  Emit,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum CompilerOptionsSourceKind {
  DenoJson,
  TsConfig,
}

/// For a given configuration type get the starting point CompilerOptions
/// used that can then be merged with user specified options.
pub fn get_base_compiler_options_for_emit(
  config_type: CompilerOptionsType,
  source_kind: CompilerOptionsSourceKind,
) -> CompilerOptions {
  match config_type {
    CompilerOptionsType::Bundle => CompilerOptions::new(json!({
      "allowImportingTsExtensions": true,
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "experimentalDecorators": true,
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": false,
      "inlineSources": false,
      "sourceMap": false,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "module": "NodeNext",
    })),
    CompilerOptionsType::Check { lib } => CompilerOptions::new(json!({
      "allowJs": true,
      "allowImportingTsExtensions": true,
      "allowSyntheticDefaultImports": true,
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "experimentalDecorators": false,
      "incremental": true,
      "jsx": "react",
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": true,
      "inlineSources": true,
      "isolatedModules": true,
      "lib": match (lib, source_kind) {
        (TsTypeLib::DenoWindow, CompilerOptionsSourceKind::DenoJson) => vec!["deno.window", "deno.unstable", "node"],
        (TsTypeLib::DenoWindow, CompilerOptionsSourceKind::TsConfig) => vec!["deno.window", "deno.unstable", "dom", "node"],
        (TsTypeLib::DenoWorker, CompilerOptionsSourceKind::DenoJson) => vec!["deno.worker", "deno.unstable", "node"],
        (TsTypeLib::DenoWorker, CompilerOptionsSourceKind::TsConfig) => vec!["deno.worker", "deno.unstable", "dom", "node"],
      },
      "module": "NodeNext",
      "moduleDetection": "force",
      "noEmit": true,
      "noImplicitOverride": match source_kind {
        CompilerOptionsSourceKind::DenoJson => true,
        CompilerOptionsSourceKind::TsConfig => false,
      },
      "resolveJsonModule": true,
      "sourceMap": false,
      "skipLibCheck": false,
      "strict": match source_kind {
        CompilerOptionsSourceKind::DenoJson => true,
        CompilerOptionsSourceKind::TsConfig => false,
      },
      "target": "esnext",
      "tsBuildInfoFile": "internal:///.tsbuildinfo",
      "useDefineForClassFields": true,
    })),
    CompilerOptionsType::Emit => CompilerOptions::new(json!({
      "allowImportingTsExtensions": true,
      "checkJs": false,
      "emitDecoratorMetadata": false,
      "experimentalDecorators": false,
      "importsNotUsedAsValues": "remove",
      "inlineSourceMap": true,
      "inlineSources": true,
      "sourceMap": false,
      "jsx": "react",
      "jsxFactory": "React.createElement",
      "jsxFragmentFactory": "React.Fragment",
      "module": "NodeNext",
      "moduleResolution": "NodeNext",
      "resolveJsonModule": true,
    })),
  }
}

#[cfg(feature = "deno_ast")]
#[derive(Debug)]
pub struct TranspileAndEmitOptions {
  pub no_transpile: bool,
  pub transpile: deno_ast::TranspileOptions,
  pub emit: deno_ast::EmitOptions,
  // stored ahead of time so we don't have to recompute this a lot
  pub pre_computed_hash: u64,
}

#[cfg(feature = "deno_ast")]
#[allow(clippy::disallowed_types)]
pub type TranspileAndEmitOptionsRc =
  deno_maybe_sync::MaybeArc<TranspileAndEmitOptions>;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct WildcardString {
  prefix: String,
  suffix_if_wildcard: Option<String>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct CompilerOptionsPaths {
  resolved: Vec<(String, WildcardString, Vec<WildcardString>)>,
}

impl CompilerOptionsPaths {
  fn new(
    object: &serde_json::Map<String, serde_json::Value>,
    base: &Url,
  ) -> Self {
    let resolved = object
      .iter()
      .filter_map(|(raw_key, value)| {
        let value = value.as_array()?;
        let mut key_prefix = raw_key.as_str();
        let mut key_suffix_if_wildcard = None;
        if let Some((prefix, suffix)) = raw_key.split_once('*') {
          if suffix.contains('*') {
            return None;
          }
          key_prefix = prefix;
          key_suffix_if_wildcard = Some(suffix);
        }
        let key_prefix = if key_prefix.starts_with("./")
          || key_prefix.starts_with("../")
          || key_prefix.starts_with("/")
        {
          base.join(key_prefix).ok()?.to_string()
        } else {
          key_prefix.to_string()
        };
        let key = WildcardString {
          prefix: key_prefix,
          suffix_if_wildcard: key_suffix_if_wildcard.map(|s| s.to_string()),
        };
        let paths = value
          .iter()
          .filter_map(|entry| {
            let mut entry_prefix = entry.as_str()?;
            let mut entry_suffix_if_wildcard = None;
            if let Some((prefix, suffix)) = entry_prefix.split_once('*') {
              if suffix.contains('*') {
                return None;
              }
              entry_prefix = prefix;
              entry_suffix_if_wildcard = Some(suffix);
            }
            let entry_prefix = base.join(entry_prefix).ok()?.to_string();
            Some(WildcardString {
              prefix: entry_prefix,
              suffix_if_wildcard: entry_suffix_if_wildcard
                .map(|s| s.to_string()),
            })
          })
          .collect();
        Some((raw_key.clone(), key, paths))
      })
      .collect();
    Self { resolved }
  }

  pub fn resolve_candidates(
    &self,
    specifier: &str,
  ) -> Option<(impl Iterator<Item = Url>, String)> {
    self.resolved.iter().find_map(|(raw_key, key, paths)| {
      let mut matched = specifier.strip_prefix(&key.prefix)?;
      if let Some(key_suffix) = &key.suffix_if_wildcard {
        matched = matched.strip_suffix(key_suffix)?;
      } else if !matched.is_empty() {
        return None;
      }
      Some((
        paths.iter().filter_map(move |path| {
          if let Some(path_suffix) = &path.suffix_if_wildcard {
            Url::parse(&format!("{}{matched}{}", &path.prefix, path_suffix))
              .ok()
          } else {
            Url::parse(&path.prefix).ok()
          }
        }),
        raw_key.clone(),
      ))
    })
  }
}

#[derive(Debug, Default)]
struct LoggedWarnings {
  experimental_decorators: AtomicFlag,
  folders: deno_maybe_sync::MaybeDashSet<Url>,
}

#[allow(clippy::disallowed_types)]
type LoggedWarningsRc = deno_maybe_sync::MaybeArc<LoggedWarnings>;

#[derive(Default, Debug)]
struct MemoizedValues {
  deno_window_check_compiler_options:
    OnceCell<Result<CompilerOptionsRc, CompilerOptionsParseError>>,
  deno_worker_check_compiler_options:
    OnceCell<Result<CompilerOptionsRc, CompilerOptionsParseError>>,
  emit_compiler_options:
    OnceCell<Result<CompilerOptionsRc, CompilerOptionsParseError>>,
  #[cfg(feature = "deno_ast")]
  transpile_options:
    OnceCell<Result<TranspileAndEmitOptionsRc, CompilerOptionsParseError>>,
  compiler_options_types: OnceCell<CompilerOptionsTypesRc>,
  jsx_import_source_config: OnceCell<
    Result<Option<JsxImportSourceConfigRc>, ToMaybeJsxImportSourceConfigError>,
  >,
  module: OnceCell<CompilerOptionsModule>,
  module_resolution: OnceCell<CompilerOptionsModuleResolution>,
  check_js: OnceCell<bool>,
  skip_lib_check: OnceCell<bool>,
  base_url: OnceCell<Option<Url>>,
  paths: OnceCell<CompilerOptionsPaths>,
  root_dirs: OnceCell<Vec<Url>>,
}

#[derive(Debug, Clone, Default)]
pub struct CompilerOptionsOverrides {
  /// Skip transpiling in the loaders.
  pub no_transpile: bool,
  /// Base to use for the source map. This is useful when bundling
  /// and you want to make file urls relative.
  pub source_map_base: Option<Url>,
  /// Preserve JSX instead of transforming it.
  ///
  /// This may be useful when bundling.
  pub preserve_jsx: bool,
}

#[derive(Debug)]
pub struct CompilerOptionsData {
  pub sources: Vec<CompilerOptionsSource>,
  pub source_kind: CompilerOptionsSourceKind,
  workspace_dir_url: Option<UrlRc>,
  memoized: MemoizedValues,
  logged_warnings: LoggedWarningsRc,
  #[cfg_attr(not(feature = "deno_ast"), allow(unused))]
  overrides: CompilerOptionsOverrides,
}

impl CompilerOptionsData {
  fn new(
    sources: Vec<CompilerOptionsSource>,
    source_kind: CompilerOptionsSourceKind,
    workspace_dir_url: Option<UrlRc>,
    logged_warnings: LoggedWarningsRc,
    overrides: CompilerOptionsOverrides,
  ) -> Self {
    Self {
      sources,
      source_kind,
      workspace_dir_url,
      memoized: Default::default(),
      logged_warnings,
      overrides,
    }
  }

  pub fn compiler_options_for_lib(
    &self,
    lib: TsTypeLib,
  ) -> Result<&CompilerOptionsRc, CompilerOptionsParseError> {
    self.compiler_options_inner(CompilerOptionsType::Check { lib })
  }

  pub fn compiler_options_for_emit(
    &self,
  ) -> Result<&CompilerOptionsRc, CompilerOptionsParseError> {
    self.compiler_options_inner(CompilerOptionsType::Emit)
  }

  fn compiler_options_inner(
    &self,
    typ: CompilerOptionsType,
  ) -> Result<&CompilerOptionsRc, CompilerOptionsParseError> {
    let cell = match typ {
      CompilerOptionsType::Bundle => unreachable!(),
      CompilerOptionsType::Check {
        lib: TsTypeLib::DenoWindow,
      } => &self.memoized.deno_window_check_compiler_options,
      CompilerOptionsType::Check {
        lib: TsTypeLib::DenoWorker,
      } => &self.memoized.deno_worker_check_compiler_options,
      CompilerOptionsType::Emit => &self.memoized.emit_compiler_options,
    };
    let result = cell.get_or_init(|| {
      let mut result = CompilerOptionsWithIgnoredOptions {
        compiler_options: get_base_compiler_options_for_emit(
          typ,
          self.source_kind,
        ),
        ignored_options: Vec::new(),
      };
      for source in &self.sources {
        let Some(compiler_options) = source.compiler_options.as_ref() else {
          continue;
        };
        let object = serde_json::from_value(compiler_options.0.clone())
          .map_err(|err| CompilerOptionsParseError {
            specifier: source.specifier.as_ref().clone(),
            source: SerdeJsonErrorArc::new(err),
          })?;
        let parsed =
          parse_compiler_options(object, Some(source.specifier.as_ref()));
        result.compiler_options.merge_object_mut(parsed.options);
        if matches!(typ, CompilerOptionsType::Check { .. })
          && let Some(compiler_options) =
            result.compiler_options.0.as_object_mut()
          && compiler_options.get("isolatedDeclarations")
            == Some(&serde_json::Value::Bool(true))
        {
          compiler_options.insert("declaration".into(), true.into());
          compiler_options.insert("allowJs".into(), false.into());
        }
        if let Some(ignored) = parsed.maybe_ignored {
          result.ignored_options.push(ignored);
        }
      }
      if self.source_kind != CompilerOptionsSourceKind::TsConfig {
        check_warn_compiler_options(&result, &self.logged_warnings);
      }
      Ok(new_rc(result.compiler_options))
    });
    result.as_ref().map_err(Clone::clone)
  }

  #[cfg(feature = "deno_ast")]
  pub fn transpile_options(
    &self,
  ) -> Result<&TranspileAndEmitOptionsRc, CompilerOptionsParseError> {
    let result = self.memoized.transpile_options.get_or_init(|| {
      let compiler_options = self.compiler_options_for_emit()?;
      compiler_options_to_transpile_and_emit_options(
        compiler_options.as_ref().clone(),
        &self.overrides,
      )
      .map(new_rc)
      .map_err(|source| CompilerOptionsParseError {
        specifier: self
          .sources
          .last()
          .map(|s| s.specifier.as_ref().clone())
          .expect(
            "Compiler options parse errors must come from a user source.",
          ),
        source: SerdeJsonErrorArc::new(source),
      })
    });
    result.as_ref().map_err(Clone::clone)
  }

  pub fn compiler_options_types(&self) -> &CompilerOptionsTypesRc {
    self.memoized.compiler_options_types.get_or_init(|| {
      let types = self
        .sources
        .iter()
        .filter_map(|s| {
          let types = s
            .compiler_options
            .as_ref()?
            .0
            .as_object()?
            .get("types")?
            .as_array()?
            .iter()
            .filter_map(|v| Some(v.as_str()?.to_string()))
            .collect();
          Some((s.specifier.as_ref().clone(), types))
        })
        .collect();
      new_rc(types)
    })
  }

  pub fn jsx_import_source_config(
    &self,
  ) -> Result<Option<&JsxImportSourceConfigRc>, ToMaybeJsxImportSourceConfigError>
  {
    let result = self.memoized.jsx_import_source_config.get_or_init(|| {
      let jsx = self.sources.iter().rev().find_map(|s| Some((s.compiler_options.as_ref()?.0.as_object()?.get("jsx")?.as_str()?, &s.specifier)));
      let is_jsx_automatic = matches!(
        jsx,
        Some(("react-jsx" | "preserve" | "react-jsxdev" | "precompile", _)),
      );
      let import_source = self.sources.iter().rev().find_map(|s| {
        Some(JsxImportSourceSpecifierConfig {
          specifier: s.compiler_options.as_ref()?.0.as_object()?.get("jsxImportSource")?.as_str()?.to_string(),
          base: s.specifier.as_ref().clone()
        })
      }).or_else(|| {
        if !is_jsx_automatic {
          return None;
        }
        Some(JsxImportSourceSpecifierConfig {
          base: self.sources.last()?.specifier.as_ref().clone(),
          specifier: "react".to_string()
        })
      });
      let import_source_types = self.sources.iter().rev().find_map(|s| {
        Some(JsxImportSourceSpecifierConfig {
          specifier: s.compiler_options.as_ref()?.0.as_object()?.get("jsxImportSourceTypes")?.as_str()?.to_string(),
          base: s.specifier.as_ref().clone()
        })
      }).or_else(|| import_source.clone());
      let module = match jsx {
        Some(("react-jsx" | "preserve", _)) => "jsx-runtime".to_string(),
        Some(("react-jsxdev", _)) => "jsx-dev-runtime".to_string(),
        Some(("react", _)) | None => {
          if let Some(import_source) = &import_source {
            return Err(
              ToMaybeJsxImportSourceConfigError::InvalidJsxImportSourceValue(
                import_source.base.clone(),
              ),
            );
          }
          if let Some(import_source_types) = &import_source_types {
            return Err(
              ToMaybeJsxImportSourceConfigError::InvalidJsxImportSourceTypesValue(
                import_source_types.base.clone(),
              ),
            );
          }
          return Ok(None);
        }
        Some(("precompile", _)) => "jsx-runtime".to_string(),
        Some((setting, setting_source)) => {
          return Err(
            ToMaybeJsxImportSourceConfigError::InvalidJsxCompilerOption {
              value: setting.to_string(),
              specifier: setting_source.as_ref().clone(),
            },
          )
        }
      };
      Ok(Some(new_rc(JsxImportSourceConfig {
        module,
        import_source,
        import_source_types,
      })))
    });
    result.as_ref().map(|c| c.as_ref()).map_err(Clone::clone)
  }

  fn module(&self) -> CompilerOptionsModule {
    *self.memoized.module.get_or_init(|| {
      let value = self.sources.iter().rev().find_map(|s| {
        s.compiler_options
          .as_ref()?
          .0
          .as_object()?
          .get("module")?
          .as_str()
      });
      match value.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("esnext") => CompilerOptionsModule::EsNext,
        Some("preserve") => CompilerOptionsModule::Preserve,
        _ => CompilerOptionsModule::NodeNext,
      }
    })
  }

  pub fn module_resolution(&self) -> CompilerOptionsModuleResolution {
    *self.memoized.module_resolution.get_or_init(|| {
      let value = self.sources.iter().rev().find_map(|s| {
        s.compiler_options
          .as_ref()?
          .0
          .as_object()?
          .get("moduleResolution")?
          .as_str()
      });
      match value.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("nodenext") => CompilerOptionsModuleResolution::NodeNext,
        Some("bundler") => CompilerOptionsModuleResolution::Bundler,
        _ => match self.module() {
          CompilerOptionsModule::NodeNext => {
            CompilerOptionsModuleResolution::NodeNext
          }
          CompilerOptionsModule::EsNext | CompilerOptionsModule::Preserve => {
            CompilerOptionsModuleResolution::Bundler
          }
        },
      }
    })
  }

  pub fn check_js(&self) -> bool {
    *self.memoized.check_js.get_or_init(|| {
      self
        .sources
        .iter()
        .rev()
        .find_map(|s| {
          s.compiler_options
            .as_ref()?
            .0
            .as_object()?
            .get("checkJs")?
            .as_bool()
        })
        .unwrap_or(false)
    })
  }

  pub fn skip_lib_check(&self) -> bool {
    *self.memoized.skip_lib_check.get_or_init(|| {
      self
        .sources
        .iter()
        .rev()
        .find_map(|s| {
          s.compiler_options
            .as_ref()?
            .0
            .as_object()?
            .get("skipLibCheck")?
            .as_bool()
        })
        .unwrap_or(false)
    })
  }

  fn base_url(&self) -> Option<&Url> {
    let base_url = self.memoized.base_url.get_or_init(|| {
      let base_url = self.sources.iter().rev().find_map(|s| {
        s.compiler_options
          .as_ref()?
          .0
          .as_object()?
          .get("baseUrl")?
          .as_str()
      })?;
      url_from_directory_path(
        &url_to_file_path(&self.sources.last()?.specifier)
          .ok()?
          .parent()?
          .join(base_url),
      )
      .ok()
    });
    base_url.as_ref()
  }

  pub fn paths(&self) -> &CompilerOptionsPaths {
    self.memoized.paths.get_or_init(|| {
      let Some((source_specifier, paths)) =
        self.sources.iter().rev().find_map(|s| {
          Some((
            &s.specifier,
            s.compiler_options
              .as_ref()?
              .0
              .as_object()?
              .get("paths")?
              .as_object()?,
          ))
        })
      else {
        return Default::default();
      };
      CompilerOptionsPaths::new(
        paths,
        self.base_url().unwrap_or(source_specifier),
      )
    })
  }

  pub fn root_dirs(&self) -> &Vec<Url> {
    self.memoized.root_dirs.get_or_init(|| {
      let Some((source_specifier, root_dirs)) =
        self.sources.iter().rev().find_map(|s| {
          Some((
            &s.specifier,
            s.compiler_options
              .as_ref()?
              .0
              .as_object()?
              .get("rootDirs")?
              .as_array()?,
          ))
        })
      else {
        return Vec::new();
      };
      root_dirs
        .iter()
        .filter_map(|s| {
          url_from_directory_path(
            &url_to_file_path(self.base_url().unwrap_or(source_specifier))
              .ok()?
              .parent()?
              .join(s.as_str()?),
          )
          .ok()
        })
        .collect()
    })
  }

  pub fn workspace_dir_or_source_url(&self) -> Option<&UrlRc> {
    self
      .workspace_dir_url
      .as_ref()
      .or_else(|| self.sources.last().map(|s| &s.specifier))
  }
}

// A resolved element of the `files` array in a tsconfig.
#[derive(Debug, Clone)]
pub struct TsConfigFile {
  pub relative_specifier: String,
  pub absolute_path: PathBuf,
}

impl TsConfigFile {
  fn from_raw(raw: &str, dir_path: impl AsRef<Path>) -> Self {
    let relative_specifier = if raw.starts_with("./")
      || raw.starts_with("../")
      || raw.starts_with('/')
    {
      raw.to_string()
    } else {
      format!("./{raw}")
    };
    let path = Path::new(raw);
    let absolute_path = if path.is_absolute() {
      normalize_path(Cow::Borrowed(path))
    } else {
      normalize_path(Cow::Owned(dir_path.as_ref().join(path)))
    };
    Self {
      relative_specifier,
      absolute_path: absolute_path.into_owned(),
    }
  }
}

#[derive(Debug)]
struct TsConfigFileFilter {
  // Note that `files`, `include` and `exclude` are overwritten, not merged,
  // when using `extends`. So we only need to store one referrer for `files`.
  // See: https://www.typescriptlang.org/tsconfig/#extends.
  files: Option<(UrlRc, Vec<TsConfigFile>)>,
  include: Option<PathOrPatternSet>,
  exclude: Option<PathOrPatternSet>,
  dir_path: PathBuf,
}

impl TsConfigFileFilter {
  fn includes_path(&self, path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    if let Some((_, files)) = &self.files
      && files.iter().any(|f| f.absolute_path == path)
    {
      return true;
    }
    if let Some(exclude) = &self.exclude
      && exclude.matches_path(path)
    {
      return false;
    }
    if let Some(include) = &self.include {
      if include.matches_path(path) {
        return true;
      }
    } else if path.starts_with(&self.dir_path) {
      return true;
    }
    false
  }
}

#[allow(clippy::disallowed_types)]
type TsConfigFileFilterRc = deno_maybe_sync::MaybeArc<TsConfigFileFilter>;

#[derive(Debug)]
pub struct TsConfigData {
  compiler_options: CompilerOptionsData,
  filter: TsConfigFileFilterRc,
  references: Vec<String>,
}

impl TsConfigData {
  pub fn files(&self) -> Option<(&UrlRc, &Vec<TsConfigFile>)> {
    let (referrer, files) = self.filter.files.as_ref()?;
    Some((referrer, files))
  }

  fn specifier(&self) -> &UrlRc {
    &self
      .compiler_options
      .sources
      .last()
      .expect("Tsconfigs should always have at least one source.")
      .specifier
  }
}

fn is_maybe_directory_error(err: &std::io::Error) -> bool {
  let kind = err.kind();
  kind == ErrorKind::IsADirectory
    // This happens on Windows for some reason.
    || cfg!(windows) && kind == ErrorKind::PermissionDenied
}

pub(crate) type TsConfigNodeResolver<TSys, TNpfr> = NodeResolver<
  DenoInNpmPackageChecker,
  DenoIsBuiltInNodeModuleChecker,
  TNpfr,
  TSys,
>;

type GetNodeResolverFn<'a, NSys, TNpfr> =
  Box<dyn Fn(&Url) -> Option<&'a TsConfigNodeResolver<NSys, TNpfr>> + 'a>;

struct TsConfigCollector<
  'a,
  'b,
  TSys: FsRead,
  NSys: NpmResolverSys,
  TNpfr: NpmPackageFolderResolver,
> {
  roots: BTreeSet<PathBuf>,
  collected: IndexMap<UrlRc, Rc<TsConfigData>>,
  read_cache: HashMap<PathBuf, Result<Rc<TsConfigData>, Rc<std::io::Error>>>,
  currently_reading: IndexSet<PathBuf>,
  sys: &'a TSys,
  get_node_resolver: GetNodeResolverFn<'b, NSys, TNpfr>,
  logged_warnings: &'a LoggedWarningsRc,
  overrides: CompilerOptionsOverrides,
}

impl<
  TSys: FsRead + std::fmt::Debug,
  NSys: NpmResolverSys,
  TNpfr: NpmPackageFolderResolver,
> std::fmt::Debug for TsConfigCollector<'_, '_, TSys, NSys, TNpfr>
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TsConfigCollector")
      .field("roots", &self.roots)
      .field("collected", &self.collected)
      .field("read_cache", &self.read_cache)
      .field("currently_reading", &self.currently_reading)
      .field("sys", &self.sys)
      .field("logged_warnings", &self.logged_warnings)
      .finish()
  }
}

impl<
  'a,
  'b,
  TSys: FsRead,
  NSys: NpmResolverSys,
  TNpfr: NpmPackageFolderResolver,
> TsConfigCollector<'a, 'b, TSys, NSys, TNpfr>
{
  fn new(
    sys: &'a TSys,
    get_node_resolver: GetNodeResolverFn<'b, NSys, TNpfr>,
    logged_warnings: &'a LoggedWarningsRc,
    overrides: CompilerOptionsOverrides,
  ) -> Self {
    Self {
      roots: Default::default(),
      collected: Default::default(),
      read_cache: Default::default(),
      currently_reading: Default::default(),
      sys,
      get_node_resolver,
      logged_warnings,
      overrides,
    }
  }

  fn add_root(&mut self, path: PathBuf) {
    self.roots.insert(path);
  }

  fn collect(mut self) -> Vec<TsConfigData> {
    for root in std::mem::take(&mut self.roots) {
      let Ok(ts_config) = self.read_ts_config_with_cache(Cow::Owned(root))
      else {
        continue;
      };
      self.visit_reference(ts_config);
    }
    let Self { collected, .. } = { self };
    collected
      .into_values()
      .map(|t| {
        Rc::try_unwrap(t).expect(
          "No other references should be held since the read cache is dropped.",
        )
      })
      .collect()
  }

  fn visit_reference(&mut self, ts_config: Rc<TsConfigData>) {
    let specifier = ts_config.specifier();
    if self.collected.contains_key(specifier) {
      return;
    }
    let Some(dir_path) = url_to_file_path(specifier)
      .ok()
      .and_then(|p| Some(p.parent()?.to_path_buf()))
    else {
      return;
    };
    for reference in &ts_config.references {
      let reference_path = Path::new(reference);
      let reference_path = if reference_path.is_absolute() {
        Cow::Borrowed(reference_path)
      } else {
        Cow::Owned(dir_path.join(reference_path))
      };
      match self.read_ts_config_with_cache(Cow::Borrowed(&reference_path)) {
        Ok(ts_config) => self.visit_reference(ts_config),
        Err(err) if is_maybe_directory_error(&err) => {
          if let Ok(ts_config) = self.read_ts_config_with_cache(Cow::Owned(
            reference_path.join("tsconfig.json"),
          )) {
            self.visit_reference(ts_config)
          }
        }
        _ => {}
      }
    }
    self.collected.insert(specifier.clone(), ts_config);
  }

  fn read_ts_config_with_cache(
    &mut self,
    path: Cow<'_, Path>,
  ) -> Result<Rc<TsConfigData>, Rc<std::io::Error>> {
    let path = normalize_path(path);
    self
      .read_cache
      .get(path.as_ref())
      .cloned()
      .unwrap_or_else(|| {
        if !self.currently_reading.insert(path.to_path_buf()) {
          return Err(Rc::new(std::io::Error::other(
            "Cycle detected while following `extends`.",
          )));
        }
        let result = self.read_ts_config(&path).map(Rc::new).map_err(Rc::new);
        self.currently_reading.pop();
        self.read_cache.insert(path.to_path_buf(), result.clone());
        result
      })
  }

  fn read_ts_config(
    &mut self,
    path: impl AsRef<Path>,
  ) -> Result<TsConfigData, std::io::Error> {
    let path = path.as_ref();
    let warn = |err: &dyn std::fmt::Display| {
      log::warn!("Failed reading {}: {}", path.display(), err);
    };
    let specifier = new_rc(
      url_from_file_path(path)
        .inspect_err(|e| warn(e))
        .map_err(|err| std::io::Error::new(ErrorKind::InvalidInput, err))?,
    );
    let text = self.sys.fs_read_to_string(path).inspect_err(|e| {
      if e.kind() != ErrorKind::NotFound && !is_maybe_directory_error(e) {
        warn(e)
      }
    })?;
    let value = jsonc_parser::parse_to_serde_value(&text, &Default::default())
      .inspect_err(|e| warn(e))
      .ok()
      .flatten();
    let object = value.as_ref().and_then(|v| v.as_object());
    let extends_targets = object
      .and_then(|o| o.get("extends"))
      .into_iter()
      .flat_map(|v| {
        if let Some(s) = v.as_str() {
          vec![s]
        } else if let Some(a) = v.as_array() {
          a.iter().filter_map(|v| v.as_str()).collect()
        } else {
          Vec::new()
        }
      })
      .filter_map(|s| {
        let node_resolver = (self.get_node_resolver)(&specifier)?;
        let node_resolution = node_resolver
          .resolve(
            s,
            &specifier,
            ResolutionMode::Require,
            NodeResolutionKind::Execution,
          )
          .ok()?;
        let url = node_resolution.into_url().ok()?;
        let path = url_to_file_path(&url).ok()?;
        self.read_ts_config_with_cache(Cow::Owned(path)).ok()
      })
      .collect::<Vec<_>>();
    let sources = extends_targets
      .iter()
      .flat_map(|t| &t.compiler_options.sources)
      .cloned()
      .chain([CompilerOptionsSource {
        specifier: specifier.clone(),
        compiler_options: object
          .and_then(|o| o.get("compilerOptions"))
          .filter(|v| !v.is_null())
          .cloned()
          .map(CompilerOptions),
      }])
      .collect();
    let dir_path = path.parent().expect("file path should have a parent");
    let files = object
      .and_then(|o| {
        let files = o
          .get("files")?
          .as_array()?
          .iter()
          .filter_map(|v| Some(TsConfigFile::from_raw(v.as_str()?, dir_path)))
          .collect();
        Some((specifier, files))
      })
      .or_else(|| {
        extends_targets
          .iter()
          .rev()
          .find_map(|t| t.filter.files.clone())
      });
    let include = object
      .and_then(|o| {
        PathOrPatternSet::from_absolute_paths(
          &o.get("include")?
            .as_array()?
            .iter()
            .filter_map(|v| {
              let path = Path::new(v.as_str()?);
              let absolute_path = if path.is_absolute() {
                normalize_path(Cow::Borrowed(path))
              } else {
                normalize_path(Cow::Owned(dir_path.join(path)))
              };
              Some(absolute_path.to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>(),
        )
        .ok()
      })
      .or_else(|| {
        extends_targets
          .iter()
          .rev()
          .find_map(|t| t.filter.include.clone())
      })
      .or_else(|| files.is_some().then(Default::default));
    let exclude = object
      .and_then(|o| {
        PathOrPatternSet::from_absolute_paths(
          &o.get("exclude")?
            .as_array()?
            .iter()
            .filter_map(|v| {
              let path = Path::new(v.as_str()?);
              let absolute_path = if path.is_absolute() {
                normalize_path(Cow::Borrowed(path))
              } else {
                normalize_path(Cow::Owned(dir_path.join(path)))
              };
              Some(absolute_path.to_string_lossy().into_owned())
            })
            .collect::<Vec<_>>(),
        )
        .ok()
      })
      .or_else(|| {
        extends_targets
          .iter()
          .rev()
          .find_map(|t| t.filter.exclude.clone())
      });
    let references = object
      .and_then(|o| o.get("references")?.as_array())
      .into_iter()
      .flatten()
      .filter_map(|v| Some(v.as_object()?.get("path")?.as_str()?.to_string()))
      .collect();
    Ok(TsConfigData {
      compiler_options: CompilerOptionsData::new(
        sources,
        CompilerOptionsSourceKind::TsConfig,
        None,
        self.logged_warnings.clone(),
        self.overrides.clone(),
      ),
      filter: new_rc(TsConfigFileFilter {
        files,
        include,
        exclude,
        dir_path: dir_path.to_path_buf(),
      }),
      references,
    })
  }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CompilerOptionsKey {
  WorkspaceConfig(Option<UrlRc>),
  TsConfig(usize),
}

impl Default for CompilerOptionsKey {
  fn default() -> Self {
    Self::WorkspaceConfig(None)
  }
}

impl std::fmt::Display for CompilerOptionsKey {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::WorkspaceConfig(None) => write!(f, "workspace-root"),
      Self::WorkspaceConfig(Some(s)) => write!(f, "workspace({s})"),
      Self::TsConfig(i) => write!(f, "ts-config({i})"),
    }
  }
}

impl Serialize for CompilerOptionsKey {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    self.to_string().serialize(serializer)
  }
}

#[derive(Debug)]
pub struct CompilerOptionsResolver {
  workspace_configs: FolderScopedWithUnscopedMap<CompilerOptionsData>,
  ts_configs: Vec<TsConfigData>,
}

impl Default for CompilerOptionsResolver {
  fn default() -> Self {
    Self {
      workspace_configs: FolderScopedWithUnscopedMap::new(
        CompilerOptionsData::new(
          Vec::new(),
          CompilerOptionsSourceKind::DenoJson,
          None,
          Default::default(),
          Default::default(),
        ),
      ),
      ts_configs: Vec::new(),
    }
  }
}

impl CompilerOptionsResolver {
  pub fn new<
    TSys: FsRead,
    NSys: NpmResolverSys,
    TNpfr: NpmPackageFolderResolver,
  >(
    sys: &TSys,
    workspace: &WorkspaceRc,
    node_resolver: &TsConfigNodeResolver<NSys, TNpfr>,
    config_discover: &ConfigDiscoveryOption,
    overrides: &CompilerOptionsOverrides,
  ) -> Self {
    if matches!(config_discover, ConfigDiscoveryOption::Disabled) {
      return Self {
        workspace_configs: FolderScopedWithUnscopedMap::new(
          CompilerOptionsData::new(
            Vec::new(),
            CompilerOptionsSourceKind::DenoJson,
            None,
            Default::default(),
            overrides.clone(),
          ),
        ),
        ts_configs: Vec::new(),
      };
    }
    let logged_warnings = new_rc(LoggedWarnings::default());
    let mut ts_config_collector = TsConfigCollector::new(
      sys,
      Box::new(|_| Some(node_resolver)),
      &logged_warnings,
      overrides.clone(),
    );
    let root_dir = workspace.root_dir();
    let mut workspace_configs =
      FolderScopedWithUnscopedMap::new(CompilerOptionsData::new(
        root_dir.to_configured_compiler_options_sources(),
        CompilerOptionsSourceKind::DenoJson,
        Some(root_dir.dir_url().clone()),
        logged_warnings.clone(),
        overrides.clone(),
      ));
    for dir in workspace.resolve_member_dirs() {
      if dir.has_deno_or_pkg_json() {
        ts_config_collector.add_root(dir.dir_path().join("tsconfig.json"));
      }
      if dir.dir_url() != root_dir.dir_url() {
        workspace_configs.insert(
          dir.dir_url().clone(),
          CompilerOptionsData::new(
            dir.to_configured_compiler_options_sources(),
            CompilerOptionsSourceKind::DenoJson,
            Some(dir.dir_url().clone()),
            logged_warnings.clone(),
            overrides.clone(),
          ),
        );
      }
    }
    Self {
      workspace_configs,
      ts_configs: ts_config_collector.collect(),
    }
  }

  pub fn for_specifier(&self, specifier: &Url) -> &CompilerOptionsData {
    let workspace_data = self.workspace_configs.get_for_specifier(specifier);
    if !workspace_data
      .sources
      .iter()
      .any(|s| s.compiler_options.is_some())
      && let Ok(path) = url_to_file_path(specifier)
    {
      for ts_config in &self.ts_configs {
        if ts_config.filter.includes_path(&path) {
          return &ts_config.compiler_options;
        }
      }
    }
    workspace_data
  }

  pub fn entry_for_specifier(
    &self,
    specifier: &Url,
  ) -> (CompilerOptionsKey, &CompilerOptionsData) {
    let (scope, workspace_data) =
      self.workspace_configs.entry_for_specifier(specifier);
    if !workspace_data
      .sources
      .iter()
      .any(|s| s.compiler_options.is_some())
      && let Ok(path) = url_to_file_path(specifier)
    {
      for (i, ts_config) in self.ts_configs.iter().enumerate() {
        if ts_config.filter.includes_path(&path) {
          return (
            CompilerOptionsKey::TsConfig(i),
            &ts_config.compiler_options,
          );
        }
      }
    }
    (
      CompilerOptionsKey::WorkspaceConfig(scope.cloned()),
      workspace_data,
    )
  }

  pub fn entries(
    &self,
  ) -> impl Iterator<
    Item = (
      CompilerOptionsKey,
      &CompilerOptionsData,
      Option<(&UrlRc, &Vec<TsConfigFile>)>,
    ),
  > {
    self
      .workspace_configs
      .entries()
      .map(|(s, r)| (CompilerOptionsKey::WorkspaceConfig(s.cloned()), r, None))
      .chain(self.ts_configs.iter().enumerate().map(|(i, t)| {
        (
          CompilerOptionsKey::TsConfig(i),
          &t.compiler_options,
          t.files(),
        )
      }))
  }

  pub fn size(&self) -> usize {
    self.workspace_configs.count() + self.ts_configs.len()
  }

  pub fn has_root_dirs(&self) -> bool {
    self.entries().any(|(_, d, _)| !d.root_dirs().is_empty())
  }

  pub fn new_for_dirs_by_scope<
    TSys: FsRead,
    NSys: NpmResolverSys,
    TNpfr: NpmPackageFolderResolver,
  >(
    sys: &TSys,
    dirs_by_scope: BTreeMap<&UrlRc, &WorkspaceDirectory>,
    get_node_resolver: GetNodeResolverFn<'_, NSys, TNpfr>,
  ) -> Self {
    let logged_warnings = new_rc(LoggedWarnings::default());
    let mut ts_config_collector = TsConfigCollector::new(
      sys,
      get_node_resolver,
      &logged_warnings,
      Default::default(),
    );
    let mut workspace_configs =
      FolderScopedWithUnscopedMap::new(CompilerOptionsData::new(
        Vec::new(),
        CompilerOptionsSourceKind::DenoJson,
        None,
        logged_warnings.clone(),
        Default::default(),
      ));
    for (scope, dir) in dirs_by_scope {
      if dir.has_deno_or_pkg_json() {
        ts_config_collector.add_root(dir.dir_path().join("tsconfig.json"));
      }
      workspace_configs.insert(
        scope.clone(),
        CompilerOptionsData::new(
          dir.to_configured_compiler_options_sources(),
          CompilerOptionsSourceKind::DenoJson,
          Some(scope.clone()),
          logged_warnings.clone(),
          Default::default(),
        ),
      );
    }
    Self {
      workspace_configs,
      ts_configs: ts_config_collector.collect(),
    }
  }
}

#[cfg(feature = "graph")]
impl deno_graph::CheckJsResolver for CompilerOptionsResolver {
  fn resolve(&self, specifier: &Url) -> bool {
    self.for_specifier(specifier).check_js()
  }
}

#[allow(clippy::disallowed_types)]
pub type CompilerOptionsResolverRc =
  deno_maybe_sync::MaybeArc<CompilerOptionsResolver>;

/// JSX config stored in `CompilerOptionsResolver`, but fallibly resolved
/// ahead of time as needed for the graph resolver.
#[derive(Debug, Default)]
pub struct JsxImportSourceConfigResolver {
  workspace_configs:
    FolderScopedWithUnscopedMap<Option<JsxImportSourceConfigRc>>,
  ts_configs: Vec<(Option<JsxImportSourceConfigRc>, TsConfigFileFilterRc)>,
}

impl JsxImportSourceConfigResolver {
  pub fn from_compiler_options_resolver(
    compiler_options_resolver: &CompilerOptionsResolver,
  ) -> Result<Self, ToMaybeJsxImportSourceConfigError> {
    Ok(Self {
      workspace_configs: compiler_options_resolver
        .workspace_configs
        .try_map(|d| Ok(d.jsx_import_source_config()?.cloned()))?,
      ts_configs: compiler_options_resolver
        .ts_configs
        .iter()
        .map(|t| {
          Ok((
            t.compiler_options.jsx_import_source_config()?.cloned(),
            t.filter.clone(),
          ))
        })
        .collect::<Result<_, _>>()?,
    })
  }

  pub fn for_specifier(
    &self,
    specifier: &Url,
  ) -> Option<&JsxImportSourceConfigRc> {
    if let Ok(path) = url_to_file_path(specifier) {
      for (config, filter) in &self.ts_configs {
        if filter.includes_path(&path) {
          return config.as_ref();
        }
      }
    }
    self.workspace_configs.get_for_specifier(specifier).as_ref()
  }
}

#[cfg(feature = "deno_ast")]
fn compiler_options_to_transpile_and_emit_options(
  config: deno_config::deno_json::CompilerOptions,
  overrides: &CompilerOptionsOverrides,
) -> Result<TranspileAndEmitOptions, serde_json::Error> {
  let options: deno_config::deno_json::EmitConfigOptions =
    serde_json::from_value(config.0)?;
  let imports_not_used_as_values =
    match options.imports_not_used_as_values.as_str() {
      "preserve" => deno_ast::ImportsNotUsedAsValues::Preserve,
      "error" => deno_ast::ImportsNotUsedAsValues::Error,
      _ => deno_ast::ImportsNotUsedAsValues::Remove,
    };
  let jsx = match overrides.preserve_jsx {
    true => None,
    false => match options.jsx.as_str() {
      "react" => {
        Some(deno_ast::JsxRuntime::Classic(deno_ast::JsxClassicOptions {
          factory: options.jsx_factory,
          fragment_factory: options.jsx_fragment_factory,
        }))
      }
      "react-jsx" => Some(deno_ast::JsxRuntime::Automatic(
        deno_ast::JsxAutomaticOptions {
          development: false,
          import_source: options.jsx_import_source,
        },
      )),
      "react-jsxdev" => Some(deno_ast::JsxRuntime::Automatic(
        deno_ast::JsxAutomaticOptions {
          development: true,
          import_source: options.jsx_import_source,
        },
      )),
      "precompile" => Some(deno_ast::JsxRuntime::Precompile(
        deno_ast::JsxPrecompileOptions {
          automatic: deno_ast::JsxAutomaticOptions {
            development: false,
            import_source: options.jsx_import_source,
          },
          skip_elements: options.jsx_precompile_skip_elements,
          dynamic_props: None,
        },
      )),
      _ => None,
    },
  };
  let source_map = if options.inline_source_map {
    deno_ast::SourceMapOption::Inline
  } else if options.source_map {
    deno_ast::SourceMapOption::Separate
  } else {
    deno_ast::SourceMapOption::None
  };
  let transpile = deno_ast::TranspileOptions {
    decorators: if options.experimental_decorators {
      deno_ast::DecoratorsTranspileOption::LegacyTypeScript {
        emit_metadata: options.emit_decorator_metadata,
      }
    } else {
      deno_ast::DecoratorsTranspileOption::Ecma
    },
    imports_not_used_as_values,
    jsx,
    var_decl_imports: false,
    // todo(dsherret): support verbatim_module_syntax here properly
    verbatim_module_syntax: false,
  };
  let emit = deno_ast::EmitOptions {
    inline_sources: options.inline_sources,
    remove_comments: false,
    source_map,
    source_map_base: overrides.source_map_base.clone(),
    source_map_file: None,
  };
  let transpile_and_emit_options_hash = {
    use std::hash::Hash;
    use std::hash::Hasher;
    let mut hasher = twox_hash::XxHash64::default();
    transpile.hash(&mut hasher);
    emit.hash(&mut hasher);
    hasher.finish()
  };
  Ok(TranspileAndEmitOptions {
    no_transpile: overrides.no_transpile,
    transpile,
    emit,
    pre_computed_hash: transpile_and_emit_options_hash,
  })
}

fn check_warn_compiler_options(
  compiler_options: &CompilerOptionsWithIgnoredOptions,
  logged_warnings: &LoggedWarnings,
) {
  for ignored_options in &compiler_options.ignored_options {
    if !ignored_options.items.is_empty()
      && ignored_options
        .maybe_specifier
        .as_ref()
        .map(|s| logged_warnings.folders.insert(s.clone()))
        .unwrap_or(true)
    {
      log::warn!("{}", ignored_options);
    }
  }
  let serde_json::Value::Object(obj) = &compiler_options.compiler_options.0
  else {
    return;
  };
  if obj.get("experimentalDecorators") == Some(&serde_json::Value::Bool(true))
    && logged_warnings.experimental_decorators.raise()
  {
    log::warn!(
      "{} experimentalDecorators compiler option is deprecated and may be removed at any time",
      colors::yellow("Warning"),
    );
  }
}
