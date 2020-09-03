// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//! This mod provides functions to remap a deno_core::deno_core::JSError based on a source map
use sourcemap::SourceMap;
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

/// Cached filename lookups. The key can be None if a previous lookup failed to
/// find a SourceMap.
pub type CachedMaps = HashMap<String, Option<SourceMap>>;

/// Apply a source map to a deno_core::JSError, returning a JSError where file
/// names and line/column numbers point to the location in the original source,
/// rather than the transpiled source code.
pub fn apply_source_map<G: SourceMapGetter>(
  js_error: &deno_core::JSError,
  getter: &G,
) -> deno_core::JSError {
  // Note that js_error.frames has already been source mapped in
  // prepareStackTrace().
  let mut mappings_map: CachedMaps = HashMap::new();

  let (script_resource_name, line_number, start_column) =
    get_maybe_orig_position(
      js_error.script_resource_name.clone(),
      js_error.line_number,
      // start_column is 0-based, we need 1-based here.
      js_error.start_column.map(|n| n + 1),
      &mut mappings_map,
      getter,
    );
  let start_column = start_column.map(|n| n - 1);
  // It is better to just move end_column to be the same distance away from
  // start column because sometimes the code point is not available in the
  // source file map.
  let end_column = match js_error.end_column {
    Some(ec) => {
      if let Some(sc) = start_column {
        Some(ec - (js_error.start_column.unwrap() - sc))
      } else {
        None
      }
    }
    _ => None,
  };
  // if there is a source line that we might be different in the source file, we
  // will go fetch it from the getter
  let source_line = match line_number {
    Some(ln)
      if js_error.source_line.is_some() && script_resource_name.is_some() =>
    {
      getter.get_source_line(
        &js_error.script_resource_name.clone().unwrap(),
        // Getter expects 0-based line numbers, but ours are 1-based.
        ln as usize - 1,
      )
    }
    _ => js_error.source_line.clone(),
  };

  deno_core::JSError {
    message: js_error.message.clone(),
    source_line,
    script_resource_name,
    line_number,
    start_column,
    end_column,
    frames: js_error.frames.clone(),
    formatted_frames: js_error.formatted_frames.clone(),
  }
}

fn get_maybe_orig_position<G: SourceMapGetter>(
  file_name: Option<String>,
  line_number: Option<i64>,
  column_number: Option<i64>,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> (Option<String>, Option<i64>, Option<i64>) {
  match (file_name, line_number, column_number) {
    (Some(file_name_v), Some(line_v), Some(column_v)) => {
      let (file_name, line_number, column_number) =
        get_orig_position(file_name_v, line_v, column_v, mappings_map, getter);
      (Some(file_name), Some(line_number), Some(column_number))
    }
    _ => (None, None, None),
  }
}

pub fn get_orig_position<G: SourceMapGetter>(
  file_name: String,
  line_number: i64,
  column_number: i64,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> (String, i64, i64) {
  let maybe_source_map = get_mappings(&file_name, mappings_map, getter);
  let default_pos = (file_name, line_number, column_number);

  // Lookup expects 0-based line and column numbers, but ours are 1-based.
  let line_number = line_number - 1;
  let column_number = column_number - 1;

  match maybe_source_map {
    None => default_pos,
    Some(source_map) => {
      match source_map.lookup_token(line_number as u32, column_number as u32) {
        None => default_pos,
        Some(token) => match token.get_source() {
          None => default_pos,
          Some(original) => (
            original.to_string(),
            i64::from(token.get_src_line()) + 1,
            i64::from(token.get_src_col()) + 1,
          ),
        },
      }
    }
  }
}

fn get_mappings<'a, G: SourceMapGetter>(
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
fn parse_map_string<G: SourceMapGetter>(
  file_name: &str,
  getter: &G,
) -> Option<SourceMap> {
  getter
    .get_source_map(file_name)
    .and_then(|raw_source_map| SourceMap::from_slice(&raw_source_map).ok())
}

#[cfg(test)]
mod tests {
  use super::*;

  struct MockSourceMapGetter {}

  impl SourceMapGetter for MockSourceMapGetter {
    fn get_source_map(&self, file_name: &str) -> Option<Vec<u8>> {
      let s = match file_name {
        "foo_bar.ts" => {
          r#"{"sources": ["foo_bar.ts"], "mappings":";;;IAIA,OAAO,CAAC,GAAG,CAAC,qBAAqB,EAAE,EAAE,CAAC,OAAO,CAAC,CAAC;IAC/C,OAAO,CAAC,GAAG,CAAC,eAAe,EAAE,IAAI,CAAC,QAAQ,CAAC,IAAI,CAAC,CAAC;IACjD,OAAO,CAAC,GAAG,CAAC,WAAW,EAAE,IAAI,CAAC,QAAQ,CAAC,EAAE,CAAC,CAAC;IAE3C,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC"}"#
        }
        "bar_baz.ts" => {
          r#"{"sources": ["bar_baz.ts"], "mappings":";;;IAEA,CAAC,KAAK,IAAI,EAAE;QACV,MAAM,GAAG,GAAG,sDAAa,OAAO,2BAAC,CAAC;QAClC,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC;IACnB,CAAC,CAAC,EAAE,CAAC;IAEQ,QAAA,GAAG,GAAG,KAAK,CAAC;IAEzB,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC"}"#
        }
        _ => return None,
      };
      Some(s.as_bytes().to_owned())
    }

    fn get_source_line(
      &self,
      file_name: &str,
      line_number: usize,
    ) -> Option<String> {
      let s = match file_name {
        "foo_bar.ts" => vec![
          "console.log('foo');",
          "console.log('foo');",
          "console.log('foo');",
          "console.log('foo');",
          "console.log('foo');",
        ],
        _ => return None,
      };
      if s.len() > line_number {
        Some(s[line_number].to_string())
      } else {
        None
      }
    }
  }

  #[test]
  fn apply_source_map_line() {
    let e = deno_core::JSError {
      message: "TypeError: baz".to_string(),
      source_line: Some("foo".to_string()),
      script_resource_name: Some("foo_bar.ts".to_string()),
      line_number: Some(4),
      start_column: Some(16),
      end_column: None,
      frames: vec![],
      formatted_frames: vec![],
    };
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&e, &getter);
    assert_eq!(actual.source_line, Some("console.log('foo');".to_string()));
  }
}
