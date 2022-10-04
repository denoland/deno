// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::cdp;
use crate::colors;
use deno_ast::swc::parser::error::SyntaxError;
use deno_ast::swc::parser::token::Token;
use deno_ast::swc::parser::token::Word;
use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::validate::ValidationContext;
use rustyline::validate::ValidationResult;
use rustyline::validate::Validator;
use rustyline::Cmd;
use rustyline::CompletionType;
use rustyline::Config;
use rustyline::Context;
use rustyline::Editor;
use rustyline::EventHandler;
use rustyline::KeyCode;
use rustyline::KeyEvent;
use rustyline::Modifiers;
use rustyline::{ConditionalEventHandler, Event, EventContext, RepeatCount};
use rustyline_derive::{Helper, Hinter};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use super::channel::RustylineSyncMessageSender;

// Provides helpers to the editor like validation for multi-line edits, completion candidates for
// tab completion.
#[derive(Helper, Hinter)]
pub struct EditorHelper {
  pub context_id: u64,
  pub sync_sender: RustylineSyncMessageSender,
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
          non_indexed_properties_only: None,
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
  if c == '.' {
    false
  } else {
    char::is_ascii_whitespace(&c) || char::is_ascii_punctuation(&c)
  }
}

fn get_expr_from_line_at_pos(line: &str, cursor_pos: usize) -> &str {
  let start = line[..cursor_pos]
    .rfind(is_word_boundary)
    .map_or_else(|| 0, |i| i);
  let end = line[cursor_pos..]
    .rfind(is_word_boundary)
    .map_or_else(|| cursor_pos, |i| cursor_pos + i);

  let word = &line[start..end];
  let word = word.strip_prefix(is_word_boundary).unwrap_or(word);
  let word = word.strip_suffix(is_word_boundary).unwrap_or(word);

  word
}

impl Completer for EditorHelper {
  type Candidate = String;

  fn complete(
    &self,
    line: &str,
    pos: usize,
    _ctx: &Context<'_>,
  ) -> Result<(usize, Vec<String>), ReadlineError> {
    let lsp_completions = self.sync_sender.lsp_completions(line, pos);
    if !lsp_completions.is_empty() {
      // assumes all lsp completions have the same start position
      return Ok((
        lsp_completions[0].range.start,
        lsp_completions.into_iter().map(|c| c.new_text).collect(),
      ));
    }

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

      Ok((pos - prop_name.len(), candidates))
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

      Ok((pos - expr.len(), candidates))
    }
  }
}

impl Validator for EditorHelper {
  fn validate(
    &self,
    ctx: &mut ValidationContext,
  ) -> Result<ValidationResult, ReadlineError> {
    let mut stack: Vec<Token> = Vec::new();
    let mut in_template = false;

    for item in deno_ast::lex(ctx.input(), deno_ast::MediaType::TypeScript) {
      if let deno_ast::TokenOrComment::Token(token) = item.inner {
        match token {
          Token::BackQuote => in_template = !in_template,
          Token::LParen
          | Token::LBracket
          | Token::LBrace
          | Token::DollarLBrace => stack.push(token),
          Token::RParen | Token::RBracket | Token::RBrace => {
            match (stack.pop(), token) {
              (Some(Token::LParen), Token::RParen)
              | (Some(Token::LBracket), Token::RBracket)
              | (Some(Token::LBrace), Token::RBrace)
              | (Some(Token::DollarLBrace), Token::RBrace) => {}
              (Some(left), _) => {
                return Ok(ValidationResult::Invalid(Some(format!(
                  "Mismatched pairs: {:?} is not properly closed",
                  left
                ))))
              }
              (None, _) => {
                // While technically invalid when unpaired, it should be V8's task to output error instead.
                // Thus marked as valid with no info.
                return Ok(ValidationResult::Valid(None));
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
                return Ok(ValidationResult::Valid(None));
              }
            }
          }
          _ => {}
        }
      }
    }

    if !stack.is_empty() || in_template {
      return Ok(ValidationResult::Incomplete);
    }

    Ok(ValidationResult::Valid(None))
  }
}

