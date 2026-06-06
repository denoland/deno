// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io::BufRead;
use std::io::IsTerminal;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use deno_ast::swc::parser::error::SyntaxError;
use deno_ast::swc::parser::token::BinOpToken;
use deno_ast::swc::parser::token::Token;
use deno_ast::swc::parser::token::Word;
use deno_ast::view::AssignOp;
use deno_core::anyhow::Context as _;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use unicode_width::UnicodeWidthStr;

use super::channel::EditorSyncMessageSender;
use crate::cdp;
use crate::colors;

const PROMPT: &str = "> ";
const DISPLAY_ALL_COMPLETIONS_THRESHOLD: usize = 100;

#[derive(Debug)]
pub enum ReadlineError {
  Interrupted,
  Eof,
  #[allow(dead_code)]
  Io(std::io::Error),
}

impl From<std::io::Error> for ReadlineError {
  fn from(err: std::io::Error) -> Self {
    Self::Io(err)
  }
}

#[derive(Debug, PartialEq, Eq)]
enum ValidationResult {
  Valid(Option<String>),
  Invalid(Option<String>),
  Incomplete,
}

// Provides helpers to the editor like validation for multi-line edits, completion candidates for
// tab completion.
pub struct EditorHelper {
  pub context_id: u64,
  pub sync_sender: EditorSyncMessageSender,
}

impl EditorHelper {
  pub fn get_global_lexical_scope_names(&self) -> Vec<String> {
    let evaluate_response = self
      .sync_sender
      .post_message(
        "Runtime.globalLexicalScopeNames",
        Some(cdp::GlobalLexicalScopeNamesArgs {
          execution_context_id: Some(self.context_id),
        }),
      )
      .unwrap();
    let evaluate_response: cdp::GlobalLexicalScopeNamesResponse =
      serde_json::from_value(evaluate_response).unwrap();
    evaluate_response.names
  }

  pub fn get_expression_property_names(&self, expr: &str) -> Vec<String> {
    // try to get the properties from the expression
    if let Some(properties) = self.get_object_expr_properties(expr) {
      return properties;
    }

    // otherwise fall back to the prototype
    let expr_type = self.get_expression_type(expr);
    let object_expr = match expr_type.as_deref() {
      // possibilities: https://chromedevtools.github.io/devtools-protocol/v8/Runtime/#type-RemoteObject
      Some("object") => "Object.prototype",
      Some("function") => "Function.prototype",
      Some("string") => "String.prototype",
      Some("boolean") => "Boolean.prototype",
      Some("bigint") => "BigInt.prototype",
      Some("number") => "Number.prototype",
      _ => return Vec::new(), // undefined, symbol, and unhandled
    };

    self
      .get_object_expr_properties(object_expr)
      .unwrap_or_default()
  }

  fn get_expression_type(&self, expr: &str) -> Option<String> {
    self.evaluate_expression(expr).map(|res| res.result.kind)
  }

  fn get_object_expr_properties(
    &self,
    object_expr: &str,
  ) -> Option<Vec<String>> {
    let evaluate_result = self.evaluate_expression(object_expr)?;
    let object_id = evaluate_result.result.object_id?;

    let get_properties_response = self
      .sync_sender
      .post_message(
        "Runtime.getProperties",
        Some(cdp::GetPropertiesArgs {
          object_id,
          own_properties: None,
          accessor_properties_only: None,
          generate_preview: None,
          non_indexed_properties_only: Some(true),
        }),
      )
      .ok()?;
    let get_properties_response: cdp::GetPropertiesResponse =
      serde_json::from_value(get_properties_response).ok()?;
    Some(
      get_properties_response
        .result
        .into_iter()
        .map(|prop| prop.name)
        .collect(),
    )
  }

  fn evaluate_expression(&self, expr: &str) -> Option<cdp::EvaluateResponse> {
    let evaluate_response = self
      .sync_sender
      .post_message(
        "Runtime.evaluate",
        Some(cdp::EvaluateArgs {
          expression: expr.to_string(),
          object_group: None,
          include_command_line_api: None,
          silent: None,
          context_id: Some(self.context_id),
          return_by_value: None,
          generate_preview: None,
          user_gesture: None,
          await_promise: None,
          throw_on_side_effect: Some(true),
          timeout: Some(200),
          disable_breaks: None,
          repl_mode: None,
          allow_unsafe_eval_blocked_by_csp: None,
          unique_context_id: None,
        }),
      )
      .ok()?;
    let evaluate_response: cdp::EvaluateResponse =
      serde_json::from_value(evaluate_response).ok()?;

    if evaluate_response.exception_details.is_some() {
      None
    } else {
      Some(evaluate_response)
    }
  }

