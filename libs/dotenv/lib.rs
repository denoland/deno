// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use sys_traits::BaseEnvVar;
use sys_traits::FsRead;
use sys_traits::PathsInErrorsExt;

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
const CHAR_DOLLAR: u8 = b'$';
const CHAR_LBRACE: u8 = b'{';

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

/// Discovers the path and content fo an env file, which can then be parsed.
///
/// This walks the ancestor directories attempting to find the env file.
pub fn find_path_and_content<'a>(
  sys: &impl FsRead,
  cwd: &Path,
  file_path: &'a Path,
) -> std::io::Result<(Cow<'a, Path>, Cow<'static, str>)> {
  let sys = sys.with_paths_in_errors();
  let (start_dir, filename, original_not_found) =
    match sys.fs_read_to_string_lossy(file_path) {
      Ok(content) => return Ok((Cow::Borrowed(file_path), content)),
      Err(err) => {
        if err.kind() != std::io::ErrorKind::NotFound {
          return Err(err);
        }
        let Some(filename) = file_path.file_name() else {
          return Err(err);
        };
        // Determine the starting directory for parent traversal.
        let start_dir = if file_path.is_absolute() {
          file_path.parent()
        } else {
          Some(cwd)
        };
        let Some(start_dir) = start_dir else {
          return Err(err);
        };
        (start_dir, filename, err)
      }
    };

  // walk parent directories looking for the file
  for dir in start_dir.ancestors().skip(1) {
    let candidate = dir.join(filename);
    match sys.fs_read_to_string_lossy(&candidate) {
      Ok(content) => return Ok((Cow::Owned(candidate), content)),
      Err(err) => {
        if err.kind() != std::io::ErrorKind::NotFound {
          return Err(err);
        }
      }
    }
  }

  Err(original_not_found)
}

type IterElement = Result<(String, String), Error>;