impl Highlighter for EditorHelper {
  fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
    hint.into()
  }

  fn highlight_candidate<'c>(
    &self,
    candidate: &'c str,
    completion: rustyline::CompletionType,
  ) -> Cow<'c, str> {
    if completion == CompletionType::List {
      candidate.into()
    } else {
      self.highlight(candidate, 0)
    }
  }

  fn highlight_char(&self, line: &str, _: usize) -> bool {
    !line.is_empty()
  }

  fn highlight<'l>(&self, line: &'l str, _: usize) -> Cow<'l, str> {
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
                if ident == *"undefined" {
                  colors::gray(&line[range]).to_string()
                } else if ident == *"Infinity" || ident == *"NaN" {
                  colors::yellow(&line[range]).to_string()
                } else if ident == *"async" || ident == *"of" {
                  colors::cyan(&line[range]).to_string()
                } else {
                  let next = lexed_items.peek().map(|item| &item.inner);
                  if matches!(
                    next,
                    Some(deno_ast::TokenOrComment::Token(Token::LParen))
                  ) {
                    // We're looking for something that looks like a function
                    // We use a simple heuristic: 'ident' followed by 'LParen'
                    colors::intense_blue(&line[range]).to_string()
                  } else {
                    line[range].to_string()
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

#[derive(Clone)]
pub struct ReplEditor {
  inner: Arc<Mutex<Editor<EditorHelper>>>,
  history_file_path: PathBuf,
}

impl ReplEditor {
  pub fn new(helper: EditorHelper, history_file_path: PathBuf) -> Self {
    let editor_config = Config::builder()
      .completion_type(CompletionType::List)
      .build();

    let mut editor =
      Editor::with_config(editor_config).expect("Failed to create editor.");
    editor.set_helper(Some(helper));
    editor.load_history(&history_file_path).unwrap_or(());
    editor.bind_sequence(
      KeyEvent(KeyCode::Char('s'), Modifiers::CTRL),
      EventHandler::Simple(Cmd::Newline),
    );
    editor.bind_sequence(
      KeyEvent(KeyCode::Tab, Modifiers::NONE),
      EventHandler::Conditional(Box::new(TabEventHandler)),
    );

    ReplEditor {
      inner: Arc::new(Mutex::new(editor)),
      history_file_path,
    }
  }

  pub fn readline(&self) -> Result<String, ReadlineError> {
    self.inner.lock().readline("> ")
  }

  pub fn add_history_entry(&self, entry: String) {
    self.inner.lock().add_history_entry(entry);
  }

  pub fn save_history(&self) -> Result<(), AnyError> {
    std::fs::create_dir_all(self.history_file_path.parent().unwrap())?;

    self.inner.lock().save_history(&self.history_file_path)?;
    Ok(())
  }
}

/// A custom tab key event handler
/// It uses a heuristic to determine if the user is requesting completion or if they want to insert an actual tab
/// The heuristic goes like this:
///   - If the last character before the cursor is whitespace, the the user wants to insert a tab
///   - Else the user is requesting completion
struct TabEventHandler;
impl ConditionalEventHandler for TabEventHandler {
  fn handle(
    &self,
    evt: &Event,
    n: RepeatCount,
    _: bool,
    ctx: &EventContext,
  ) -> Option<Cmd> {
    debug_assert_eq!(
      *evt,
      Event::from(KeyEvent(KeyCode::Tab, Modifiers::NONE))
    );
    if ctx.line().is_empty()
      || ctx.line()[..ctx.pos()]
        .chars()
        .rev()
        .next()
        .filter(|c| c.is_whitespace())
        .is_some()
    {
      if cfg!(target_os = "windows") {
        // Inserting a tab is broken in windows with rustyline
        // use 4 spaces as a workaround for now
        Some(Cmd::Insert(n, "    ".into()))
      } else {
        Some(Cmd::Insert(n, "\t".into()))
      }
    } else {
      None // default complete
    }
  }
}
