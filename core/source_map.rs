// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! This mod provides functions to remap a `JsError` based on a source map.

use crate::resolve_url;
pub use sourcemap::SourceMap;
use std::collections::HashMap;
use std::str;

pub trait SourceMapGetter {
  /// Returns the raw source map file.
  fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>>;
  fn get_source_line(
    &self,
    file_name: &str,
    line_number: usize,
  ) -> Option<String>;
}

#[derive(Debug, Default)]
pub struct SourceMapCache {
  maps: HashMap<String, Option<SourceMap>>,
  source_lines: HashMap<(String, i64), Option<String>>,
}

pub fn apply_source_map<G: SourceMapGetter + ?Sized>(
  file_name: String,
  line_number: i64,
  column_number: i64,
  cache: &mut SourceMapCache,
  getter: &G,
) -> (String, i64, i64) {
  // Lookup expects 0-based line and column numbers, but ours are 1-based.
  let line_number = line_number - 1;
  let column_number = column_number - 1;

  let default_pos = (file_name.clone(), line_number, column_number);
  let maybe_source_map =
    cache.maps.entry(file_name.clone()).or_insert_with(|| {
      getter
        .get_source_map(&file_name)
        .and_then(|raw_source_map| SourceMap::from_slice(&raw_source_map).ok())
    });
  let (file_name, line_number, column_number) = match maybe_source_map {
    None => default_pos,
    Some(source_map) => {
      match source_map.lookup_token(line_number as u32, column_number as u32) {
        None => default_pos,
        Some(token) => match token.get_source() {
          None => default_pos,
          Some(source_file_name) => {
            // The `source_file_name` written by tsc in the source map is
            // sometimes only the basename of the URL, or has unwanted `<`/`>`
            // around it. Use the `file_name` we get from V8 if
            // `source_file_name` does not parse as a URL.
            let file_name = match resolve_url(source_file_name) {
              Ok(m) if m.scheme() == "blob" => file_name,
              Ok(m) => m.to_string(),
              Err(_) => file_name,
            };
            (
              file_name,
              i64::from(token.get_src_line()),
              i64::from(token.get_src_col()),
            )
          }
        },
      }
    }
  };
  (file_name, line_number + 1, column_number + 1)
}

const MAX_SOURCE_LINE_LENGTH: usize = 150;

pub fn get_source_line<G: SourceMapGetter + ?Sized>(
  file_name: &str,
  line_number: i64,
  cache: &mut SourceMapCache,
  getter: &G,
) -> Option<String> {
  cache
    .source_lines
    .entry((file_name.to_string(), line_number))
    .or_insert_with(|| {
      // Source lookup expects a 0-based line number, ours are 1-based.
      let s = getter.get_source_line(file_name, (line_number - 1) as usize);
      s.filter(|s| s.len() <= MAX_SOURCE_LINE_LENGTH)
    })
    .clone()
}
