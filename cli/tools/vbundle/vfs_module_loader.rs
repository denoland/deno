// Copyright 2018-2026 the Deno authors. MIT license.

//! VFS-backed module loader for transparent transformation support.
//!
//! This module provides a `ModuleLoader` implementation that uses the VFS
//! to load and transform modules on-demand. This enables `deno run` to
//! transparently handle non-JS files like `.svelte` or `.vue` that need
//! transformation before execution.
//!
//! # Usage
//!
//! The VFS module loader wraps an inner module loader and intercepts
//! requests for files that need transformation. For standard JS/TS files,
//! it delegates to the inner loader.
//!
//! ```ignore
//! let vfs = Arc::new(BundlerVirtualFS::new());
//! let inner_loader = /* existing module loader */;
//! let loader = VfsModuleLoader::new(vfs, inner_loader);
//! ```

use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::ModuleSource;
use deno_core::ModuleSourceCode;
use deno_core::ModuleType;
use deno_core::error::AnyError;

use super::virtual_fs::BundlerVirtualFS;

/// A module loader that uses the VFS for on-demand transformation.
pub struct VfsModuleLoader {
  /// The virtual file system for transformations.
  vfs: Arc<BundlerVirtualFS>,
  /// Whether to enable transformation (can be disabled for passthrough).
  enabled: bool,
}

impl VfsModuleLoader {
  /// Create a new VFS module loader.
  pub fn new(vfs: Arc<BundlerVirtualFS>) -> Self {
    Self { vfs, enabled: true }
  }

  /// Create a passthrough loader that doesn't transform.
  pub fn passthrough() -> Self {
    Self {
      vfs: Arc::new(BundlerVirtualFS::passthrough()),
      enabled: false,
    }
  }

  /// Check if this specifier needs VFS transformation.
  pub fn needs_transform(&self, specifier: &ModuleSpecifier) -> bool {
    self.enabled && self.vfs.needs_transform(specifier)
  }

  /// Load a module through the VFS.
  pub async fn load_transformed(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Result<ModuleSource, AnyError> {
    let transformed = self.vfs.load(specifier).await?;

    // Determine module type from the transformed media type
    let module_type = match transformed.media_type {
      deno_ast::MediaType::JavaScript
      | deno_ast::MediaType::Mjs
      | deno_ast::MediaType::Jsx => ModuleType::JavaScript,
      deno_ast::MediaType::TypeScript
      | deno_ast::MediaType::Mts
      | deno_ast::MediaType::Tsx
      | deno_ast::MediaType::Dts
      | deno_ast::MediaType::Dmts
      | deno_ast::MediaType::Dcts => ModuleType::JavaScript, // Will be transpiled
      deno_ast::MediaType::Json => ModuleType::Json,
      _ => ModuleType::JavaScript, // Default to JS for transformed content
    };

    Ok(ModuleSource::new(
      module_type,
      ModuleSourceCode::String(transformed.code.to_string().into()),
      specifier,
      None,
    ))
  }

  /// Get the VFS for external access.
  pub fn vfs(&self) -> &Arc<BundlerVirtualFS> {
    &self.vfs
  }
}

/// Configuration for VFS module loading.
#[derive(Debug, Clone, Default)]
pub struct VfsLoaderConfig {
  /// File extensions that should be transformed.
  pub transform_extensions: Vec<String>,
  /// Whether to cache transformed results.
  pub enable_cache: bool,
  /// Whether to generate source maps.
  pub source_maps: bool,
}

impl VfsLoaderConfig {
  /// Create a config for Svelte files.
  pub fn svelte() -> Self {
    Self {
      transform_extensions: vec![".svelte".to_string()],
      enable_cache: true,
      source_maps: true,
    }
  }

  /// Create a config for Vue files.
  pub fn vue() -> Self {
    Self {
      transform_extensions: vec![".vue".to_string()],
      enable_cache: true,
      source_maps: true,
    }
  }

  /// Add a custom extension to transform.
  pub fn with_extension(mut self, ext: impl Into<String>) -> Self {
    self.transform_extensions.push(ext.into());
    self
  }
}

/// Error position mapper for transformed files.
///
/// Maps error positions from transformed code back to original source.
pub struct ErrorPositionMapper {
  vfs: Arc<BundlerVirtualFS>,
}

impl ErrorPositionMapper {
  /// Create a new error position mapper.
  pub fn new(vfs: Arc<BundlerVirtualFS>) -> Self {
    Self { vfs }
  }

  /// Map an error position from transformed to original.
  pub fn map_position(
    &self,
    specifier: &ModuleSpecifier,
    line: u32,
    column: u32,
  ) -> (u32, u32) {
    // Use VFS source map to map position
    let position = super::source_map::Position { line, column };
    let mapped = self.vfs.map_error_position(specifier, position);
    (mapped.line, mapped.column)
  }

  /// Map an error range from transformed to original.
  pub fn map_range(
    &self,
    specifier: &ModuleSpecifier,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
  ) -> ((u32, u32), (u32, u32)) {
    let start = super::source_map::Position {
      line: start_line,
      column: start_column,
    };
    let end = super::source_map::Position {
      line: end_line,
      column: end_column,
    };
    let range = super::source_map::SourceRange { start, end };
    let mapped = self.vfs.map_error_range(specifier, range);
    (
      (mapped.start.line, mapped.start.column),
      (mapped.end.line, mapped.end.column),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_vfs_loader_config() {
    let config = VfsLoaderConfig::svelte().with_extension(".astro");
    assert!(config.transform_extensions.contains(&".svelte".to_string()));
    assert!(config.transform_extensions.contains(&".astro".to_string()));
    assert!(config.enable_cache);
    assert!(config.source_maps);
  }

  #[test]
  fn test_vfs_loader_passthrough() {
    let loader = VfsModuleLoader::passthrough();
    assert!(!loader.enabled);
  }
}
