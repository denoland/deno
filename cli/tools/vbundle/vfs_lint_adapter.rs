// Copyright 2018-2026 the Deno authors. MIT license.

//! VFS adapter for the Deno linter.
//!
//! This module provides integration between the VFS and the Deno linter,
//! enabling linting of transformed files (e.g., `.svelte`, `.vue` files
//! that have been transformed to JavaScript).
//!
//! # How It Works
//!
//! 1. The VFS transforms non-JS files to JavaScript
//! 2. The linter runs on the transformed JavaScript
//! 3. Diagnostic positions are mapped back to the original source
//!
//! # Usage
//!
//! ```ignore
//! let vfs = Arc::new(BundlerVirtualFS::new());
//! let adapter = VfsLintAdapter::new(vfs);
//! let diagnostics = adapter.lint_files(&specifiers).await?;
//! ```

use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;

use super::source_map::Position;
use super::source_map::SourceRange;
use super::virtual_fs::BundlerVirtualFS;

/// A lint diagnostic with position information.
#[derive(Debug, Clone)]
pub struct LintDiagnostic {
  /// The file specifier.
  pub specifier: ModuleSpecifier,
  /// The diagnostic message.
  pub message: String,
  /// The lint rule code (e.g., "no-unused-vars").
  pub code: String,
  /// The severity level.
  pub severity: LintSeverity,
  /// Start position in the source.
  pub start: Position,
  /// End position in the source.
  pub end: Position,
  /// Optional hint for fixing the issue.
  pub hint: Option<String>,
}

/// Severity level for lint diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
  /// An error that should be fixed.
  Error,
  /// A warning that may indicate a problem.
  Warning,
  /// Informational message.
  Info,
}

/// Adapter for linting files through the VFS.
pub struct VfsLintAdapter {
  /// The virtual file system.
  vfs: Arc<BundlerVirtualFS>,
}

impl VfsLintAdapter {
  /// Create a new VFS lint adapter.
  pub fn new(vfs: Arc<BundlerVirtualFS>) -> Self {
    Self { vfs }
  }

  /// Lint a single file, returning diagnostics with positions mapped to original source.
  pub async fn lint_file(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<Vec<LintDiagnostic>, AnyError> {
    // Load and transform the file through VFS
    let transformed = self.vfs.load(specifier).await?;

    // Get the media type for linting
    let media_type = transformed.media_type;

    // Only lint JavaScript/TypeScript files
    if !matches!(
      media_type,
      deno_ast::MediaType::JavaScript
        | deno_ast::MediaType::Jsx
        | deno_ast::MediaType::Mjs
        | deno_ast::MediaType::TypeScript
        | deno_ast::MediaType::Tsx
        | deno_ast::MediaType::Mts
    ) {
      return Ok(Vec::new());
    }

    // Run the linter on transformed code
    let raw_diagnostics =
      self.run_linter(specifier, &transformed.code, media_type)?;

    // Map diagnostic positions back to original source
    let mapped_diagnostics = raw_diagnostics
      .into_iter()
      .map(|diag| self.map_diagnostic(specifier, diag))
      .collect();

    Ok(mapped_diagnostics)
  }

  /// Lint multiple files.
  pub async fn lint_files(
    &self,
    specifiers: &[ModuleSpecifier],
  ) -> Result<Vec<LintDiagnostic>, AnyError> {
    let mut all_diagnostics = Vec::new();

    for specifier in specifiers {
      match self.lint_file(specifier).await {
        Ok(diagnostics) => all_diagnostics.extend(diagnostics),
        Err(e) => {
          // Log error but continue linting other files
          log::warn!("Failed to lint {}: {}", specifier, e);
        }
      }
    }

    Ok(all_diagnostics)
  }

  /// Run the linter on code and return raw diagnostics.
  fn run_linter(
    &self,
    specifier: &ModuleSpecifier,
    code: &str,
    media_type: deno_ast::MediaType,
  ) -> Result<Vec<LintDiagnostic>, AnyError> {
    // Parse the code
    let parsed = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: true,
      scope_analysis: true,
      maybe_syntax: None,
    })?;

