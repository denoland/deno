// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
//! This mod adds source maps and ANSI color display to deno::DenoDiagnostic.
use crate::ansi;
use deno;
use deno::DenoDiagnostic;
use deno::DenoDiagnosticCategory;
use deno::DenoDiagnosticFrame;
use deno::DenoDiagnosticSources;
use source_map_mappings::parse_mappings;
use source_map_mappings::Bias;
use source_map_mappings::Mappings;
use std::collections::HashMap;
use std::fmt;
use std::str;

/// Wrapper around DenoDiagnostic which provides color to_string.
pub struct DenoDiagnosticColor<'a>(pub &'a DenoDiagnostic);

struct DenoDiagnosticFrameColor<'a>(&'a DenoDiagnosticFrame);

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

impl<'a> fmt::Display for DenoDiagnosticFrameColor<'a> {
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

impl<'a> fmt::Display for DenoDiagnosticColor<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let e = self.0;

    write!(
      f,
      "{}{}{}{}{}",
      format_source_name_color(e, 0),
      format_category_and_code_color(e),
      format_message_color(e, 0),
      format_source_line_color(e, 0),
      format_related_info_color(e),
    )?;

    match &e.frames {
      Some(frames) => {
        for frame in frames {
          write!(f, "\n{}", DenoDiagnosticFrameColor(&frame).to_string())?;
        }
      }
      _ => (),
    }

    match &e.next {
      Some(next) => write!(f, "\n{}", DenoDiagnosticColor(next).to_string())?,
      _ => (),
    }

    Ok(())
  }
}

/// Format the category and code for a given diagnostic.  This currently only
/// pertains to diagnostics coming from TypeScript.
fn format_category_and_code_color(diagnostic: &DenoDiagnostic) -> String {
  if diagnostic.source != DenoDiagnosticSources::TypeScript {
    return "".to_owned();
  }

  let category = match diagnostic.category {
    DenoDiagnosticCategory::Error => {
      format!("- {}", ansi::red("error".to_string()))
    }
    DenoDiagnosticCategory::Warning => "- warn".to_string(),
    DenoDiagnosticCategory::Debug => "- debug".to_string(),
    DenoDiagnosticCategory::Info => "- info".to_string(),
    _ => "".to_string(),
  };

  let code = match diagnostic.code {
    Some(code_int) => {
      ansi::grey(format!(" TS{}:", code_int.to_string())).to_string()
    }
    None => "".to_string(),
  };

  format!("{}{} ", category, code)
}

/// Format the message of a diagnostic.
fn format_message_color(diagnostic: &DenoDiagnostic, level: usize) -> String {
  if diagnostic.diagnostics.is_none() {
    return format!("{:indent$}{}", "", diagnostic.message, indent = level);
  }

  let mut s = String::new();
  let mut i = level / 2;
  for diagnostic in diagnostic.diagnostics.clone().unwrap() {
    s.push_str(&std::iter::repeat(" ").take(i * 2).collect::<String>());
    s.push_str(&diagnostic.message);
    s.push('\n');
    i += 1;
  }
  s.pop();

  s
}

/// Format the related information from a diagnostic.  Currently only TypeScript
/// diagnostics optionally contain related information, where the compiler
/// can advise on the source of a diagnostic error.
fn format_related_info_color(diagnostic: &DenoDiagnostic) -> String {
  if diagnostic.related_information.is_none() {
    return "".to_string();
  }

  let mut s = String::new();
  for related_diagnostic in diagnostic.related_information.clone().unwrap() {
    let rd = &related_diagnostic;
    s.push_str(&format!(
      "\n{}{}{}\n",
      format_source_name_color(rd, 2),
      format_source_line_color(rd, 4),
      format_message_color(rd, 4),
    ));
  }

  s
}

