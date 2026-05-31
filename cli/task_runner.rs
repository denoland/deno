// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::futures::future::LocalBoxFuture;
use deno_task_shell::ExecutableCommand;
use deno_task_shell::ExecuteResult;
use deno_task_shell::KillSignal;
use deno_task_shell::ShellCommand;
use deno_task_shell::ShellCommandContext;
use deno_task_shell::ShellPipeReader;
use deno_task_shell::ShellPipeWriter;
use tokio::task::JoinHandle;
use tokio::task::LocalSet;
use tokio_util::sync::CancellationToken;

use crate::node::CliNodeResolver;
use crate::npm::CliManagedNpmResolver;
use crate::npm::CliNpmResolver;
use crate::util::fs::canonicalize_path;

pub fn get_script_with_args(script: &str, argv: &[String]) -> String {
  let additional_args = argv
    .iter()
    // Wrap each argument in single quotes so the shell preserves the
    // content literally (including backslashes, $, `, etc.). For an
    // argument containing a single quote, splice in a double-quoted `'`
    // using the POSIX idiom `'foo'"'"'bar'`.
    .map(|a| format!("'{}'", a.replace('\'', "'\"'\"'")))
    .collect::<Vec<_>>()
    .join(" ");

  let script = format!("{script} {additional_args}");
  script.trim().to_owned()
}

/// Maximum number of items a single brace sequence (`{a..b}`) may expand to.
const MAX_SEQUENCE_LEN: usize = 10_000;

/// Performs bash-style brace expansion on a task script prior to parsing.
///
/// `deno_task_shell`'s parser does not support brace expansion (e.g. `{a,b}`
/// or `{1..5}`), so we expand it here, mirroring bash where brace expansion is
/// an early, purely textual phase that happens before word splitting and other
/// expansions.
///
/// This is intentionally conservative: only unquoted brace groups that contain
/// a top-level comma or a valid numeric/character sequence are expanded.
/// `${VAR}`, `$(cmd)`, backtick command substitutions, quoted text, and escaped
/// braces are left untouched. Because the parser previously rejected any
/// unquoted `{`, no script that currently parses can contain an unquoted brace
/// group, so this expansion cannot change the behavior of existing tasks.
pub fn expand_braces(script: &str) -> Cow<'_, str> {
  if !script.contains('{') {
    return Cow::Borrowed(script);
  }
  let chars: Vec<char> = script.chars().collect();
  let len = chars.len();
  let mut out = String::with_capacity(script.len());
  let mut i = 0;
  while i < len {
    let c = chars[i];
    if is_word_separator(c) {
      out.push(c);
      i += 1;
      continue;
    }
    // Read a full word (respecting quotes and substitutions), then expand it.
    let start = i;
    while i < len {
      let c = chars[i];
      if is_word_separator(c) {
        break;
      }
      if c == '\\' {
        i = (i + 2).min(len);
        continue;
      }
      if matches!(c, '\'' | '"' | '`' | '$')
        && let Some(j) = skip_quoted_or_subst(&chars, i)
      {
        i = j;
        continue;
      }
      i += 1;
    }
    let word: String = chars[start..i.min(len)].iter().collect();
    out.push_str(&brace_expand(&word).join(" "));
  }
  Cow::Owned(out)
}

/// Characters that terminate a shell "word" when they appear unquoted.
fn is_word_separator(c: char) -> bool {
  matches!(
    c,
    ' ' | '\t' | '\r' | '\n' | '|' | '&' | ';' | '<' | '>' | '(' | ')'
  )
}

/// If `chars[i]` begins a quoted string or a `$`/backtick substitution, returns
/// the index just past it. Otherwise returns `None`. These regions are treated
/// as opaque so braces, commas, and separators inside them are not interpreted.
fn skip_quoted_or_subst(chars: &[char], i: usize) -> Option<usize> {
  let len = chars.len();
  match chars[i] {
    '\'' => {
      let mut j = i + 1;
      while j < len && chars[j] != '\'' {
        j += 1;
      }
      Some((j + 1).min(len))
    }
    '"' => {
      let mut j = i + 1;
      while j < len {
        match chars[j] {
          '\\' => j = (j + 2).min(len),
          '"' => return Some(j + 1),
          _ => j += 1,
        }
      }
      Some(len)
    }
    '`' => {
      let mut j = i + 1;
      while j < len {
        match chars[j] {
          '\\' => j = (j + 2).min(len),
          '`' => return Some(j + 1),
          _ => j += 1,
        }
      }
      Some(len)
    }
    '$' if i + 1 < len && chars[i + 1] == '{' => {
      Some(skip_balanced(chars, i + 1, '{', '}').unwrap_or((i + 2).min(len)))
    }
    '$' if i + 1 < len && chars[i + 1] == '(' => {
      Some(skip_balanced(chars, i + 1, '(', ')').unwrap_or((i + 2).min(len)))
    }
    _ => None,
  }
}

