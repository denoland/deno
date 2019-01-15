// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Note that source_map_mappings requires 0-indexed line and column numbers but
// V8 Exceptions are 1-indexed.

// TODO: This currently only applies to uncaught exceptions. It would be nice to
// also have source maps for situations like this:
//   const err = new Error("Boo!");
//   console.log(err.stack);
// It would require calling into Rust from Error.prototype.prepareStackTrace.

use serde_json;
use source_map_mappings::parse_mappings;
use source_map_mappings::Bias;
use source_map_mappings::Mappings;
use std::collections::HashMap;

pub trait SourceMapGetter {
  /// Returns the raw source map file.
  fn get_source_map(&self, script_name: &str) -> Option<String>;
}

struct SourceMap {
  mappings: Mappings,
  sources: Vec<String>,
}

/// Cached filename lookups. The key can be None if a previous lookup failed to
/// find a SourceMap.
type CachedMaps = HashMap<String, Option<SourceMap>>;

#[derive(Debug, PartialEq)]
pub struct StackFrame {
  pub line: u32,   // zero indexed
  pub column: u32, // zero indexed
  pub script_name: String,
  pub function_name: String,
  pub is_eval: bool,
  pub is_constructor: bool,
  pub is_wasm: bool,
}

#[derive(Debug, PartialEq)]
pub struct JSError {
  pub message: String,

  pub source_line: Option<String>,
  pub script_resource_name: Option<String>,
  pub line_number: Option<i64>,
  pub start_position: Option<i64>,
  pub end_position: Option<i64>,
  pub error_level: Option<i64>,
  pub start_column: Option<i64>,
  pub end_column: Option<i64>,

  pub frames: Vec<StackFrame>,
}

impl ToString for StackFrame {
  fn to_string(&self) -> String {
    // Note when we print to string, we change from 0-indexed to 1-indexed.
    let (line, column) = (self.line + 1, self.column + 1);
    if !self.function_name.is_empty() {
      format!(
        "    at {} ({}:{}:{})",
        self.function_name, self.script_name, line, column
      )
    } else if self.is_eval {
      format!("    at eval ({}:{}:{})", self.script_name, line, column)
    } else {
      format!("    at {}:{}:{}", self.script_name, line, column)
    }
  }
}

impl ToString for JSError {
  fn to_string(&self) -> String {
    // TODO Improve the formatting of these error messages.
    let mut s = String::new();

    if self.script_resource_name.is_some() {
      let script_resource_name = self.script_resource_name.as_ref().unwrap();
      // Avoid showing internal code from gen/bundle/main.js
      if script_resource_name != "gen/bundle/main.js" {
        s.push_str(script_resource_name);
        if self.line_number.is_some() {
          s.push_str(&format!(
            ":{}:{}",
            self.line_number.unwrap(),
            self.start_column.unwrap()
          ));
          assert!(self.start_column.is_some());
        }
        if self.source_line.is_some() {
          s.push_str("\n");
          s.push_str(self.source_line.as_ref().unwrap());
          s.push_str("\n\n");
        }
      }
    }

    s.push_str(&self.message);

    for frame in &self.frames {
      s.push_str("\n");
      s.push_str(&frame.to_string());
    }
    s
  }
}

