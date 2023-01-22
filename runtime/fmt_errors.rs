// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
//! This mod provides DenoError to unify errors across Deno.
use crate::colors::cyan;
use crate::colors::italic_bold;
use crate::colors::red;
use crate::colors::yellow;
use deno_core::error::format_file_name;
use deno_core::error::JsError;
use deno_core::error::JsStackFrame;
use std::fmt::Write as _;

/// Compares all properties of JsError, except for JsError::cause.
/// This function is used to detect that 2 JsError objects in a JsError::cause
/// chain are identical, ie. there is a recursive cause.
/// 02_console.js, which also detects recursive causes, can use JS object
/// comparisons to compare errors. We don't have access to JS object identity in
/// format_js_error().
fn errors_are_equal_without_cause(a: &JsError, b: &JsError) -> bool {
  a.name == b.name
    && a.message == b.message
    && a.stack == b.stack
    // `a.cause == b.cause` omitted, because it is absent in recursive errors,
    // despite the error being identical to a previously seen one.
    && a.exception_message == b.exception_message
    && a.frames == b.frames
    && a.source_line == b.source_line
    && a.source_line_frame_index == b.source_line_frame_index
    && a.aggregated == b.aggregated
}

#[derive(Debug, Clone)]
struct ErrorReference<'a> {
  from: &'a JsError,
  to: &'a JsError,
}

#[derive(Debug, Clone)]
struct IndexedErrorReference<'a> {
  reference: ErrorReference<'a>,
  index: usize,
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

fn find_recursive_cause(js_error: &JsError) -> Option<ErrorReference> {
  let mut history = Vec::<&JsError>::new();

  let mut current_error: &JsError = js_error;

  while let Some(cause) = &current_error.cause {
    history.push(current_error);

    if let Some(seen) = history
      .iter()
      .find(|&el| errors_are_equal_without_cause(el, cause.as_ref()))
    {
      return Some(ErrorReference {
        from: current_error,
        to: seen,
      });
    } else {
      current_error = cause;
    }
  }

  None
}

fn format_aggregated_error(
  aggregated_errors: &Vec<JsError>,
  circular_reference_index: usize,
) -> String {
  let mut s = String::new();
  let mut nested_circular_reference_index = circular_reference_index;

  for js_error in aggregated_errors {
    let aggregated_circular = find_recursive_cause(js_error);
    if aggregated_circular.is_some() {
      nested_circular_reference_index += 1;
    }
    let error_string = format_js_error_inner(
      js_error,
      aggregated_circular.map(|reference| IndexedErrorReference {
        reference,
        index: nested_circular_reference_index,
      }),
      false,
    );

    for line in error_string.trim_start_matches("Uncaught ").lines() {
      write!(s, "\n    {}", line).unwrap();
    }
  }

  s
}

fn format_js_error_inner(
  js_error: &JsError,
  circular: Option<IndexedErrorReference>,
  include_source_code: bool,
) -> String {
  let mut s = String::new();

  s.push_str(&js_error.exception_message);

  if let Some(circular) = &circular {
    if errors_are_equal_without_cause(js_error, circular.reference.to) {
      write!(s, " {}", cyan(format!("<ref *{}>", circular.index))).unwrap();
    }
  }

  if let Some(aggregated) = &js_error.aggregated {
    let aggregated_message = format_aggregated_error(
      aggregated,
      circular.as_ref().map_or(0, |circular| circular.index),
    );
    s.push_str(&aggregated_message);
  }

  let column_number = js_error
    .source_line_frame_index
    .and_then(|i| js_error.frames.get(i).unwrap().column_number);
  s.push_str(&format_maybe_source_line(
    if include_source_code {
      js_error.source_line.as_deref()
    } else {
      None
    },
    column_number,
    true,
    0,
  ));
  for frame in &js_error.frames {
    write!(s, "\n    at {}", format_frame(frame)).unwrap();
  }
  if let Some(cause) = &js_error.cause {
    let is_caused_by_circular = circular.as_ref().map_or(false, |circular| {
      errors_are_equal_without_cause(circular.reference.from, js_error)
    });

    let error_string = if is_caused_by_circular {
      cyan(format!("[Circular *{}]", circular.unwrap().index)).to_string()
    } else {
      format_js_error_inner(cause, circular, false)
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
  let circular =
    find_recursive_cause(js_error).map(|reference| IndexedErrorReference {
      reference,
      index: 1,
    });

  format_js_error_inner(js_error, circular, true)
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
