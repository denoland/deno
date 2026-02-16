// Copyright 2018-2026 the Deno authors. MIT license.

//! Shell argument parser using deno_task_shell's POSIX shell parser.
//!
//! Replaces the hand-rolled TypeScript splitShellSuffix/splitShellArgs
//! with a proper AST-based parser that extracts command arguments and
//! reconstructs shell operators (redirects, pipes, etc.) as a suffix.

use deno_core::op2;
use deno_task_shell::parser::BooleanList;
use deno_task_shell::parser::Command;
use deno_task_shell::parser::CommandInner;
use deno_task_shell::parser::IoFile;
use deno_task_shell::parser::PipeSequenceOperator;
use deno_task_shell::parser::PipelineInner;
use deno_task_shell::parser::Redirect;
use deno_task_shell::parser::RedirectFd;
use deno_task_shell::parser::RedirectOp;
use deno_task_shell::parser::RedirectOpInput;
use deno_task_shell::parser::RedirectOpOutput;
use deno_task_shell::parser::Sequence;
use deno_task_shell::parser::SequentialList;
use deno_task_shell::parser::Word;
use deno_task_shell::parser::WordPart;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ParsedShellArgs {
  args: Vec<String>,
  shell_suffix: String,
}

/// Parses a shell command string into its arguments and shell operator suffix.
///
/// Uses deno_task_shell's parser to properly handle quoting, redirections,
/// pipes, and boolean operators. Falls back to simple quote-aware splitting
/// if the parser cannot handle the input.
#[op2]
#[serde]
pub fn op_node_parse_shell_args(#[string] input: String) -> ParsedShellArgs {
  parse_shell_args(&input)
}

fn parse_shell_args(input: &str) -> ParsedShellArgs {
  match deno_task_shell::parser::parse(input) {
    Ok(list) => extract_args_and_suffix(&list),
    Err(_) => fallback_split(input),
  }
}

/// Fallback: simple quote-aware arg splitting with empty suffix.
fn fallback_split(input: &str) -> ParsedShellArgs {
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

  ParsedShellArgs {
    args,
    shell_suffix: String::new(),
  }
}

// --- Argument extraction ---

/// Extract args from the first SimpleCommand in the AST and build
/// the shell suffix from everything else (redirects, pipes, etc.).
fn extract_args_and_suffix(list: &SequentialList) -> ParsedShellArgs {
  if list.items.is_empty() {
    return ParsedShellArgs {
      args: vec![],
      shell_suffix: String::new(),
    };
  }

  let first_item = &list.items[0];
  let mut suffix = String::new();
  let args = extract_from_sequence(&first_item.sequence, &mut suffix);

  if first_item.is_async {
    append_spaced(&mut suffix, "&");
  }

  for (i, item) in list.items.iter().enumerate().skip(1) {
    if !list.items[i - 1].is_async {
      append_spaced(&mut suffix, ";");
    }
    append_spaced(&mut suffix, &serialize_sequence(&item.sequence));
    if item.is_async {
      append_spaced(&mut suffix, "&");
    }
  }

  ParsedShellArgs {
    args,
    shell_suffix: suffix,
  }
}

fn extract_from_sequence(seq: &Sequence, suffix: &mut String) -> Vec<String> {
  match seq {
    Sequence::Pipeline(pipeline) => {
      extract_from_pipeline_inner(&pipeline.inner, suffix)
    }
    Sequence::BooleanList(bl) => extract_from_boolean_list(bl, suffix),
    Sequence::ShellVar(_) => vec![],
  }
}

fn extract_from_boolean_list(
  bl: &BooleanList,
  suffix: &mut String,
) -> Vec<String> {
  let args = extract_from_sequence(&bl.current, suffix);
  append_spaced(suffix, bl.op.as_str());
  append_spaced(suffix, &serialize_sequence(&bl.next));
  args
}

fn extract_from_pipeline_inner(
  inner: &PipelineInner,
  suffix: &mut String,
) -> Vec<String> {
  match inner {
    PipelineInner::Command(cmd) => extract_from_command(cmd, suffix),
    PipelineInner::PipeSequence(seq) => {
      let args = extract_from_command(&seq.current, suffix);
      let op_str = match seq.op {
        PipeSequenceOperator::Stdout => "|",
        PipeSequenceOperator::StdoutStderr => "|&",
      };
      append_spaced(suffix, op_str);
      append_spaced(suffix, &serialize_pipeline_inner(&seq.next));
      args
    }
  }
}

fn extract_from_command(cmd: &Command, suffix: &mut String) -> Vec<String> {
  let args = match &cmd.inner {
    CommandInner::Simple(simple) => {
      simple.args.iter().map(word_to_value).collect()
    }
    CommandInner::Subshell(_) => vec![],
  };
  if let Some(ref redirect) = cmd.redirect {
    append_spaced(suffix, &serialize_redirect(redirect));
  }
  args
}

// --- Word value extraction (strips quotes) ---

fn word_to_value(word: &Word) -> String {
  let mut result = String::new();
  for part in word.parts() {
    word_part_to_value(part, &mut result);
  }
  result
}