/// Skips a balanced `open`/`close` region starting at `chars[start]` (which must
/// equal `open`). Nested quotes and substitutions are skipped so their contents
/// do not affect the nesting depth. Returns the index just past the matching
/// `close`, or `None` if unbalanced.
fn skip_balanced(
  chars: &[char],
  start: usize,
  open: char,
  close: char,
) -> Option<usize> {
  let len = chars.len();
  let mut i = start;
  let mut depth = 0usize;
  while i < len {
    let c = chars[i];
    if c == '\\' {
      i = (i + 2).min(len);
      continue;
    }
    if matches!(c, '\'' | '"' | '`' | '$')
      && let Some(j) = skip_quoted_or_subst(chars, i)
    {
      i = j;
      continue;
    }
    if c == open {
      depth += 1;
    } else if c == close {
      depth -= 1;
      if depth == 0 {
        return Some(i + 1);
      }
    }
    i += 1;
  }
  None
}

enum BraceGroup {
  List {
    open: usize,
    close: usize,
  },
  Sequence {
    open: usize,
    close: usize,
    items: Vec<String>,
  },
}

/// Recursively expands the brace groups in a single shell word, returning one
/// string per expansion (or the original word when there is nothing to expand).
fn brace_expand(word: &str) -> Vec<String> {
  let chars: Vec<char> = word.chars().collect();
  let group = match find_brace_group(&chars) {
    Some(group) => group,
    None => return vec![word.to_string()],
  };
  let (open, close, options, recurse_options) = match group {
    BraceGroup::List { open, close } => {
      (open, close, split_top_commas(&chars[open + 1..close]), true)
    }
    BraceGroup::Sequence { open, close, items } => (open, close, items, false),
  };
  let pre: String = chars[..open].iter().collect();
  let post: String = chars[close + 1..].iter().collect();
  // The postscript may itself contain further brace groups (cartesian product).
  let post_expanded = brace_expand(&post);
  let mut result = Vec::new();
  for opt in &options {
    let expanded_opts = if recurse_options {
      brace_expand(opt)
    } else {
      vec![opt.clone()]
    };
    for eo in &expanded_opts {
      for ep in &post_expanded {
        result.push(format!("{pre}{eo}{ep}"));
      }
    }
  }
  result
}

/// Finds the first top-level brace group eligible for expansion in `chars`.
fn find_brace_group(chars: &[char]) -> Option<BraceGroup> {
  let len = chars.len();
  let mut i = 0;
  while i < len {
    let c = chars[i];
    if c == '\\' {
      i = (i + 2).min(len);
      continue;
    }
    if matches!(c, '\'' | '"' | '`' | '$')
      && let Some(j) = skip_quoted_or_subst(chars, i)
    {
      i = j;
      continue;
    }
    if c == '{'
      && let Some((close, has_comma)) = match_closing_brace(chars, i)
    {
      let body = &chars[i + 1..close];
      if has_comma {
        return Some(BraceGroup::List { open: i, close });
      }
      if let Some(items) = parse_sequence(body) {
        return Some(BraceGroup::Sequence {
          open: i,
          close,
          items,
        });
      }
      // Not an expandable group; keep scanning so nested groups such as
      // `{a{b,c}}` are still found.
    }
    i += 1;
  }
  None
}

/// Finds the `}` matching the `{` at `open`, returning its index and whether the
/// group body contains a top-level comma. Returns `None` if unbalanced.
fn match_closing_brace(chars: &[char], open: usize) -> Option<(usize, bool)> {
  let len = chars.len();
  let mut i = open + 1;
  let mut depth = 0usize;
  let mut has_comma = false;
  while i < len {
    let c = chars[i];
    if c == '\\' {
      i = (i + 2).min(len);
      continue;
    }
    if matches!(c, '\'' | '"' | '`' | '$')
      && let Some(j) = skip_quoted_or_subst(chars, i)
    {
      i = j;
      continue;
    }
    match c {
      '{' => depth += 1,
      '}' => {
        if depth == 0 {
          return Some((i, has_comma));
        }
        depth -= 1;
      }
      ',' if depth == 0 => has_comma = true,
      _ => {}
    }
    i += 1;
  }
  None
}

/// Splits a brace body on commas that appear at the top level (not nested in
/// inner braces, quotes, or substitutions).
fn split_top_commas(body: &[char]) -> Vec<String> {
  let len = body.len();
  let mut parts = Vec::new();
  let mut start = 0;
  let mut i = 0;
  let mut depth = 0usize;
  while i < len {
    let c = body[i];
    if c == '\\' {
      i = (i + 2).min(len);
      continue;
    }
    if matches!(c, '\'' | '"' | '`' | '$')
      && let Some(j) = skip_quoted_or_subst(body, i)
    {
      i = j;
      continue;
    }
    match c {
      '{' => depth += 1,
      '}' => depth = depth.saturating_sub(1),
      ',' if depth == 0 => {
        parts.push(body[start..i].iter().collect());
        start = i + 1;
      }
      _ => {}
    }
    i += 1;
  }
  parts.push(body[start..].iter().collect());
  parts
}

