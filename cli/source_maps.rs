// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
//! This mod provides functions to remap a deno::JSError based on a source map
use deno::JSError;
use deno::StackFrame;
use serde_json;
use source_map_mappings::parse_mappings;
use source_map_mappings::Bias;
use source_map_mappings::Mappings;
use std::collections::HashMap;
use std::str;

pub trait SourceMapGetter {
  /// Returns the raw source map file.
  fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>>;
  fn get_source_line(&self, script_name: &str, line: usize) -> Option<String>;
}

/// Cached filename lookups. The key can be None if a previous lookup failed to
/// find a SourceMap.
type CachedMaps = HashMap<String, Option<SourceMap>>;

struct SourceMap {
  mappings: Mappings,
  sources: Vec<String>,
}

impl SourceMap {
  /// Take a JSON string and attempt to decode it, returning an optional
  /// instance of `SourceMap`.
  fn from_json(json_str: &str) -> Option<Self> {
    // Ugly. Maybe use serde_derive.
    match serde_json::from_str::<serde_json::Value>(json_str) {
      Ok(serde_json::Value::Object(map)) => match map["mappings"].as_str() {
        None => None,
        Some(mappings_str) => {
          match parse_mappings::<()>(mappings_str.as_bytes()) {
            Err(_) => None,
            Ok(mappings) => {
              if !map["sources"].is_array() {
                return None;
              }
              let sources_val = map["sources"].as_array().unwrap();
              let mut sources = Vec::<String>::new();

              for source_val in sources_val {
                match source_val.as_str() {
                  None => return None,
                  Some(source) => {
                    sources.push(source.to_string());
                  }
                }
              }

              Some(SourceMap { sources, mappings })
            }
          }
        }
      },
      _ => None,
    }
  }
}

// The bundle does not get built for 'cargo check', so we don't embed the
// bundle source map.  The built in source map is the source map for the main
// JavaScript bundle which is then used to create the snapshot.  Runtime stack
// traces can contain positions within the bundle which we will map to the
// original Deno TypeScript code.
#[cfg(feature = "check-only")]
fn builtin_source_map(_: &str) -> Option<Vec<u8>> {
  None
}

#[cfg(not(feature = "check-only"))]
fn builtin_source_map(script_name: &str) -> Option<Vec<u8>> {
  match script_name {
    "gen/cli/bundle/main.js" => Some(
      include_bytes!(concat!(
        env!("GN_OUT_DIR"),
        "/gen/cli/bundle/main.js.map"
      )).to_vec(),
    ),
    "gen/cli/bundle/compiler.js" => Some(
      include_bytes!(concat!(
        env!("GN_OUT_DIR"),
        "/gen/cli/bundle/compiler.js.map"
      )).to_vec(),
    ),
    _ => None,
  }
}

/// Apply a source map to a JSError, returning a JSError where the filenames,
/// the lines and the columns point to their original source location, not their
/// transpiled location if applicable.
pub fn apply_source_map<G: SourceMapGetter>(
  js_error: &JSError,
  getter: &G,
) -> JSError {
  let mut mappings_map: CachedMaps = HashMap::new();

  let mut frames = Vec::<StackFrame>::new();
  for frame in &js_error.frames {
    let f = frame_apply_source_map(&frame, &mut mappings_map, getter);
    frames.push(f);
  }

  let (script_resource_name, line_number, start_column) =
    get_maybe_orig_position(
      js_error.script_resource_name.clone(),
      js_error.line_number,
      js_error.start_column,
      &mut mappings_map,
      getter,
    );
  // It is better to just move end_column to be the same distance away from
  // start column because sometimes the code point is not available in the
  // source file map.
  let end_column = match js_error.end_column {
    Some(ec) => {
      if start_column.is_some() {
        Some(ec - (js_error.start_column.unwrap() - start_column.unwrap()))
      } else {
        None
      }
    }
    _ => None,
  };
  // if there is a source line that we might be different in the source file, we
  // will go fetch it from the getter
  let source_line = if js_error.source_line.is_some()
    && script_resource_name.is_some()
    && line_number.is_some()
  {
    getter.get_source_line(
      &js_error.script_resource_name.clone().unwrap(),
      line_number.unwrap() as usize,
    )
  } else {
    js_error.source_line.clone()
  };

  JSError {
    message: js_error.message.clone(),
    frames,
    error_level: js_error.error_level,
    source_line,
    script_resource_name,
    line_number,
    start_column,
    end_column,
    // These are difficult to map to their original position and they are not
    // currently used in any output, so we don't remap them.
    start_position: js_error.start_position,
    end_position: js_error.end_position,
  }
}