impl StackFrame {
  // TODO Maybe use serde_derive?
  fn from_json_value(v: &serde_json::Value) -> Option<Self> {
    if !v.is_object() {
      return None;
    }
    let obj = v.as_object().unwrap();

    let line_v = &obj["line"];
    if !line_v.is_u64() {
      return None;
    }
    let line = line_v.as_u64().unwrap() as u32;

    let column_v = &obj["column"];
    if !column_v.is_u64() {
      return None;
    }
    let column = column_v.as_u64().unwrap() as u32;

    let script_name_v = &obj["scriptName"];
    if !script_name_v.is_string() {
      return None;
    }
    let script_name = String::from(script_name_v.as_str().unwrap());

    // Optional fields. See EncodeExceptionAsJSON() in libdeno.
    // Sometimes V8 doesn't provide all the frame information.

    let mut function_name = String::from(""); // default
    if obj.contains_key("functionName") {
      let function_name_v = &obj["functionName"];
      if function_name_v.is_string() {
        function_name = String::from(function_name_v.as_str().unwrap());
      }
    }

    let mut is_eval = false; // default
    if obj.contains_key("isEval") {
      let is_eval_v = &obj["isEval"];
      if is_eval_v.is_boolean() {
        is_eval = is_eval_v.as_bool().unwrap();
      }
    }

    let mut is_constructor = false; // default
    if obj.contains_key("isConstructor") {
      let is_constructor_v = &obj["isConstructor"];
      if is_constructor_v.is_boolean() {
        is_constructor = is_constructor_v.as_bool().unwrap();
      }
    }

    let mut is_wasm = false; // default
    if obj.contains_key("isWasm") {
      let is_wasm_v = &obj["isWasm"];
      if is_wasm_v.is_boolean() {
        is_wasm = is_wasm_v.as_bool().unwrap();
      }
    }

    Some(StackFrame {
      line: line - 1,
      column: column - 1,
      script_name: script_name,
      function_name,
      is_eval,
      is_constructor,
      is_wasm,
    })
  }

  fn apply_source_map(
    &self,
    mappings_map: &mut CachedMaps,
    getter: &dyn SourceMapGetter,
  ) -> StackFrame {
    let maybe_sm =
      get_mappings(self.script_name.as_ref(), mappings_map, getter);
    let frame_pos = (self.script_name.to_owned(), self.line, self.column);
    let (script_name, line, column) = match maybe_sm {
      None => frame_pos,
      Some(sm) => match sm.mappings.original_location_for(
        self.line,
        self.column,
        Bias::default(),
      ) {
        None => frame_pos,
        Some(mapping) => match &mapping.original {
          None => frame_pos,
          Some(original) => {
            let orig_source = sm.sources[original.source as usize].clone();
            (
              orig_source,
              original.original_line,
              original.original_column,
            )
          }
        },
      },
    };

    StackFrame {
      script_name,
      function_name: self.function_name.clone(),
      line,
      column,
      is_eval: self.is_eval,
      is_constructor: self.is_constructor,
      is_wasm: self.is_wasm,
    }
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

impl JSError {
  /// Creates a new JSError by parsing the raw exception JSON string from V8.
  pub fn from_v8_exception(json_str: &str) -> Option<Self> {
    let v = serde_json::from_str::<serde_json::Value>(json_str);
    if v.is_err() {
      return None;
    }
    let v = v.unwrap();

    if !v.is_object() {
      return None;
    }
    let obj = v.as_object().unwrap();

    let message_v = &obj["message"];
    if !message_v.is_string() {
      return None;
    }
    let message = String::from(message_v.as_str().unwrap());

    let source_line = obj
      .get("sourceLine")
      .and_then(|v| v.as_str().map(String::from));
    let script_resource_name = obj
      .get("scriptResourceName")
      .and_then(|v| v.as_str().map(String::from));
    let line_number = obj.get("lineNumber").and_then(|v| v.as_i64());
    let start_position = obj.get("startPosition").and_then(|v| v.as_i64());
    let end_position = obj.get("endPosition").and_then(|v| v.as_i64());
    let error_level = obj.get("errorLevel").and_then(|v| v.as_i64());
    let start_column = obj.get("startColumn").and_then(|v| v.as_i64());
    let end_column = obj.get("endColumn").and_then(|v| v.as_i64());

    let frames_v = &obj["frames"];
    if !frames_v.is_array() {
      return None;
    }
    let frame_values = frames_v.as_array().unwrap();

    let mut frames = Vec::<StackFrame>::new();
    for frame_v in frame_values {
      match StackFrame::from_json_value(frame_v) {
        None => return None,
        Some(frame) => frames.push(frame),
      }
    }

    Some(JSError {
      message,
      source_line,
      script_resource_name,
      line_number,
      start_position,
      end_position,
      error_level,
      start_column,
      end_column,
      frames,
    })
  }

  pub fn apply_source_map(&self, getter: &dyn SourceMapGetter) -> Self {
    let mut mappings_map: CachedMaps = HashMap::new();
    let mut frames = Vec::<StackFrame>::new();
    for frame in &self.frames {
      let f = frame.apply_source_map(&mut mappings_map, getter);
      frames.push(f);
    }
    JSError {
      message: self.message.clone(),
      frames,
      error_level: self.error_level,
      source_line: self.source_line.clone(),
      // TODO the following need to be source mapped:
      script_resource_name: self.script_resource_name.clone(),
      line_number: self.line_number,
      start_position: self.start_position,
      end_position: self.end_position,
      start_column: self.start_column,
      end_column: self.end_column,
    }
  }
}

fn parse_map_string(
  script_name: &str,
  getter: &dyn SourceMapGetter,
) -> Option<SourceMap> {
  match script_name {
    // The bundle does not get built for 'cargo check', so we don't embed the
    // bundle source map.
    #[cfg(not(feature = "check-only"))]
    "gen/bundle/main.js" => {
      let s =
        include_str!(concat!(env!("GN_OUT_DIR"), "/gen/bundle/main.js.map"));
      SourceMap::from_json(s)
    }
    _ => match getter.get_source_map(script_name) {
      None => None,
      Some(raw_source_map) => SourceMap::from_json(&raw_source_map),
    },
  }
}

fn get_mappings<'a>(
  script_name: &str,
  mappings_map: &'a mut CachedMaps,
  getter: &dyn SourceMapGetter,
) -> &'a Option<SourceMap> {
  mappings_map
    .entry(script_name.to_string())
    .or_insert_with(|| parse_map_string(script_name, getter))
}