/// Parses a sequence body such as `1..5`, `1..10..2`, or `a..e`, returning the
/// expanded items, or `None` if it is not a valid sequence.
fn parse_sequence(body: &[char]) -> Option<Vec<String>> {
  let s: String = body.iter().collect();
  let segments: Vec<&str> = s.split("..").collect();
  if segments.len() != 2 && segments.len() != 3 {
    return None;
  }
  let step_str = segments.get(2).copied();
  // Integer sequence (e.g. `1..5`, `01..03`, `1..10..2`).
  if let (Ok(start), Ok(end)) =
    (segments[0].parse::<i64>(), segments[1].parse::<i64>())
  {
    let step = match step_str {
      Some(s) => s.parse::<i64>().ok()?,
      None => 1,
    };
    return integer_sequence(start, end, step, segments[0], segments[1]);
  }
  // Character sequence (e.g. `a..e`). Both bounds must be single ASCII letters.
  let start_chars: Vec<char> = segments[0].chars().collect();
  let end_chars: Vec<char> = segments[1].chars().collect();
  if start_chars.len() == 1
    && end_chars.len() == 1
    && start_chars[0].is_ascii_alphabetic()
    && end_chars[0].is_ascii_alphabetic()
  {
    let step = match step_str {
      Some(s) => s.parse::<i64>().ok()?,
      None => 1,
    };
    return char_sequence(start_chars[0], end_chars[0], step);
  }
  None
}

fn integer_sequence(
  start: i64,
  end: i64,
  step: i64,
  start_str: &str,
  end_str: &str,
) -> Option<Vec<String>> {
  let step = if step == 0 { 1 } else { step.abs() };
  // Bash zero-pads the output when either bound has a leading zero.
  let width = sequence_pad_width(start_str).max(sequence_pad_width(end_str));
  let mut items = Vec::new();
  let mut cur = start;
  while (start <= end && cur <= end) || (start > end && cur >= end) {
    items.push(format_padded(cur, width));
    if items.len() > MAX_SEQUENCE_LEN {
      return None;
    }
    if start <= end {
      cur += step;
    } else {
      cur -= step;
    }
  }
  Some(items)
}

fn char_sequence(start: char, end: char, step: i64) -> Option<Vec<String>> {
  let step = if step == 0 {
    1
  } else {
    step.unsigned_abs() as u32
  };
  let (s, e) = (start as u32, end as u32);
  let mut items = Vec::new();
  let mut cur = s;
  loop {
    items.push(char::from_u32(cur)?.to_string());
    if items.len() > MAX_SEQUENCE_LEN {
      return None;
    }
    if s <= e {
      if cur + step > e {
        break;
      }
      cur += step;
    } else {
      if cur < step || cur - step < e {
        break;
      }
      cur -= step;
    }
  }
  Some(items)
}

/// Width to zero-pad sequence output to, or `0` when no padding is needed.
fn sequence_pad_width(s: &str) -> usize {
  let digits = s.strip_prefix(|c| c == '-' || c == '+').unwrap_or(s);
  if digits.len() > 1 && digits.starts_with('0') {
    digits.len()
  } else {
    0
  }
}

fn format_padded(n: i64, width: usize) -> String {
  if width == 0 {
    return n.to_string();
  }
  let digits = n.unsigned_abs().to_string();
  let padded = if digits.len() < width {
    format!("{}{}", "0".repeat(width - digits.len()), digits)
  } else {
    digits
  };
  if n < 0 { format!("-{padded}") } else { padded }
}

pub struct TaskStdio(Option<ShellPipeReader>, ShellPipeWriter);

pub struct PrefixedWriter<W: std::io::Write> {
  prefix: Vec<u8>,
  inner: W,
  line_buf: Vec<u8>,
  at_line_start: bool,
}

impl<W: std::io::Write> PrefixedWriter<W> {
  pub fn new(prefix: String, inner: W) -> Self {
    Self {
      prefix: prefix.into_bytes(),
      inner,
      line_buf: Vec::new(),
      at_line_start: true,
    }
  }
}