/// If a diagnostic contains a source line, return a string that formats it
/// underlining the span of code related to the diagnostic
fn format_source_line_color(
  diagnostic: &DenoDiagnostic,
  level: usize,
) -> String {
  if diagnostic.source_line.is_none() {
    return "".to_owned();
  }

  let source_line = diagnostic.source_line.as_ref().unwrap();
  // sometimes source_line gets set with an empty string, which then outputs
  // an empty source line when displayed, so need just short circuit here
  if source_line.len() == 0 {
    return "".to_owned();
  }
  assert!(diagnostic.line_number.is_some());
  assert!(diagnostic.start_column.is_some());
  assert!(diagnostic.end_column.is_some());
  let line = match diagnostic.source {
    DenoDiagnosticSources::TypeScript => {
      (1 + diagnostic.line_number.unwrap()).to_string()
    }
    _ => diagnostic.line_number.unwrap().to_string(),
  };
  let line_color = ansi::black_on_white(line.to_string());
  let line_len = line.clone().len();
  let line_padding =
    ansi::black_on_white(format!("{:indent$}", "", indent = line_len))
      .to_string();
  let mut s = String::new();
  let start_column = diagnostic.start_column.unwrap();
  let end_column = diagnostic.end_column.unwrap();
  // TypeScript uses `~` always, but V8 would utilise `^` always, even when
  // doing ranges, so here, if we only have one marker (very common with V8
  // errors) we will use `^` instead.
  let underline_char =
    if (end_column - start_column) <= 1 {
      '^'
    } else {
      '~'
    };
  for i in 0..end_column {
    if i >= start_column {
      s.push(underline_char);
    } else {
      s.push(' ');
    }
  }
  let color_underline = match diagnostic.category {
    DenoDiagnosticCategory::Error => ansi::red(s).to_string(),
    _ => ansi::cyan(s).to_string(),
  };

  let indent = format!("{:indent$}", "", indent = level);

  format!(
    "\n\n{}{} {}\n{}{} {}\n",
    indent, line_color, source_line, indent, line_padding, color_underline
  )
}