    // For now, return empty diagnostics
    // Full implementation would use deno_lint crate
    // TODO: Integrate with deno_lint for actual linting
    let _ = parsed;
    Ok(Vec::new())
  }

  /// Map a diagnostic's positions from transformed to original source.
  fn map_diagnostic(
    &self,
    specifier: &ModuleSpecifier,
    mut diag: LintDiagnostic,
  ) -> LintDiagnostic {
    // Only map if the file was transformed
    if !self.vfs.needs_transform(specifier) {
      return diag;
    }

    // Map start and end positions
    let range = SourceRange {
      start: diag.start,
      end: diag.end,
    };
    let mapped = self.vfs.map_error_range(specifier, range);

    diag.start = mapped.start;
    diag.end = mapped.end;
    diag
  }

  /// Check if a file should be linted (based on extension).
  pub fn should_lint(&self, specifier: &ModuleSpecifier) -> bool {
    let path = specifier.path();

    // Standard lintable extensions
    let standard_extensions = [".js", ".jsx", ".ts", ".tsx", ".mjs", ".mts"];
    if standard_extensions.iter().any(|ext| path.ends_with(ext)) {
      return true;
    }

    // Check if VFS can transform this file to lintable code
    self.vfs.needs_transform(specifier)
  }

  /// Get the original source for a file (for diagnostic display).
  pub fn get_original_source(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<String> {
    self.vfs.get_original_source(specifier)
  }
}

/// Format a lint diagnostic for display.
pub fn format_diagnostic(diag: &LintDiagnostic) -> String {
  let severity = match diag.severity {
    LintSeverity::Error => "error",
    LintSeverity::Warning => "warning",
    LintSeverity::Info => "info",
  };

  let mut output = format!(
    "{} at {}:{}:{}\n  {} ({})",
    severity,
    diag.specifier,
    diag.start.line + 1,
    diag.start.column + 1,
    diag.message,
    diag.code,
  );

  if let Some(hint) = &diag.hint {
    output.push_str(&format!("\n  hint: {}", hint));
  }

  output
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_lint_severity() {
    assert_eq!(LintSeverity::Error, LintSeverity::Error);
    assert_ne!(LintSeverity::Error, LintSeverity::Warning);
  }

  #[test]
  fn test_format_diagnostic() {
    let diag = LintDiagnostic {
      specifier: ModuleSpecifier::parse("file:///app/test.ts").unwrap(),
      message: "Unused variable".to_string(),
      code: "no-unused-vars".to_string(),
      severity: LintSeverity::Warning,
      start: Position {
        line: 5,
        column: 10,
      },
      end: Position {
        line: 5,
        column: 15,
      },
      hint: Some("Remove or use the variable".to_string()),
    };

    let formatted = format_diagnostic(&diag);
    assert!(formatted.contains("warning"));
    assert!(formatted.contains("test.ts"));
    assert!(formatted.contains("6:11")); // 1-indexed
    assert!(formatted.contains("Unused variable"));
    assert!(formatted.contains("no-unused-vars"));
    assert!(formatted.contains("hint:"));
  }

  #[test]
  fn test_should_lint_standard_extensions() {
    let vfs = Arc::new(BundlerVirtualFS::passthrough());
    let adapter = VfsLintAdapter::new(vfs);

    assert!(
      adapter
        .should_lint(&ModuleSpecifier::parse("file:///app/test.ts").unwrap())
    );
    assert!(
      adapter
        .should_lint(&ModuleSpecifier::parse("file:///app/test.js").unwrap())
    );
    assert!(
      adapter
        .should_lint(&ModuleSpecifier::parse("file:///app/test.tsx").unwrap())
    );
    assert!(
      adapter
        .should_lint(&ModuleSpecifier::parse("file:///app/test.jsx").unwrap())
    );
    assert!(
      adapter
        .should_lint(&ModuleSpecifier::parse("file:///app/test.mjs").unwrap())
    );
    assert!(
      adapter
        .should_lint(&ModuleSpecifier::parse("file:///app/test.mts").unwrap())
    );
  }
}