#[cfg(test)]
mod tests {
  use super::*;

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
    fn get_source_map(&self, script_name: &str) -> Option<String> {
      let s = match script_name {
        "foo_bar.ts" => r#"{"sources": ["foo_bar.ts"], "mappings":";;;IAIA,OAAO,CAAC,GAAG,CAAC,qBAAqB,EAAE,EAAE,CAAC,OAAO,CAAC,CAAC;IAC/C,OAAO,CAAC,GAAG,CAAC,eAAe,EAAE,IAAI,CAAC,QAAQ,CAAC,IAAI,CAAC,CAAC;IACjD,OAAO,CAAC,GAAG,CAAC,WAAW,EAAE,IAAI,CAAC,QAAQ,CAAC,EAAE,CAAC,CAAC;IAE3C,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC"}"#,
        "bar_baz.ts" => r#"{"sources": ["bar_baz.ts"], "mappings":";;;IAEA,CAAC,KAAK,IAAI,EAAE;QACV,MAAM,GAAG,GAAG,sDAAa,OAAO,2BAAC,CAAC;QAClC,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC;IACnB,CAAC,CAAC,EAAE,CAAC;IAEQ,QAAA,GAAG,GAAG,KAAK,CAAC;IAEzB,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC"}"#,
        _ => return None,
      };
      Some(s.to_string())
    }
  }

  #[test]
  fn stack_frame_from_json_value_1() {
    let v = serde_json::from_str::<serde_json::Value>(
      r#"{
        "line":2,
        "column":11,
        "functionName":"foo",
        "scriptName":"/Users/rld/src/deno/tests/error_001.ts",
        "isEval":true,
        "isConstructor":false,
        "isWasm":false
      }"#,
    ).unwrap();
    let r = StackFrame::from_json_value(&v);
    assert_eq!(
      r,
      Some(StackFrame {
        line: 1,
        column: 10,
        script_name: "/Users/rld/src/deno/tests/error_001.ts".to_string(),
        function_name: "foo".to_string(),
        is_eval: true,
        is_constructor: false,
        is_wasm: false,
      })
    );
  }

  #[test]
  fn stack_frame_from_json_value_2() {
    let v = serde_json::from_str::<serde_json::Value>(
      r#"{
        "scriptName": "/Users/rld/src/deno/tests/error_001.ts",
        "line": 2,
        "column": 11
      }"#,
    ).unwrap();
    let r = StackFrame::from_json_value(&v);
    assert!(r.is_some());
    let f = r.unwrap();
    assert_eq!(f.line, 1);
    assert_eq!(f.column, 10);
    assert_eq!(f.script_name, "/Users/rld/src/deno/tests/error_001.ts");
  }

  #[test]
  fn js_error_from_v8_exception() {
    let r = JSError::from_v8_exception(
      r#"{
        "message":"Uncaught Error: bad",
        "frames":[
          {
            "line":2,
            "column":11,
            "functionName":"foo",
            "scriptName":"/Users/rld/src/deno/tests/error_001.ts",
            "isEval":true,
            "isConstructor":false,
            "isWasm":false
          }, {
            "line":5,
            "column":5,
            "functionName":"bar",
            "scriptName":"/Users/rld/src/deno/tests/error_001.ts",
            "isEval":true,
            "isConstructor":false,
            "isWasm":false
          }
        ]}"#,
    );
    assert!(r.is_some());
    let e = r.unwrap();
    assert_eq!(e.message, "Uncaught Error: bad");
    assert_eq!(e.frames.len(), 2);
    assert_eq!(
      e.frames[0],
      StackFrame {
        line: 1,
        column: 10,
        script_name: "/Users/rld/src/deno/tests/error_001.ts".to_string(),
        function_name: "foo".to_string(),
        is_eval: true,
        is_constructor: false,
        is_wasm: false,
      }
    )
  }

  #[test]
  fn js_error_from_v8_exception2() {
    let r = JSError::from_v8_exception(
      "{\"message\":\"Error: boo\",\"sourceLine\":\"throw Error('boo');\",\"scriptResourceName\":\"a.js\",\"lineNumber\":3,\"startPosition\":8,\"endPosition\":9,\"errorLevel\":8,\"startColumn\":6,\"endColumn\":7,\"isSharedCrossOrigin\":false,\"isOpaque\":false,\"frames\":[{\"line\":3,\"column\":7,\"functionName\":\"\",\"scriptName\":\"a.js\",\"isEval\":false,\"isConstructor\":false,\"isWasm\":false}]}"
    );
    assert!(r.is_some());
    let e = r.unwrap();
    assert_eq!(e.message, "Error: boo");
    assert_eq!(e.source_line, Some("throw Error('boo');".to_string()));
    assert_eq!(e.script_resource_name, Some("a.js".to_string()));
    assert_eq!(e.line_number, Some(3));
    assert_eq!(e.start_position, Some(8));
    assert_eq!(e.end_position, Some(9));
    assert_eq!(e.error_level, Some(8));
    assert_eq!(e.start_column, Some(6));
    assert_eq!(e.end_column, Some(7));
    assert_eq!(e.frames.len(), 1);
  }

  #[test]
  fn stack_frame_to_string() {
    let e = error1();
    assert_eq!("    at foo (foo_bar.ts:5:17)", e.frames[0].to_string());
    assert_eq!("    at qat (bar_baz.ts:6:21)", e.frames[1].to_string());
  }

  #[test]
  fn js_error_to_string() {
    let e = error1();
    assert_eq!("Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2", e.to_string());
  }

  #[test]
  fn js_error_apply_source_map_1() {
    let e = error1();
    let getter = MockSourceMapGetter {};
    let actual = e.apply_source_map(&getter);
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
        script_name: "gen/bundle/main.js".to_string(),
        function_name: "setLogDebug".to_string(),
        is_eval: false,
        is_constructor: false,
        is_wasm: false,
      }],
    };
    let getter = MockSourceMapGetter {};
    let actual = e.apply_source_map(&getter);
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