  fn complete(&self, line: &str, pos: usize) -> (usize, Vec<String>) {
    let expr = get_expr_from_line_at_pos(line, pos);

    // check if the expression is in the form `obj.prop`
    if let Some(index) = expr.rfind('.') {
      let sub_expr = &expr[..index];
      let prop_name = &expr[index + 1..];
      let candidates = self
        .get_expression_property_names(sub_expr)
        .into_iter()
        .filter(|n| !n.starts_with("Symbol(") && n.starts_with(prop_name))
        .collect();

      (pos - prop_name.len(), candidates)
    } else {
      // combine results of declarations and globalThis properties
      let mut candidates = self
        .get_expression_property_names("globalThis")
        .into_iter()
        .chain(self.get_global_lexical_scope_names())
        .filter(|n| n.starts_with(expr))
        .collect::<Vec<_>>();

      // sort and remove duplicates
      candidates.sort();
      candidates.dedup(); // make sure to sort first

      (pos - expr.len(), candidates)
    }
  }

  fn highlight<'l>(&self, line: &'l str) -> Cow<'l, str> {
    let mut out_line = String::from(line);

    let mut lexed_items = deno_ast::lex(line, deno_ast::MediaType::TypeScript)
      .into_iter()
      .peekable();
    while let Some(item) = lexed_items.next() {
      // Adding color adds more bytes to the string,
      // so an offset is needed to stop spans falling out of sync.
      let offset = out_line.len() - line.len();
      let range = item.range;

      out_line.replace_range(
        range.start + offset..range.end + offset,
        &match item.inner {
          deno_ast::TokenOrComment::Token(token) => match token {
            Token::Str { .. } | Token::Template { .. } | Token::BackQuote => {
              colors::green(&line[range]).to_string()
            }
            Token::Regex(_, _) => colors::red(&line[range]).to_string(),
            Token::Num { .. } | Token::BigInt { .. } => {
              colors::yellow(&line[range]).to_string()
            }
            Token::Word(word) => match word {
              Word::True | Word::False | Word::Null => {
                colors::yellow(&line[range]).to_string()
              }
              Word::Keyword(_) => colors::cyan(&line[range]).to_string(),
              Word::Ident(ident) => match ident.as_ref() {
                "undefined" => colors::gray(&line[range]).to_string(),
                "Infinity" | "NaN" => colors::yellow(&line[range]).to_string(),
                "async" | "of" => colors::cyan(&line[range]).to_string(),
                _ => {
                  let next = lexed_items.peek().map(|item| &item.inner);
                  if matches!(
                    next,
                    Some(deno_ast::TokenOrComment::Token(Token::LParen))
                  ) {
                    // We're looking for something that looks like a function
                    // We use a simple heuristic: 'ident' followed by 'LParen'
                    colors::intense_blue(&line[range]).to_string()
                  } else if ident.as_ref() == "from"
                    && matches!(
                      next,
                      Some(deno_ast::TokenOrComment::Token(Token::Str { .. }))
                    )
                  {
                    // When ident 'from' is followed by a string literal, highlight it
                    // E.g. "export * from 'something'" or "import a from 'something'"
                    colors::cyan(&line[range]).to_string()
                  } else {
                    line[range].to_string()
                  }
                }
              },
            },
            _ => line[range].to_string(),
          },
          deno_ast::TokenOrComment::Comment { .. } => {
            colors::gray(&line[range]).to_string()
          }
        },
      );
    }

    out_line.into()
  }
}

fn is_word_boundary(c: char) -> bool {
  if matches!(c, '.' | '_' | '$') {
    false
  } else {
    char::is_ascii_whitespace(&c) || char::is_ascii_punctuation(&c)
  }
}

fn get_expr_from_line_at_pos(line: &str, cursor_pos: usize) -> &str {
  let start = line[..cursor_pos].rfind(is_word_boundary).unwrap_or(0);
  let word = &line[start..cursor_pos];
  word.strip_prefix(is_word_boundary).unwrap_or(word)
}

