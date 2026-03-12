// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::ModuleSpecifier;

use crate::dependency::Dependency;
use crate::js::hmr_info::HmrInfo;
use crate::js::module_info::ModuleInfo;
use crate::loader::Loader;

/// Whether a module uses ESM or CJS syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleType {
  Esm,
  Cjs,
}

/// Side-effect annotation from package.json `sideEffects` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideEffectFlag {
  True,
  False,
  Unknown,
}

/// A module in the bundler graph with all bundler-specific metadata.
#[derive(Debug)]
pub struct BundlerModule {
  /// The module's URL specifier (matches deno_graph).
  pub specifier: ModuleSpecifier,
  /// How this module is loaded/parsed (may change after transpilation to Js).
  pub loader: Loader,
  /// The original loader before transpilation (used by file watcher to
  /// know whether a re-read file needs re-transpilation).
  pub original_loader: Loader,
  /// ESM or CJS.
  pub module_type: ModuleType,
  /// Resolved dependency edges.
  pub dependencies: Vec<Dependency>,
  /// Side-effect flag from package.json.
  pub side_effects: SideEffectFlag,
  /// The module's source code.
  pub source: String,
  /// Import/export/scope analysis (populated after parsing).
  pub module_info: Option<ModuleInfo>,
  /// HMR metadata (populated after parsing).
  pub hmr_info: Option<HmrInfo>,
  /// Whether this module uses top-level await.
  pub is_async: bool,
  /// External import specifiers found in this module.
  pub external_imports: Vec<String>,
}