impl<W: std::io::Write> std::io::Write for PrefixedWriter<W> {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    let mut rest = buf;
    while !rest.is_empty() {
      if self.at_line_start {
        self.line_buf.extend_from_slice(&self.prefix);
        self.at_line_start = false;
      }
      match rest.iter().position(|&b| b == b'\n') {
        Some(pos) => {
          self.line_buf.extend_from_slice(&rest[..pos + 1]);
          self.inner.write_all(&self.line_buf)?;
          self.line_buf.clear();
          self.at_line_start = true;
          rest = &rest[pos + 1..];
        }
        None => {
          self.line_buf.extend_from_slice(rest);
          break;
        }
      }
    }
    Ok(buf.len())
  }

  fn flush(&mut self) -> std::io::Result<()> {
    if !self.line_buf.is_empty() {
      self.inner.write_all(&self.line_buf)?;
      self.line_buf.clear();
      self.at_line_start = true;
    }
    self.inner.flush()
  }
}

pub fn make_prefixed_task_io(prefix: String) -> (TaskIo, Vec<JoinHandle<()>>) {
  let (out_r, out_w) = deno_task_shell::pipe();
  let (err_r, err_w) = deno_task_shell::pipe();

  let out_prefix = prefix.clone();
  let out_handle = tokio::task::spawn_blocking(move || {
    let mut writer = PrefixedWriter::new(out_prefix, std::io::stdout());
    let _ = out_r.pipe_to(&mut writer);
    let _ = writer.flush();
  });

  let err_handle = tokio::task::spawn_blocking(move || {
    let mut writer = PrefixedWriter::new(prefix, std::io::stderr());
    let _ = err_r.pipe_to(&mut writer);
    let _ = writer.flush();
  });

  (
    TaskIo {
      stdout: TaskStdio(None, out_w),
      stderr: TaskStdio(None, err_w),
    },
    vec![out_handle, err_handle],
  )
}

impl TaskStdio {
  pub fn stdout() -> Self {
    Self(None, ShellPipeWriter::stdout())
  }

  pub fn stderr() -> Self {
    Self(None, ShellPipeWriter::stderr())
  }

  pub fn piped() -> Self {
    let (r, w) = deno_task_shell::pipe();
    Self(Some(r), w)
  }
}

pub struct TaskIo {
  pub stdout: TaskStdio,
  pub stderr: TaskStdio,
}

impl Default for TaskIo {
  fn default() -> Self {
    Self {
      stdout: TaskStdio::stdout(),
      stderr: TaskStdio::stderr(),
    }
  }
}

pub struct RunTaskOptions<'a> {
  pub task_name: &'a str,
  pub script: &'a str,
  pub cwd: PathBuf,
  pub init_cwd: &'a Path,
  pub env_vars: HashMap<OsString, OsString>,
  pub argv: &'a [String],
  pub custom_commands: HashMap<String, Rc<dyn ShellCommand>>,
  /// Directories to prepend to `PATH` for the duration of the task,
  /// ordered closest-first (closest entry ends up first in `PATH`).
  pub node_modules_bin_dirs: &'a [PathBuf],
  pub stdio: Option<TaskIo>,
  pub kill_signal: KillSignal,
}

pub type TaskCustomCommands = HashMap<String, Rc<dyn ShellCommand>>;

pub struct TaskResult {
  pub exit_code: i32,
  pub stdout: Option<Vec<u8>>,
  pub stderr: Option<Vec<u8>>,
}

pub async fn run_task(
  mut opts: RunTaskOptions<'_>,
) -> Result<TaskResult, AnyError> {
  let script = get_script_with_args(opts.script, opts.argv);
  let script = expand_braces(&script);
  let seq_list = deno_task_shell::parser::parse(script.as_ref())
    .with_context(|| format!("Error parsing script '{}'.", opts.task_name))?;
  let env_vars =
    prepare_env_vars(opts.env_vars, opts.init_cwd, opts.node_modules_bin_dirs);
  if !opts.custom_commands.contains_key("deno") {
    opts
      .custom_commands
      .insert("deno".to_string(), Rc::new(DenoCommand::default()));
  }
  let state = deno_task_shell::ShellState::new(
    env_vars,
    opts.cwd,
    opts.custom_commands,
    opts.kill_signal,
  );
  let stdio = opts.stdio.unwrap_or_default();
  let (
    TaskStdio(stdout_read, stdout_write),
    TaskStdio(stderr_read, stderr_write),
  ) = (stdio.stdout, stdio.stderr);

  fn read(reader: ShellPipeReader) -> JoinHandle<Result<Vec<u8>, AnyError>> {
    tokio::task::spawn_blocking(move || {
      let mut buf = Vec::new();
      reader.pipe_to(&mut buf)?;
      Ok(buf)
    })
  }

  let stdout = stdout_read.map(read);
  let stderr = stderr_read.map(read);

  let local = LocalSet::new();
  let future = async move {
    let exit_code = deno_task_shell::execute_with_pipes(
      seq_list,
      state,
      ShellPipeReader::stdin(),
      stdout_write,
      stderr_write,
    )
    .await;
    Ok::<_, AnyError>(TaskResult {
      exit_code,
      stdout: if let Some(stdout) = stdout {
        Some(stdout.await??)
      } else {
        None
      },
      stderr: if let Some(stderr) = stderr {
        Some(stderr.await??)
      } else {
        None
      },
    })
  };
  local.run_until(future).await
}

