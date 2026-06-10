// Copyright 2018-2026 the Deno authors. MIT license.

//! `console.table` table rendering, ported from the cli_table fork in
//! `01_console.js`.

use super::width::get_string_width;

const MIDDLE_MIDDLE: &str = "\u{2500}";
const ROW_MIDDLE: &str = "\u{253c}";
const TOP_RIGHT: &str = "\u{2510}";
const TOP_LEFT: &str = "\u{250c}";
const LEFT_MIDDLE: &str = "\u{251c}";
const TOP_MIDDLE: &str = "\u{252c}";
const BOTTOM_RIGHT: &str = "\u{2518}";
const BOTTOM_LEFT: &str = "\u{2514}";
const BOTTOM_MIDDLE: &str = "\u{2534}";
const RIGHT_MIDDLE: &str = "\u{2524}";
const LEFT: &str = "\u{2502} ";
const RIGHT: &str = " \u{2502}";
const MIDDLE: &str = " \u{2502} ";

fn render_row(
  row: &[String],
  column_widths: &[usize],
  column_right_align: Option<&[bool]>,
) -> String {
  let mut out = String::from(LEFT);
  for (i, cell) in row.iter().enumerate() {
    let len = get_string_width(cell, true);
    let padding = " ".repeat(column_widths[i].saturating_sub(len));
    if column_right_align.map(|a| a[i]).unwrap_or(false) {
      out.push_str(&padding);
      out.push_str(cell);
    } else {
      out.push_str(cell);
      out.push_str(&padding);
    }
    if i != row.len() - 1 {
      out.push_str(MIDDLE);
    }
  }
  out.push_str(RIGHT);
  out
}

/// `cliTable(head, columns)` from 01_console.js. `columns[i]` is the column
/// under `head[i]`; a `None` cell means the row has no entry for that column
/// (JS `hasOwnProperty(column, j)` false -> "").
pub fn cli_table(head: &[String], columns: &[Vec<Option<String>>]) -> String {
  let mut rows: Vec<Vec<String>> = Vec::new();
  let mut column_widths: Vec<usize> =
    head.iter().map(|h| get_string_width(h, true)).collect();
  let longest_column = columns.iter().map(|a| a.len()).max().unwrap_or(0);
  let mut column_right_align = vec![true; column_widths.len()];

  for i in 0..head.len() {
    let empty = Vec::new();
    let column = columns.get(i).unwrap_or(&empty);
    for j in 0..longest_column {
      if rows.len() <= j {
        rows.resize_with(j + 1, Vec::new);
      }
      let value = column
        .get(j)
        .and_then(|v| v.clone())
        .unwrap_or_default();
      let counted = get_string_width(&value, true);
      column_widths[i] = column_widths[i].max(counted);
      // JS: columnRightAlign[i] &= NumberIsInteger(+value);
      // `+""` is 0 (an integer); `+x` of non-numeric strings is NaN.
      column_right_align[i] = column_right_align[i] && is_integer_like(&value);
      let row = &mut rows[j];
      if row.len() <= i {
        row.resize_with(i + 1, String::new);
      }
      row[i] = value;
    }
  }

  let divider: Vec<String> = column_widths
    .iter()
    .map(|w| MIDDLE_MIDDLE.repeat(w + 2))
    .collect();

  let mut result = format!(
    "{}{}{}\n{}\n{}{}{}\n",
    TOP_LEFT,
    divider.join(TOP_MIDDLE),
    TOP_RIGHT,
    render_row(head, &column_widths, None),
    LEFT_MIDDLE,
    divider.join(ROW_MIDDLE),
    RIGHT_MIDDLE,
  );

  for row in &rows {
    // Rows may be shorter than head when a column had no entries.
    let mut padded = row.clone();
    padded.resize_with(head.len(), String::new);
    result.push_str(&render_row(&padded, &column_widths, Some(&column_right_align)));
    result.push('\n');
  }

  result.push_str(&format!(
    "{}{}{}",
    BOTTOM_LEFT,
    divider.join(BOTTOM_MIDDLE),
    BOTTOM_RIGHT,
  ));

  result
}

/// JS `NumberIsInteger(+value)`: empty string coerces to 0 (integer),
/// numeric strings parse with Number() semantics (including hex, exponents,
/// Infinity -> not integer... Infinity is not an integer), whitespace-only
/// strings coerce to 0.
fn is_integer_like(value: &str) -> bool {
  let trimmed = value.trim_matches(|c: char| c.is_whitespace());
  if trimmed.is_empty() {
    return true; // +"" == 0
  }
  // Number() semantics subset: decimal, hex/octal/binary literals, exponent.
  let n = js_string_to_number(trimmed);
  n.is_finite() && n.fract() == 0.0
}

fn js_string_to_number(s: &str) -> f64 {
  let unsigned = s.strip_prefix(['+', '-']).unwrap_or(s);
  if let Some(hex) = unsigned.strip_prefix("0x").or_else(|| unsigned.strip_prefix("0X")) {
    return match u128::from_str_radix(hex, 16) {
      Ok(v) => v as f64,
      Err(_) => f64::NAN,
    };
  }
  if let Some(oct) = unsigned.strip_prefix("0o").or_else(|| unsigned.strip_prefix("0O")) {
    return match u128::from_str_radix(oct, 8) {
      Ok(v) => v as f64,
      Err(_) => f64::NAN,
    };
  }
  if let Some(bin) = unsigned.strip_prefix("0b").or_else(|| unsigned.strip_prefix("0B")) {
    return match u128::from_str_radix(bin, 2) {
      Ok(v) => v as f64,
      Err(_) => f64::NAN,
    };
  }
  if unsigned == "Infinity" {
    return if s.starts_with('-') {
      f64::NEG_INFINITY
    } else {
      f64::INFINITY
    };
  }
  // Rust's f64 parser accepts "inf"/"nan" spellings JS does not; restrict to
  // decimal/exponent characters before delegating.
  if !unsigned
    .bytes()
    .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
  {
    return f64::NAN;
  }
  s.parse::<f64>().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn simple_table() {
    let head = vec!["(idx)".to_string(), "Values".to_string()];
    let columns = vec![
      vec![Some("0".to_string()), Some("1".to_string())],
      vec![Some("1".to_string()), Some("2".to_string())],
    ];
    let out = cli_table(&head, &columns);
    assert!(out.contains("(idx)"));
    assert!(out.starts_with(TOP_LEFT));
    assert!(out.ends_with(BOTTOM_RIGHT));
  }
}
