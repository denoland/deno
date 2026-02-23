// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;

const CHAR_NL: u8 = b'\n';
const CHAR_CR: u8 = b'\r';
const CHAR_TAB: u8 = b'\t';
const CHAR_SPACE: u8 = b' ';
const CHAR_HASH: u8 = b'#';
const CHAR_EQ: u8 = b'=';
const CHAR_DQUOTE: u8 = b'"';
const CHAR_SQUOTE: u8 = b'\'';
const CHAR_BQUOTE: u8 = b'`';
const CHAR_BSLASH: u8 = b'\\';
const CHAR_N: u8 = b'n';

#[derive(Debug)]
pub enum Error {
  LineParse(String, usize),
  Io(std::io::Error),
}

impl std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Error::LineParse(line, index) => {
        write!(f, "Error parsing line at index {}: {}", index, line)
      }
      Error::Io(err) => write!(f, "{}", err),
    }
  }
}

impl std::error::Error for Error {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Error::Io(err) => Some(err),
      _ => None,
    }
  }
}

impl From<std::io::Error> for Error {
  fn from(err: std::io::Error) -> Self {
    Error::Io(err)
  }
}

/// Ported from:
/// https://github.com/nodejs/node/blob/9cc7fcc26dece769d9ffa06c453f0171311b01f8/src/node_dotenv.cc#L138-L315
pub fn parse_env_content_hook(content: &str, mut cb: impl FnMut(&str, &str)) {
  let raw = content.as_bytes();
  let mut filtered = Vec::new();
  let mut saw_cr = false;
  let mut text = {
    // Handle windows newlines "\r\n": remove "\r" and keep only "\n"
    let mut i = 0;
    while i < raw.len() {
      if raw[i] == CHAR_CR {
        saw_cr = true;
        filtered = Vec::with_capacity(raw.len() - 1);
        filtered.extend_from_slice(&raw[..i]);
        i += 1;
        while i < raw.len() {
          let c = raw[i];
          if c != CHAR_CR {
            filtered.push(c);
          }
          i += 1;
        }
        break;
      }
      i += 1;
    }
    if saw_cr {
      trim_spaces_slice(&filtered)
    } else {
      trim_spaces_slice(raw)
    }
  };

  while !text.is_empty() {
    let first = text[0];

    // Skip empty lines and comments
    // Check if the first character of the content is a newline or a hash
    if first == CHAR_NL || first == CHAR_HASH {
      // Remove everything up to and including the newline character
      if let Some(newline) = find_char(text, CHAR_NL, 0) {
        text = &text[newline + 1..];
      } else {
        // If no newline is found, clear the content
        text = &[];
      }
      // Skip the remaining code in the loop and continue with the next
      // iteration.
      continue;
    }

    // Find the next equals sign or newline in a single pass.
    // This optimizes the search by avoiding multiple iterations.
    let equal_or_newline = {
      let mut index = None;
      let mut i = 0;
      while i < text.len() {
        let c = text[i];
        if c == CHAR_EQ || c == CHAR_NL {
          index = Some(i);
          break;
        }
        i += 1;
      }
      match index {
        Some(index) => index,
        None => break,
      }
    };

    // If we found nothing or found a newline before equals, the line is invalid
    if text[equal_or_newline] == CHAR_NL {
      text = trim_spaces_slice(&text[equal_or_newline + 1..]);
      continue;
    }

    // We found an equals sign, extract the key
    let mut key = trim_spaces_slice(&text[..equal_or_newline]);
    text = &text[equal_or_newline + 1..];

    // If the value is not present (e.g. KEY=) set it to an empty string
    if text.is_empty() || text[0] == CHAR_NL {
      let key_str = std::str::from_utf8(key).unwrap();
      cb(key_str, "");
      continue;
    }

    text = trim_spaces_slice(text);

    // Skip lines with empty keys after trimming spaces.
    // Examples of invalid keys that would be skipped:
    //   =value
    //   "   "=value
    if key.is_empty() {
      continue;
    }

    // Remove export prefix from key and ensure proper spacing.
    // Example: export FOO=bar -> FOO=bar
    if key.len() >= 7
      && key[0] == b'e'
      && key[1] == b'x'
      && key[2] == b'p'
      && key[3] == b'o'
      && key[4] == b'r'
      && key[5] == b't'
      && key[6] == CHAR_SPACE
    {
      // Trim spaces after removing export prefix to handle cases like:
      // export   FOO=bar
      key = trim_spaces_slice(&key[7..]);
    }

    let key_str = std::str::from_utf8(key).unwrap();

    // SAFETY: Content is guaranteed to have at least one character
    // In case the last line is a single key without value
    // Example: KEY= (without a newline at the EOF)
    if text.is_empty() {
      cb(key_str, "");
      break;
    }

    // Expand new line if \n it's inside double quotes
    // Example: EXPAND_NEWLINES = 'expand\nnew\nlines'
    if text[0] == CHAR_DQUOTE
      && let Some(closing) = find_char(text, CHAR_DQUOTE, 1)
    {
      let slice = &text[1..closing];
      let mut needs_unescape = false;
      let mut i = 0;
      while i + 1 < slice.len() {
        if slice[i] == CHAR_BSLASH && slice[i + 1] == CHAR_N {
          needs_unescape = true;
          break;
        }
        i += 1;
      }
      let value_str = if !needs_unescape {
        Cow::Borrowed(std::str::from_utf8(slice).unwrap())
      } else {
        let mut out = Vec::with_capacity(slice.len());
        let mut i = 0;
        // Replace \n with actual newlines in double-quoted strings
        while i < slice.len() {
          let c = slice[i];
          if c == CHAR_BSLASH && i + 1 < slice.len() && slice[i + 1] == CHAR_N {
            out.push(CHAR_NL);
            i += 2;
            continue;
          }
          out.push(c);
          i += 1;
        }
        Cow::Owned(String::from_utf8(out).unwrap())
      };
      cb(key_str, &value_str);

      if let Some(newline) = find_char(text, CHAR_NL, closing + 1) {
        text = &text[newline + 1..];
      } else {
        // In case the last line is a single key/value pair
        // Example: KEY=VALUE (without a newline at the EOF)
        text = &[];
      }
      // No valid data here, skip to next line
      continue;
    }

    // Handle quoted values (single quotes, double quotes, backticks)
    let quote = text[0];
    if quote == CHAR_SQUOTE || quote == CHAR_DQUOTE || quote == CHAR_BQUOTE {
      if let Some(closing) = find_char(text, quote, 1) {
        // Found closing quote - take content between quotes
        let value = &text[1..closing];
        cb(key_str, std::str::from_utf8(value).unwrap());

        if let Some(newline) = find_char(text, CHAR_NL, closing + 1) {
          text = &text[newline + 1..];
        } else {
          text = &[];
        }
        // No valid data here, skip to next line
        continue;
      } else {
        // Check if the closing quote is not found
        // Example: KEY="value
        // Check if newline exists. If it does, take the entire line as the value
        // Example: KEY="value\nKEY2=value2
        // The value pair should be `"value`
        if let Some(newline) = find_char(text, CHAR_NL, 0) {
          let value = &text[..newline];
          cb(key_str, std::str::from_utf8(value).unwrap());
          text = &text[newline + 1..];
        } else {
          // No newline - take rest of content
          cb(key_str, std::str::from_utf8(text).unwrap());
          break;
        }
      }
    } else {
      // Regular key value pair.
      // Example: `KEY=this is value`
      if let Some(newline) = find_char(text, CHAR_NL, 0) {
        let mut value = &text[..newline];
        // Check if there is a comment in the line
        // Example: KEY=value # comment
        // The value pair should be `value`
        if let Some(hash) = find_char(value, CHAR_HASH, 0) {
          value = &value[..hash];
        }
        let value = trim_spaces_slice(value);
        cb(key_str, std::str::from_utf8(value).unwrap());
        text = &text[newline + 1..];
      } else {
        // Last line without newline
        let mut value = text;
        if let Some(hash) = find_char(value, CHAR_HASH, 0) {
          value = &value[..hash];
        }
        let value = trim_spaces_slice(value);
        cb(key_str, std::str::from_utf8(value).unwrap());
        text = &[];
      }
    }

    text = trim_spaces_slice(text);
  }
}