fn validate(input: &str) -> ValidationResult {
  let line_info = text_lines::TextLines::new(input);
  let mut stack: Vec<Token> = Vec::new();
  let mut in_template = false;
  let mut div_token_count_on_current_line = 0;
  let mut last_line_index = 0;
  let mut queued_validation_error = None;
  let tokens = deno_ast::lex(input, deno_ast::MediaType::TypeScript)
    .into_iter()
    .filter_map(|item| match item.inner {
      deno_ast::TokenOrComment::Token(token) => Some((token, item.range)),
      deno_ast::TokenOrComment::Comment { .. } => None,
    });

  for (token, range) in tokens {
    let current_line_index = line_info.line_index(range.start);
    if current_line_index != last_line_index {
      div_token_count_on_current_line = 0;
      last_line_index = current_line_index;

      if let Some(error) = queued_validation_error {
        return error;
      }
    }
    match token {
      Token::BinOp(BinOpToken::Div) | Token::AssignOp(AssignOp::DivAssign) => {
        // it's too complicated to write code to detect regular expression literals
        // which are no longer tokenized, so if a `/` or `/=` happens twice on the same
        // line, then we bail
        div_token_count_on_current_line += 1;
        if div_token_count_on_current_line >= 2 {
          return ValidationResult::Valid(None);
        }
      }
      Token::BackQuote => in_template = !in_template,
      Token::LParen | Token::LBracket | Token::LBrace | Token::DollarLBrace => {
        stack.push(token)
      }
      Token::RParen | Token::RBracket | Token::RBrace => {
        match (stack.pop(), token) {
          (Some(Token::LParen), Token::RParen)
          | (Some(Token::LBracket), Token::RBracket)
          | (Some(Token::LBrace), Token::RBrace)
          | (Some(Token::DollarLBrace), Token::RBrace) => {}
          (Some(left), _) => {
            // queue up a validation error to surface once we've finished examining the current line
            queued_validation_error = Some(ValidationResult::Invalid(Some(
              format!("Mismatched pairs: {left:?} is not properly closed"),
            )));
          }
          (None, _) => {
            // While technically invalid when unpaired, it should be V8's task to output error instead.
            // Thus marked as valid with no info.
            return ValidationResult::Valid(None);
          }
        }
      }
      Token::Error(error) => {
        match error.kind() {
          // If there is unterminated template, it continues to read input.
          SyntaxError::UnterminatedTpl => {}
          _ => {
            // If it failed parsing, it should be V8's task to output error instead.
            // Thus marked as valid with no info.
            return ValidationResult::Valid(None);
          }
        }
      }
      _ => {}
    }
  }

  if let Some(error) = queued_validation_error {
    error
  } else if !stack.is_empty() || in_template {
    ValidationResult::Incomplete
  } else {
    ValidationResult::Valid(None)
  }
}

#[derive(Default)]
struct CompletionState {
  line: String,
  cursor: usize,
  start: usize,
  candidates: Vec<String>,
}

struct EditorState {
  helper: EditorHelper,
  history: Vec<String>,
}

#[derive(Clone)]
pub struct ReplEditor {
  inner: Arc<Mutex<EditorState>>,
  history_file_path: Option<PathBuf>,
  errored_on_history_save: Arc<AtomicBool>,
  should_exit_on_interrupt: Arc<AtomicBool>,
}

impl ReplEditor {
  pub fn new(
    helper: EditorHelper,
    history_file_path: Option<PathBuf>,
  ) -> Result<Self, AnyError> {
    let history = history_file_path
      .as_ref()
      .and_then(|history_file_path| {
        std::fs::read_to_string(history_file_path).ok()
      })
      .map(|text| text.lines().map(ToOwned::to_owned).collect())
      .unwrap_or_default();

    if let Some(history_file_path) = &history_file_path {
      let history_file_dir = history_file_path.parent().unwrap();
      std::fs::create_dir_all(history_file_dir).with_context(|| {
        format!(
          "Unable to create directory for the history file: {}",
          history_file_dir.display()
        )
      })?;
    }

    Ok(ReplEditor {
      inner: Arc::new(Mutex::new(EditorState { helper, history })),
      history_file_path,
      errored_on_history_save: Arc::new(AtomicBool::new(false)),
      should_exit_on_interrupt: Arc::new(AtomicBool::new(false)),
    })
  }