fn frame_apply_source_map<G: SourceMapGetter>(
  frame: &StackFrame,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> StackFrame {
  let (script_name, line, column) = get_orig_position(
    frame.script_name.to_string(),
    frame.line,
    frame.column,
    mappings_map,
    getter,
  );

  StackFrame {
    script_name,
    function_name: frame.function_name.clone(),
    line,
    column,
    is_eval: frame.is_eval,
    is_constructor: frame.is_constructor,
    is_wasm: frame.is_wasm,
  }
}

fn get_maybe_orig_position<G: SourceMapGetter>(
  script_name: Option<String>,
  line: Option<i64>,
  column: Option<i64>,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> (Option<String>, Option<i64>, Option<i64>) {
  match (script_name, line, column) {
    (Some(script_name_v), Some(line_v), Some(column_v)) => {
      let (script_name, line, column) = get_orig_position(
        script_name_v,
        line_v - 1,
        column_v,
        mappings_map,
        getter,
      );
      (Some(script_name), Some(line), Some(column))
    }
    _ => (None, None, None),
  }
}

fn get_orig_position<G: SourceMapGetter>(
  script_name: String,
  line: i64,
  column: i64,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> (String, i64, i64) {
  let maybe_sm = get_mappings(&script_name, mappings_map, getter);
  let default_pos = (script_name, line, column);

  match maybe_sm {
    None => default_pos,
    Some(sm) => match sm.mappings.original_location_for(
      line as u32,
      column as u32,
      Bias::default(),
    ) {
      None => default_pos,
      Some(mapping) => match &mapping.original {
        None => default_pos,
        Some(original) => {
          let orig_source = sm.sources[original.source as usize].clone();
          (
            orig_source,
            i64::from(original.original_line),
            i64::from(original.original_column),
          )
        }
      },
    },
  }
}

fn get_mappings<'a, G: SourceMapGetter>(
  script_name: &str,
  mappings_map: &'a mut CachedMaps,
  getter: &G,
) -> &'a Option<SourceMap> {
  mappings_map
    .entry(script_name.to_string())
    .or_insert_with(|| parse_map_string(script_name, getter))
}

// TODO(kitsonk) parsed source maps should probably be cached in state in
// the module meta data.
fn parse_map_string<G: SourceMapGetter>(
  script_name: &str,
  getter: &G,
) -> Option<SourceMap> {
  builtin_source_map(script_name)
    .or_else(|| getter.get_source_map(script_name))
    .and_then(|raw_source_map| {
      SourceMap::from_json(str::from_utf8(&raw_source_map).unwrap())
    })
}

#[cfg(test)]
mod tests {
  use super::*;

  struct MockSourceMapGetter {}

  impl SourceMapGetter for MockSourceMapGetter {
    fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>> {
      let s = match script_name {
        "foo_bar.ts" => r#"{"sources": ["foo_bar.ts"], "mappings":";;;IAIA,OAAO,CAAC,GAAG,CAAC,qBAAqB,EAAE,EAAE,CAAC,OAAO,CAAC,CAAC;IAC/C,OAAO,CAAC,GAAG,CAAC,eAAe,EAAE,IAAI,CAAC,QAAQ,CAAC,IAAI,CAAC,CAAC;IACjD,OAAO,CAAC,GAAG,CAAC,WAAW,EAAE,IAAI,CAAC,QAAQ,CAAC,EAAE,CAAC,CAAC;IAE3C,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC"}"#,
        "bar_baz.ts" => r#"{"sources": ["bar_baz.ts"], "mappings":";;;IAEA,CAAC,KAAK,IAAI,EAAE;QACV,MAAM,GAAG,GAAG,sDAAa,OAAO,2BAAC,CAAC;QAClC,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC;IACnB,CAAC,CAAC,EAAE,CAAC;IAEQ,QAAA,GAAG,GAAG,KAAK,CAAC;IAEzB,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC"}"#,
        _ => return None,
      };
      Some(s.as_bytes().to_owned())
    }

    fn get_source_line(
      &self,
      script_name: &str,
      line: usize,
    ) -> Option<String> {
      let s = match script_name {
        "foo_bar.ts" => vec![
          "console.log('foo');",
          "console.log('foo');",
          "console.log('foo');",
          "console.log('foo');",
          "console.log('foo');",
        ],
        _ => return None,
      };
      if s.len() > line {
        Some(s[line].to_string())
      } else {
        None
      }
    }
  }

  fn error1() -> JSError {
    JSError {
      message: "Error: foo bar".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      error_level: None,
      start_column: None,
      end_column: None,
      frames: vec![
        StackFrame {
          line: 4,
          column: 16,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        StackFrame {
          line: 5,
          column: 20,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        StackFrame {
          line: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
      ],
    }
  }

  #[test]
  fn js_error_apply_source_map_1() {
    let e = error1();
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&e, &getter);
    let expected = JSError {
      message: "Error: foo bar".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      error_level: None,
      start_column: None,
      end_column: None,
      frames: vec![
        StackFrame {
          line: 5,
          column: 12,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        StackFrame {
          line: 4,
          column: 14,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        StackFrame {
          line: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
      ],
    };
    assert_eq!(actual, expected);
  }

  #[test]
  fn js_error_apply_source_map_2() {
    let e = JSError {
      message: "TypeError: baz".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      error_level: None,
      start_column: None,
      end_column: None,
      frames: vec![StackFrame {
        line: 11,
        column: 12,
        script_name: "gen/cli/bundle/main.js".to_string(),
        function_name: "setLogDebug".to_string(),
        is_eval: false,
        is_constructor: false,
        is_wasm: false,
      }],
    };
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&e, &getter);
    assert_eq!(actual.message, "TypeError: baz");
    // Because this is accessing the live bundle, this test might be more fragile
    assert_eq!(actual.frames.len(), 1);
    assert!(actual.frames[0].script_name.ends_with("js/util.ts"));
  }

  #[test]
  fn js_error_apply_source_map_line() {
    let e = JSError {
      message: "TypeError: baz".to_string(),
      source_line: Some("foo".to_string()),
      script_resource_name: Some("foo_bar.ts".to_string()),
      line_number: Some(4),
      start_position: None,
      end_position: None,
      error_level: None,
      start_column: Some(16),
      end_column: None,
      frames: vec![],
    };
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&e, &getter);
    assert_eq!(actual.source_line, Some("console.log('foo');".to_string()));
  }

  #[test]
  fn source_map_from_json() {
    let json = r#"{"version":3,"file":"error_001.js","sourceRoot":"","sources":["file:///Users/rld/src/deno/tests/error_001.ts"],"names":[],"mappings":"AAAA,SAAS,GAAG;IACV,MAAM,KAAK,CAAC,KAAK,CAAC,CAAC;AACrB,CAAC;AAED,SAAS,GAAG;IACV,GAAG,EAAE,CAAC;AACR,CAAC;AAED,GAAG,EAAE,CAAC"}"#;
    let sm = SourceMap::from_json(json).unwrap();
    assert_eq!(sm.sources.len(), 1);
    assert_eq!(
      sm.sources[0],
      "file:///Users/rld/src/deno/tests/error_001.ts"
    );
    let mapping = sm
      .mappings
      .original_location_for(1, 10, Bias::default())
      .unwrap();
    assert_eq!(mapping.generated_line, 1);
    assert_eq!(mapping.generated_column, 10);
    assert_eq!(
      mapping.original,
      Some(source_map_mappings::OriginalLocation {
        source: 0,
        original_line: 1,
        original_column: 8,
        name: None
      })
    );
  }
}
