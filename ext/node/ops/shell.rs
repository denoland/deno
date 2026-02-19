// Copyright 2018-2026 the Deno authors. MIT license.

//! Shell argument splitter for child_process command transformation.
//!
//! Splits a shell command string into arguments and a "shell suffix"
//! (everything after the first unquoted shell operator: redirects,
//! pipes, boolean operators, etc.). The suffix is preserved verbatim
//! so it can be passed through to the shell unchanged.

use deno_core::op2;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ParsedShellArgs {
  args: Vec<String>,
  shell_suffix: String,
}

/// Parses a shell command string into its arguments and shell operator suffix.
#[op2]
#[serde]
pub fn op_node_parse_shell_args(#[string] input: &str) -> ParsedShellArgs {
  scan_and_split(input)
}

/// Scan for the first unquoted shell operator (`<`, `>`, `|`, `&`, `;`)
/// and split the input into args and suffix. On non-Windows, respects
/// backslash escapes outside single quotes.
fn scan_and_split(input: &str) -> ParsedShellArgs {
  let bytes = input.as_bytes();
  let mut in_double = false;
  let mut in_single = false;
  let mut i = 0;

  while i < bytes.len() {
    let ch = bytes[i];

    // Backslash escapes next char (POSIX only, not on Windows)
    if !cfg!(windows) && ch == b'\\' && !in_single {
      i += 2;
      continue;
    }

    if ch == b'"' && !in_single {
      in_double = !in_double;
    } else if ch == b'\'' && !in_double {
      in_single = !in_single;
    } else if !in_double
      && !in_single
      && (ch == b'<' || ch == b'>' || ch == b'|' || ch == b';' || ch == b'&')
    {
      // Walk back for fd prefix on redirects (e.g. 2>, &>)
      let mut split_idx = i;
      if ch == b'<' || ch == b'>' {
        let mut j = i;
        while j > 0 && bytes[j - 1].is_ascii_digit() {
          j -= 1;
        }
        let mut fd_start = j;
        if j > 0 && bytes[j - 1] == b'&' {
          fd_start = j - 1;
        }
        if fd_start < i
          && (fd_start == 0
            || bytes[fd_start - 1] == b' '
            || bytes[fd_start - 1] == b'\t')
        {
          split_idx = fd_start;
        }
      }
      return ParsedShellArgs {
        args: split_args(input[..split_idx].trim_end()),
        shell_suffix: input[split_idx..].to_string(),
      };
    }
    i += 1;
  }

  ParsedShellArgs {
    args: split_args(input),
    shell_suffix: String::new(),
  }
}

/// Simple quote-aware arg splitting on whitespace.
fn split_args(input: &str) -> Vec<String> {
  let mut args = Vec::new();
  let mut current = String::new();
  let mut in_double = false;
  let mut in_single = false;

  for ch in input.chars() {
    if ch == '"' && !in_single {
      in_double = !in_double;
    } else if ch == '\'' && !in_double {
      in_single = !in_single;
    } else if ch == ' ' && !in_double && !in_single {
      if !current.is_empty() {
        args.push(std::mem::take(&mut current));
      }
    } else {
      current.push(ch);
    }
  }
  if !current.is_empty() {
    args.push(current);
  }
  args
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(input: &str) -> ParsedShellArgs {
    scan_and_split(input)
  }

  #[test]
  fn test_simple_args() {
    let r = parse("arg1 arg2 arg3");
    assert_eq!(r.args, vec!["arg1", "arg2", "arg3"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_quoted_double() {
    let r = parse(r#""quoted arg" simple"#);
    assert_eq!(r.args, vec!["quoted arg", "simple"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_quoted_single() {
    let r = parse("'single quoted' arg");
    assert_eq!(r.args, vec!["single quoted", "arg"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_output_redirect() {
    let r = parse("arg1 > file.txt");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "> file.txt");
  }

  #[test]
  fn test_input_redirect() {
    let r = parse("arg1 < input.txt");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "< input.txt");
  }

  #[test]
  fn test_append_redirect() {
    let r = parse("arg1 >> output.txt");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, ">> output.txt");
  }

  #[test]
  fn test_fd_redirect_stderr_to_stdout() {
    let r = parse("arg1 2>&1");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "2>&1");
  }

  #[test]
  fn test_pipe() {
    let r = parse("arg1 | cmd2");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "| cmd2");
  }

  #[test]
  fn test_boolean_and() {
    let r = parse("arg1 && cmd2");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "&& cmd2");
  }

  #[test]
  fn test_boolean_or() {
    let r = parse("arg1 || cmd2");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "|| cmd2");
  }

  #[test]
  fn test_semicolon() {
    let r = parse("arg1 ; cmd2");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "; cmd2");
  }

  #[test]
  fn test_background() {
    let r = parse("arg1 & cmd2");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "& cmd2");
  }

  #[test]
  fn test_fallback_on_reserved_word() {
    let r = parse("if true");
    assert_eq!(r.args, vec!["if", "true"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_single_flag() {
    let r = parse("--version");
    assert_eq!(r.args, vec!["--version"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_multiple_args_with_redirect() {
    let r = parse("--inspect script.js > output.txt");
    assert_eq!(r.args, vec!["--inspect", "script.js"]);
    assert_eq!(r.shell_suffix, "> output.txt");
  }

  #[test]
  fn test_single_quoted_literal_in_suffix() {
    let r = parse("arg1 | grep '$VAR'");
    assert_eq!(r.args, vec!["arg1"]);
    assert!(
      r.shell_suffix.contains("'$VAR'"),
      "suffix: {}",
      r.shell_suffix
    );
  }

  #[test]
  fn test_double_quoted_var_in_suffix() {
    let r = parse(r#"arg1 | grep "$VAR""#);
    assert_eq!(r.args, vec!["arg1"]);
    assert!(
      r.shell_suffix.contains("\"$VAR\""),
      "suffix: {}",
      r.shell_suffix
    );
  }

  #[test]
  fn test_escaped_posix_shell_vars() {
    let r = parse(r#""${ESCAPED_1}" < "${ESCAPED_2}""#);
    assert_eq!(r.args, vec!["${ESCAPED_1}"]);
    assert_eq!(r.shell_suffix, r#"< "${ESCAPED_2}""#);
  }

  #[test]
  fn test_windows_paths() {
    let r =
      scan_and_split(r#""D:\path\to\script.js" < "D:\path\to\input.txt""#);
    assert_eq!(r.args, vec![r"D:\path\to\script.js"]);
    assert_eq!(r.shell_suffix, r#"< "D:\path\to\input.txt""#);
  }

  #[test]
  fn test_fd_redirect_scan() {
    let r = scan_and_split("arg1 2>&1");
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "2>&1");
  }

  #[test]
  fn test_pipe_scan() {
    let r = scan_and_split("arg1 arg2 | cmd2");
    assert_eq!(r.args, vec!["arg1", "arg2"]);
    assert_eq!(r.shell_suffix, "| cmd2");
  }

  #[test]
  fn test_fd_close_redirect() {
    let r = parse(r#""${ESCAPED_1}" child 1>&- 2>&-"#);
    assert_eq!(r.args, vec!["${ESCAPED_1}", "child"]);
    assert_eq!(r.shell_suffix, "1>&- 2>&-");
  }
}