type IterElement = Result<(String, String), Error>;

pub fn from_path_sanitized_iter(
  path: impl AsRef<Path>,
) -> Result<std::vec::IntoIter<IterElement>, Error> {
  let content = std::fs::read_to_string(path.as_ref()).map_err(Error::Io)?;
  let mut pairs = Vec::new();
  parse_env_content_hook(&content, |k, v| {
    if let Some(index) = k
      .find('\0')
      .or_else(|| v.find('\0').map(|i| k.len() + i + 1))
    {
      pairs.push(Err(Error::LineParse(format!("{}={}", k, v), index)));
    } else {
      pairs.push(Ok((k.to_string(), v.to_string())));
    }
  });
  Ok(pairs.into_iter())
}

pub fn from_path(filename: impl AsRef<Path>) -> Result<(), Error> {
  for item in from_path_sanitized_iter(filename)? {
    let (key, val) = item?;
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe {
      std::env::set_var(&key, &val);
    }
  }
  Ok(())
}

fn trim_spaces_slice(input: &[u8]) -> &[u8] {
  if input.is_empty() {
    return input;
  }
  let mut start = 0;
  let mut end = input.len();

  while start < end {
    let c = input[start];
    if c != CHAR_SPACE && c != CHAR_TAB && c != CHAR_NL {
      break;
    }
    start += 1;
  }

  while end > start {
    let c = input[end - 1];
    if c != CHAR_SPACE && c != CHAR_TAB && c != CHAR_NL {
      break;
    }
    end -= 1;
  }

  &input[start..end]
}

