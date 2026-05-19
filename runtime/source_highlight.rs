// Copyright 2018-2026 the Deno authors. MIT license.

//! A quick-and-dirty JavaScript/TypeScript syntax highlighter for source lines
//! displayed in error stack traces. This is a simple character-scanning lexer,
//! not a parser — it works on arbitrary single lines that may not be
//! syntactically valid.
//!
//! Inspired by Bun's `QuickAndDirtyJavaScriptSyntaxHighlighter`.

// ANSI color codes
const RESET: &str = "\x1b[0m";
const BRIGHT_BLUE: &str = "\x1b[94m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const BOLD: &str = "\x1b[1m";
const GRAY: &str = "\x1b[90m";

#[derive(Clone, Copy, PartialEq, Eq)]
enum KeywordKind {
  ControlFlow,
  TypeKeyword,
  BooleanLiteral, // true, false — yellow (matches console)
  Null,           // null — bold (matches console)
  Undefined,      // undefined — gray (matches console)
  ThisLiteral,    // this, NaN, Infinity — yellow
  Delete,
}

/// Keywords that get context-dependent coloring of the *following* identifier.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PrevKeyword {
  New,
  TypeLike, // type, interface, namespace, declare, abstract, enum
  Import,
}

fn classify_keyword(word: &str) -> Option<KeywordKind> {
  match word {
    // Control flow & declarations — magenta
    "async" | "await" | "break" | "case" | "catch" | "class" | "const"
    | "continue" | "debugger" | "default" | "do" | "else" | "export"
    | "extends" | "finally" | "for" | "function" | "if" | "import" | "in"
    | "instanceof" | "let" | "new" | "package" | "return" | "static"
    | "super" | "switch" | "throw" | "try" | "typeof" | "var" | "void"
    | "while" | "with" | "yield" | "of" | "from" => {
      Some(KeywordKind::ControlFlow)
    }
    // TypeScript type keywords — cyan
    "abstract" | "as" | "declare" | "enum" | "implements" | "interface"
    | "namespace" | "type" | "keyof" | "infer" | "is" | "readonly"
    | "override" | "satisfies"
    // TS built-in type names
    | "string" | "number" | "boolean" | "symbol" | "any" | "object"
    | "unknown" | "never" | "bigint" => Some(KeywordKind::TypeKeyword),
    // Literals — colors chosen to match console.log inspect output
    "true" | "false" => Some(KeywordKind::BooleanLiteral),
    "null" => Some(KeywordKind::Null),
    "undefined" => Some(KeywordKind::Undefined),
    "this" | "NaN" | "Infinity" => Some(KeywordKind::ThisLiteral),
    // Delete — red
    "delete" => Some(KeywordKind::Delete),
    _ => None,
  }
}

fn prev_keyword_for(word: &str) -> Option<PrevKeyword> {
  match word {
    "new" => Some(PrevKeyword::New),
    "type" | "interface" | "namespace" | "declare" | "abstract" | "enum" => {
      Some(PrevKeyword::TypeLike)
    }
    "import" => Some(PrevKeyword::Import),
    _ => None,
  }
}