fn prepare_env_vars(
  mut env_vars: HashMap<OsString, OsString>,
  initial_cwd: &Path,
  node_modules_bin_dirs: &[PathBuf],
) -> HashMap<OsString, OsString> {
  const INIT_CWD_NAME: &str = "INIT_CWD";
  if !env_vars.contains_key(OsStr::new(INIT_CWD_NAME)) {
    // if not set, set an INIT_CWD env var that has the cwd
    env_vars.insert(
      INIT_CWD_NAME.into(),
      initial_cwd.to_path_buf().into_os_string(),
    );
  }
  if !env_vars
    .contains_key(OsStr::new(crate::npm::NPM_CONFIG_USER_AGENT_ENV_VAR))
  {
    env_vars.insert(
      crate::npm::NPM_CONFIG_USER_AGENT_ENV_VAR.into(),
      crate::npm::get_npm_config_user_agent().into(),
    );
  }
  // Prepend in reverse so the closest dir ends up first in `PATH`.
  for bin_dir in node_modules_bin_dirs.iter().rev() {
    prepend_to_path(&mut env_vars, bin_dir.as_os_str().to_os_string());
  }
  env_vars
}

fn prepend_to_path(
  env_vars: &mut HashMap<OsString, OsString>,
  value: OsString,
) {
  match env_vars.get_mut(OsStr::new("PATH")) {
    Some(path) => {
      if path.is_empty() {
        *path = value;
      } else {
        let mut new_path = value;
        new_path.push(if cfg!(windows) { ";" } else { ":" });
        new_path.push(&path);
        *path = new_path;
      }
    }
    None => {
      env_vars.insert("PATH".into(), value);
    }
  }
}

pub fn real_env_vars() -> HashMap<OsString, OsString> {
  std::env::vars_os()
    .map(|(k, v)| {
      if cfg!(windows) {
        (k.to_ascii_uppercase(), v)
      } else {
        (k, v)
      }
    })
    .collect()
}

// WARNING: Do not depend on this env var in user code. It's not stable API.
pub(crate) static USE_PKG_JSON_HIDDEN_ENV_VAR_NAME: &str =
  "DENO_INTERNAL_TASK_USE_PKG_JSON";

pub struct NpmCommand;

impl ShellCommand for NpmCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    if context.args.first().and_then(|s| s.to_str()) == Some("run")
      && context.args.len() >= 2
      // for now, don't run any npm scripts that have a flag because
      // we don't handle stuff like `--workspaces` properly
      && !context.args.iter().any(|s| s.to_string_lossy().starts_with('-'))
    {
      // run with deno task instead
      let mut args: Vec<OsString> = Vec::with_capacity(context.args.len());
      args.push("task".into());
      args.extend(context.args.into_iter().skip(1));

      let mut state = context.state;
      state.apply_env_var(
        OsStr::new(USE_PKG_JSON_HIDDEN_ENV_VAR_NAME),
        OsStr::new("1"),
      );
      return ExecutableCommand::new(
        "deno".to_string(),
        std::env::current_exe()
          .and_then(|p| canonicalize_path(&p))
          .unwrap(),
      )
      .execute(ShellCommandContext {
        args,
        state,
        ..context
      });
    }

    // fallback to running the real npm command
    let npm_path = match context.state.resolve_command_path(OsStr::new("npm")) {
      Ok(path) => path,
      Err(err) => {
        let _ = context.stderr.write_line(&format!("{}", err));
        return Box::pin(std::future::ready(ExecuteResult::from_exit_code(
          err.exit_code(),
        )));
      }
    };
    ExecutableCommand::new("npm".to_string(), npm_path).execute(context)
  }
}

pub struct DenoCommand(ExecutableCommand);

impl Default for DenoCommand {
  fn default() -> Self {
    Self(ExecutableCommand::new(
      "deno".to_string(),
      std::env::current_exe()
        .and_then(|p| canonicalize_path(&p))
        .unwrap(),
    ))
  }
}

impl ShellCommand for DenoCommand {
  fn execute(
    &self,
    context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    self.0.execute(context)
  }
}

pub struct NodeCommand;