fn find_char(input: &[u8], char_code: u8, from: usize) -> Option<usize> {
  let mut i = from;
  while i < input.len() {
    if input[i] == char_code {
      return Some(i);
    }
    i += 1;
  }
  None
}

#[cfg(test)]
mod tests {
  use std::collections::BTreeMap;
  use std::collections::HashMap;

  use super::*;

  /// Helper: parse content and return a HashMap for easy assertion
  fn parse_map(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    parse_env_content_hook(content, |key, value| {
      map.insert(key.to_string(), value.to_string());
    });
    map
  }

  fn assert_parsed_eq(content: &str, expected: &[(&str, &str)]) {
    let actual = parse_map(content).into_iter().collect::<BTreeMap<_, _>>();
    let expected = expected
      .iter()
      .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
      .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
  }

  #[test]
  fn test_valid_env() {
    // https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/benchmark/fixtures/valid.env
    let content = r#"BASIC=basic

# COMMENTS=work
#BASIC=basic2
#BASIC=basic3

# previous line intentionally left blank
AFTER_LINE=after_line
A="B=C"
B=C=D
EMPTY=
EMPTY_SINGLE_QUOTES=''
EMPTY_DOUBLE_QUOTES=""
EMPTY_BACKTICKS=``
SINGLE_QUOTES='single_quotes'
SINGLE_QUOTES_SPACED='    single quotes    '
DOUBLE_QUOTES="double_quotes"
DOUBLE_QUOTES_SPACED="    double quotes    "
DOUBLE_QUOTES_INSIDE_SINGLE='double "quotes" work inside single quotes'
DOUBLE_QUOTES_WITH_NO_SPACE_BRACKET="{ port: $MONGOLAB_PORT}"
SINGLE_QUOTES_INSIDE_DOUBLE="single 'quotes' work inside double quotes"
BACKTICKS_INSIDE_SINGLE='`backticks` work inside single quotes'
BACKTICKS_INSIDE_DOUBLE="`backticks` work inside double quotes"
BACKTICKS=`backticks`
BACKTICKS_SPACED=`    backticks    `
DOUBLE_QUOTES_INSIDE_BACKTICKS=`double "quotes" work inside backticks`
SINGLE_QUOTES_INSIDE_BACKTICKS=`single 'quotes' work inside backticks`
DOUBLE_AND_SINGLE_QUOTES_INSIDE_BACKTICKS=`double "quotes" and single 'quotes' work inside backticks`
EXPAND_NEWLINES="expand\nnew\nlines"
DONT_EXPAND_UNQUOTED=dontexpand\nnewlines
DONT_EXPAND_SQUOTED='dontexpand\nnewlines'
# COMMENTS=work
INLINE_COMMENTS=inline comments # work #very #well
INLINE_COMMENTS_SINGLE_QUOTES='inline comments outside of #singlequotes' # work
INLINE_COMMENTS_DOUBLE_QUOTES="inline comments outside of #doublequotes" # work
INLINE_COMMENTS_BACKTICKS=`inline comments outside of #backticks` # work
INLINE_COMMENTS_SPACE=inline comments start with a#number sign. no space required.
EQUAL_SIGNS=equals==
RETAIN_INNER_QUOTES={"foo": "bar"}
RETAIN_INNER_QUOTES_AS_STRING='{"foo": "bar"}'
RETAIN_INNER_QUOTES_AS_BACKTICKS=`{"foo": "bar's"}`
TRIM_SPACE_FROM_UNQUOTED=    some spaced out string
SPACE_BEFORE_DOUBLE_QUOTES=   "space before double quotes"
EMAIL=therealnerdybeast@example.tld
    SPACED_KEY = parsed
EDGE_CASE_INLINE_COMMENTS="VALUE1" # or "VALUE2" or "VALUE3"

MULTI_DOUBLE_QUOTED="THIS
IS
A
MULTILINE
STRING"

MULTI_SINGLE_QUOTED='THIS
IS
A
MULTILINE
STRING'

MULTI_BACKTICKED=`THIS
IS
A
"MULTILINE'S"
STRING`
export EXPORT_EXAMPLE = ignore export

MULTI_NOT_VALID_QUOTE="
MULTI_NOT_VALID=THIS
IS NOT MULTILINE
"#;
    assert_parsed_eq(
      content,
      &[
        ("BASIC", "basic"),
        ("AFTER_LINE", "after_line"),
        ("A", "B=C"),
        ("B", "C=D"),
        ("EMPTY", ""),
        ("EMPTY_SINGLE_QUOTES", ""),
        ("EMPTY_DOUBLE_QUOTES", ""),
        ("EMPTY_BACKTICKS", ""),
        ("SINGLE_QUOTES", "single_quotes"),
        ("SINGLE_QUOTES_SPACED", "    single quotes    "),
        ("DOUBLE_QUOTES", "double_quotes"),
        ("DOUBLE_QUOTES_SPACED", "    double quotes    "),
        (
          "DOUBLE_QUOTES_INSIDE_SINGLE",
          r#"double "quotes" work inside single quotes"#,
        ),
        (
          "DOUBLE_QUOTES_WITH_NO_SPACE_BRACKET",
          "{ port: $MONGOLAB_PORT}",
        ),
        (
          "SINGLE_QUOTES_INSIDE_DOUBLE",
          "single 'quotes' work inside double quotes",
        ),
        (
          "BACKTICKS_INSIDE_SINGLE",
          "`backticks` work inside single quotes",
        ),
        (
          "BACKTICKS_INSIDE_DOUBLE",
          "`backticks` work inside double quotes",
        ),
        ("BACKTICKS", "backticks"),
        ("BACKTICKS_SPACED", "    backticks    "),
        (
          "DOUBLE_QUOTES_INSIDE_BACKTICKS",
          r#"double "quotes" work inside backticks"#,
        ),
        (
          "SINGLE_QUOTES_INSIDE_BACKTICKS",
          "single 'quotes' work inside backticks",
        ),
        (
          "DOUBLE_AND_SINGLE_QUOTES_INSIDE_BACKTICKS",
          "double \"quotes\" and single 'quotes' work inside backticks",
        ),
        ("EXPAND_NEWLINES", "expand\nnew\nlines"),
        ("DONT_EXPAND_UNQUOTED", "dontexpand\\nnewlines"),
        ("DONT_EXPAND_SQUOTED", "dontexpand\\nnewlines"),
        ("INLINE_COMMENTS", "inline comments"),
        (
          "INLINE_COMMENTS_SINGLE_QUOTES",
          "inline comments outside of #singlequotes",
        ),
        (
          "INLINE_COMMENTS_DOUBLE_QUOTES",
          "inline comments outside of #doublequotes",
        ),
        (
          "INLINE_COMMENTS_BACKTICKS",
          "inline comments outside of #backticks",
        ),
        ("INLINE_COMMENTS_SPACE", "inline comments start with a"),
        ("EQUAL_SIGNS", "equals=="),
        ("RETAIN_INNER_QUOTES", r#"{"foo": "bar"}"#),
        ("RETAIN_INNER_QUOTES_AS_STRING", r#"{"foo": "bar"}"#),
        ("RETAIN_INNER_QUOTES_AS_BACKTICKS", r#"{"foo": "bar's"}"#),
        ("TRIM_SPACE_FROM_UNQUOTED", "some spaced out string"),
        ("SPACE_BEFORE_DOUBLE_QUOTES", "space before double quotes"),
        ("EMAIL", "therealnerdybeast@example.tld"),
        ("SPACED_KEY", "parsed"),
        ("EDGE_CASE_INLINE_COMMENTS", "VALUE1"),
        ("MULTI_DOUBLE_QUOTED", "THIS\nIS\nA\nMULTILINE\nSTRING"),
        ("MULTI_SINGLE_QUOTED", "THIS\nIS\nA\nMULTILINE\nSTRING"),
        ("MULTI_BACKTICKED", "THIS\nIS\nA\n\"MULTILINE'S\"\nSTRING"),
        ("EXPORT_EXAMPLE", "ignore export"),
        ("MULTI_NOT_VALID_QUOTE", "\""),
        ("MULTI_NOT_VALID", "THIS"),
      ],
    );
  }

  #[test]
  fn test_eof_without_value() {
    // https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/test/fixtures/dotenv/eof-without-value.env
    let content = "BASIC=value\nEMPTY=\n";
    assert_parsed_eq(content, &[("BASIC", "value"), ("EMPTY", "")]);
  }

  #[test]
  fn test_eof_without_value_no_trailing_newline() {
    let content = "BASIC=value\nEMPTY=";
    assert_parsed_eq(content, &[("BASIC", "value"), ("EMPTY", "")]);
  }

  #[test]
  fn test_multiline() {
    // https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/test/fixtures/dotenv/multiline.env
    let content = r#"JWT_PUBLIC_KEY="-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAnNl1tL3QjKp3DZWM0T3u
LgGJQwu9WqyzHKZ6WIA5T+7zPjO1L8l3S8k8YzBrfH4mqWOD1GBI8Yjq2L1ac3Y/
bTdfHN8CmQr2iDJC0C6zY8YV93oZB3x0zC/LPbRYpF8f6OqX1lZj5vo2zJZy4fI/
kKcI5jHYc8VJq+KCuRZrvn+3V+KuL9tF9v8ZgjF2PZbU+LsCy5Yqg1M8f5Jp5f6V
u4QuUoobAgMBAAE=
-----END PUBLIC KEY-----"
"#;
    assert_parsed_eq(
      content,
      &[(
        "JWT_PUBLIC_KEY",
        "-----BEGIN PUBLIC KEY-----\n\
         MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAnNl1tL3QjKp3DZWM0T3u\n\
         LgGJQwu9WqyzHKZ6WIA5T+7zPjO1L8l3S8k8YzBrfH4mqWOD1GBI8Yjq2L1ac3Y/\n\
         bTdfHN8CmQr2iDJC0C6zY8YV93oZB3x0zC/LPbRYpF8f6OqX1lZj5vo2zJZy4fI/\n\
         kKcI5jHYc8VJq+KCuRZrvn+3V+KuL9tF9v8ZgjF2PZbU+LsCy5Yqg1M8f5Jp5f6V\n\
         u4QuUoobAgMBAAE=\n\
         -----END PUBLIC KEY-----",
      )],
    );
  }

  #[test]
  fn test_lines_with_only_spaces() {
    // https://github.com/nodejs/node/blob/70f6b58ac655234435a99d72b857dd7b316d34bf/test/fixtures/dotenv/lines-with-only-spaces.env
    let content = "\nEMPTY_LINE='value after an empty line'\n      \nSPACES_LINE='value after a line with just some spaces'\n\t\t\t\t\nTABS_LINE='value after a line with just some tabs'\n\t    \t\t\t\nSPACES_TABS_LINE='value after a line with just some spaces and tabs'\n";
    assert_parsed_eq(
      content,
      &[
        ("EMPTY_LINE", "value after an empty line"),
        ("SPACES_LINE", "value after a line with just some spaces"),
        ("TABS_LINE", "value after a line with just some tabs"),
        (
          "SPACES_TABS_LINE",
          "value after a line with just some spaces and tabs",
        ),
      ],
    );
  }

  #[test]
  fn test_windows_line_endings() {
    let content = "KEY1=value1\r\nKEY2=value2\r\nKEY3=value3\r\n";
    assert_parsed_eq(
      content,
      &[("KEY1", "value1"), ("KEY2", "value2"), ("KEY3", "value3")],
    );
  }

  #[test]
  fn test_empty_content() {
    let env = parse_map("");
    assert!(env.is_empty());
  }

  #[test]
  fn test_only_comments() {
    let content = "# this is a comment\n# another comment\n";
    let env = parse_map(content);
    assert!(env.is_empty());
  }

  #[test]
  fn test_export_prefix() {
    let content = "export FOO=bar\nexport   BAZ=qux\n";
    assert_parsed_eq(content, &[("FOO", "bar"), ("BAZ", "qux")]);
  }

  #[test]
  fn test_callback_order() {
    let content = "A=1\nB=2\nC=3\n";
    let mut entries = Vec::new();
    parse_env_content_hook(content, |key, value| {
      entries.push((key.to_string(), value.to_string()));
    });
    assert_eq!(
      entries,
      vec![
        ("A".to_string(), "1".to_string()),
        ("B".to_string(), "2".to_string()),
        ("C".to_string(), "3".to_string()),
      ]
    );
  }

  #[test]
  fn test_empty_key_skipped() {
    let content = "=value\n";
    let env = parse_map(content);
    assert!(env.is_empty());
  }

  #[test]
  fn test_single_key_no_value_eof() {
    let content = "KEY=";
    assert_parsed_eq(content, &[("KEY", "")]);
  }

  #[test]
  fn test_no_newline_at_eof() {
    let content = "KEY=value";
    assert_parsed_eq(content, &[("KEY", "value")]);
  }

  #[test]
  fn test_no_newline_at_eof_with_single_quote() {
    let content = "KEY='value'";
    assert_parsed_eq(content, &[("KEY", "value")]);
  }
}
