// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

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