pub fn from_content_sanitized_iter_with_substitution(
  sys: &dyn BaseEnvVar,
  content: &str,
) -> Result<std::vec::IntoIter<IterElement>, Error> {
  let mut pairs = Vec::new();
  parse_env_content_with_substitution_hook(sys, content, &mut |k, v| {
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

/// Ported from:
/// https://github.com/nodejs/node/blob/9cc7fcc26dece769d9ffa06c453f0171311b01f8/src/node_dotenv.cc#L138-L315
pub fn parse_env_content_hook(content: &str, cb: &mut dyn FnMut(&str, &str)) {
  parse_env_content_hook_impl(content, None, cb);
}

pub fn parse_env_content_with_substitution_hook(
  sys: &dyn BaseEnvVar,
  content: &str,
  mut cb: &mut dyn FnMut(&str, &str),
) {
  let mut substitution_map = HashMap::new();
  parse_env_content_hook_impl(
    content,
    Some(SubstitutionMap {
      sys,
      map: &mut substitution_map,
    }),
    &mut cb,
  );
}

struct SubstitutionMap<'a, 'b> {
  sys: &'a dyn BaseEnvVar,
  map: &'b mut HashMap<String, String>,
}

fn parse_env_content_hook_impl(
  content: &str,
  mut substitution_map: Option<SubstitutionMap<'_, '_>>,
  cb: &mut dyn FnMut(&str, &str),
) {
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

  let mut emit_value = |key: &str, value: &str, can_substitute: bool| {
    if let Some(data) = &mut substitution_map {
      let sys = data.sys;
      let substitution_map = &mut data.map;
      let emitted_value = if can_substitute {
        apply_value_substitution(sys, value, substitution_map)
      } else {
        value.to_string()
      };
      substitution_map.insert(key.to_string(), emitted_value.clone());
      cb(key, &emitted_value);
    } else {
      cb(key, value);
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
      emit_value(key_str, "", false);
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
      emit_value(key_str, "", false);
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
      emit_value(key_str, &value_str, true);

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
        emit_value(
          key_str,
          std::str::from_utf8(value).unwrap(),
          quote == CHAR_DQUOTE,
        );

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
          emit_value(key_str, std::str::from_utf8(value).unwrap(), false);
          text = &text[newline + 1..];
        } else {
          // No newline - take rest of content
          emit_value(key_str, std::str::from_utf8(text).unwrap(), false);
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
        emit_value(key_str, std::str::from_utf8(value).unwrap(), true);
        text = &text[newline + 1..];
      } else {
        // Last line without newline
        let mut value = text;
        if let Some(hash) = find_char(value, CHAR_HASH, 0) {
          value = &value[..hash];
        }
        let value = trim_spaces_slice(value);
        emit_value(key_str, std::str::from_utf8(value).unwrap(), true);
        text = &[];
      }
    }

    text = trim_spaces_slice(text);
  }
}

fn lookup_substitution(
  sys: &dyn BaseEnvVar,
  substitution_name: &str,
  substitution_map: &HashMap<String, String>,
  output: &mut String,
) {
  if let Some(environment_value) = sys
    .base_env_var_os(substitution_name.as_ref())
    .and_then(|n| n.into_string().ok())
  {
    output.push_str(&environment_value);
  } else if let Some(stored_value) = substitution_map.get(substitution_name) {
    output.push_str(stored_value);
  }
}

fn apply_value_substitution(
  sys: &dyn BaseEnvVar,
  value: &str,
  substitution_map: &HashMap<String, String>,
) -> String {
  if !value.as_bytes().contains(&CHAR_DOLLAR) {
    return value.to_string();
  }

  let mut output = String::with_capacity(value.len());
  let mut i = 0;

  while i < value.len() {
    let remaining = &value[i..];

    if remaining.as_bytes().starts_with(b"\\$") {
      output.push('$');
      i += 2;
      continue;
    }

    if remaining.as_bytes().starts_with(b"$") {
      if remaining.as_bytes().get(1) == Some(&CHAR_LBRACE) {
        let mut end = value.len();
        for (offset, ch) in value[i + 2..].char_indices() {
          if ch == '}' {
            end = i + 2 + offset;
            break;
          }
        }
        let substitution_name = &value[i + 2..end];
        lookup_substitution(
          sys,
          substitution_name,
          substitution_map,
          &mut output,
        );
        i = if end < value.len() { end + 1 } else { end };
        continue;
      }

      let mut end = i + 1;
      for (offset, ch) in value[i + 1..].char_indices() {
        if ch.is_ascii_alphanumeric() {
          end = i + 1 + offset + ch.len_utf8();
        } else {
          break;
        }
      }
      let substitution_name = &value[i + 1..end];
      lookup_substitution(
        sys,
        substitution_name,
        substitution_map,
        &mut output,
      );
      i = end;
      continue;
    }

    let ch = remaining.chars().next().unwrap();
    output.push(ch);
    i += ch.len_utf8();
  }

  output
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

  use sys_traits::EnvSetVar;
  use sys_traits::FsCreateDirAll;
  use sys_traits::FsWrite;
  use sys_traits::impls::InMemorySys;

  use super::*;

  /// Helper: parse content and return a HashMap for easy assertion
  fn parse_map(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    parse_env_content_hook(content, &mut |key, value| {
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

  fn parse_map_with_substitution(
    sys: &dyn BaseEnvVar,
    content: &str,
  ) -> HashMap<String, String> {
    let mut map = HashMap::new();
    parse_env_content_with_substitution_hook(
      sys,
      content,
      &mut |key, value| {
        map.insert(key.to_string(), value.to_string());
      },
    );
    map
  }

  fn assert_parsed_eq_with_substitution(
    sys: &dyn BaseEnvVar,
    content: &str,
    expected: &[(&str, &str)],
  ) {
    let actual = parse_map_with_substitution(sys, content)
      .into_iter()
      .collect::<BTreeMap<_, _>>();
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
    parse_env_content_hook(content, &mut |key, value| {
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

  #[test]
  fn test_variable_in_parenthesis_surrounded_by_quotes() {
    let sys = sys_traits::impls::InMemorySys::default();
    assert_parsed_eq_with_substitution(
      &sys,
      r#"
      KEY=test
      KEY1="${KEY}"
      "#,
      &[("KEY", "test"), ("KEY1", "test")],
    );
  }

  #[test]
  fn test_substitute_undefined_variables_to_empty_string() {
    let sys = sys_traits::impls::InMemorySys::default();
    assert_parsed_eq_with_substitution(
      &sys,
      r#"KEY=">$KEY1<>${KEY2}<""#,
      &[("KEY", "><><")],
    );
  }

  #[test]
  fn test_do_not_substitute_variables_with_dollar_escaped() {
    let sys = sys_traits::impls::InMemorySys::default();
    assert_parsed_eq_with_substitution(
      &sys,
      r#"KEY=>\$KEY1<>\${KEY2}<"#,
      &[("KEY", ">$KEY1<>${KEY2}<")],
    );
  }

  #[test]
  fn test_do_not_substitute_variables_in_strong_quotes() {
    let sys = sys_traits::impls::InMemorySys::default();
    assert_parsed_eq_with_substitution(
      &sys,
      "KEY='>${KEY1}<>$KEY2<'",
      &[("KEY", ">${KEY1}<>$KEY2<")],
    );
  }

  #[test]
  fn test_same_variable_reused() {
    let sys = sys_traits::impls::InMemorySys::default();
    assert_parsed_eq_with_substitution(
      &sys,
      r#"
      KEY=VALUE
      KEY1=$KEY$KEY
      "#,
      &[("KEY", "VALUE"), ("KEY1", "VALUEVALUE")],
    );
  }

  #[test]
  fn test_variable_without_parenthesis_is_substituted_before_separators() {
    let sys = sys_traits::impls::InMemorySys::default();
    assert_parsed_eq_with_substitution(
      &sys,
      r#"
      KEY1=test_user
      KEY1_1=test_user_with_separator
      KEY=">$KEY1_1<>$KEY1}<>$KEY1{<"
      "#,
      &[
        ("KEY1", "test_user"),
        ("KEY1_1", "test_user_with_separator"),
        ("KEY", ">test_user_1<>test_user}<>test_user{<"),
      ],
    );
  }

  #[test]
  fn test_substitute_variable_from_env_variable() {
    let sys = sys_traits::impls::InMemorySys::default();
    sys.env_set_var("DENO_DOTENV_TEST_KEY11", "overriden from process env");

    assert_parsed_eq_with_substitution(
      &sys,
      r#"KEY=">${DENO_DOTENV_TEST_KEY11}<""#,
      &[("KEY", ">overriden from process env<")],
    );
  }

  #[test]
  fn test_substitute_variable_env_variable_overrides_dotenv_in_substitution() {
    let sys = sys_traits::impls::InMemorySys::default();
    sys.env_set_var("DENO_DOTENV_TEST_KEY11", "overriden from process env");

    assert_parsed_eq_with_substitution(
      &sys,
      r#"
      DENO_DOTENV_TEST_KEY11=test_user
      KEY=">${DENO_DOTENV_TEST_KEY11}<"
      "#,
      &[
        ("DENO_DOTENV_TEST_KEY11", "test_user"),
        ("KEY", ">overriden from process env<"),
      ],
    );
  }

  #[test]
  fn test_consequent_substitutions() {
    let sys = sys_traits::impls::InMemorySys::default();
    assert_parsed_eq_with_substitution(
      &sys,
      r#"
      KEY1=test_user
      KEY2=$KEY1_2
      KEY=>${KEY1}<>${KEY2}<
      "#,
      &[
        ("KEY1", "test_user"),
        ("KEY2", "test_user_2"),
        ("KEY", ">test_user<>test_user_2<"),
      ],
    );
  }

  #[test]
  fn test_consequent_substitutions_with_one_missing() {
    let sys = sys_traits::impls::InMemorySys::default();
    assert_parsed_eq_with_substitution(
      &sys,
      r#"
      KEY2=$KEY1_2
      KEY=>${KEY1}<>${KEY2}<
      "#,
      &[("KEY2", "_2"), ("KEY", "><>_2<")],
    );
  }

  #[test]
  fn find_path_and_content_reads_file_directly() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/project").unwrap();
    sys.fs_write("/project/.env", "KEY=value").unwrap();

    let (path, content) = find_path_and_content(
      &sys,
      Path::new("/project"),
      Path::new("/project/.env"),
    )
    .unwrap();
    assert_eq!(path, Path::new("/project/.env"));
    assert_eq!(content.as_ref(), "KEY=value");
  }

  #[test]
  fn find_path_and_content_traverses_parent_dirs() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/project/sub/deep").unwrap();
    sys.fs_write("/project/.env", "FOUND=true").unwrap();

    let (path, content) = find_path_and_content(
      &sys,
      Path::new("/project/sub/deep"),
      Path::new(".env"),
    )
    .unwrap();
    assert_eq!(path, Path::new("/project/.env"));
    assert_eq!(content.as_ref(), "FOUND=true");
  }

  #[test]
  fn find_path_and_content_returns_closest_ancestor() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/project/sub/deep").unwrap();
    sys.fs_write("/project/.env", "ROOT=true").unwrap();
    sys.fs_write("/project/sub/.env", "MID=true").unwrap();

    // starting from /project/sub/deep, should find /project/sub/.env first
    let (path, content) = find_path_and_content(
      &sys,
      Path::new("/project/sub/deep"),
      Path::new(".env"),
    )
    .unwrap();
    assert_eq!(path, Path::new("/project/sub/.env"));
    assert_eq!(content.as_ref(), "MID=true");
  }

  #[test]
  fn find_path_and_content_not_found() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/project").unwrap();

    let result =
      find_path_and_content(&sys, Path::new("/project"), Path::new(".env"));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
  }

  #[test]
  fn find_path_and_content_custom_filename() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/project/child").unwrap();
    sys.fs_write("/project/.env.local", "LOCAL=1").unwrap();

    let (path, content) = find_path_and_content(
      &sys,
      Path::new("/project/child"),
      Path::new(".env.local"),
    )
    .unwrap();
    assert_eq!(path, Path::new("/project/.env.local"));
    assert_eq!(content.as_ref(), "LOCAL=1");
  }

  #[test]
  fn find_path_and_content_relative_subdir_traverses_ancestors() {
    let sys = InMemorySys::default();
    sys.fs_create_dir_all("/project/sub/deep").unwrap();
    // file doesn't exist at sub/.envfile relative to cwd, but exists in
    // an ancestor directory — traversal uses just the filename component
    sys.fs_write("/project/.envfile", "ANCESTOR=found").unwrap();

    let (path, content) = find_path_and_content(
      &sys,
      Path::new("/project/sub/deep"),
      Path::new("sub/.envfile"),
    )
    .unwrap();
    assert_eq!(path, Path::new("/project/.envfile"));
    assert_eq!(content.as_ref(), "ANCESTOR=found");
  }
}