#[inline]
fn is_ident_start(b: u8) -> bool {
  b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

#[inline]
fn is_ident_continue(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Core highlighting logic. Operates on a byte slice and appends colored
/// output to `result`. Used both for top-level lines and recursively for
/// template literal interpolations.
fn highlight_inner(source: &str, bytes: &[u8], result: &mut String) {
  let len = bytes.len();
  let mut i = 0;
  let mut prev_kw: Option<PrevKeyword> = None;

  while i < len {
    let b = bytes[i];

    if is_ident_start(b) {
      // Consume identifier
      let start = i;
      i += 1;
      while i < len && is_ident_continue(bytes[i]) {
        i += 1;
      }
      let word = &source[start..i];

      if let Some(kind) = classify_keyword(word) {
        match kind {
          KeywordKind::Null => {
            result.push_str(BOLD);
            result.push_str(word);
            result.push_str(RESET);
          }
          KeywordKind::Undefined => {
            result.push_str(GRAY);
            result.push_str(word);
            result.push_str(RESET);
          }
          _ => {
            let color = match kind {
              KeywordKind::ControlFlow => BRIGHT_BLUE,
              KeywordKind::TypeKeyword => CYAN,
              KeywordKind::BooleanLiteral | KeywordKind::ThisLiteral => YELLOW,
              KeywordKind::Delete => RED,
              KeywordKind::Null | KeywordKind::Undefined => unreachable!(),
            };
            result.push_str(color);
            result.push_str(word);
            result.push_str(RESET);
          }
        }
        prev_kw = prev_keyword_for(word);
      } else {
        // Non-keyword identifier — apply context-dependent coloring
        match prev_kw {
          Some(PrevKeyword::New) => {
            result.push_str(word);
          }
          Some(PrevKeyword::TypeLike) => {
            result.push_str(CYAN);
            result.push_str(BOLD);
            result.push_str(word);
            result.push_str(RESET);
          }
          _ => {
            result.push_str(word);
          }
        }
        prev_kw = None;
      }
    } else if b == b'"' || b == b'\'' {
      // Regular string literal (not template)
      prev_kw = None;
      let start = i;
      let quote = b;
      i += 1;
      while i < len && bytes[i] != quote {
        if bytes[i] == b'\\' && i + 1 < len {
          i += 1;
        }
        i += 1;
      }
      if i < len {
        i += 1; // closing quote
      }
      result.push_str(GREEN);
      result.push_str(&source[start..i]);
      result.push_str(RESET);
    } else if b == b'`' {
      // Template literal — highlight interpolations separately
      prev_kw = None;
      i += 1;
      result.push_str(GREEN);
      result.push('`');
      while i < len && bytes[i] != b'`' {
        if bytes[i] == b'\\' && i + 1 < len {
          result.push(bytes[i] as char);
          result.push(bytes[i + 1] as char);
          i += 2;
        } else if bytes[i] == b'$' && i + 1 < len && bytes[i + 1] == b'{' {
          // End green for the string part, emit ${
          result.push_str(RESET);
          result.push_str("${");
          i += 2;
          // Find matching closing brace (track nesting)
          let interp_start = i;
          let mut depth: u32 = 1;
          while i < len && depth > 0 {
            match bytes[i] {
              b'{' => depth += 1,
              b'}' => depth -= 1,
              b'\'' | b'"' | b'`' => {
                // Skip over strings inside interpolation
                let q = bytes[i];
                i += 1;
                while i < len && bytes[i] != q {
                  if bytes[i] == b'\\' && i + 1 < len {
                    i += 1;
                  }
                  i += 1;
                }
                if i < len {
                  i += 1;
                }
                continue;
              }
              _ => {}
            }
            if depth > 0 {
              i += 1;
            }
          }
          let interp_end = if depth == 0 { i } else { i };
          // Recursively highlight the interpolation content
          let inner_source = &source[interp_start..interp_end];
          let inner_bytes = &bytes[interp_start..interp_end];
          highlight_inner(inner_source, inner_bytes, result);
          if depth == 0 {
            result.push('}');
            i += 1; // skip the closing }
          }
          // Resume green for the rest of the template
          result.push_str(GREEN);
        } else {
          result.push(bytes[i] as char);
          i += 1;
        }
      }
      if i < len {
        result.push('`');
        i += 1; // closing backtick
      }
      result.push_str(RESET);
    } else if b.is_ascii_digit() {
      // Number literal
      prev_kw = None;
      let start = i;
      i += 1;

      if bytes[start] == b'0'
        && i < len
        && (bytes[i] == b'x' || bytes[i] == b'X')
      {
        i += 1;
        while i < len && bytes[i].is_ascii_hexdigit() {
          i += 1;
        }
      } else if bytes[start] == b'0'
        && i < len
        && (bytes[i] == b'o' || bytes[i] == b'O')
      {
        i += 1;
        while i < len && matches!(bytes[i], b'0'..=b'7') {
          i += 1;
        }
      } else if bytes[start] == b'0'
        && i < len
        && (bytes[i] == b'b' || bytes[i] == b'B')
      {
        i += 1;
        while i < len && matches!(bytes[i], b'0' | b'1' | b'_') {
          i += 1;
        }
      } else {
        while i < len
          && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'_')
        {
          i += 1;
        }
        if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
          i += 1;
          if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
          }
          while i < len && bytes[i].is_ascii_digit() {
            i += 1;
          }
        }
      }
      if i < len && bytes[i] == b'n' {
        i += 1;
      }

      result.push_str(YELLOW);
      result.push_str(&source[start..i]);
      result.push_str(RESET);
    } else if b == b'/'
      && i + 1 < len
      && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*')
    {
      prev_kw = None;
      if bytes[i + 1] == b'/' {
        result.push_str(GRAY);
        result.push_str(&source[i..]);
        result.push_str(RESET);
        break;
      } else {
        let start = i;
        i += 2;
        loop {
          if i + 1 >= len {
            i = len;
            break;
          }
          if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            i += 2;
            break;
          }
          i += 1;
        }
        result.push_str(GRAY);
        result.push_str(&source[start..i]);
        result.push_str(RESET);
      }
    } else {
      if !b.is_ascii_whitespace() {
        prev_kw = None;
      }
      result.push(b as char);
      i += 1;
    }
  }
}