  pub fn readline(&self) -> Result<String, ReadlineError> {
    if !std::io::stdin().is_terminal() {
      return self.read_line_without_tty();
    }

    let _raw_mode = RawModeGuard::new()?;
    let mut stdout = std::io::stdout();
    let mut line = String::new();
    let mut cursor = 0;
    let mut history_index = None;
    let mut completion_state = CompletionState::default();
    let mut pending_completion_confirmation: Option<Vec<String>> = None;

    write!(stdout, "{PROMPT}")?;
    stdout.flush()?;

    loop {
      let event = crossterm::event::read()?;
      let Event::Key(key_event) = event else {
        continue;
      };
      if key_event.kind == KeyEventKind::Release {
        continue;
      }

      if let Some(candidates) = pending_completion_confirmation.take() {
        match key_event.code {
          KeyCode::Char('y') | KeyCode::Char('Y') => {
            write!(stdout, "y")?;
            self.write_completions(&mut stdout, &candidates)?;
            self.redraw(&mut stdout, &line, cursor)?;
          }
          _ => {
            self.redraw(&mut stdout, &line, cursor)?;
          }
        }
        stdout.flush()?;
        continue;
      }

      match key_event {
        KeyEvent {
          code: KeyCode::Char('c'),
          modifiers: KeyModifiers::CONTROL,
          ..
        } => {
          writeln!(stdout)?;
          stdout.flush()?;
          return Err(ReadlineError::Interrupted);
        }
        KeyEvent {
          code: KeyCode::Char('d'),
          modifiers: KeyModifiers::CONTROL,
          ..
        } if line.is_empty() => {
          writeln!(stdout)?;
          stdout.flush()?;
          return Err(ReadlineError::Eof);
        }
        KeyEvent {
          code: KeyCode::Char('s'),
          modifiers: KeyModifiers::CONTROL,
          ..
        } => {
          insert_str(&mut line, &mut cursor, "\n");
          writeln!(stdout)?;
          stdout.flush()?;
        }
        KeyEvent {
          code: KeyCode::Char('r'),
          modifiers: KeyModifiers::CONTROL,
          ..
        } => {
          self.should_exit_on_interrupt.store(false, Relaxed);
        }
        KeyEvent {
          code: KeyCode::Char('\n' | '\r'),
          ..
        }
        | KeyEvent {
          code: KeyCode::Char('j' | 'm'),
          modifiers: KeyModifiers::CONTROL,
          ..
        }
        | KeyEvent {
          code: KeyCode::Enter,
          ..
        } => match validate(&line) {
          ValidationResult::Incomplete => {
            insert_str(&mut line, &mut cursor, "\n");
            writeln!(stdout)?;
            stdout.flush()?;
          }
          _ => {
            writeln!(stdout)?;
            stdout.flush()?;
            return Ok(line);
          }
        },
        KeyEvent {
          code: KeyCode::Tab, ..
        } => {
          self.handle_tab(
            &mut stdout,
            &mut line,
            &mut cursor,
            &mut completion_state,
            &mut pending_completion_confirmation,
          )?;
          history_index = None;
        }
        KeyEvent {
          code: KeyCode::Backspace,
          ..
        } => {
          if cursor > 0 {
            let previous = previous_char_boundary(&line, cursor);
            line.replace_range(previous..cursor, "");
            cursor = previous;
            self.redraw(&mut stdout, &line, cursor)?;
          }
          history_index = None;
          completion_state = CompletionState::default();
        }
        KeyEvent {
          code: KeyCode::Delete,
          ..
        } => {
          if cursor < line.len() {
            let next = next_char_boundary(&line, cursor);
            line.replace_range(cursor..next, "");
            self.redraw(&mut stdout, &line, cursor)?;
          }
          history_index = None;
          completion_state = CompletionState::default();
        }
        KeyEvent {
          code: KeyCode::Left,
          ..
        } => {
          if cursor > 0 {
            cursor = previous_char_boundary(&line, cursor);
            self.redraw(&mut stdout, &line, cursor)?;
          }
        }
        KeyEvent {
          code: KeyCode::Right,
          ..
        } => {
          if cursor < line.len() {
            cursor = next_char_boundary(&line, cursor);
            self.redraw(&mut stdout, &line, cursor)?;
          }
        }
        KeyEvent {
          code: KeyCode::Home,
          ..
        } => {
          cursor = 0;
          self.redraw(&mut stdout, &line, cursor)?;
        }
        KeyEvent {
          code: KeyCode::End, ..
        } => {
          cursor = line.len();
          self.redraw(&mut stdout, &line, cursor)?;
        }
        KeyEvent {
          code: KeyCode::Up, ..
        } => {
          let history = &self.inner.lock().history;
          if !history.is_empty() {
            let next_index = history_index
              .map(|index: usize| index.saturating_sub(1))
              .unwrap_or_else(|| history.len() - 1);
            history_index = Some(next_index);
            line = history[next_index].clone();
            cursor = line.len();
            self.redraw(&mut stdout, &line, cursor)?;
          }
          completion_state = CompletionState::default();
        }
        KeyEvent {
          code: KeyCode::Down,
          ..
        } => {
          if let Some(index) = history_index {
            let history = &self.inner.lock().history;
            if index + 1 < history.len() {
              let next_index = index + 1;
              history_index = Some(next_index);
              line = history[next_index].clone();
            } else {
              history_index = None;
              line.clear();
            }
            cursor = line.len();
            self.redraw(&mut stdout, &line, cursor)?;
          }
          completion_state = CompletionState::default();
        }
        KeyEvent {
          code: KeyCode::Char(ch),
          modifiers,
          ..
        } if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT => {
          insert_str(&mut line, &mut cursor, &ch.to_string());
          if line.contains('\n') {
            write!(stdout, "{ch}")?;
          } else {
            self.redraw(&mut stdout, &line, cursor)?;
          }
          history_index = None;
          completion_state = CompletionState::default();
        }
        _ => {}
      }

      stdout.flush()?;
    }
  }

