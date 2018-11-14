// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use serde_json;
use serde_json::Value;
use source_map_mappings::parse_mappings;
use source_map_mappings::Bias;
use source_map_mappings::Mappings;
use std::collections::HashMap;

pub struct StackFrame {
  pub line_number: u32,
  pub column: u32,
  pub source_url: String,
  pub function_name: String,
  pub is_eval: bool,
  pub is_constructor: bool,
  pub is_wasm: bool,
}

pub struct JavaScriptError {
  pub message: String,
  pub stack_trace: Vec<StackFrame>,
}

fn main_map_mappings() -> (String, Vec<Value>) {
  let main_map_json =
    include_str!(concat!(env!("GN_OUT_DIR"), "/gen/bundle/main.js.map"));
  let main_map: serde_json::Value = serde_json::from_str(main_map_json).unwrap();
  let mappings = main_map["mappings"].as_str().unwrap().to_string();
  let sources = main_map["sources"].as_array().unwrap().to_vec();
  (mappings, sources)
}

fn get_original_source(index: u32) -> String {
  let (_, sources) = main_map_mappings();
  sources[index as usize].as_str().unwrap().to_string()
}

fn parse_map_string(
  source_url: &str,
  get_map: &Fn(&str) -> String,
) -> Option<Mappings> {
  let (mappings, _) = main_map_mappings();
  let source_map = match source_url {
    "gen/bundle/main.js" => mappings,
    _ => get_map(source_url),
  };
  parse_mappings::<()>(source_map.as_bytes()).ok()
}

fn get_mappings<'a>(
  source_url: &str,
  mappings_map: &'a mut HashMap<String, Option<Mappings>>,
  get_map: &'a Fn(&str) -> String,
) -> &'a Option<Mappings> {
  mappings_map
    .entry(source_url.to_string())
    .or_insert_with(|| parse_map_string(source_url, get_map))
}

fn parse_stack_frame(
  frame: &StackFrame,
  mappings_map: &mut HashMap<String, Option<Mappings>>,
  get_map: &Fn(&str) -> String,
) -> String {
  let mappings = get_mappings(frame.source_url.as_ref(), mappings_map, get_map);
  let frame_pos = (frame.source_url.to_owned(), frame.line_number, frame.column);
  let (source_url, line_number, column) = match mappings {
    Some(mappings) => match mappings.original_location_for(
      frame.line_number,
      frame.column,
      Bias::default(),
    ) {
      Some(mapping) => match &mapping.original {
        Some(original) => {
          let source_name = match frame.source_url.as_ref() {
            "gen/bundle/main.js" => get_original_source(original.source),
            _ => frame.source_url.to_owned()
          };
          (source_name, original.original_line, original.original_column)
        },
        None => frame_pos,
      },
      None => frame_pos,
    },
    None => frame_pos,
  };
  if frame.function_name.len() > 0 {
    format!(
      "\n    at {} ({}:{}:{})",
      frame.function_name, source_url, line_number, column
    )
  } else {
    format!("\n    at {}:{}:{}", source_url, line_number, column)
  }
}

pub fn parse_javascript_error(
  error: &JavaScriptError,
  get_map: &Fn(&str) -> String,
) -> String {
  let mut msg = error.message.to_owned();
  let mut mappings_map: HashMap<String, Option<Mappings>> = HashMap::new();
  for frame in &error.stack_trace {
    msg.push_str(&parse_stack_frame(frame, &mut mappings_map, &get_map));
  }
  msg.push_str("\n");
  msg
}

#[cfg(test)]
pub fn get_map_stub(filename: &str) -> String {
  match filename {
    "foo_bar.ts" => ";;;IAIA,OAAO,CAAC,GAAG,CAAC,qBAAqB,EAAE,EAAE,CAAC,OAAO,CAAC,CAAC;IAC/C,OAAO,CAAC,GAAG,CAAC,eAAe,EAAE,IAAI,CAAC,QAAQ,CAAC,IAAI,CAAC,CAAC;IACjD,OAAO,CAAC,GAAG,CAAC,WAAW,EAAE,IAAI,CAAC,QAAQ,CAAC,EAAE,CAAC,CAAC;IAE3C,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC".to_string(),
    "bar_baz.ts" => ";;;IAEA,CAAC,KAAK,IAAI,EAAE;QACV,MAAM,GAAG,GAAG,sDAAa,OAAO,2BAAC,CAAC;QAClC,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC;IACnB,CAAC,CAAC,EAAE,CAAC;IAEQ,QAAA,GAAG,GAAG,KAAK,CAAC;IAEzB,OAAO,CAAC,GAAG,CAAC,GAAG,CAAC,CAAC".to_string(),
    _ => "".to_string(),
  }
}

#[test]
fn test_parse_javascript_error_01() {
  let error = JavaScriptError {
    message: "Error: foo bar".to_string(),
    stack_trace: vec![],
  };
  let result = parse_javascript_error(&error, &get_map_stub);
  assert_eq!("Error: foo bar\n", result);
}

#[test]
fn test_parse_javascript_error_02() {
  let error = JavaScriptError {
    message: "Error: foo bar".to_string(),
    stack_trace: vec![
      StackFrame {
        line_number: 4,
        column: 16,
        source_url: "foo_bar.ts".to_string(),
        function_name: "foo".to_string(),
        is_eval: false,
        is_constructor: false,
        is_wasm: false,
      },
      StackFrame {
        line_number: 5,
        column: 20,
        source_url: "bar_baz.ts".to_string(),
        function_name: "qat".to_string(),
        is_eval: false,
        is_constructor: false,
        is_wasm: false,
      },
      StackFrame {
        line_number: 1,
        column: 1,
        source_url: "deno_main.js".to_string(),
        function_name: "".to_string(),
        is_eval: false,
        is_constructor: false,
        is_wasm: false,
      },
    ],
  };
  let result = parse_javascript_error(&error, &get_map_stub);
  assert_eq!("Error: foo bar\n    at foo (foo_bar.ts:5:12)\n    at qat (bar_baz.ts:4:14)\n    at deno_main.js:1:1\n", result);
}

#[test]
fn test_parse_javascript_error_03() {
  // Because this is accessing the live bundle, this test might be more fragile
  let error = JavaScriptError {
    message: "TypeError: baz".to_string(),
    stack_trace: vec![
      StackFrame {
        line_number: 11,
        column: 12,
        source_url: "gen/bundle/main.js".to_string(),
        function_name: "setLogDebug".to_string(),
        is_eval: false,
        is_constructor: false,
        is_wasm: false,
      },
    ],
  };
  let result = parse_javascript_error(&error, &get_map_stub);
  assert_eq!("TypeError: baz\n    at setLogDebug (deno/js/util.ts:7:2)\n", result);
}
