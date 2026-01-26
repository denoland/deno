// Copyright 2018-2026 the Deno authors. MIT license.

//! Source map handling for the bundler VFS.
//!
//! This module provides utilities for:
//! - Parsing and storing source maps
//! - Mapping positions from transformed code back to original source
//! - Combining source maps when multiple transformations are chained

use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_core::error::AnyError;
use deno_core::sourcemap::SourceMap;

/// A position in source code (0-indexed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
  /// Line number (0-indexed).
  pub line: u32,
  /// Column number (0-indexed).
  pub column: u32,
}

impl Position {
  pub fn new(line: u32, column: u32) -> Self {
    Self { line, column }
  }
}

/// A range in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceRange {
  pub start: Position,
  pub end: Position,
}

impl SourceRange {
  pub fn new(start: Position, end: Position) -> Self {
    Self { start, end }
  }
}

/// Wrapper around a source map with helper methods.
#[derive(Debug, Clone)]
pub struct SourceMapInfo {
  /// The parsed source map.
  source_map: Arc<SourceMap>,
  /// The original source specifier.
  original_specifier: ModuleSpecifier,
  /// Cached original source content (if embedded in source map).
  original_source: Option<Arc<str>>,
}

impl SourceMapInfo {
  /// Create a new SourceMapInfo from a parsed source map.
  pub fn new(
    source_map: SourceMap,
    original_specifier: ModuleSpecifier,
  ) -> Self {
    // Try to extract original source from source map
    let original_source = source_map
      .get_source_contents(0)
      .map(|s| Arc::from(s.to_string()));

    Self {
      source_map: Arc::new(source_map),
      original_specifier,
      original_source,
    }
  }

  /// Parse a source map from JSON string.
  pub fn from_json(
    json: &str,
    original_specifier: ModuleSpecifier,
  ) -> Result<Self, AnyError> {
    let source_map = SourceMap::from_slice(json.as_bytes())
      .map_err(|e| deno_core::anyhow::anyhow!("Failed to parse source map: {}", e))?;
    Ok(Self::new(source_map, original_specifier))
  }

  /// Get the original specifier.
  pub fn original_specifier(&self) -> &ModuleSpecifier {
    &self.original_specifier
  }

  /// Get the original source content if available.
  pub fn original_source(&self) -> Option<&str> {
    self.original_source.as_deref()
  }

  /// Look up the original position for a generated position.
  pub fn lookup(&self, generated: Position) -> Option<Position> {
    let token = self
      .source_map
      .lookup_token(generated.line, generated.column)?;

    Some(Position {
      line: token.get_src_line(),
      column: token.get_src_col(),
    })
  }

  /// Look up the original range for a generated range.
  pub fn lookup_range(&self, generated: SourceRange) -> Option<SourceRange> {
    let start = self.lookup(generated.start)?;
    let end = self.lookup(generated.end)?;
    Some(SourceRange::new(start, end))
  }

  /// Get the underlying source map.
  pub fn source_map(&self) -> &SourceMap {
    &self.source_map
  }
}

/// Cache for source maps indexed by specifier.
#[derive(Debug, Default)]
pub struct SourceMapCache {
  maps: HashMap<ModuleSpecifier, SourceMapInfo>,
}

impl SourceMapCache {
  pub fn new() -> Self {
    Self::default()
  }

  /// Insert a source map into the cache.
  pub fn insert(&mut self, specifier: ModuleSpecifier, info: SourceMapInfo) {
    self.maps.insert(specifier, info);
  }

  /// Get a source map from the cache.
  pub fn get(&self, specifier: &ModuleSpecifier) -> Option<&SourceMapInfo> {
    self.maps.get(specifier)
  }

  /// Check if a source map exists for the specifier.
  pub fn contains(&self, specifier: &ModuleSpecifier) -> bool {
    self.maps.contains_key(specifier)
  }

  /// Remove a source map from the cache.
  pub fn remove(&mut self, specifier: &ModuleSpecifier) -> Option<SourceMapInfo> {
    self.maps.remove(specifier)
  }

  /// Clear all cached source maps.
  pub fn clear(&mut self) {
    self.maps.clear();
  }

  /// Get the number of cached source maps.
  pub fn len(&self) -> usize {
    self.maps.len()
  }

  /// Check if the cache is empty.
  pub fn is_empty(&self) -> bool {
    self.maps.is_empty()
  }

  /// Map a position in generated code to the original source.
  pub fn map_position(
    &self,
    specifier: &ModuleSpecifier,
    pos: Position,
  ) -> Position {
    if let Some(info) = self.get(specifier) {
      info.lookup(pos).unwrap_or(pos)
    } else {
      pos
    }
  }

  /// Map a range in generated code to the original source.
  pub fn map_range(
    &self,
    specifier: &ModuleSpecifier,
    range: SourceRange,
  ) -> SourceRange {
    if let Some(info) = self.get(specifier) {
      info.lookup_range(range).unwrap_or(range)
    } else {
      range
    }
  }
}

/// Convert a byte offset to a Position in source text.
pub fn offset_to_position(source: &str, offset: usize) -> Position {
  let mut line = 0u32;
  let mut column = 0u32;
  let mut current_offset = 0usize;

  for ch in source.chars() {
    if current_offset >= offset {
      break;
    }
    if ch == '\n' {
      line += 1;
      column = 0;
    } else {
      column += 1;
    }
    current_offset += ch.len_utf8();
  }

  Position { line, column }
}

/// Convert a Position to a byte offset in source text.
pub fn position_to_offset(source: &str, pos: Position) -> Option<usize> {
  let mut current_line = 0u32;
  let mut current_col = 0u32;
  let mut offset = 0usize;

  for ch in source.chars() {
    if current_line == pos.line && current_col == pos.column {
      return Some(offset);
    }
    if ch == '\n' {
      if current_line == pos.line {
        // Position is past end of line
        return None;
      }
      current_line += 1;
      current_col = 0;
    } else {
      current_col += 1;
    }
    offset += ch.len_utf8();
  }

  // Handle position at end of file
  if current_line == pos.line && current_col == pos.column {
    return Some(offset);
  }

  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_offset_to_position() {
    let source = "line1\nline2\nline3";

    assert_eq!(offset_to_position(source, 0), Position::new(0, 0));
    assert_eq!(offset_to_position(source, 5), Position::new(0, 5));
    assert_eq!(offset_to_position(source, 6), Position::new(1, 0));
    assert_eq!(offset_to_position(source, 12), Position::new(2, 0));
  }

  #[test]
  fn test_position_to_offset() {
    let source = "line1\nline2\nline3";

    assert_eq!(position_to_offset(source, Position::new(0, 0)), Some(0));
    assert_eq!(position_to_offset(source, Position::new(0, 5)), Some(5));
    assert_eq!(position_to_offset(source, Position::new(1, 0)), Some(6));
    assert_eq!(position_to_offset(source, Position::new(2, 0)), Some(12));
    assert_eq!(position_to_offset(source, Position::new(2, 5)), Some(17));
  }

  #[test]
  fn test_source_map_cache() {
    let cache = SourceMapCache::new();
    let spec = ModuleSpecifier::parse("file:///test.ts").unwrap();

    // Without source map, position should be unchanged
    let pos = Position::new(10, 5);
    assert_eq!(cache.map_position(&spec, pos), pos);
  }
}