  fn read_line_without_tty(&self) -> Result<String, ReadlineError> {
    let mut stdin = std::io::stdin().lock();
    let mut line = String::new();

    loop {
      let mut current_line = String::new();
      if stdin.read_line(&mut current_line)? == 0 {
        if line.is_empty() {
          return Err(ReadlineError::Eof);
        } else {
          return Ok(line);
        }
      }

      let current_line = current_line.trim_end_matches(['\r', '\n']);
      if !line.is_empty() {
        line.push('\n');
      }
      line.push_str(current_line);

      if validate(&line) != ValidationResult::Incomplete {
        return Ok(line);
      }
    }
  }

  fn handle_tab(
    &self,
    stdout: &mut std::io::Stdout,
    line: &mut String,
    cursor: &mut usize,
    completion_state: &mut CompletionState,
    pending_completion_confirmation: &mut Option<Vec<String>>,
  ) -> Result<(), ReadlineError> {
    if line.is_empty()
      || line[..*cursor]
        .chars()
        .next_back()
        .filter(|c| c.is_whitespace())
        .is_some()
    {
      if cfg!(target_os = "windows") {
        insert_str(line, cursor, "    ");
      } else {
        insert_str(line, cursor, "\t");
      }
      if line.contains('\n') {
        if cfg!(target_os = "windows") {
          write!(stdout, "    ")?;
        } else {
          write!(stdout, "\t")?;
        }
      } else {
        self.redraw(stdout, line, *cursor)?;
      }
      *completion_state = CompletionState::default();
      return Ok(());
    }

    let (start, mut candidates) =
      self.inner.lock().helper.complete(line, *cursor);
    candidates.sort();
    candidates.dedup();
    if candidates.is_empty() {
      *completion_state = CompletionState::default();
      return Ok(());
    }

    if candidates.len() == 1 {
      line.replace_range(start..*cursor, &candidates[0]);
      *cursor = start + candidates[0].len();
      self.redraw(stdout, line, *cursor)?;
      *completion_state = CompletionState::default();
      return Ok(());
    }

    let repeated_tab = completion_state.line == *line
      && completion_state.cursor == *cursor
      && completion_state.start == start
      && completion_state.candidates == candidates;

    if !repeated_tab
      && let Some(prefix) = common_prefix(&candidates)
      && prefix.len() > *cursor - start
    {
      line.replace_range(start..*cursor, &prefix);
      *cursor = start + prefix.len();
      self.redraw(stdout, line, *cursor)?;
      *completion_state = CompletionState::default();
      return Ok(());
    }

    *completion_state = CompletionState {
      line: line.clone(),
      cursor: *cursor,
      start,
      candidates: candidates.clone(),
    };

    if repeated_tab {
      if candidates.len() > DISPLAY_ALL_COMPLETIONS_THRESHOLD {
        write!(
          stdout,
          "\r\nDisplay all {} possibilities? (y or n)",
          candidates.len()
        )?;
        *pending_completion_confirmation = Some(candidates);
      } else {
        self.write_completions(stdout, &candidates)?;
        self.redraw(stdout, line, *cursor)?;
      }
    }

    Ok(())
  }

