// Copyright 2018-2026 the Deno authors. MIT license.

//! String quoting/escaping ported from `01_console.js`
//! (`quoteString`, `replaceEscapeSequences`, `maybeQuoteSymbol`, `meta`).

/// Escaped control characters (plus the single quote and the backslash) —
/// the `meta` table from 01_console.js, used to escape symbol descriptions
/// and property keys.
fn meta_escape(c: char) -> Option<&'static str> {
  Some(match c as u32 {
    0x00 => "\\x00",
    0x01 => "\\x01",
    0x02 => "\\x02",
    0x03 => "\\x03",
    0x04 => "\\x04",
    0x05 => "\\x05",
    0x06 => "\\x06",
    0x07 => "\\x07",
    0x08 => "\\b",
    0x09 => "\\t",
    0x0a => "\\n",
    0x0b => "\\x0B",
    0x0c => "\\f",
    0x0d => "\\r",
    0x0e => "\\x0E",
    0x0f => "\\x0F",
    0x10 => "\\x10",
    0x11 => "\\x11",
    0x12 => "\\x12",
    0x13 => "\\x13",
    0x14 => "\\x14",
    0x15 => "\\x15",
    0x16 => "\\x16",
    0x17 => "\\x17",
    0x18 => "\\x18",
    0x19 => "\\x19",
    0x1a => "\\x1A",
    0x1b => "\\x1B",
    0x1c => "\\x1C",
    0x1d => "\\x1D",
    0x1e => "\\x1E",
    0x1f => "\\x1F",
    0x27 => "\\'",
    0x5c => "\\\\",
    0x7f => "\\x7F",
    0x80 => "\\x80",
    0x81 => "\\x81",
    0x82 => "\\x82",
    0x83 => "\\x83",
    0x84 => "\\x84",
    0x85 => "\\x85",
    0x86 => "\\x86",
    0x87 => "\\x87",
    0x88 => "\\x88",
    0x89 => "\\x89",
    0x8a => "\\x8A",
    0x8b => "\\x8B",
    0x8c => "\\x8C",
    0x8d => "\\x8D",
    0x8e => "\\x8E",
    0x8f => "\\x8F",
    0x90 => "\\x90",
    0x91 => "\\x91",
    0x92 => "\\x92",
    0x93 => "\\x93",
    0x94 => "\\x94",
    0x95 => "\\x95",
    0x96 => "\\x96",
    0x97 => "\\x97",
    0x98 => "\\x98",
    0x99 => "\\x99",
    0x9a => "\\x9A",
    0x9b => "\\x9B",
    0x9c => "\\x9C",
    0x9d => "\\x9D",
    0x9e => "\\x9E",
    0x9f => "\\x9F",
    _ => return None,
  })
}

/// `strEscapeSequencesReplacer` application: escape
/// `[\x00-\x1f\x27\x5c\x7f-\x9f]` using the meta table. Used on symbol
/// descriptions in property position.
pub fn escape_meta_chars(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  for c in s.chars() {
    let code = c as u32;
    if code <= 0x1f || code == 0x27 || code == 0x5c || (0x7f..=0x9f).contains(&code) {
      if let Some(esc) = meta_escape(c) {
        out.push_str(esc);
        continue;
      }
    }
    out.push(c);
  }
  out
}

const ESCAPE_MAP: &[(char, &str)] = &[
  ('\u{8}', "\\b"),
  ('\u{c}', "\\f"),
  ('\n', "\\n"),
  ('\r', "\\r"),
  ('\t', "\\t"),
  ('\u{b}', "\\v"),
];

/// `replaceEscapeSequences` from 01_console.js.
pub fn replace_escape_sequences(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  for c in s.chars() {
    if let Some((_, esc)) = ESCAPE_MAP.iter().find(|(k, _)| *k == c) {
      out.push_str(esc);
      continue;
    }
    let code = c as u32;
    if code <= 0x1f || (0x7f..=0x9f).contains(&code) {
      out.push_str(&format!("\\x{:02x}", code));
      continue;
    }
    out.push(c);
  }
  out
}

/// `quoteString` from 01_console.js: pick the first quote from `quotes` not
/// present in the string (else `quotes[0]`), backslash-escape that quote and
/// backslashes, then optionally escape control sequences.
pub fn quote_string(string: &str, quotes: &[String], escape_sequences: bool) -> String {
  let default_quote = "\"".to_string();
  let quote = quotes
    .iter()
    .find(|q| !string.contains(q.as_str()))
    .or(quotes.first())
    .unwrap_or(&default_quote)
    .clone();

  // JS: insert "\\" before any position matching (?=[<quote>\\]).
  let mut escaped = String::with_capacity(string.len());
  let quote_char = quote.chars().next();
  for c in string.chars() {
    if Some(c) == quote_char || c == '\\' {
      escaped.push('\\');
    }
    escaped.push(c);
  }
  let escaped = if escape_sequences {
    replace_escape_sequences(&escaped)
  } else {
    escaped
  };
  format!("{quote}{escaped}{quote}")
}

/// `QUOTE_SYMBOL_REG` test: `^[a-zA-Z_][a-zA-Z_.0-9]*$`
pub fn symbol_description_needs_no_quotes(description: &str) -> bool {
  let mut chars = description.chars();
  match chars.next() {
    Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
    _ => return false,
  }
  chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// `keyStrRegExp` test: `^[a-zA-Z_][a-zA-Z_0-9]*$`
pub fn is_identifier_like(key: &str) -> bool {
  let mut chars = key.chars();
  match chars.next() {
    Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
    _ => return false,
  }
  chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `numberRegExp` test: `^(0|[1-9][0-9]*)$`
pub fn is_canonical_index(key: &str) -> bool {
  if key == "0" {
    return true;
  }
  let mut chars = key.chars();
  match chars.next() {
    Some(c) if ('1'..='9').contains(&c) => {}
    _ => return false,
  }
  chars.all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn q(s: &str) -> String {
    quote_string(
      s,
      &["\"".to_string(), "'".to_string(), "`".to_string()],
      true,
    )
  }

  #[test]
  fn quoting() {
    assert_eq!(q("hello"), "\"hello\"");
    assert_eq!(q("he\"llo"), "'he\"llo'");
    assert_eq!(q("he\"l'lo"), "`he\"l'lo`");
    assert_eq!(q("he\"l'l`o"), "\"he\\\"l'l`o\"");
    assert_eq!(q("a\nb"), "\"a\\nb\"");
    assert_eq!(q("a\\b"), "\"a\\\\b\"");
  }
}