impl ShellCommand for NodeCommand {
  fn execute(
    &self,
    context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    // continue to use Node if the first argument is a flag
    // or there are no arguments provided for some reason
    if context.args.is_empty()
      || ({
        let first_arg = context.args[0].to_string_lossy();
        first_arg.starts_with('-') // has a flag
      })
    {
      return ExecutableCommand::new("node".to_string(), PathBuf::from("node"))
        .execute(context);
    }

    let mut args: Vec<OsString> = Vec::with_capacity(7 + context.args.len());
    args.extend([
      "run".into(),
      "-A".into(),
      "--unstable-bare-node-builtins".into(),
      "--unstable-detect-cjs".into(),
      "--unstable-sloppy-imports".into(),
      "--unstable-unsafe-proto".into(),
    ]);
    args.extend(context.args);

    let mut state = context.state;
    state.apply_env_var(
      OsStr::new(USE_PKG_JSON_HIDDEN_ENV_VAR_NAME),
      OsStr::new("1"),
    );
    ExecutableCommand::new(
      "deno".to_string(),
      std::env::current_exe()
        .and_then(|p| canonicalize_path(&p))
        .unwrap(),
    )
    .execute(ShellCommandContext {
      args,
      state,
      ..context
    })
  }
}

pub struct NodeGypCommand;

impl ShellCommand for NodeGypCommand {
  fn execute(
    &self,
    context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    // at the moment this shell command is just to give a warning if node-gyp is not found
    // in the future, we could try to run/install node-gyp for the user with deno
    if context
      .state
      .resolve_command_path(OsStr::new("node-gyp"))
      .is_err()
    {
      log::warn!(
        "{} node-gyp was used in a script, but was not listed as a dependency. Either add it as a dependency or install it globally (e.g. `npm install -g node-gyp`)",
        crate::colors::yellow("Warning")
      );
      Box::pin(std::future::ready(ExecuteResult::from_exit_code(0)))
    } else {
      ExecutableCommand::new(
        "node-gyp".to_string(),
        "node-gyp".to_string().into(),
      )
      .execute(context)
    }
  }
}

pub struct NpxCommand;

impl ShellCommand for NpxCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    if let Some(first_arg) = context.args.first().cloned() {
      match context.state.resolve_custom_command(&first_arg) {
        Some(command) => {
          let context = ShellCommandContext {
            args: context.args.into_iter().skip(1).collect::<Vec<_>>(),
            ..context
          };
          command.execute(context)
        }
        _ => {
          // can't find the command, so fallback to running the real npx command
          let npx_path =
            match context.state.resolve_command_path(OsStr::new("npx")) {
              Ok(npx) => npx,
              Err(err) => {
                let _ = context.stderr.write_line(&format!("{}", err));
                return Box::pin(std::future::ready(
                  ExecuteResult::from_exit_code(err.exit_code()),
                ));
              }
            };
          ExecutableCommand::new("npx".to_string(), npx_path).execute(context)
        }
      }
    } else {
      let _ = context.stderr.write_line("npx: missing command");
      Box::pin(std::future::ready(ExecuteResult::from_exit_code(1)))
    }
  }
}

/// Runs a module in the node_modules folder.
#[derive(Clone)]
pub struct NodeModulesFileRunCommand {
  pub command_name: String,
  pub path: PathBuf,
}

impl ShellCommand for NodeModulesFileRunCommand {
  fn execute(
    &self,
    mut context: ShellCommandContext,
  ) -> LocalBoxFuture<'static, ExecuteResult> {
    let mut args: Vec<OsString> = vec![
      "run".into(),
      "--ext=js".into(),
      "-A".into(),
      self.path.clone().into_os_string(),
    ];
    args.extend(context.args);
    let executable_command = deno_task_shell::ExecutableCommand::new(
      "deno".to_string(),
      std::env::current_exe()
        .and_then(|p| canonicalize_path(&p))
        .unwrap(),
    );
    // set this environment variable so that the launched process knows the npm command name
    context.state.apply_env_var(
      OsStr::new("DENO_INTERNAL_NPM_CMD_NAME"),
      OsStr::new(&self.command_name),
    );
    executable_command.execute(ShellCommandContext { args, ..context })
  }
}

pub fn resolve_custom_commands(
  node_resolver: &CliNodeResolver,
  npm_resolver: &CliNpmResolver,
  bin_dirs: &[PathBuf],
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut commands = match npm_resolver {
    CliNpmResolver::Byonm(_) => {
      // Walk the bin dirs in order (closest first) and merge; closest wins.
      let mut commands: HashMap<String, Rc<dyn ShellCommand>> = HashMap::new();
      for bin_dir in bin_dirs {
        for (name, cmd) in
          resolve_npm_commands_from_bin_dir(bin_dir, node_resolver)
        {
          commands.entry(name).or_insert(cmd);
        }
      }
      commands
    }
    CliNpmResolver::Managed(npm_resolver) => {
      resolve_managed_npm_commands(node_resolver, npm_resolver)?
    }
  };
  commands.insert("npm".to_string(), Rc::new(NpmCommand));
  Ok(commands)
}

