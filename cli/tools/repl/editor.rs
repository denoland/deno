// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

use crossterm::cursor;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal;
use deno_ast::swc::parser::error::SyntaxError;
use deno_ast::swc::parser::token::BinOpToken;
use deno_ast::swc::parser::token::Token;
use deno_ast::swc::parser::token::Word;
use deno_ast::view::AssignOp;
use deno_core::anyhow::Context as _;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use thiserror::Error;

use super::channel::ReplSyncMessageSender;
use crate::cdp;
use crate::colors;
use crate::util::console::RawMode;

#[derive(Debug, Error)]
pub enum ReadLineError {
  #[error("interrupted")]
  Interrupted,
  #[error("EOF")]
  Eof,
  #[error(transparent)]
  Io(#[from] std::io::Error),
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
  pub sync_sender: ReplSyncMessageSender,
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

impl EditorHelper {
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
              Word::Ident(ident) => {
                match ident.as_ref() {
                  "undefined" => colors::gray(&line[range]).to_string(),
                  "Infinity" | "NaN" => {
                    colors::yellow(&line[range]).to_string()
                  }
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
                        Some(deno_ast::TokenOrComment::Token(
                          Token::Str { .. }
                        ))
                      )
                    {
                      // When ident 'from' is followed by a string literal, highlight it
                      // E.g. "export * from 'something'" or "import a from 'something'"
                      colors::cyan(&line[range]).to_string()
                    } else {
                      line[range].to_string()
                    }
                  }
                }
              }
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

#[derive(Clone)]
pub struct ReplEditor {
  helper: Arc<EditorHelper>,
  history: Arc<Mutex<Vec<String>>>,
  history_file_path: Option<PathBuf>,
  errored_on_history_save: Arc<AtomicBool>,
  should_exit_on_interrupt: Arc<AtomicBool>,
}

impl ReplEditor {
  pub fn new(
    helper: EditorHelper,
    history_file_path: Option<PathBuf>,
  ) -> Result<Self, AnyError> {
    let should_exit_on_interrupt = Arc::new(AtomicBool::new(false));
    let history = if let Some(history_file_path) = &history_file_path {
      std::fs::read_to_string(history_file_path)
        .map(|text| text.lines().map(ToOwned::to_owned).collect())
        .unwrap_or_default()
    } else {
      Vec::new()
    };

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
      helper: Arc::new(helper),
      history: Arc::new(Mutex::new(history)),
      history_file_path,
      errored_on_history_save: Arc::new(AtomicBool::new(false)),
      should_exit_on_interrupt,
    })
  }

  pub fn readline(&self) -> Result<String, ReadLineError> {
    LineEditor::new(
      &self.helper,
      &self.history.lock(),
      &self.should_exit_on_interrupt,
    )
    .readline()
  }

  pub fn update_history(&self, entry: String) {
    if entry.is_empty() {
      return;
    }

    self.history.lock().push(entry.clone());
    if let Some(history_file_path) = &self.history_file_path {
      match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_file_path)
        .and_then(|mut file| writeln!(file, "{entry}"))
      {
        Ok(_) => {}
        Err(e) => {
          if self.errored_on_history_save.load(Relaxed) {
            return;
          }

          self.errored_on_history_save.store(true, Relaxed);
          log::warn!("Unable to save history file: {}", e);
        }
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

struct LineEditor<'a> {
  helper: &'a EditorHelper,
  history: &'a [String],
  should_exit_on_interrupt: &'a AtomicBool,
  line: String,
  cursor: usize,
  rendered_cursor_line: usize,
  history_index: Option<usize>,
}

impl<'a> LineEditor<'a> {
  fn new(
    helper: &'a EditorHelper,
    history: &'a [String],
    should_exit_on_interrupt: &'a AtomicBool,
  ) -> Self {
    Self {
      helper,
      history,
      should_exit_on_interrupt,
      line: String::new(),
      cursor: 0,
      rendered_cursor_line: 0,
      history_index: None,
    }
  }

  fn readline(mut self) -> Result<String, ReadLineError> {
    let mut raw_mode = Some(RawMode::enable()?);
    let mut stdout = std::io::stdout();
    write!(stdout, "> ")?;
    stdout.flush()?;

    loop {
      let Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        ..
      }) = crossterm::event::read()?
      else {
        continue;
      };

      match (code, modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
          drop(raw_mode.take());
          writeln!(stdout)?;
          return Err(ReadLineError::Interrupted);
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) if self.line.is_empty() => {
          drop(raw_mode.take());
          writeln!(stdout)?;
          return Err(ReadLineError::Eof);
        }
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => self.insert_char('\n'),
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
          self.should_exit_on_interrupt.store(false, Relaxed);
        }
        (KeyCode::Enter, _) => match validate(&self.line) {
          ValidationResult::Incomplete => self.insert_char('\n'),
          ValidationResult::Invalid(message) => {
            drop(raw_mode.take());
            writeln!(stdout)?;
            if let Some(message) = message {
              writeln!(stdout, "{message}")?;
            }
            raw_mode = Some(RawMode::enable()?);
            break;
          }
          ValidationResult::Valid(_) => break,
        },
        (KeyCode::Backspace, _) => self.backspace(),
        (KeyCode::Delete, _) => self.delete(),
        (KeyCode::Left, _) => self.move_prev(),
        (KeyCode::Right, _) => self.move_next(),
        (KeyCode::Home, _) => self.cursor = 0,
        (KeyCode::End, _) => self.cursor = self.line.len(),
        (KeyCode::Up, _) => self.history_prev(),
        (KeyCode::Down, _) => self.history_next(),
        (KeyCode::Tab, _) => self.complete(&mut stdout, &mut raw_mode)?,
        (KeyCode::Char(ch), _) => self.insert_char(ch),
        _ => {}
      }

      self.redraw(&mut stdout)?;
    }

    drop(raw_mode.take());
    writeln!(stdout)?;
    Ok(self.line)
  }

  fn insert_char(&mut self, ch: char) {
    self.line.insert(self.cursor, ch);
    self.cursor += ch.len_utf8();
  }

  fn backspace(&mut self) {
    if let Some((idx, _)) = self.line[..self.cursor].char_indices().next_back()
    {
      self.line.drain(idx..self.cursor);
      self.cursor = idx;
    }
  }

  fn delete(&mut self) {
    if self.cursor < self.line.len() {
      let next = self.line[self.cursor..]
        .char_indices()
        .nth(1)
        .map(|(idx, _)| self.cursor + idx)
        .unwrap_or(self.line.len());
      self.line.drain(self.cursor..next);
    }
  }

  fn move_prev(&mut self) {
    if let Some((idx, _)) = self.line[..self.cursor].char_indices().next_back()
    {
      self.cursor = idx;
    }
  }

  fn move_next(&mut self) {
    if self.cursor < self.line.len() {
      self.cursor = self.line[self.cursor..]
        .char_indices()
        .nth(1)
        .map(|(idx, _)| self.cursor + idx)
        .unwrap_or(self.line.len());
    }
  }

  fn history_prev(&mut self) {
    if self.history.is_empty() {
      return;
    }
    let idx = self
      .history_index
      .map(|idx| idx.saturating_sub(1))
      .unwrap_or(self.history.len() - 1);
    self.history_index = Some(idx);
    self.line = self.history[idx].clone();
    self.cursor = self.line.len();
  }

  fn history_next(&mut self) {
    let Some(idx) = self.history_index else {
      return;
    };
    if idx + 1 < self.history.len() {
      self.history_index = Some(idx + 1);
      self.line = self.history[idx + 1].clone();
    } else {
      self.history_index = None;
      self.line.clear();
    }
    self.cursor = self.line.len();
  }

  fn complete(
    &mut self,
    stdout: &mut std::io::Stdout,
    raw_mode: &mut Option<RawMode>,
  ) -> Result<(), ReadLineError> {
    if self.line.is_empty()
      || self.line[..self.cursor]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_whitespace())
    {
      if cfg!(target_os = "windows") {
        for _ in 0..4 {
          self.insert_char(' ');
        }
      } else {
        self.insert_char('\t');
      }
      return Ok(());
    }

    let (start, candidates) = self.helper.complete(&self.line, self.cursor);
    if candidates.is_empty() {
      return Ok(());
    }

    let current = &self.line[start..self.cursor];
    let completion = common_completion(current, &candidates);
    if completion.len() > current.len() {
      self.line.replace_range(start..self.cursor, &completion);
      self.cursor = start + completion.len();
    } else {
      drop(raw_mode.take());
      writeln!(stdout)?;
      for candidate in candidates {
        writeln!(stdout, "{candidate}")?;
      }
      *raw_mode = Some(RawMode::enable()?);
    }
    Ok(())
  }

  fn redraw(
    &mut self,
    stdout: &mut std::io::Stdout,
  ) -> Result<(), ReadLineError> {
    let before_cursor = &self.line[..self.cursor];
    let cursor_col = visible_width_after_last_newline(before_cursor) + 2;
    let lines_before_cursor = before_cursor.matches('\n').count();
    let lines_after_cursor = self.line[self.cursor..].matches('\n').count();
    if self.rendered_cursor_line > 0 {
      crossterm::execute!(
        stdout,
        cursor::MoveUp(self.rendered_cursor_line as u16)
      )?;
    }
    crossterm::execute!(
      stdout,
      cursor::MoveToColumn(0),
      terminal::Clear(terminal::ClearType::FromCursorDown),
    )?;
    write!(stdout, "> {}", self.helper.highlight(&self.line))?;
    if lines_after_cursor > 0 {
      crossterm::execute!(stdout, cursor::MoveUp(lines_after_cursor as u16))?;
    }
    crossterm::execute!(stdout, cursor::MoveToColumn(cursor_col as u16))?;
    self.rendered_cursor_line = lines_before_cursor;
    stdout.flush()?;
    Ok(())
  }
}

fn common_completion(current: &str, candidates: &[String]) -> String {
  let Some(first) = candidates.first() else {
    return current.to_string();
  };
  let mut end = first.len();
  while !first.is_char_boundary(end) {
    end -= 1;
  }
  for candidate in candidates.iter().skip(1) {
    while end > 0 && !candidate.starts_with(&first[..end]) {
      end -= 1;
      while !first.is_char_boundary(end) {
        end -= 1;
      }
    }
  }
  first[..end].to_string()
}

fn visible_width_after_last_newline(text: &str) -> usize {
  text.rsplit('\n').next().unwrap_or(text).chars().count()
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
