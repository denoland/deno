// Copyright 2018-2026 the Deno authors. MIT license.

//! String display-width helpers ported from `01_console.js`
//! (`getStringWidth`, `stripVTControlCharacters`, full/zero width tables).

/// Matches the `ansiPattern` regex from 01_console.js (adopted from
/// chalk/ansi-regex). Implemented as a hand-rolled scanner so we don't pay
/// for a regex engine on the hot path.
///
/// Pattern: `[\x1b\x9b][[\]()#;?]*` followed by either
/// - `(?:(?:;[-a-zA-Z\d\/#&.:=?%@~_]+)*|[a-zA-Z\d]+(?:;[-a-zA-Z\d\/#&.:=?%@~_]*)*)?(?:\x07|\x1b\\|\x9c)`
/// - `(?:\d{1,4}(?:;\d{0,4})*)?[\dA-PR-TZcf-nq-uy=><~]`
fn ansi_sequence_len(chars: &[char]) -> Option<usize> {
  let first = chars[0];
  if first != '\u{1b}' && first != '\u{9b}' {
    return None;
  }
  let mut i = 1;
  while i < chars.len()
    && matches!(chars[i], '[' | ']' | '(' | ')' | '#' | ';' | '?')
  {
    i += 1;
  }
  let body_start = i;

  // Alternative 2 (CSI-style): (?:\d{1,4}(?:;\d{0,4})*)?[\dA-PR-TZcf-nq-uy=><~]
  // Try it first when it matches at the shortest position, mirroring the
  // regex alternation order: alternative 1 (terminated string) is listed
  // first in the pattern, so attempt it first.

  // Alternative 1: optional parameter body then a terminator
  // (BEL | ESC-backslash | ST).
  {
    let mut j = body_start;
    let is_param_char = |c: char| {
      c.is_ascii_alphanumeric()
        || matches!(
          c,
          '-' | '/' | '#' | '&' | '.' | ':' | '=' | '?' | '%' | '@' | '~' | '_'
        )
    };
    // (?:;[-...]+)* | [a-zA-Z\d]+(?:;[-...]*)*
    if j < chars.len() && chars[j] == ';' {
      while j < chars.len() && chars[j] == ';' {
        let mut k = j + 1;
        let start = k;
        while k < chars.len() && is_param_char(chars[k]) && chars[k] != ';' {
          k += 1;
        }
        if k == start {
          break;
        }
        j = k;
      }
    } else if j < chars.len() && chars[j].is_ascii_alphanumeric() {
      while j < chars.len() && chars[j].is_ascii_alphanumeric() {
        j += 1;
      }
      while j < chars.len() && chars[j] == ';' {
        j += 1;
        while j < chars.len() && is_param_char(chars[j]) && chars[j] != ';' {
          j += 1;
        }
      }
    }
    if j < chars.len() {
      if chars[j] == '\u{07}' || chars[j] == '\u{9c}' {
        return Some(j + 1);
      }
      if chars[j] == '\u{1b}' && j + 1 < chars.len() && chars[j + 1] == '\u{5c}'
      {
        return Some(j + 2);
      }
    }
  }

  // Alternative 2: (?:\d{1,4}(?:;\d{0,4})*)?[\dA-PR-TZcf-nq-uy=><~]
  {
    let is_term = |c: char| {
      c.is_ascii_digit()
        || ('A'..='P').contains(&c)
        || ('R'..='T').contains(&c)
        || c == 'Z'
        || c == 'c'
        || ('f'..='n').contains(&c)
        || ('q'..='u').contains(&c)
        || c == 'y'
        || matches!(c, '=' | '>' | '<' | '~')
    };
    let mut j = body_start;
    let mut digits = 0;
    while j < chars.len() && chars[j].is_ascii_digit() && digits < 4 {
      j += 1;
      digits += 1;
    }
    if digits > 0 {
      while j < chars.len() && chars[j] == ';' {
        j += 1;
        let mut d = 0;
        while j < chars.len() && chars[j].is_ascii_digit() && d < 4 {
          j += 1;
          d += 1;
        }
      }
    }
    // The terminator class includes `\d`, which overlaps the optional numeric
    // parameter. The regex backtracks the trailing parameter so its last digit
    // can serve as the terminator, so truncated sequences like "\x1b[5" and
    // "\x1b[1;2" still match. Mirror that: prefer a terminator right after the
    // greedily-consumed body, otherwise reuse the last body char that is a
    // valid terminator (longest match wins).
    if j < chars.len() && is_term(chars[j]) {
      return Some(j + 1);
    }
    let mut t = j;
    while t > body_start {
      t -= 1;
      if is_term(chars[t]) {
        return Some(t + 1);
      }
    }
  }

  None
}

/// `stripVTControlCharacters` from 01_console.js.
pub fn strip_vt_control_characters(s: &str) -> String {
  let chars: Vec<char> = s.chars().collect();
  let mut out = String::with_capacity(s.len());
  let mut i = 0;
  while i < chars.len() {
    if let Some(len) = ansi_sequence_len(&chars[i..]) {
      i += len;
    } else {
      out.push(chars[i]);
      i += 1;
    }
  }
  out
}

