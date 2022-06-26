// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
//! This mod provides DenoError to unify errors across Deno.
use crate::colors::cyan;
use crate::colors::italic_bold;
use crate::colors::red;
use crate::colors::yellow;
use deno_core::error::format_file_name;
use deno_core::error::JsError;
use deno_core::error::JsStackFrame;

// Keep in sync with `/core/error.js`.
pub fn format_location(frame: &JsStackFrame) -> String {
  let _internal = frame
    .file_name
    .as_ref()
    .map_or(false, |f| f.starts_with("deno:"));
  if frame.is_native {
    return cyan("native").to_string();
  }
  let mut result = String::new();
  let file_name = frame.file_name.clone().unwrap_or_default();
  if !file_name.is_empty() {
    result += &cyan(&format_file_name(&file_name)).to_string();
  } else {
    if frame.is_eval {
      result +=
        &(cyan(&frame.eval_origin.as_ref().unwrap()).to_string() + ", ");
    }
    result += &cyan("<anonymous>").to_string();
  }
  if let Some(line_number) = frame.line_number {
    result += &format!("{}{}", ":", yellow(&line_number.to_string()));
    if let Some(column_number) = frame.column_number {
      result += &format!("{}{}", ":", yellow(&column_number.to_string()));
    }
  }
  result
}

fn format_frame(frame: &JsStackFrame) -> String {
  let _internal = frame
    .file_name
    .as_ref()
    .map_or(false, |f| f.starts_with("deno:"));
  let is_method_call =
    !(frame.is_top_level.unwrap_or_default() || frame.is_constructor);
  let mut result = String::new();
  if frame.is_async {
    result += "async ";
  }
  if frame.is_promise_all {
    result += &italic_bold(&format!(
      "Promise.all (index {})",
      frame.promise_index.unwrap_or_default()
    ))
    .to_string();
    return result;
  }
  if is_method_call {
    let mut formatted_method = String::new();
    if let Some(function_name) = &frame.function_name {
      if let Some(type_name) = &frame.type_name {
        if !function_name.starts_with(type_name) {
          formatted_method += &format!("{}.", type_name);
        }
      }
      formatted_method += function_name;
      if let Some(method_name) = &frame.method_name {
        if !function_name.ends_with(method_name) {
          formatted_method += &format!(" [as {}]", method_name);
        }
      }
    } else {
      if let Some(type_name) = &frame.type_name {
        formatted_method += &format!("{}.", type_name);
      }
      if let Some(method_name) = &frame.method_name {
        formatted_method += method_name
      } else {
        formatted_method += "<anonymous>";
      }
    }
    result += &italic_bold(&formatted_method).to_string();
  } else if frame.is_constructor {
    result += "new ";
    if let Some(function_name) = &frame.function_name {
      result += &italic_bold(&function_name).to_string();
    } else {
      result += &cyan("<anonymous>").to_string();
    }
  } else if let Some(function_name) = &frame.function_name {
    result += &italic_bold(&function_name).to_string();
  } else {
    result += &format_location(frame);
    return result;
  }
  result += &format!(" ({})", format_location(frame));
  result
}

/// Take an optional source line and associated information to format it into
/// a pretty printed version of that line.
fn format_maybe_source_line(
  source_line: Option<&str>,
  column_number: Option<i64>,
  is_error: bool,
  level: usize,
) -> String {
  if source_line.is_none() || column_number.is_none() {
    return "".to_string();
  }

  let source_line = source_line.unwrap();
  // sometimes source_line gets set with an empty string, which then outputs
  // an empty source line when displayed, so need just short circuit here.
  if source_line.is_empty() {
    return "".to_string();
  }
  if source_line.contains("Couldn't format source line: ") {
    return format!("\n{}", source_line);
  }

  let mut s = String::new();
  let column_number = column_number.unwrap();

  if column_number as usize > source_line.len() {
    return format!(
      "\n{} Couldn't format source line: Column {} is out of bounds (source may have changed at runtime)",
      crate::colors::yellow("Warning"), column_number,
    );
  }

  for _i in 0..(column_number - 1) {
    if source_line.chars().nth(_i as usize).unwrap() == '\t' {
      s.push('\t');
    } else {
      s.push(' ');
    }
  }
  s.push('^');
  let color_underline = if is_error {
    red(&s).to_string()
  } else {
    cyan(&s).to_string()
  };

  let indent = format!("{:indent$}", "", indent = level);

  format!("\n{}{}\n{}{}", indent, source_line, indent, color_underline)
}

fn format_js_error_inner(js_error: &JsError, is_child: bool) -> String {
  let mut s = String::new();
  s.push_str(&js_error.exception_message);
  if let Some(aggregated) = &js_error.aggregated {
    for aggregated_error in aggregated {
      let error_string = format_js_error_inner(aggregated_error, true);
      for line in error_string.trim_start_matches("Uncaught ").lines() {
        s.push_str(&format!("\n    {}", line));
      }
    }
  }
  let column_number = js_error
    .source_line_frame_index
    .and_then(|i| js_error.frames.get(i).unwrap().column_number);
  s.push_str(&format_maybe_source_line(
    if is_child {
      None
    } else {
      js_error.source_line.as_deref()
    },
    column_number,
    true,
    0,
  ));
  for frame in &js_error.frames {
    s.push_str(&format!("\n    at {}", format_frame(frame)));
  }
  if let Some(cause) = &js_error.cause {
    let error_string = format_js_error_inner(cause, true);
    s.push_str(&format!(
      "\nCaused by: {}",
      error_string.trim_start_matches("Uncaught ")
    ));
  }
  s
}

pub fn format_js_error(js_error: &JsError) -> String {
  format_js_error_inner(js_error, false)
}

#[cfg(test)]
mod tests {
  use super::*;
  use test_util::strip_ansi_codes;

  #[test]
  fn test_format_none_source_line() {
    let actual = format_maybe_source_line(None, None, false, 0);
    assert_eq!(actual, "");
  }

  #[test]
  fn test_format_some_source_line() {
    let actual =
      format_maybe_source_line(Some("console.log('foo');"), Some(9), true, 0);
    assert_eq!(
      strip_ansi_codes(&actual),
      "\nconsole.log(\'foo\');\n        ^"
    );
  }
}
