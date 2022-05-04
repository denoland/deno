// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! This mod provides functions to remap a `JsError` based on a source map.

use crate::resolve_url;
pub use sourcemap::SourceMap;
use std::collections::HashMap;
use std::str;

pub trait SourceMapGetter: Sync + Send {
  /// Returns the raw source map file.
  fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>>;
  fn get_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String>;
}

/// Cached filename lookups. The key can be None if a previous lookup failed to
/// find a SourceMap.
pub type CachedMaps = HashMap<String, Option<SourceMap>>;

pub fn apply_source_map<G: SourceMapGetter + ?Sized>(
  file_name: String,
  line_number: i64,
  column_number: i64,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> (String, i64, i64, Option<String>) {
  // Lookup expects 0-based line and column numbers, but ours are 1-based.
  let line_number = line_number - 1;
  let column_number = column_number - 1;

  let default_pos = (file_name.clone(), line_number, column_number, None);
  let maybe_source_map = get_mappings(&file_name, mappings_map, getter);
  let (mapped_file_name, line_number, column_number, mut source_line) =
    match maybe_source_map {
      None => default_pos,
      Some(source_map) => {
        match source_map.lookup_token(line_number as u32, column_number as u32)
        {
          None => default_pos,
          Some(token) => match token.get_source() {
            None => default_pos,
            Some(source_file_name) => {
              // sometimes source files are relative, in this case, we will use
              // the file_name as a base and the source_file_name joined on to
              // it, if this doesn't work
              let file_name = match resolve_url(&file_name) {
                // we preserve the blob URLs
                Ok(s) if s.scheme() == "blob" => file_name.clone(),
                Ok(s) => s
                  .join(source_file_name)
                  .map(|u| u.to_string())
                  .unwrap_or_else(|_| file_name.clone()),
                _ => file_name.clone(),
              };
              let source_line =
                if let Some(source_view) = token.get_source_view() {
                  source_view
                    .get_line(token.get_src_line())
                    .map(|s| s.to_string())
                } else {
                  None
                };
              (
                file_name,
                i64::from(token.get_src_line()),
                i64::from(token.get_src_col()),
                source_line,
              )
            }
          },
        }
      }
    };
  if file_name != mapped_file_name {
    source_line = source_line.or_else(|| {
      getter.get_source_line(&mapped_file_name, line_number as usize)
    });
  }
  (
    mapped_file_name,
    line_number + 1,
    column_number + 1,
    source_line,
  )
}

fn get_mappings<'a, G: SourceMapGetter + ?Sized>(
  file_name: &str,
  mappings_map: &'a mut CachedMaps,
  getter: &G,
) -> &'a Option<SourceMap> {
  mappings_map
    .entry(file_name.to_string())
    .or_insert_with(|| parse_map_string(file_name, getter))
}

// TODO(kitsonk) parsed source maps should probably be cached in state in
// the module meta data.
fn parse_map_string<G: SourceMapGetter + ?Sized>(
  file_name: &str,
  getter: &G,
) -> Option<SourceMap> {
  getter
    .get_source_map(file_name)
    .and_then(|raw_source_map| SourceMap::from_slice(&raw_source_map).ok())
}
