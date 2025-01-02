// Copyright 2018-2025 the Deno authors. MIT license.

#[derive(Debug)]
pub struct FileMetrics {
  long_lines_count: usize,
  total_lines: usize,
  whitespace_ratio: f64,
  has_license_comment: bool,
}

impl FileMetrics {
  #[inline]
  pub fn is_likely_minified(&self) -> bool {
    let long_lines_ratio =
      self.long_lines_count as f64 / self.total_lines as f64;

    (long_lines_ratio >= 0.2 || self.whitespace_ratio < 0.05)
      && !(self.has_license_comment && self.total_lines < 3)
  }
}

/// Analyze the content and tell if the file is most likely a minified file or not.
pub fn is_likely_minified(content: &str) -> bool {
  const LONG_LINE_LEN: usize = 250;
  let mut total_lines = 0;
  let mut long_lines_count = 0;
  let mut whitespace_count = 0;
  let mut total_chars = 0;
  let mut has_license = false;
  let mut in_multiline_comment = false;

  // If total len of a file is shorter than the "long line" length, don't bother analyzing
  // and consider non-minified.
  if content.len() < LONG_LINE_LEN {
    return false;
  }

  // Preallocate a line buffer to avoid per-line allocations
  let mut line_buffer = String::with_capacity(1024);

  // Process the content character by character to avoid line allocations
  let mut chars = content.chars().peekable();
  while let Some(c) = chars.next() {
    total_chars += 1;

    if c.is_whitespace() {
      whitespace_count += 1;
    }

    line_buffer.push(c);

    // Check for end of line or end of content
    if c == '\n' || chars.peek().is_none() {
      total_lines += 1;
      let trimmed = line_buffer.trim();

      // Check for license/copyright only if we haven't found one yet
      if !has_license && !trimmed.is_empty() {
        // Avoid allocating a new string for case comparison
        has_license = trimmed.chars().any(|c| c.is_ascii_alphabetic())
          && (trimmed.contains("license")
            || trimmed.contains("LICENSE")
            || trimmed.contains("copyright")
            || trimmed.contains("COPYRIGHT")
            || trimmed.contains("(c)")
            || trimmed.contains("(C)"));
      }

      // Handle comments without allocating new strings
      if trimmed.starts_with("/*") {
        in_multiline_comment = true;
      }
      if trimmed.ends_with("*/") {
        in_multiline_comment = false;
        line_buffer.clear();
        continue;
      }
      if in_multiline_comment || trimmed.starts_with("//") {
        line_buffer.clear();
        continue;
      }

      // Check line length
      if line_buffer.len() > LONG_LINE_LEN {
        long_lines_count += 1;
      }

      line_buffer.clear();
    }
  }

  // Handle case where file doesn't end with newline
  if !line_buffer.is_empty() {
    total_lines += 1;
  }

  let whitespace_ratio = if total_chars > 0 {
    whitespace_count as f64 / total_chars as f64
  } else {
    0.0
  };

  let metrics = FileMetrics {
    long_lines_count,
    total_lines,
    whitespace_ratio,
    has_license_comment: has_license,
  };

  metrics.is_likely_minified()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_normal_js() {
    let content = r#"
function hello() {
    // This is a normal comment
    console.log("Hello, world!");
}

// Another comment
const x = 42;

/* Multi-line
    comment */
"#;
    assert!(!is_likely_minified(content));
  }

  #[test]
  fn empty_file() {
    assert!(!is_likely_minified(""));
  }

  #[test]
  fn test_minified_file_col_length() {
    let content =
      "const LOREM_IPSUM = `Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.`";
    assert!(is_likely_minified(content));
  }

  #[test]
  fn test_minified_js() {
    let content = "function hello(){console.log(\"Hello, world!\")}const x=42;function veryLongFunction(){return\"This is a very long line that exceeds 250 characters and contains lots of code and stuff and more code and even more stuff until we definitely exceed the limit we set for considering a line to be very long in our minification detection algorithm\"}";
    assert!(is_likely_minified(content));
  }

  #[test]
  fn test_minified_file_whitespace() {
    let content =
      "function f(a,b){return a.concat(b)}var x=function(n){return n+1};";
    assert!(!is_likely_minified(content));
  }

  #[test]
  fn test_license_only() {
    let content = r#"/* 
* Copyright (c) 2024 Example Corp.
* Licensed under MIT License
*/
"#;
    assert!(!is_likely_minified(content));
  }

  #[test]
  fn test_normal_file() {
    let content = r#"
function concatenateArrays(array1, array2) {
    return array1.concat(array2);
}

const incrementNumber = function(number) {
    return number + 1;
};"#;
    assert!(!is_likely_minified(content));
  }
}
