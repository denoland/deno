// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//! This mod provides DenoError to unify errors across Deno.
use crate::colors;
use crate::source_maps::apply_source_map;
use crate::source_maps::SourceMapGetter;
use deno_core::ErrBox;
use std::error::Error;
use std::fmt;
use std::ops::Deref;

const SOURCE_ABBREV_THRESHOLD: usize = 150;

pub fn format_location(filename: &str, line: i64, col: i64) -> String {
  format!(
    "{}:{}:{}",
    colors::cyan(filename),
    colors::yellow(&line.to_string()),
    colors::yellow(&col.to_string())
  )
}

pub fn format_stack(
  is_error: bool,
  message_line: &str,
  source_line: Option<&str>,
  start_column: Option<i64>,
  end_column: Option<i64>,
  formatted_frames: &[String],
  level: usize,
) -> String {
  let mut s = String::new();
  s.push_str(&format!("{:indent$}{}", "", message_line, indent = level));
  s.push_str(&format_maybe_source_line(
    source_line,
    start_column,
    end_column,
    is_error,
    level,
  ));
  for formatted_frame in formatted_frames {
    s.push_str(&format!(
      "\n{:indent$}    at {}",
      "",
      formatted_frame,
      indent = level
    ));
  }
  s
}

/// Take an optional source line and associated information to format it into
/// a pretty printed version of that line.
fn format_maybe_source_line(
  source_line: Option<&str>,
  start_column: Option<i64>,
  end_column: Option<i64>,
  is_error: bool,
  level: usize,
) -> String {
  if source_line.is_none() || start_column.is_none() || end_column.is_none() {
    return "".to_string();
  }

  let source_line = source_line.unwrap();
  // sometimes source_line gets set with an empty string, which then outputs
  // an empty source line when displayed, so need just short circuit here.
  // Also short-circuit on error line too long.
  if source_line.is_empty() || source_line.len() > SOURCE_ABBREV_THRESHOLD {
    return "".to_string();
  }

  assert!(start_column.is_some());
  assert!(end_column.is_some());
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
  for _i in 0..start_column {
    if source_line.chars().nth(_i as usize).unwrap() == '\t' {
      s.push('\t');
    } else {
      s.push(' ');
    }
  }
  for _i in 0..(end_column - start_column) {
    s.push(underline_char);
  }
  let color_underline = if is_error {
    colors::red(&s).to_string()
  } else {
    colors::cyan(&s).to_string()
  };

  let indent = format!("{:indent$}", "", indent = level);

  format!("\n{}{}\n{}{}", indent, source_line, indent, color_underline)
}

/// Wrapper around deno_core::JSError which provides color to_string.
#[derive(Debug)]
pub struct JSError(deno_core::JSError);

impl JSError {
  pub fn create(
    core_js_error: deno_core::JSError,
    source_map_getter: &impl SourceMapGetter,
  ) -> ErrBox {
    let core_js_error = apply_source_map(&core_js_error, source_map_getter);
    let js_error = Self(core_js_error);
    ErrBox::from(js_error)
  }
}

impl Deref for JSError {
  type Target = deno_core::JSError;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl fmt::Display for JSError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut formatted_frames = self.0.formatted_frames.clone();

    // The formatted_frames passed from prepareStackTrace() are colored.
    if !colors::use_color() {
      formatted_frames = formatted_frames
        .iter()
        .map(|s| colors::strip_ansi_codes(s).to_string())
        .collect();
    }

    // When the stack frame array is empty, but the source location given by
    // (script_resource_name, line_number, start_column + 1) exists, this is
    // likely a syntax error. For the sake of formatting we treat it like it was
    // given as a single stack frame.
    if formatted_frames.is_empty()
      && self.0.script_resource_name.is_some()
      && self.0.line_number.is_some()
      && self.0.start_column.is_some()
    {
      formatted_frames = vec![format_location(
        self.0.script_resource_name.as_ref().unwrap(),
        self.0.line_number.unwrap(),
        self.0.start_column.unwrap() + 1,
      )]
    };

    write!(
      f,
      "{}",
      &format_stack(
        true,
        &self.0.message,
        self.0.source_line.as_deref(),
        self.0.start_column,
        self.0.end_column,
        &formatted_frames,
        0
      )
    )?;
    Ok(())
  }
}

impl Error for JSError {}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::colors::strip_ansi_codes;

  #[test]
  fn test_format_none_source_line() {
    let actual = format_maybe_source_line(None, None, None, false, 0);
    assert_eq!(actual, "");
  }

  #[test]
  fn test_format_some_source_line() {
    let actual = format_maybe_source_line(
      Some("console.log('foo');"),
      Some(8),
      Some(11),
      true,
      0,
    );
    assert_eq!(
      strip_ansi_codes(&actual),
      "\nconsole.log(\'foo\');\n        ~~~"
    );
  }
}