/// Builds the list of `node_modules/.bin` directories to consult for a task.
///
/// For BYONM this walks up the filesystem from `cwd` collecting every
/// `<ancestor>/node_modules/.bin` directory (matching how Node, npm, and pnpm
/// resolve bin commands). For the managed npm resolver there is only ever a
/// single `node_modules/.bin`.
pub fn resolve_task_node_modules_bin_dirs(
  npm_resolver: &CliNpmResolver,
  cwd: &Path,
) -> Vec<PathBuf> {
  match npm_resolver {
    CliNpmResolver::Byonm(_) => cwd
      .ancestors()
      .map(|dir| dir.join("node_modules").join(".bin"))
      .collect(),
    CliNpmResolver::Managed(npm_resolver) => npm_resolver
      .root_node_modules_path()
      .map(|p| vec![p.join(".bin")])
      .unwrap_or_default(),
  }
}

pub fn resolve_npm_commands_from_bin_dir(
  bin_dir: &Path,
  node_resolver: &CliNodeResolver,
) -> HashMap<String, Rc<dyn ShellCommand>> {
  let bin_commands = node_resolver.resolve_npm_commands_from_bin_dir(bin_dir);
  bin_commands
    .into_iter()
    .map(|(command_name, path)| {
      (
        command_name.clone(),
        Rc::new(NodeModulesFileRunCommand {
          command_name,
          path: path.path().to_path_buf(),
        }) as Rc<dyn ShellCommand>,
      )
    })
    .collect()
}

fn resolve_managed_npm_commands(
  node_resolver: &CliNodeResolver,
  npm_resolver: &CliManagedNpmResolver,
) -> Result<HashMap<String, Rc<dyn ShellCommand>>, AnyError> {
  let mut result = HashMap::new();
  for id in npm_resolver.resolution().top_level_packages() {
    let package_folder = npm_resolver.resolve_pkg_folder_from_pkg_id(&id)?;
    let bins =
      node_resolver.resolve_npm_binary_commands_for_package(&package_folder)?;
    result.extend(bins.into_iter().map(|(command_name, path)| {
      (
        command_name.clone(),
        Rc::new(NodeModulesFileRunCommand {
          command_name,
          path: path.path().to_path_buf(),
        }) as Rc<dyn ShellCommand>,
      )
    }));
  }
  if !result.contains_key("npx") {
    result.insert("npx".to_string(), Rc::new(NpxCommand));
  }
  Ok(result)
}

/// Runs a deno task future forwarding any signals received
/// to the process.
///
/// Signal listeners and ctrl+c listening will be setup.
pub async fn run_future_forwarding_signals<TOutput>(
  kill_signal: KillSignal,
  future: impl std::future::Future<Output = TOutput>,
) -> TOutput {
  fn spawn_future_with_cancellation(
    future: impl std::future::Future<Output = ()> + 'static,
    token: CancellationToken,
  ) {
    deno_core::unsync::spawn(async move {
      tokio::select! {
        _ = future => {}
        _ = token.cancelled() => {}
      }
    });
  }

  let token = CancellationToken::new();
  let _token_drop_guard = token.clone().drop_guard();
  let _drop_guard = kill_signal.clone().drop_guard();

  spawn_future_with_cancellation(
    listen_ctrl_c(kill_signal.clone()),
    token.clone(),
  );
  #[cfg(unix)]
  spawn_future_with_cancellation(
    listen_and_forward_all_signals(kill_signal),
    token,
  );

  future.await
}

async fn listen_ctrl_c(kill_signal: KillSignal) {
  while let Ok(()) = deno_signals::ctrl_c().await {
    // On windows, ctrl+c is sent to the process group, so the signal would
    // have already been sent to the child process. We still want to listen
    // for ctrl+c here to keep the process alive when receiving it, but no
    // need to forward the signal because it's already been sent.
    if !cfg!(windows) {
      kill_signal.send(deno_task_shell::SignalKind::SIGINT)
    }
  }
}

#[cfg(unix)]
async fn listen_and_forward_all_signals(kill_signal: KillSignal) {
  use deno_core::futures::FutureExt;
  use deno_signals::SIGNAL_NUMS;

  // listen and forward every signal we support
  let mut futures = Vec::with_capacity(SIGNAL_NUMS.len());
  for signo in SIGNAL_NUMS.iter().copied() {
    if signo == libc::SIGKILL || signo == libc::SIGSTOP {
      continue; // skip, can't listen to these
    }

    let kill_signal = kill_signal.clone();
    futures.push(
      async move {
        let Ok(mut stream) = deno_signals::signal_stream(signo) else {
          return;
        };
        let signal_kind: deno_task_shell::SignalKind = signo.into();
        while let Some(()) = stream.recv().await {
          kill_signal.send(signal_kind);
        }
      }
      .boxed_local(),
    )
  }
  deno_core::futures::future::join_all(futures).await;
}

#[cfg(test)]
mod test {

