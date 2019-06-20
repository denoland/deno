// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
//! This mod provides DenoError to unify errors across Deno.
use crate::ansi;
use deno::JSError;
use deno::StackFrame;
use std::fmt;

/// A trait which specifies parts of a diagnostic like item needs to be able to
/// generate to conform its display to other diagnostic like items
pub trait DisplayFormatter {
  fn format_category_and_code(&self) -> String;
  fn format_message(&self, level: usize) -> String;
  fn format_related_info(&self) -> String;
  fn format_source_line(&self, level: usize) -> String;
  fn format_source_name(&self) -> String;
}

fn format_source_name(script_name: String, line: i64, column: i64) -> String {
  let script_name_c = ansi::cyan(script_name);
  let line_c = ansi::yellow((1 + line).to_string());
  let column_c = ansi::yellow((1 + column).to_string());
  format!("{}:{}:{}", script_name_c, line_c, column_c,)
}

/// Formats optional source, line and column into a single string.
pub fn format_maybe_source_name(
  script_name: Option<String>,
  line: Option<i64>,
  column: Option<i64>,
) -> String {
  if script_name.is_none() {
    return "".to_string();
  }

  assert!(line.is_some());
  assert!(column.is_some());
  format_source_name(script_name.unwrap(), line.unwrap(), column.unwrap())
}

/// Take an optional source line and associated information to format it into
/// a pretty printed version of that line.
pub fn format_maybe_source_line(
  source_line: Option<String>,
  line_number: Option<i64>,
  start_column: Option<i64>,
  end_column: Option<i64>,
  is_error: bool,
  level: usize,
) -> String {
  if source_line.is_none() || line_number.is_none() {
    return "".to_string();
  }

  let source_line = source_line.as_ref().unwrap();
  // sometimes source_line gets set with an empty string, which then outputs
  // an empty source line when displayed, so need just short circuit here
  if source_line.is_empty() {
    return "".to_string();
  }

  assert!(start_column.is_some());
  assert!(end_column.is_some());
  let line = (1 + line_number.unwrap()).to_string();
  let line_color = ansi::black_on_white(line.to_string());
  let line_len = line.clone().len();
  let line_padding =
    ansi::black_on_white(format!("{:indent$}", "", indent = line_len))
      .to_string();
  let mut s = String::new();
  let start_column = start_column.unwrap();
  let end_column = end_column.unwrap();
  // TypeScript uses `~` always, but V8 would utilise `^` always, even when
  // doing ranges, so here, if we only have one marker (very common with V8
  // errors) we will use `^` instead.
  let underline_char = if (end_column - start_column) <= 1 {
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
  let color_underline = if is_error {
    ansi::red(s).to_string()
  } else {
    ansi::cyan(s).to_string()
  };

  let indent = format!("{:indent$}", "", indent = level);

  format!(
    "\n\n{}{} {}\n{}{} {}\n",
    indent, line_color, source_line, indent, line_padding, color_underline
  )
}

/// Format a message to preface with `error: ` with ansi codes for red.
pub fn format_error_message(msg: String) -> String {
  let preamble = ansi::red("error:".to_string());
  format!("{} {}", preamble, msg)
}

/// Wrapper around JSError which provides color to_string.
pub struct JSErrorColor<'a>(pub &'a JSError);

impl<'a> DisplayFormatter for JSErrorColor<'a> {
  fn format_category_and_code(&self) -> String {
    "".to_string()
  }

  fn format_message(&self, _level: usize) -> String {
    format!(
      "{}{}",
      ansi::red_bold("error: ".to_string()),
      self.0.message.clone()
    )
  }

  fn format_related_info(&self) -> String {
    "".to_string()
  }

  fn format_source_line(&self, level: usize) -> String {
    format_maybe_source_line(
      self.0.source_line.clone(),
      self.0.line_number,
      self.0.start_column,
      self.0.end_column,
      true,
      level,
    )
  }

  fn format_source_name(&self) -> String {
    let e = self.0;
    if e.script_resource_name.is_none() {
      return "".to_string();
    }

    format!(
      "\n► {}",
      format_maybe_source_name(
        e.script_resource_name.clone(),
        e.line_number,
        e.start_column,
      )
    )
  }
}

impl<'a> fmt::Display for JSErrorColor<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{}{}{}",
      self.format_message(0),
      self.format_source_name(),
      self.format_source_line(0),
    )?;

    for frame in &self.0.frames {
      write!(f, "\n{}", StackFrameColor(&frame).to_string())?;
    }
    Ok(())
  }
}

struct StackFrameColor<'a>(&'a StackFrame);

impl<'a> fmt::Display for StackFrameColor<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let frame = self.0;
    // Note when we print to string, we change from 0-indexed to 1-indexed.
    let function_name = ansi::italic_bold(frame.function_name.clone());
    let script_line_column =
      format_source_name(frame.script_name.clone(), frame.line, frame.column);

    if !frame.function_name.is_empty() {
      write!(f, "    at {} ({})", function_name, script_line_column)
    } else if frame.is_eval {
      write!(f, "    at eval ({})", script_line_column)
    } else {
      write!(f, "    at {}", script_line_column)
    }
  }
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

  #[test]
  fn js_error_to_string() {
    let e = error1();
    assert_eq!("error: Error: foo bar\n    at foo (foo_bar.ts:5:17)\n    at qat (bar_baz.ts:6:21)\n    at deno_main.js:2:2", strip_ansi_codes(&JSErrorColor(&e).to_string()));
  }

  #[test]
  fn test_format_none_source_name() {
    let actual = format_maybe_source_name(None, None, None);
    assert_eq!(actual, "");
  }

  #[test]
  fn test_format_some_source_name() {
    let actual = format_maybe_source_name(
      Some("file://foo/bar.ts".to_string()),
      Some(1),
      Some(2),
    );
    assert_eq!(strip_ansi_codes(&actual), "file://foo/bar.ts:2:3");
  }

  #[test]
  fn test_format_none_source_line() {
    let actual = format_maybe_source_line(None, None, None, None, false, 0);
    assert_eq!(actual, "");
  }

  #[test]
  fn test_format_some_source_line() {
    let actual = format_maybe_source_line(
      Some("console.log('foo');".to_string()),
      Some(8),
      Some(8),
      Some(11),
      true,
      0,
    );
    assert_eq!(
      strip_ansi_codes(&actual),
      "\n\n9 console.log(\'foo\');\n          ~~~\n"
    );
  }

  #[test]
  fn test_format_error_message() {
    let actual = format_error_message("foo".to_string());
    assert_eq!(strip_ansi_codes(&actual), "error: foo");
  }
}