fn word_part_to_value(part: &WordPart, out: &mut String) {
  match part {
    WordPart::Text(s) => out.push_str(s),
    WordPart::Quoted(parts) => {
      for p in parts {
        word_part_to_value(p, out);
      }
    }
    WordPart::Variable(name) => {
      out.push('$');
      out.push_str(name);
    }
    WordPart::Tilde => out.push('~'),
    WordPart::Command(list) => {
      out.push_str("$(");
      out.push_str(&serialize_sequential_list(list));
      out.push(')');
    }
  }
}

// --- AST serializers (for suffix reconstruction) ---

fn serialize_word(word: &Word) -> String {
  let mut result = String::new();
  for part in word.parts() {
    serialize_word_part(part, &mut result);
  }
  result
}

fn serialize_word_part(part: &WordPart, out: &mut String) {
  match part {
    WordPart::Text(s) => out.push_str(s),
    WordPart::Quoted(parts) => {
      out.push('"');
      for p in parts {
        match p {
          WordPart::Text(s) => out.push_str(s),
          WordPart::Variable(name) => {
            out.push('$');
            out.push_str(name);
          }
          WordPart::Tilde => out.push('~'),
          WordPart::Command(list) => {
            out.push_str("$(");
            out.push_str(&serialize_sequential_list(list));
            out.push(')');
          }
          WordPart::Quoted(inner) => {
            for ip in inner {
              serialize_word_part(ip, out);
            }
          }
        }
      }
      out.push('"');
    }
    WordPart::Variable(name) => {
      out.push('$');
      out.push_str(name);
    }
    WordPart::Tilde => out.push('~'),
    WordPart::Command(list) => {
      out.push_str("$(");
      out.push_str(&serialize_sequential_list(list));
      out.push(')');
    }
  }
}

fn serialize_redirect(redirect: &Redirect) -> String {
  let mut result = String::new();
  if let Some(ref fd) = redirect.maybe_fd {
    match fd {
      RedirectFd::Fd(n) => result.push_str(&n.to_string()),
      RedirectFd::StdoutStderr => result.push('&'),
    }
  }
  match &redirect.op {
    RedirectOp::Input(RedirectOpInput::Redirect) => result.push('<'),
    RedirectOp::Output(RedirectOpOutput::Overwrite) => result.push('>'),
    RedirectOp::Output(RedirectOpOutput::Append) => result.push_str(">>"),
  }
  match &redirect.io_file {
    IoFile::Word(word) => {
      result.push(' ');
      result.push_str(&serialize_word(word));
    }
    IoFile::Fd(fd) => {
      result.push('&');
      result.push_str(&fd.to_string());
    }
  }
  result
}

fn serialize_command(command: &Command) -> String {
  let mut result = String::new();
  match &command.inner {
    CommandInner::Simple(simple) => {
      for env_var in &simple.env_vars {
        result.push_str(&env_var.name);
        result.push('=');
        result.push_str(&serialize_word(&env_var.value));
        result.push(' ');
      }
      let args: Vec<String> = simple.args.iter().map(serialize_word).collect();
      result.push_str(&args.join(" "));
    }
    CommandInner::Subshell(list) => {
      result.push('(');
      result.push_str(&serialize_sequential_list(list));
      result.push(')');
    }
  }
  if let Some(ref redirect) = command.redirect {
    result.push(' ');
    result.push_str(&serialize_redirect(redirect));
  }
  result
}

fn serialize_pipeline_inner(inner: &PipelineInner) -> String {
  match inner {
    PipelineInner::Command(cmd) => serialize_command(cmd),
    PipelineInner::PipeSequence(seq) => {
      let mut result = serialize_command(&seq.current);
      let op = match seq.op {
        PipeSequenceOperator::Stdout => " | ",
        PipeSequenceOperator::StdoutStderr => " |& ",
      };
      result.push_str(op);
      result.push_str(&serialize_pipeline_inner(&seq.next));
      result
    }
  }
}

fn serialize_sequence(sequence: &Sequence) -> String {
  match sequence {
    Sequence::ShellVar(env_var) => {
      format!("{}={}", env_var.name, serialize_word(&env_var.value))
    }
    Sequence::Pipeline(pipeline) => {
      let mut result = String::new();
      if pipeline.negated {
        result.push_str("! ");
      }
      result.push_str(&serialize_pipeline_inner(&pipeline.inner));
      result
    }
    Sequence::BooleanList(bl) => {
      let mut result = serialize_sequence(&bl.current);
      result.push(' ');
      result.push_str(bl.op.as_str());
      result.push(' ');
      result.push_str(&serialize_sequence(&bl.next));
      result
    }
  }
}

fn serialize_sequential_list(list: &SequentialList) -> String {
  let mut parts = Vec::new();
  for item in &list.items {
    let mut s = serialize_sequence(&item.sequence);
    if item.is_async {
      s.push_str(" &");
    }
    parts.push(s);
  }
  parts.join("; ")
}

fn append_spaced(s: &mut String, part: &str) {
  if !s.is_empty() {
    s.push(' ');
  }
  s.push_str(part);
}

#[cfg(test)]
mod tests {
  use super::*;

  fn parse(input: &str) -> ParsedShellArgs {
    parse_shell_args(input)
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
}