  fn write_completions(
    &self,
    stdout: &mut std::io::Stdout,
    candidates: &[String],
  ) -> Result<(), ReadlineError> {
    writeln!(stdout)?;
    for candidate in candidates {
      writeln!(stdout, "{candidate}")?;
    }
    Ok(())
  }

  fn redraw(
    &self,
    stdout: &mut std::io::Stdout,
    line: &str,
    cursor: usize,
  ) -> Result<(), ReadlineError> {
    if line.contains('\n') {
      return Ok(());
    }

    let highlighted = self.inner.lock().helper.highlight(line);
    write!(stdout, "\r\x1b[2K{PROMPT}{highlighted}")?;
    let suffix_width = UnicodeWidthStr::width(&line[cursor..]);
    if suffix_width > 0 {
      write!(stdout, "\x1b[{suffix_width}D")?;
    }
    Ok(())
  }

  pub fn update_history(&self, entry: String) {
    self.inner.lock().history.push(entry.clone());
    if let Some(history_file_path) = &self.history_file_path {
      let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_file_path)
        .and_then(|mut file| writeln!(file, "{entry}"));
      if let Err(e) = result {
        if self.errored_on_history_save.load(Relaxed) {
          return;
        }

        self.errored_on_history_save.store(true, Relaxed);
        log::warn!("Unable to save history file: {}", e);
      }
    }
  }

  pub fn should_exit_on_interrupt(&self) -> bool {
    self.should_exit_on_interrupt.load(Relaxed)
  }

  pub fn set_should_exit_on_interrupt(&self, yes: bool) {
    self.should_exit_on_interrupt.store(yes, Relaxed);
  }
}

struct RawModeGuard;

impl RawModeGuard {
  fn new() -> Result<Self, std::io::Error> {
    enable_raw_mode()?;
    Ok(Self)
  }
}

impl Drop for RawModeGuard {
  fn drop(&mut self) {
    let _ = disable_raw_mode();
  }
}

fn insert_str(line: &mut String, cursor: &mut usize, value: &str) {
  line.insert_str(*cursor, value);
  *cursor += value.len();
}

fn previous_char_boundary(line: &str, cursor: usize) -> usize {
  line[..cursor]
    .char_indices()
    .next_back()
    .map(|(index, _)| index)
    .unwrap_or(0)
}

fn next_char_boundary(line: &str, cursor: usize) -> usize {
  line[cursor..]
    .char_indices()
    .nth(1)
    .map(|(index, _)| cursor + index)
    .unwrap_or(line.len())
}

fn common_prefix(candidates: &[String]) -> Option<String> {
  let first = candidates.first()?;
  let mut end = first.len();

  for candidate in &candidates[1..] {
    while !candidate.is_char_boundary(end)
      || !first[..end].is_char_boundary(end)
      || !candidate.starts_with(&first[..end])
    {
      end = previous_char_boundary(first, end);
      if end == 0 {
        return Some(String::new());
      }
    }
  }

  Some(first[..end].to_string())
}

#[cfg(test)]
mod test {
  use super::ValidationResult;
  use super::validate;

  #[test]
  fn validate_only_one_forward_slash_per_line() {
    let code = r#"function test(arr){
if( arr.length <= 1) return arr.map(a => a / 2)
let left = test( arr.slice( 0 , arr.length/2 ) )"#;
    assert!(matches!(validate(code), ValidationResult::Incomplete));
  }

  #[test]
  fn validate_regex_looking_code() {
    let code = r#"/testing/;"#;
    assert!(matches!(validate(code), ValidationResult::Valid(_)));
  }
}
