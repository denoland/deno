// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
//! This mod adds source maps and ANSI color display to deno::JSError.
use crate::ansi;
use deno::JSError;
use deno::StackFrame;
use source_map_mappings::parse_mappings;
use source_map_mappings::Bias;
use source_map_mappings::Mappings;
use std::collections::HashMap;
use std::fmt;
use std::str;

/// Wrapper around JSError which provides color to_string.
pub struct JSErrorColor<'a>(pub &'a JSError);

struct StackFrameColor<'a>(&'a StackFrame);

pub trait SourceMapGetter {
  /// Returns the raw source map file.
  fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>>;
}

/// Cached filename lookups. The key can be None if a previous lookup failed to
/// find a SourceMap.
type CachedMaps = HashMap<String, Option<SourceMap>>;

struct SourceMap {
  mappings: Mappings,
  sources: Vec<String>,
}

impl<'a> fmt::Display for StackFrameColor<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let frame = self.0;
    // Note when we print to string, we change from 0-indexed to 1-indexed.
    let function_name = ansi::italic_bold(frame.function_name.clone());
    let script_line_column =
      format_script_line_column(&frame.script_name, frame.line, frame.column);

    if !frame.function_name.is_empty() {
      write!(f, "    at {} ({})", function_name, script_line_column)
    } else if frame.is_eval {
      write!(f, "    at eval ({})", script_line_column)
    } else {
      write!(f, "    at {}", script_line_column)
    }
  }
}

fn format_script_line_column(
  script_name: &str,
  line: i64,
  column: i64,
) -> String {
  // TODO match this style with how typescript displays errors.
  let line = ansi::yellow((1 + line).to_string());
  let column = ansi::yellow((1 + column).to_string());
  let script_name = ansi::cyan(script_name.to_string());
  format!("{}:{}:{}", script_name, line, column)
}

impl<'a> fmt::Display for JSErrorColor<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let e = self.0;
    if e.script_resource_name.is_some() {
      let script_resource_name = e.script_resource_name.as_ref().unwrap();
      // Avoid showing internal code from gen/cli/bundle/main.js
      if script_resource_name != "gen/cli/bundle/main.js"
        && script_resource_name != "gen/cli/bundle/compiler.js"
      {
        if e.line_number.is_some() && e.start_column.is_some() {
          assert!(e.line_number.is_some());
          assert!(e.start_column.is_some());
          let script_line_column = format_script_line_column(
            script_resource_name,
            e.line_number.unwrap() - 1,
            e.start_column.unwrap() - 1,
          );
          write!(f, "{}", script_line_column)?;
        }
        if e.source_line.is_some() {
          write!(f, "\n{}\n", e.source_line.as_ref().unwrap())?;
          let mut s = String::new();
          for i in 0..e.end_column.unwrap() {
            if i >= e.start_column.unwrap() {
              s.push('^');
            } else {
              s.push(' ');
            }
          }
          writeln!(f, "{}", ansi::red_bold(s))?;
        }
      }
    }

    write!(f, "{}", ansi::bold(e.message.clone()))?;

    for frame in &e.frames {
      write!(f, "\n{}", StackFrameColor(&frame).to_string())?;
    }
    Ok(())
  }
}

impl SourceMap {
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

fn frame_apply_source_map<G: SourceMapGetter>(
  frame: &StackFrame,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> StackFrame {
  let maybe_sm = get_mappings(frame.script_name.as_ref(), mappings_map, getter);
  let frame_pos = (
    frame.script_name.to_owned(),
    frame.line as i64,
    frame.column as i64,
  );
  let (script_name, line, column) = match maybe_sm {
    None => frame_pos,
    Some(sm) => match sm.mappings.original_location_for(
      frame.line as u32,
      frame.column as u32,
      Bias::default(),
    ) {
      None => frame_pos,
      Some(mapping) => match &mapping.original {
        None => frame_pos,
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
  };

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
  JSError {
    message: js_error.message.clone(),
    frames,
    error_level: js_error.error_level,
    source_line: js_error.source_line.clone(),
    // TODO the following need to be source mapped:
    script_resource_name: js_error.script_resource_name.clone(),
    line_number: js_error.line_number,
    start_position: js_error.start_position,
    end_position: js_error.end_position,
    start_column: js_error.start_column,
    end_column: js_error.end_column,
  }
}

// The bundle does not get built for 'cargo check', so we don't embed the
// bundle source map.
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

fn get_mappings<'a, G: SourceMapGetter>(
  script_name: &str,
  mappings_map: &'a mut CachedMaps,
  getter: &G,
) -> &'a Option<SourceMap> {
  mappings_map
    .entry(script_name.to_string())
    .or_insert_with(|| parse_map_string(script_name, getter))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::ansi::strip_ansi_codes;

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
  }

  #[test]
  fn js_error_to_string() {
    let e = error1();
    assert_eq!("Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2", strip_ansi_codes(&e.to_string()));
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