/// Format the source resource name, along with line and column information from
/// a diagnostic into a single line.
fn format_source_name_color(
  diagnostic: &DenoDiagnostic,
  level: usize,
) -> String {
  if diagnostic.script_resource_name.is_none() {
    return "".to_owned();
  }

  let script_name =
    ansi::cyan(diagnostic.script_resource_name.clone().unwrap());
  assert!(diagnostic.line_number.is_some());
  assert!(diagnostic.start_column.is_some());
  let line = ansi::yellow(match diagnostic.source {
    DenoDiagnosticSources::TypeScript => {
      (1 + diagnostic.line_number.unwrap()).to_string()
    }
    _ => diagnostic.line_number.unwrap().to_string(),
  });
  let column = ansi::yellow(match diagnostic.source {
    DenoDiagnosticSources::TypeScript => {
      (1 + diagnostic.start_column.unwrap()).to_string()
    }
    _ => diagnostic.start_column.unwrap().to_string(),
  });
  format!(
    "{:indent$}{}:{}:{} ",
    "",
    script_name,
    line,
    column,
    indent = level
  )
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

fn frame_apply_source_map(
  frame: &DenoDiagnosticFrame,
  mappings_map: &mut CachedMaps,
  getter: &dyn SourceMapGetter,
) -> DenoDiagnosticFrame {
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

  DenoDiagnosticFrame {
    script_name,
    function_name: frame.function_name.clone(),
    line,
    column,
    is_eval: frame.is_eval,
    is_constructor: frame.is_constructor,
    is_wasm: frame.is_wasm,
  }
}

pub fn apply_source_map(
  diagnostic: &DenoDiagnostic,
  getter: &dyn SourceMapGetter,
) -> DenoDiagnostic {
  let mut mappings_map: CachedMaps = HashMap::new();
  let frames = match &diagnostic.frames {
    Some(diagnostic_frames) => {
      let mut frames = Vec::<DenoDiagnosticFrame>::new();
      for frame in diagnostic_frames {
        let f = frame_apply_source_map(&frame, &mut mappings_map, getter);
        frames.push(f);
      }

      Some(frames)
    }
    _ => None,
  };

  DenoDiagnostic {
    message: diagnostic.message.clone(),
    diagnostics: diagnostic.diagnostics.clone(),
    related_information: diagnostic.related_information.clone(),
    frames,
    code: diagnostic.code,
    category: diagnostic.category.clone(),
    source: diagnostic.source.clone(),
    source_line: diagnostic.source_line.clone(),
    // TODO the following need to be source mapped:
    script_resource_name: diagnostic.script_resource_name.clone(),
    line_number: diagnostic.line_number,
    start_position: diagnostic.start_position,
    end_position: diagnostic.end_position,
    start_column: diagnostic.start_column,
    end_column: diagnostic.end_column,
    next: None,
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

fn parse_map_string(
  script_name: &str,
  getter: &dyn SourceMapGetter,
) -> Option<SourceMap> {
  builtin_source_map(script_name)
    .or_else(|| getter.get_source_map(script_name))
    .and_then(|raw_source_map| {
      SourceMap::from_json(str::from_utf8(&raw_source_map).unwrap())
    })
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
  use crate::ansi::strip_ansi_codes;

  fn error1() -> DenoDiagnostic {
    DenoDiagnostic {
      message: "Error: foo bar".to_string(),
      diagnostics: None,
      related_information: None,
      code: None,
      source: deno::DenoDiagnosticSources::V8,
      category: deno::DenoDiagnosticCategory::Error,
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      start_column: None,
      end_column: None,
      frames: Some(vec![
        DenoDiagnosticFrame {
          line: 4,
          column: 16,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        DenoDiagnosticFrame {
          line: 5,
          column: 20,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        DenoDiagnosticFrame {
          line: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
      ]),
      next: None,
    }
  }

  fn diagnostic1() -> DenoDiagnostic {
    DenoDiagnostic {
      message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
      diagnostics: Some(vec![
        DenoDiagnostic {
          message: "Type '(o: T) => { v: any; f: (x: B) => string; }[]' is not assignable to type '(r: B) => Value<B>[]'.".to_string(),
          diagnostics: None,
          related_information: None,
          source: DenoDiagnosticSources::TypeScript,
          code: Some(2322),
          source_line: None,
          script_resource_name: None,
          line_number: None,
          start_position: None,
          end_position: None,
          category: DenoDiagnosticCategory::Error,
          start_column: None,
          end_column: None,
          frames: None,
          next: None,
        },
        DenoDiagnostic {
          message: "Types of parameters 'o' and 'r' are incompatible.".to_string(),
          diagnostics: None,
          related_information: None,
          source: DenoDiagnosticSources::TypeScript,
          code: Some(2328),
          source_line: None,
          script_resource_name: None,
          line_number: None,
          start_position: None,
          end_position: None,
          category: DenoDiagnosticCategory::Error,
          start_column: None,
          end_column: None,
          frames: None,
          next: None,
        },
        DenoDiagnostic {
          message: "Type 'B' is not assignable to type 'T'.".to_string(),
          diagnostics: None,
          related_information: None,
          source: DenoDiagnosticSources::TypeScript,
          code: Some(2322),
          source_line: None,
          script_resource_name: None,
          line_number: None,
          start_position: None,
          end_position: None,
          category: DenoDiagnosticCategory::Error,
          start_column: None,
          end_column: None,
          frames: None,
          next: None,
        },
      ]),
      code: Some(2322),
      category: DenoDiagnosticCategory::Error,
      source: DenoDiagnosticSources::TypeScript,
      start_position: Some(267),
      end_position: Some(273),
      source_line: Some("  values: o => [".to_string()),
      line_number: Some(18),
      script_resource_name: Some("deno/tests/complex_diagnostics.ts".to_string()),
      start_column: Some(2),
      end_column: Some(8),
      related_information: Some(vec![
        DenoDiagnostic {
          message: "The expected type comes from property 'values' which is declared here on type 'SettingsInterface<B>'".to_string(),
          diagnostics: None,
          related_information: None,
          source: DenoDiagnosticSources::TypeScript,
          code: Some(6500),
          source_line: Some("  values?: (r: T) => Array<Value<T>>;".to_string()),
          script_resource_name: Some("deno/tests/complex_diagnostics.ts".to_string()),
          line_number: Some(6),
          start_position: Some(94),
          end_position: Some(100),
          category: DenoDiagnosticCategory::Info,
          start_column: Some(2),
          end_column: Some(8),
          frames: None,
          next: None,
        }
      ]),
      frames: None,
      next: None,
    }
  }

  fn diagnostic2() -> DenoDiagnostic {
    DenoDiagnostic {
      message: "Example 1".to_string(),
      diagnostics: None,
      code: Some(2322),
      category: DenoDiagnosticCategory::Error,
      source: DenoDiagnosticSources::TypeScript,
      start_position: Some(267),
      end_position: Some(273),
      source_line: Some("  values: o => [".to_string()),
      line_number: Some(18),
      script_resource_name: Some("deno/tests/complex_diagnostics.ts".to_string()),
      start_column: Some(2),
      end_column: Some(8),
      related_information: None,
      frames: None,
      next: Some(Box::new(DenoDiagnostic {
        message: "Example 2".to_string(),
        diagnostics: None,
        code: Some(2000),
        category: DenoDiagnosticCategory::Error,
        source: DenoDiagnosticSources::TypeScript,
        start_position: Some(2),
        end_position: Some(2),
        source_line: Some("  values: undefined,".to_string()),
        line_number: Some(128),
        script_resource_name: Some("/foo/bar.ts".to_string()),
        start_column: Some(2),
        end_column: Some(8),
        related_information: None,
        frames: None,
        next: None,
      })),
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
  fn js_error_to_color_string() {
    let e = error1();
    let expected = "Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2";
    assert_eq!(expected, strip_ansi_codes(&DenoDiagnosticColor(&e).to_string()));
  }

  #[test]
  fn ts_diagnostic_to_color_string1() {
    let d = diagnostic1();
    let expected = "deno/tests/complex_diagnostics.ts:19:3 - error TS2322: Type \'(o: T) => { v: any; f: (x: B) => string; }[]\' is not assignable to type \'(r: B) => Value<B>[]\'.\n  Types of parameters \'o\' and \'r\' are incompatible.\n    Type \'B\' is not assignable to type \'T\'.\n\n19   values: o => [\n     ~~~~~~\n\n  deno/tests/complex_diagnostics.ts:7:3 \n\n    7   values?: (r: T) => Array<Value<T>>;\n        ~~~~~~\n    The expected type comes from property \'values\' which is declared here on type \'SettingsInterface<B>\'\n";
    assert_eq!(expected, strip_ansi_codes(&DenoDiagnosticColor(&d).to_string()));
  }

  #[test]
  fn ts_diagnostic_to_color_string2() {
    let d = diagnostic2();
    let expected = "deno/tests/complex_diagnostics.ts:19:3 - error TS2322: Example 1\n\n19   values: o => [\n     ~~~~~~\n\n/foo/bar.ts:129:3 - error TS2000: Example 2\n\n129   values: undefined,\n      ~~~~~~\n";
    assert_eq!(expected, strip_ansi_codes(&DenoDiagnosticColor(&d).to_string()));
  }

  #[test]
  fn js_error_apply_source_map_1() {
    let e = error1();
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&e, &getter);
    let expected = DenoDiagnostic {
      message: "Error: foo bar".to_string(),
      diagnostics: None,
      related_information: None,
      source: deno::DenoDiagnosticSources::V8,
      code: None,
      category: deno::DenoDiagnosticCategory::Error,
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      start_column: None,
      end_column: None,
      frames: Some(vec![
        DenoDiagnosticFrame {
          line: 5,
          column: 12,
          script_name: "foo_bar.ts".to_string(),
          function_name: "foo".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        DenoDiagnosticFrame {
          line: 4,
          column: 14,
          script_name: "bar_baz.ts".to_string(),
          function_name: "qat".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
        DenoDiagnosticFrame {
          line: 1,
          column: 1,
          script_name: "deno_main.js".to_string(),
          function_name: "".to_string(),
          is_eval: false,
          is_constructor: false,
          is_wasm: false,
        },
      ]),
      next: None,
    };
    assert_eq!(actual, expected);
  }

  #[test]
  fn js_error_apply_source_map_2() {
    let e = DenoDiagnostic {
      message: "TypeError: baz".to_string(),
      diagnostics: None,
      related_information: None,
      source: deno::DenoDiagnosticSources::V8,
      code: None,
      category: deno::DenoDiagnosticCategory::Error,
      source_line: None,
      script_resource_name: None,
      line_number: None,
      start_position: None,
      end_position: None,
      start_column: None,
      end_column: None,
      frames: Some(vec![DenoDiagnosticFrame {
        line: 11,
        column: 12,
        script_name: "gen/cli/bundle/main.js".to_string(),
        function_name: "setLogDebug".to_string(),
        is_eval: false,
        is_constructor: false,
        is_wasm: false,
      }]),
      next: None,
    };
    let getter = MockSourceMapGetter {};
    let actual = apply_source_map(&e, &getter);
    assert_eq!(actual.message, "TypeError: baz");
    // Because this is accessing the live bundle, this test might be more fragile
    let frames = actual.frames.unwrap();
    assert_eq!(frames.len(), 1);
    assert!(frames[0].script_name.ends_with("js/util.ts"));
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