/// Syntax-highlight a single line of JavaScript/TypeScript source code
/// with ANSI color codes.
///
/// When `use_colors` is false, returns the input unchanged.
/// Bails out (returns input unchanged) if the line is longer than 2048
/// bytes or contains non-ASCII characters.
pub fn syntax_highlight_source_line(source: &str, use_colors: bool) -> String {
  if !use_colors || source.len() > 2048 || !source.is_ascii() {
    return source.to_string();
  }

  let bytes = source.as_bytes();
  let mut result = String::with_capacity(source.len() * 2);
  highlight_inner(source, bytes, &mut result);
  result
}

#[cfg(test)]
mod tests {
  use super::*;

  // Helper: strip ANSI codes for content verification
  fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
      if c == '\x1b' {
        // Skip until 'm'
        for c2 in chars.by_ref() {
          if c2 == 'm' {
            break;
          }
        }
      } else {
        result.push(c);
      }
    }
    result
  }

  // Helper: check that a token appears with a specific color
  fn has_colored(output: &str, color: &str, token: &str) -> bool {
    let colored = format!("{}{}{}", color, token, RESET);
    output.contains(&colored)
  }

  #[test]
  fn no_colors_returns_unchanged() {
    let line = "const x = 42;";
    assert_eq!(syntax_highlight_source_line(line, false), line);
  }

  #[test]
  fn non_ascii_returns_unchanged() {
    let line = "const x = '日本語';";
    assert_eq!(syntax_highlight_source_line(line, true), line);
  }

  #[test]
  fn long_line_returns_unchanged() {
    let line = "x".repeat(2049);
    assert_eq!(syntax_highlight_source_line(&line, true), line);
  }

  #[test]
  fn content_preserved_after_highlighting() {
    let line = r#"const foo = "hello" + 42;"#;
    let result = syntax_highlight_source_line(line, true);
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn keywords_colored_magenta() {
    let line = "const x = 1;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "const"));
  }

  #[test]
  fn multiple_keywords() {
    let line = "if (x) { return y; }";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "if"));
    assert!(has_colored(&result, BRIGHT_BLUE, "return"));
  }

  #[test]
  fn ts_type_keywords_colored_cyan() {
    let line = "interface Foo extends Bar {}";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, CYAN, "interface"));
  }

  #[test]
  fn type_name_after_type_keyword() {
    let line = "type Foo = string;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, CYAN, "type"));
    // "Foo" should be cyan+bold
    let type_name = format!("{}{}{}{}", CYAN, BOLD, "Foo", RESET);
    assert!(result.contains(&type_name), "result: {result}");
  }

  #[test]
  fn boolean_literals_colored_yellow() {
    let line = "return true || false;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "true"));
    assert!(has_colored(&result, YELLOW, "false"));
  }

  #[test]
  fn null_colored_bold() {
    let line = "const x = null;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BOLD, "null"));
  }

  #[test]
  fn undefined_colored_gray() {
    let line = "const x = undefined;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, GRAY, "undefined"));
  }

  #[test]
  fn this_colored_yellow() {
    let line = "this.foo = 1;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "this"));
  }

  #[test]
  fn delete_colored_red() {
    let line = "delete obj.key;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, RED, "delete"));
  }

  #[test]
  fn string_double_quotes() {
    let line = r#"const x = "hello world";"#;
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, GREEN, "\"hello world\""));
  }

  #[test]
  fn string_single_quotes() {
    let line = "const x = 'hello';";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, GREEN, "'hello'"));
  }

  #[test]
  fn template_literal_simple() {
    let line = "const x = `hello`;";
    let result = syntax_highlight_source_line(line, true);
    // backtick string parts are green
    assert!(result.contains(GREEN), "result: {result}");
    assert!(result.contains("`hello`"), "result: {result}");
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn template_literal_with_interpolation() {
    let line = "const x = `hello ${name}!`;";
    let result = syntax_highlight_source_line(line, true);
    // The string parts should be green, "name" is a plain identifier
    assert_eq!(strip_ansi(&result), line);
    // "${" and "}" should not be green — they're outside the string color
    // "name" should be plain (no keyword)
    assert!(result.contains("name"), "result: {result}");
  }

  #[test]
  fn template_literal_interpolation_with_keyword() {
    let line = "`value is ${typeof x}`";
    let result = syntax_highlight_source_line(line, true);
    // typeof inside interpolation should be magenta
    assert!(
      has_colored(&result, BRIGHT_BLUE, "typeof"),
      "result: {result}"
    );
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn template_literal_interpolation_with_number() {
    let line = "`code ${42}`";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "42"), "result: {result}");
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn template_literal_interpolation_with_function_call() {
    let line = "`result: ${foo(x)}`";
    let result = syntax_highlight_source_line(line, true);
    // foo is a plain identifier, not colored
    assert!(!has_colored(&result, CYAN, "foo"), "result: {result}");
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn template_literal_nested_braces() {
    let line = "`${obj[key]}`";
    let result = syntax_highlight_source_line(line, true);
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn string_with_escape() {
    let line = r#"const x = "he\"llo";"#;
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, GREEN, r#""he\"llo""#));
  }

  #[test]
  fn number_decimal() {
    let line = "const x = 42;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "42"));
  }

  #[test]
  fn number_float() {
    let line = "const x = 3.14;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "3.14"));
  }

  #[test]
  fn number_hex() {
    let line = "const x = 0xFF;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "0xFF"));
  }

  #[test]
  fn number_binary() {
    let line = "const x = 0b1010;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "0b1010"));
  }

  #[test]
  fn number_octal() {
    let line = "const x = 0o77;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "0o77"));
  }

  #[test]
  fn number_bigint() {
    let line = "const x = 42n;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "42n"));
  }

  #[test]
  fn number_scientific() {
    let line = "const x = 1e10;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "1e10"));
  }

  #[test]
  fn line_comment() {
    let line = "const x = 1; // a comment";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, GRAY, "// a comment"));
  }

  #[test]
  fn block_comment() {
    let line = "const x = /* inline */ 1;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, GRAY, "/* inline */"));
  }

  #[test]
  fn unclosed_block_comment() {
    let line = "const x = /* unclosed";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, GRAY, "/* unclosed"));
  }

  #[test]
  fn new_constructor_not_styled() {
    let line = "new Map();";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "new"));
    // Constructor name after `new` is NOT styled (no bold, no color)
    // "Map" should appear without any ANSI codes around it
    assert!(!has_colored(&result, BOLD, "Map"), "result: {result}");
    assert!(!has_colored(&result, CYAN, "Map"), "result: {result}");
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn async_await() {
    let line = "async function foo() { await bar(); }";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "async"));
    assert!(has_colored(&result, BRIGHT_BLUE, "function"));
    assert!(has_colored(&result, BRIGHT_BLUE, "await"));
  }

  #[test]
  fn arrow_function() {
    let line = "const f = (x) => x + 1;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "const"));
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn typescript_interface() {
    let line = "interface Options { timeout: number; }";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, CYAN, "interface"));
    assert!(has_colored(&result, CYAN, "number"));
    let type_name = format!("{}{}{}{}", CYAN, BOLD, "Options", RESET);
    assert!(result.contains(&type_name), "result: {result}");
  }

  #[test]
  fn typescript_as_satisfies() {
    let line = "const x = foo as string;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, CYAN, "as"));
    assert!(has_colored(&result, CYAN, "string"));
  }

  #[test]
  fn typescript_enum() {
    let line = "enum Color { Red, Green, Blue }";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, CYAN, "enum"));
    let type_name = format!("{}{}{}{}", CYAN, BOLD, "Color", RESET);
    assert!(result.contains(&type_name), "result: {result}");
  }

  #[test]
  fn mixed_line() {
    let line = r#"if (typeof x === "string") { return true; }"#;
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "if"));
    assert!(has_colored(&result, BRIGHT_BLUE, "typeof"));
    assert!(has_colored(&result, GREEN, "\"string\""));
    assert!(has_colored(&result, BRIGHT_BLUE, "return"));
    assert!(has_colored(&result, YELLOW, "true"));
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn empty_string() {
    assert_eq!(syntax_highlight_source_line("", true), "");
  }

  #[test]
  fn plain_identifier() {
    let line = "foo";
    let result = syntax_highlight_source_line(line, true);
    // No coloring for plain identifiers (not followed by `(`)
    assert_eq!(result, "foo");
  }

  #[test]
  fn identifiers_not_colored() {
    let line = "console.log(x);";
    let result = syntax_highlight_source_line(line, true);
    // Plain identifiers (including function calls) are not colored
    assert!(!has_colored(&result, CYAN, "log"), "result: {result}");
    assert!(!has_colored(&result, CYAN, "console"), "result: {result}");
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn import_from_statement() {
    let line = r#"import { foo } from "bar";"#;
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "import"));
    assert!(has_colored(&result, BRIGHT_BLUE, "from"));
    assert!(has_colored(&result, GREEN, "\"bar\""));
  }

  #[test]
  fn class_extends() {
    let line = "class Foo extends Bar {}";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "class"));
    assert!(has_colored(&result, BRIGHT_BLUE, "extends"));
  }

  #[test]
  fn unclosed_string() {
    let line = "  throw new Error(\"something went wrong";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, BRIGHT_BLUE, "throw"));
    assert!(has_colored(&result, BRIGHT_BLUE, "new"));
    assert!(
      result.contains(&format!("{}\"something went wrong{}", GREEN, RESET)),
      "result: {result}"
    );
    assert_eq!(strip_ansi(&result), line);
  }

  #[test]
  fn private_public_readonly() {
    let line = "private readonly name: string;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, CYAN, "readonly"));
    assert!(has_colored(&result, CYAN, "string"));
  }

  #[test]
  fn numeric_separator() {
    let line = "const x = 1_000_000;";
    let result = syntax_highlight_source_line(line, true);
    assert!(has_colored(&result, YELLOW, "1_000_000"));
  }

  #[test]
  fn method_chain_not_colored() {
    let line = "items.map((x) => x + 1);";
    let result = syntax_highlight_source_line(line, true);
    assert!(!has_colored(&result, CYAN, "map"), "result: {result}");
    assert!(!has_colored(&result, CYAN, "items"), "result: {result}");
    assert_eq!(strip_ansi(&result), line);
  }
}