  use super::*;

  #[test]
  fn test_prepend_to_path() {
    let mut env_vars = HashMap::new();

    prepend_to_path(&mut env_vars, "/example".into());
    assert_eq!(
      env_vars,
      HashMap::from([("PATH".into(), "/example".into())])
    );

    prepend_to_path(&mut env_vars, "/example2".into());
    let separator = if cfg!(windows) { ";" } else { ":" };
    assert_eq!(
      env_vars,
      HashMap::from([(
        "PATH".into(),
        format!("/example2{}/example", separator).into()
      )])
    );

    env_vars.get_mut(OsStr::new("PATH")).unwrap().clear();
    prepend_to_path(&mut env_vars, "/example".into());
    assert_eq!(
      env_vars,
      HashMap::from([("PATH".into(), "/example".into())])
    );
  }

  #[test]
  fn test_get_script_with_args() {
    let cases: &[(&[&str], &str)] = &[
      (&[], "echo"),
      (&["hello"], "echo 'hello'"),
      (&["hello", "world"], "echo 'hello' 'world'"),
      // Windows path with trailing backslash (issue #31453).
      (&[".\\dist\\"], "echo '.\\dist\\'"),
      // Dollar sign and backtick must not be expanded.
      (&["$HOME"], "echo '$HOME'"),
      (&["`cmd`"], "echo '`cmd`'"),
      // Double quotes pass through literally.
      (&["foo\"bar"], "echo 'foo\"bar'"),
      // Single quote uses the POSIX `'"'"'` idiom.
      (&["it's"], "echo 'it'\"'\"'s'"),
    ];
    for (argv, expected) in cases {
      let argv: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
      assert_eq!(get_script_with_args("echo", &argv), *expected);
    }
  }

  #[test]
  fn test_expand_braces() {
    let cases = &[
      // The case from the issue (denoland/deno#24500): glob brace lists.
      ("mocha test/{*,**/**}.js", "mocha test/*.js test/**/**.js"),
      // Basic comma list with preamble and postscript.
      ("echo a{b,c}d", "echo abd acd"),
      ("echo {foo,bar,baz}", "echo foo bar baz"),
      ("echo pre{a,b}post", "echo preapost prebpost"),
      // Empty options.
      ("echo x{a,}y", "echo xay xy"),
      ("echo x{,a}y", "echo xy xay"),
      // Multiple groups in one word produce a cartesian product.
      ("echo {a,b}{c,d}", "echo ac ad bc bd"),
      // Nested groups.
      ("echo {a,b{c,d}}", "echo a bc bd"),
      ("echo {a{b,c}}", "echo {ab} {ac}"),
      // File extension lists.
      (
        "eslint src/**/*.{js,ts,tsx}",
        "eslint src/**/*.js src/**/*.ts src/**/*.tsx",
      ),
      // Numeric sequences.
      ("echo {1..5}", "echo 1 2 3 4 5"),
      ("echo {5..1}", "echo 5 4 3 2 1"),
      ("echo {1..10..2}", "echo 1 3 5 7 9"),
      ("echo {01..03}", "echo 01 02 03"),
      ("echo {-2..2}", "echo -2 -1 0 1 2"),
      // Character sequences.
      ("echo {a..e}", "echo a b c d e"),
      ("echo {A..C}", "echo A B C"),
      ("echo {e..a}", "echo e d c b a"),
      // Groups across multiple words are each expanded independently.
      ("cp {a,b} dest/{x,y}", "cp a b dest/x dest/y"),
      // --- Cases that must NOT expand ---
      // No comma and not a sequence -> literal.
      ("echo {a}", "echo {a}"),
      ("echo {}", "echo {}"),
      ("echo {a..}", "echo {a..}"),
      ("echo {a..3}", "echo {a..3}"),
      // Quoted braces are literal.
      ("echo '{a,b}'", "echo '{a,b}'"),
      ("echo \"{a,b}\"", "echo \"{a,b}\""),
      // Variable and command substitutions are left untouched.
      ("echo ${FOO}", "echo ${FOO}"),
      ("echo ${FOO:-{a,b}}", "echo ${FOO:-{a,b}}"),
      ("echo $(echo hi)", "echo $(echo hi)"),
      // Escaped braces are literal.
      ("echo \\{a,b\\}", "echo \\{a,b\\}"),
      // Unbalanced braces are left as-is.
      ("echo {a,b", "echo {a,b"),
      // Script without any brace is returned untouched.
      ("deno run main.ts", "deno run main.ts"),
    ];
    for &(input, expected) in cases {
      assert_eq!(expand_braces(input).as_ref(), expected, "input: {input}");
    }
  }

  #[test]
  fn test_expand_braces_no_alloc_when_no_brace() {
    assert!(matches!(expand_braces("deno test"), Cow::Borrowed(_)));
  }
}
