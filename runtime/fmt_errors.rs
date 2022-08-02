// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
//! This mod provides DenoError to unify errors across Deno.
use crate::colors::cyan;
use crate::colors::italic_bold;
use crate::colors::red;
use crate::colors::yellow;
use deno_core::error::format_file_name;
use deno_core::error::JsError;
use deno_core::error::JsStackFrame;
use std::fmt::Write as _;

#[derive(Debug, PartialEq, Clone)]
struct ErrorIdentity {
  name: Option<String>,
  message: Option<String>,
  stack: Option<String>,
  // `cause` omitted, because it is absent in nested errors referring to a
  // parent
  exception_message: String,
  frames: Vec<JsStackFrame>,
  source_line: Option<String>,
  source_line_frame_index: Option<usize>,
  aggregated: Option<Vec<JsError>>,
}

#[derive(Debug, Clone)]
struct ErrorReference {
  from: ErrorIdentity,
  to: ErrorIdentity,
}

impl From<&JsError> for ErrorIdentity {
  fn from(error: &JsError) -> ErrorIdentity {
    ErrorIdentity {
      name: error.name.clone(),
      message: error.message.clone(),
      stack: error.stack.clone(),
      exception_message: error.exception_message.clone(),
      frames: error.frames.clone(),
      source_line: error.source_line.clone(),
      source_line_frame_index: error.source_line_frame_index.clone(),
      aggregated: error.aggregated.clone(),
    }
  }
}

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
    write!(result, ":{}", yellow(&line_number.to_string())).unwrap();
    if let Some(column_number) = frame.column_number {
      write!(result, ":{}", yellow(&column_number.to_string())).unwrap();
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
          write!(formatted_method, "{}.", type_name).unwrap();
        }
      }
      formatted_method += function_name;
      if let Some(method_name) = &frame.method_name {
        if !function_name.ends_with(method_name) {
          write!(formatted_method, " [as {}]", method_name).unwrap();
        }
      }
    } else {
      if let Some(type_name) = &frame.type_name {
        write!(formatted_method, "{}.", type_name).unwrap();
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
      write!(result, "{}", italic_bold(&function_name)).unwrap();
    } else {
      result += &cyan("<anonymous>").to_string();
    }
  } else if let Some(function_name) = &frame.function_name {
    result += &italic_bold(&function_name).to_string();
  } else {
    result += &format_location(frame);
    return result;
  }
  write!(result, " ({})", format_location(frame)).unwrap();
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
      yellow("Warning"), column_number,
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

// TOOD: make this non-recursive
fn find_error_references_inner(
  history: &mut Vec<ErrorIdentity>,
  parent: Option<ErrorIdentity>,
  js_error: &JsError,
) -> Option<ErrorReference> {
  assert!(history.is_empty() == parent.is_none());

  let error_identity = ErrorIdentity::from(js_error);
  history.push(error_identity.clone());

  if let Some(cause) = &js_error.cause {
    let cause_identity = ErrorIdentity::from(cause.as_ref());

    if let Some(seen) = history.iter().find(|&el| el == &cause_identity) {
      return Some(ErrorReference {
        from: parent.unwrap().clone(),
        to: seen.clone(),
      });
    } else {
      return find_error_references_inner(
        history,
        Some(error_identity),
        cause.as_ref(),
      );
    }
  } else {
    return None;
  }
}

fn find_error_references(js_error: &JsError) -> Option<ErrorReference> {
  let mut history = Vec::<ErrorIdentity>::new();
  return find_error_references_inner(&mut history, None, js_error);
}

fn format_js_error_inner(
  circular: Option<ErrorReference>,
  js_error: &JsError,
  is_child: bool,
) -> String {
  let mut s = String::new();

  let error_identity = ErrorIdentity::from(js_error);

  s.push_str(&js_error.exception_message);

  if let Some(c) = &circular {
    if error_identity == c.to {
      write!(s, " {}", cyan("<ref *1>")).unwrap();
    }
  }

  if let Some(aggregated) = &js_error.aggregated {
    for aggregated_error in aggregated {
      // TOOD: handle aggregate errors
      let error_string = format_js_error_inner(None, aggregated_error, true);
      for line in error_string.trim_start_matches("Uncaught ").lines() {
        write!(s, "\n    {}", line).unwrap();
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
    write!(s, "\n    at {}", format_frame(frame)).unwrap();
  }
  if let Some(cause) = &js_error.cause {
    let is_circular = if let Some(c) = &circular {
      if ErrorIdentity::from(cause.as_ref()) == c.from {
        true
      } else {
        false
      }
    } else {
      false
    };

    let error_string = if is_circular {
      cyan("[Circular *1]").to_string()
    } else {
      format_js_error_inner(circular, cause, true)
    };

    write!(
      s,
      "\nCaused by: {}",
      error_string.trim_start_matches("Uncaught ")
    )
    .unwrap();
  }
  s
}

/// Format a [`JsError`] for terminal output.
pub fn format_js_error(js_error: &JsError) -> String {
  let circular = find_error_references(js_error);
  format_js_error_inner(circular, js_error, false)
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
