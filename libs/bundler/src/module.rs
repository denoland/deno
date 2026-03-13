// Copyright 2018-2026 the Deno authors. MIT license.

use deno_ast::swc::ast::Program;
use deno_ast::MediaType;
use deno_ast::ModuleSpecifier;
use deno_ast::ParseParams;
use deno_ast::ParsedSource;

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
// Manual Debug impl because `Program` doesn't implement Debug.
impl std::fmt::Debug for BundlerModule {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BundlerModule")
      .field("specifier", &self.specifier)
      .field("loader", &self.loader)
      .field("module_type", &self.module_type)
      .field("is_async", &self.is_async)
      .finish_non_exhaustive()
  }
}

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
  /// Source map (v3 JSON) mapping this module's `source` back to the
  /// original file (e.g. TS → JS). Set during transpilation.
  pub source_map: Option<String>,
  /// Hash of the original (pre-transform) source, for incremental builds.
  pub source_hash: Option<u64>,
  /// Cached parsed AST. Cleared when `source` changes.
  pub parsed: Option<ParsedSource>,
  /// Post-transform AST (set by `transform_graph`). Used by analysis
  /// to avoid re-parsing after transforms mutate the AST.
  pub transformed_program: Option<Program>,
  /// Import/export/scope analysis (populated after parsing).
  pub module_info: Option<ModuleInfo>,
  /// HMR metadata (populated after parsing).
  pub hmr_info: Option<HmrInfo>,
  /// Whether this module uses top-level await.
  pub is_async: bool,
  /// External import specifiers found in this module.
  pub external_imports: Vec<String>,
}

impl BundlerModule {
  /// Parse this module if not already cached.
  /// Returns `None` for non-JS loaders or parse errors.
  pub fn ensure_parsed(&mut self) -> Option<&ParsedSource> {
    if self.parsed.is_some() {
      return self.parsed.as_ref();
    }
    if !matches!(
      self.loader,
      Loader::Js | Loader::Jsx | Loader::Ts | Loader::Tsx
    ) {
      return None;
    }
    let parsed = deno_ast::parse_module(ParseParams {
      specifier: self.specifier.clone(),
      text: self.source.clone().into(),
      media_type: MediaType::JavaScript,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .ok()?;
    self.parsed = Some(parsed);
    self.parsed.as_ref()
  }
}
