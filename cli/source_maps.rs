// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//! This mod provides functions to remap a deno_core::deno_core::JSError based on a source map
use deno_core;
use deno_core::JSStackFrame;
use sourcemap::SourceMap;
use std::collections::HashMap;
use std::str;

pub trait SourceMapGetter {
  /// Returns the raw source map file.
  fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>>;
  fn get_source_line(
    &self,
    script_name: &str,
    line_number: usize,
  ) -> Option<String>;
}

/// Cached filename lookups. The key can be None if a previous lookup failed to
/// find a SourceMap.
pub type CachedMaps = HashMap<String, Option<SourceMap>>;

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
  if script_name.ends_with("CLI_SNAPSHOT.js") {
    Some(crate::js::CLI_SNAPSHOT_MAP.to_vec())
  } else if script_name.ends_with("COMPILER_SNAPSHOT.js") {
    Some(crate::js::COMPILER_SNAPSHOT_MAP.to_vec())
  } else {
    None
  }
}

/// Apply a source map to a deno_core::JSError, returning a JSError where file
/// names and line/column numbers point to the location in the original source,
/// rather than the transpiled source code.
pub fn apply_source_map<G: SourceMapGetter>(
  js_error: &deno_core::JSError,
  getter: &G,
) -> deno_core::JSError {
  let mut mappings_map: CachedMaps = HashMap::new();

  let mut frames = Vec::<JSStackFrame>::new();
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
        ln as usize,
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
    frames,
  }
}

fn frame_apply_source_map<G: SourceMapGetter>(
  frame: &JSStackFrame,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> JSStackFrame {
  let (script_name, line_number, column) = get_orig_position(
    frame.script_name.to_string(),
    frame.line_number,
    frame.column,
    mappings_map,
    getter,
  );

  JSStackFrame {
    script_name,
    function_name: frame.function_name.clone(),
    line_number,
    column,
    is_eval: frame.is_eval,
    is_constructor: frame.is_constructor,
  }
}

fn get_maybe_orig_position<G: SourceMapGetter>(
  script_name: Option<String>,
  line_number: Option<i64>,
  column: Option<i64>,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> (Option<String>, Option<i64>, Option<i64>) {
  match (script_name, line_number, column) {
    (Some(script_name_v), Some(line_v), Some(column_v)) => {
      let (script_name, line_number, column) = get_orig_position(
        script_name_v,
        line_v - 1,
        column_v,
        mappings_map,
        getter,
      );
      (Some(script_name), Some(line_number), Some(column))
    }
    _ => (None, None, None),
  }
}

pub fn get_orig_position<G: SourceMapGetter>(
  script_name: String,
  line_number: i64,
  column: i64,
  mappings_map: &mut CachedMaps,
  getter: &G,
) -> (String, i64, i64) {
  let maybe_source_map = get_mappings(&script_name, mappings_map, getter);
  let default_pos = (script_name, line_number, column);

  match maybe_source_map {
    None => default_pos,
    Some(source_map) => {
      match source_map.lookup_token(line_number as u32, column as u32) {
        None => default_pos,
        Some(token) => match token.get_source() {
          None => default_pos,
          Some(original) => (
            original.to_string(),
            i64::from(token.get_src_line()),
            i64::from(token.get_src_col()),
          ),
        },
      }
    }
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
    .and_then(|raw_source_map| SourceMap::from_slice(&raw_source_map).ok())
}

#[cfg(test)]
mod tests {
  use super::*;

  struct MockSourceMapGetter {}

  impl SourceMapGetter for MockSourceMapGetter {
    fn get_source_map(&self, script_name: &str) -> Option<Vec<u8>> {
      let s = match script_name {
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
      script_name: &str,
      line_number: usize,
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
      if s.len() > line_number {
        Some(s[line_number].to_string())
      } else {
        None
      }
    }
  }

  #[test]
  fn apply_source_map_1() {
    let core_js_error = deno_core::JSError {
      message: "Error: foo bar".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_column: None,
      end_column: None,
      frames: vec![
        JSStackFrame {
          line_number: 4,
          column: 16,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
        },
        JSStackFrame {
          line_number: 5,
          column: 20,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
        },
        JSStackFrame {
          line_number: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
        },
      ],
    };
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&core_js_error, &getter);
    let expected = deno_core::JSError {
      message: "Error: foo bar".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_column: None,
      end_column: None,
      frames: vec![
        JSStackFrame {
          line_number: 5,
          column: 12,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
        },
        JSStackFrame {
          line_number: 4,
          column: 14,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
        },
        JSStackFrame {
          line_number: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
        },
      ],
    };
    assert_eq!(actual, expected);
  }

  #[test]
  fn apply_source_map_2() {
    let e = deno_core::JSError {
      message: "TypeError: baz".to_string(),
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_column: None,
      end_column: None,
      frames: vec![JSStackFrame {
        line_number: 11,
        column: 12,
        script_name: "CLI_SNAPSHOT.js".to_string(),
        function_name: "setLogDebug".to_string(),
        is_eval: false,
        is_constructor: false,
      }],
    };
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&e, &getter);
    assert_eq!(actual.message, "TypeError: baz");
    // Because this is accessing the live bundle, this test might be more fragile
    assert_eq!(actual.frames.len(), 1);
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
    };
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&e, &getter);
    assert_eq!(actual.source_line, Some("console.log('foo');".to_string()));
  }
}