/// `isZeroWidthCodePoint` from 01_console.js.
fn is_zero_width_code_point(code: u32) -> bool {
  code <= 0x1f // C0 control codes
    || (0x7f..=0x9f).contains(&code) // C1 control codes
    || (0x300..=0x36f).contains(&code) // Combining Diacritical Marks
    || (0x200b..=0x200f).contains(&code) // Modifying Invisible Characters
    // Combining Diacritical Marks for Symbols
    || (0x20d0..=0x20ff).contains(&code)
    || (0xfe00..=0xfe0f).contains(&code) // Variation Selectors
    || (0xfe20..=0xfe2f).contains(&code) // Combining Half Marks
    || (0xe0100..=0xe01ef).contains(&code) // Variation Selectors
}

/// `isFullWidthCodePoint` from 01_console.js.
pub fn is_full_width_code_point(code: u32) -> bool {
  code >= 0x1100
    && (code <= 0x115f // Hangul Jamo
      || code == 0x2329 // LEFT-POINTING ANGLE BRACKET
      || code == 0x232a // RIGHT-POINTING ANGLE BRACKET
      // CJK Radicals Supplement .. Enclosed CJK Letters and Months
      || ((0x2e80..=0x3247).contains(&code) && code != 0x303f)
      // Enclosed CJK Letters and Months .. CJK Unified Ideographs Extension A
      || (0x3250..=0x4dbf).contains(&code)
      // CJK Unified Ideographs .. Yi Radicals
      || (0x4e00..=0xa4c6).contains(&code)
      // Hangul Jamo Extended-A
      || (0xa960..=0xa97c).contains(&code)
      // Hangul Syllables
      || (0xac00..=0xd7a3).contains(&code)
      // CJK Compatibility Ideographs
      || (0xf900..=0xfaff).contains(&code)
      // Vertical Forms
      || (0xfe10..=0xfe19).contains(&code)
      // CJK Compatibility Forms .. Small Form Variants
      || (0xfe30..=0xfe6b).contains(&code)
      // Halfwidth and Fullwidth Forms
      || (0xff01..=0xff60).contains(&code)
      || (0xffe0..=0xffe6).contains(&code)
      // Kana Supplement
      || (0x1b000..=0x1b001).contains(&code)
      // Enclosed Ideographic Supplement
      || (0x1f200..=0x1f251).contains(&code)
      // Miscellaneous Symbols and Pictographs / Emoticons
      || (0x1f300..=0x1f64f).contains(&code)
      // CJK Unified Ideographs Extension B .. Tertiary Ideographic Plane
      || (0x20000..=0x3fffd).contains(&code))
}

/// `getStringWidth` from 01_console.js. The JS version iterates the string
/// with a string iterator after NFC normalization; surrogate pairs yield a
/// single code point, like `char` iteration here.
pub fn get_string_width(s: &str, remove_control_chars: bool) -> usize {
  let stripped;
  let s = if remove_control_chars {
    stripped = strip_vt_control_characters(s);
    &stripped
  } else {
    s
  };
  // NFC normalize, mirroring StringPrototypeNormalize(str, "NFC").
  let normalized = nfc_normalize(s);
  let mut width = 0;
  for ch in normalized.chars() {
    let code = ch as u32;
    if is_full_width_code_point(code) {
      width += 2;
    } else if !is_zero_width_code_point(code) {
      width += 1;
    }
  }
  width
}

/// NFC normalization. JS relies on `String.prototype.normalize("NFC")`.
/// Most console strings are ASCII; fast-path those.
fn nfc_normalize(s: &str) -> std::borrow::Cow<'_, str> {
  use unicode_normalization::UnicodeNormalization;
  if s.is_ascii() {
    return std::borrow::Cow::Borrowed(s);
  }
  std::borrow::Cow::Owned(s.nfc().collect())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn widths() {
    assert_eq!(get_string_width("hello", true), 5);
    assert_eq!(get_string_width("\u{1b}[31mhi\u{1b}[39m", true), 2);
    assert_eq!(get_string_width("デノ", true), 4);
    assert_eq!(get_string_width("a\u{300}", true), 1); // a + combining accent (NFC -> U+00E0)
  }

  #[test]
  fn strip_ansi() {
    assert_eq!(
      strip_vt_control_characters("\u{1b}[4mcake\u{1b}[24m"),
      "cake"
    );
    assert_eq!(strip_vt_control_characters("no escapes"), "no escapes");
  }

  #[test]
  fn strip_truncated_escapes() {
    // Truncated CSI sequences: the terminator class includes digits, so the
    // last parameter digit doubles as the terminator (regex backtracking).
    assert_eq!(strip_vt_control_characters("\u{1b}[5"), "");
    assert_eq!(strip_vt_control_characters("\u{1b}[1;2"), "");
    assert_eq!(strip_vt_control_characters("a\u{1b}[5b"), "ab");
    // A trailing `;` is not a terminator; the match stops before it.
    assert_eq!(strip_vt_control_characters("\u{1b}[1;"), ";");
    assert_eq!(get_string_width("\u{1b}[5", true), 0);
  }
}
