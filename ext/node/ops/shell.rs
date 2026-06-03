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
/// When `is_cmd_exe` is true, backslashes are not treated as escape characters
/// (cmd.exe semantics). When false, POSIX backslash escaping is used.
#[op2]
#[serde]
pub fn op_node_parse_shell_args(
  #[string] input: &str,
  is_cmd_exe: bool,
) -> ParsedShellArgs {
  scan_and_split(input, is_cmd_exe)
}

/// Scan for the first unquoted shell operator (`<`, `>`, `|`, `&`, `;`)
/// and split the input into args and suffix. When `is_cmd_exe` is false,
/// backslash escapes the next character outside single quotes (POSIX).
fn scan_and_split(input: &str, is_cmd_exe: bool) -> ParsedShellArgs {
  let bytes = input.as_bytes();
  let mut in_double = false;
  let mut in_single = false;
  let mut i = 0;

  while i < bytes.len() {
    let ch = bytes[i];

    // POSIX: backslash escapes next char outside single quotes
    if !is_cmd_exe && ch == b'\\' && !in_single {
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
    scan_and_split(input, false)
  }

  fn parse_cmd(input: &str) -> ParsedShellArgs {
    scan_and_split(input, true)
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
    let r = scan_and_split(
      r#""D:\path\to\script.js" < "D:\path\to\input.txt""#,
      false,
    );
    assert_eq!(r.args, vec![r"D:\path\to\script.js"]);
    assert_eq!(r.shell_suffix, r#"< "D:\path\to\input.txt""#);
  }

  #[test]
  fn test_fd_redirect_scan() {
    let r = scan_and_split("arg1 2>&1", false);
    assert_eq!(r.args, vec!["arg1"]);
    assert_eq!(r.shell_suffix, "2>&1");
  }

  #[test]
  fn test_pipe_scan() {
    let r = scan_and_split("arg1 arg2 | cmd2", false);
    assert_eq!(r.args, vec!["arg1", "arg2"]);
    assert_eq!(r.shell_suffix, "| cmd2");
  }

  #[test]
  fn test_fd_close_redirect() {
    let r = parse(r#""${ESCAPED_1}" child 1>&- 2>&-"#);
    assert_eq!(r.args, vec!["${ESCAPED_1}", "child"]);
    assert_eq!(r.shell_suffix, "1>&- 2>&-");
  }

  // --- POSIX backslash escape tests (is_cmd_exe = false) ---

  #[test]
  fn test_posix_escaped_redirect() {
    // \> is a literal >, not a redirect in POSIX shells
    let r = parse(r"arg1 \> file.txt");
    assert_eq!(r.args, vec![r"arg1", r"\>", "file.txt"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_posix_escaped_pipe() {
    // \| is a literal |, not a pipe in POSIX shells
    let r = parse(r"arg1 \| arg2");
    assert_eq!(r.args, vec!["arg1", r"\|", "arg2"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_posix_escaped_semicolon() {
    let r = parse(r"arg1 \; arg2");
    assert_eq!(r.args, vec!["arg1", r"\;", "arg2"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_posix_escaped_ampersand() {
    let r = parse(r"arg1 \& arg2");
    assert_eq!(r.args, vec!["arg1", r"\&", "arg2"]);
    assert_eq!(r.shell_suffix, "");
  }

  #[test]
  fn test_posix_backslash_in_single_quotes_not_escape() {
    // Inside single quotes, backslash is literal (POSIX rule)
    let r = parse(r"arg1 '\>' > file.txt");
    assert_eq!(r.args, vec!["arg1", r"\>"]);
    assert_eq!(r.shell_suffix, "> file.txt");
  }

  // --- cmd.exe tests (is_cmd_exe = true) ---

  #[test]
  fn test_cmd_backslash_not_escape() {
    // cmd.exe: \ is not an escape char, so > is still a redirect
    let r = parse_cmd(r"arg1 \> file.txt");
    assert_eq!(r.args, vec![r"arg1", r"\"]);
    assert_eq!(r.shell_suffix, r"> file.txt");
  }

  #[test]
  fn test_cmd_backslash_pipe_not_escape() {
    let r = parse_cmd(r"arg1 \| cmd2");
    assert_eq!(r.args, vec!["arg1", r"\"]);
    assert_eq!(r.shell_suffix, r"| cmd2");
  }

  #[test]
  fn test_cmd_windows_paths_with_redirect() {
    // cmd.exe: backslashes in paths are path separators, > is a redirect
    let r = parse_cmd(r"run C:\scripts\test.js > output.txt");
    assert_eq!(r.args, vec!["run", r"C:\scripts\test.js"]);
    assert_eq!(r.shell_suffix, "> output.txt");
  }

  #[test]
  fn test_cmd_quoted_windows_paths() {
    let r = parse_cmd(r#""C:\my scripts\test.js" > output.txt"#);
    assert_eq!(r.args, vec![r"C:\my scripts\test.js"]);
    assert_eq!(r.shell_suffix, "> output.txt");
  }

  #[test]
  fn test_posix_and_cmd_agree_on_quoted_operators() {
    // Both modes: operators inside quotes are not split points
    let posix = parse(r#""hello > world" > file.txt"#);
    let cmd = parse_cmd(r#""hello > world" > file.txt"#);
    assert_eq!(posix.args, vec!["hello > world"]);
    assert_eq!(posix.shell_suffix, "> file.txt");
    assert_eq!(cmd.args, vec!["hello > world"]);
    assert_eq!(cmd.shell_suffix, "> file.txt");
  }
}
