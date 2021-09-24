// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::ast::transpile;
use crate::ast::ImportsNotUsedAsValues;
use crate::colors;
use crate::proc_state::ProcState;
use deno_ast::swc::parser::error::SyntaxError;
use deno_ast::swc::parser::token::{Token, Word};
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::LocalInspectorSession;
use deno_runtime::worker::MainWorker;
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::validate::ValidationContext;
use rustyline::validate::ValidationResult;
use rustyline::validate::Validator;
use rustyline::CompletionType;
use rustyline::Config;
use rustyline::Context;
use rustyline::Editor;
use rustyline_derive::{Helper, Hinter};
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

// Provides helpers to the editor like validation for multi-line edits, completion candidates for
// tab completion.
#[derive(Helper, Hinter)]
struct EditorHelper {
  context_id: u64,
  message_tx: Sender<(String, Option<Value>)>,
  response_rx: RefCell<UnboundedReceiver<Result<Value, AnyError>>>,
}

impl EditorHelper {
  pub fn get_global_lexical_scope_names(&self) -> Vec<String> {
    let evaluate_response = self
      .post_message(
        "Runtime.globalLexicalScopeNames",
        Some(json!({
          "executionContextId": self.context_id,
        })),
      )
      .unwrap();

    evaluate_response
      .get("names")
      .unwrap()
      .as_array()
      .unwrap()
      .iter()
      .map(|n| n.as_str().unwrap().to_string())
      .collect()
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
      .unwrap_or_else(Vec::new)
  }

  fn get_expression_type(&self, expr: &str) -> Option<String> {
    self
      .evaluate_expression(expr)?
      .get("result")?
      .get("type")?
      .as_str()
      .map(|s| s.to_string())
  }

  fn get_object_expr_properties(
    &self,
    object_expr: &str,
  ) -> Option<Vec<String>> {
    let evaluate_result = self.evaluate_expression(object_expr)?;
    let object_id = evaluate_result.get("result")?.get("objectId")?;

    let get_properties_response = self
      .post_message(
        "Runtime.getProperties",
        Some(json!({
          "objectId": object_id,
        })),
      )
      .ok()?;

    Some(
      get_properties_response
        .get("result")?
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.get("name").unwrap().as_str().unwrap().to_string())
        .collect(),
    )
  }

  fn evaluate_expression(&self, expr: &str) -> Option<Value> {
    let evaluate_response = self
      .post_message(
        "Runtime.evaluate",
        Some(json!({
          "contextId": self.context_id,
          "expression": expr,
          "throwOnSideEffect": true,
          "timeout": 200,
        })),
      )
      .ok()?;

    if evaluate_response.get("exceptionDetails").is_some() {
      None
    } else {
      Some(evaluate_response)
    }
  }

  fn post_message(
    &self,
    method: &str,
    params: Option<Value>,
  ) -> Result<Value, AnyError> {
    self
      .message_tx
      .blocking_send((method.to_string(), params))?;
    self.response_rx.borrow_mut().blocking_recv().unwrap()
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

    for item in deno_ast::lex(line, deno_ast::MediaType::TypeScript) {
      // Adding color adds more bytes to the string,
      // so an offset is needed to stop spans falling out of sync.
      let offset = out_line.len() - line.len();
      let span = std::ops::Range {
        start: item.span.lo.0 as usize,
        end: item.span.hi.0 as usize,
      };

      out_line.replace_range(
        span.start + offset..span.end + offset,
        &match item.inner {
          deno_ast::TokenOrComment::Token(token) => match token {
            Token::Str { .. } | Token::Template { .. } | Token::BackQuote => {
              colors::green(&line[span]).to_string()
            }
            Token::Regex(_, _) => colors::red(&line[span]).to_string(),
            Token::Num(_) | Token::BigInt(_) => {
              colors::yellow(&line[span]).to_string()
            }
            Token::Word(word) => match word {
              Word::True | Word::False | Word::Null => {
                colors::yellow(&line[span]).to_string()
              }
              Word::Keyword(_) => colors::cyan(&line[span]).to_string(),
              Word::Ident(ident) => {
                if ident == *"undefined" {
                  colors::gray(&line[span]).to_string()
                } else if ident == *"Infinity" || ident == *"NaN" {
                  colors::yellow(&line[span]).to_string()
                } else if ident == *"async" || ident == *"of" {
                  colors::cyan(&line[span]).to_string()
                } else {
                  line[span].to_string()
                }
              }
            },
            _ => line[span].to_string(),
          },
          deno_ast::TokenOrComment::Comment { .. } => {
            colors::gray(&line[span]).to_string()
          }
        },
      );
    }

    out_line.into()
  }
}

#[derive(Clone)]
struct ReplEditor {
  inner: Arc<Mutex<Editor<EditorHelper>>>,
  history_file_path: PathBuf,
}

impl ReplEditor {
  pub fn new(helper: EditorHelper, history_file_path: PathBuf) -> Self {
    let editor_config = Config::builder()
      .completion_type(CompletionType::List)
      .build();

    let mut editor = Editor::with_config(editor_config);
    editor.set_helper(Some(helper));
    editor.load_history(&history_file_path).unwrap_or(());

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

static PRELUDE: &str = r#"
Object.defineProperty(globalThis, "_", {
  configurable: true,
  get: () => Deno[Deno.internal].lastEvalResult,
  set: (value) => {
   Object.defineProperty(globalThis, "_", {
     value: value,
     writable: true,
     enumerable: true,
     configurable: true,
   });
   console.log("Last evaluation result is no longer saved to _.");
  },
});

Object.defineProperty(globalThis, "_error", {
  configurable: true,
  get: () => Deno[Deno.internal].lastThrownError,
  set: (value) => {
   Object.defineProperty(globalThis, "_error", {
     value: value,
     writable: true,
     enumerable: true,
     configurable: true,
   });

   console.log("Last thrown error is no longer saved to _error.");
  },
});
"#;

enum EvaluationOutput {
  Value(String),
  Error(String),
}

impl std::fmt::Display for EvaluationOutput {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      EvaluationOutput::Value(value) => f.write_str(value),
      EvaluationOutput::Error(value) => f.write_str(value),
    }
  }
}

struct ReplSession {
  worker: MainWorker,
  session: LocalInspectorSession,
  pub context_id: u64,
}

impl ReplSession {
  pub async fn initialize(mut worker: MainWorker) -> Result<Self, AnyError> {
    let mut session = worker.create_inspector_session().await;

    worker
      .with_event_loop(
        session.post_message("Runtime.enable", None).boxed_local(),
      )
      .await?;

    // Enabling the runtime domain will always send trigger one executionContextCreated for each
    // context the inspector knows about so we grab the execution context from that since
    // our inspector does not support a default context (0 is an invalid context id).
    let mut context_id: u64 = 0;
    for notification in session.notifications() {
      let method = notification.get("method").unwrap().as_str().unwrap();
      let params = notification.get("params").unwrap();

      if method == "Runtime.executionContextCreated" {
        context_id = params
          .get("context")
          .unwrap()
          .get("id")
          .unwrap()
          .as_u64()
          .unwrap();
      }
    }

    let mut repl_session = ReplSession {
      worker,
      session,
      context_id,
    };

    // inject prelude
    repl_session.evaluate_expression(PRELUDE).await?;

    Ok(repl_session)
  }

  pub async fn is_closing(&mut self) -> Result<bool, AnyError> {
    let closed = self
      .evaluate_expression("(globalThis.closed)")
      .await?
      .get("result")
      .unwrap()
      .get("value")
      .unwrap()
      .as_bool()
      .unwrap();

    Ok(closed)
  }

  pub async fn post_message_with_event_loop(
    &mut self,
    method: &str,
    params: Option<Value>,
  ) -> Result<Value, AnyError> {
    self
      .worker
      .with_event_loop(self.session.post_message(method, params).boxed_local())
      .await
  }

  pub async fn run_event_loop(&mut self) -> Result<(), AnyError> {
    self.worker.run_event_loop(true).await
  }

  pub async fn evaluate_line_and_get_output(
    &mut self,
    line: &str,
  ) -> Result<EvaluationOutput, AnyError> {
    match self.evaluate_line_with_object_wrapping(line).await {
      Ok(evaluate_response) => {
        let evaluate_result = evaluate_response.get("result").unwrap();
        let evaluate_exception_details =
          evaluate_response.get("exceptionDetails");

        if evaluate_exception_details.is_some() {
          self.set_last_thrown_error(evaluate_result).await?;
        } else {
          self.set_last_eval_result(evaluate_result).await?;
        }

        let value = self.get_eval_value(evaluate_result).await?;
        Ok(match evaluate_exception_details {
          Some(_) => EvaluationOutput::Error(format!("Uncaught {}", value)),
          None => EvaluationOutput::Value(value),
        })
      }
      Err(err) => {
        // handle a parsing diagnostic
        match err.downcast_ref::<deno_ast::Diagnostic>() {
          Some(diagnostic) => Ok(EvaluationOutput::Error(format!(
            "{}: {} at {}:{}",
            colors::red("parse error"),
            diagnostic.message,
            diagnostic.display_position.line_number,
            diagnostic.display_position.column_number,
          ))),
          None => Err(err),
        }
      }
    }
  }

  async fn evaluate_line_with_object_wrapping(
    &mut self,
    line: &str,
  ) -> Result<Value, AnyError> {
    // It is a bit unexpected that { "foo": "bar" } is interpreted as a block
    // statement rather than an object literal so we interpret it as an expression statement
    // to match the behavior found in a typical prompt including browser developer tools.
    let wrapped_line = if line.trim_start().starts_with('{')
      && !line.trim_end().ends_with(';')
    {
      format!("({})", &line)
    } else {
      line.to_string()
    };

    let evaluate_response = self.evaluate_ts_expression(&wrapped_line).await?;

    // If that fails, we retry it without wrapping in parens letting the error bubble up to the
    // user if it is still an error.
    let evaluate_response =
      if evaluate_response.get("exceptionDetails").is_some()
        && wrapped_line != line
      {
        self.evaluate_ts_expression(line).await?
      } else {
        evaluate_response
      };

    Ok(evaluate_response)
  }

  async fn set_last_thrown_error(
    &mut self,
    error: &Value,
  ) -> Result<(), AnyError> {
    self.post_message_with_event_loop(
      "Runtime.callFunctionOn",
      Some(json!({
        "executionContextId": self.context_id,
        "functionDeclaration": "function (object) { Deno[Deno.internal].lastThrownError = object; }",
        "arguments": [
          error,
        ],
      })),
    ).await?;
    Ok(())
  }

  async fn set_last_eval_result(
    &mut self,
    evaluate_result: &Value,
  ) -> Result<(), AnyError> {
    self.post_message_with_event_loop(
      "Runtime.callFunctionOn",
      Some(json!({
        "executionContextId": self.context_id,
        "functionDeclaration": "function (object) { Deno[Deno.internal].lastEvalResult = object; }",
        "arguments": [
          evaluate_result,
        ],
      })),
    ).await?;
    Ok(())
  }

  pub async fn get_eval_value(
    &mut self,
    evaluate_result: &Value,
  ) -> Result<String, AnyError> {
    // TODO(caspervonb) we should investigate using previews here but to keep things
    // consistent with the previous implementation we just get the preview result from
    // Deno.inspectArgs.
    let inspect_response = self.post_message_with_event_loop(
      "Runtime.callFunctionOn",
      Some(json!({
        "executionContextId": self.context_id,
        "functionDeclaration": r#"function (object) {
          try {
            return Deno[Deno.internal].inspectArgs(["%o", object], { colors: !Deno.noColor });
          } catch (err) {
            return Deno[Deno.internal].inspectArgs(["%o", err]);
          }
        }"#,
        "arguments": [
          evaluate_result,
        ],
      })),
    ).await?;

    let inspect_result = inspect_response.get("result").unwrap();
    let value = inspect_result.get("value").unwrap().as_str().unwrap();

    Ok(value.to_string())
  }

  async fn evaluate_ts_expression(
    &mut self,
    expression: &str,
  ) -> Result<Value, AnyError> {
    let parsed_module = deno_ast::parse_module(deno_ast::ParseParams {
      specifier: "repl.ts".to_string(),
      source: deno_ast::SourceTextInfo::from_string(expression.to_string()),
      media_type: deno_ast::MediaType::TypeScript,
      capture_tokens: false,
      maybe_syntax: None,
    })?;

    let transpiled_src = transpile(
      &parsed_module,
      &crate::ast::EmitOptions {
        emit_metadata: false,
        source_map: false,
        inline_source_map: false,
        inline_sources: false,
        imports_not_used_as_values: ImportsNotUsedAsValues::Preserve,
        // JSX is not supported in the REPL
        transform_jsx: false,
        jsx_factory: "React.createElement".into(),
        jsx_fragment_factory: "React.Fragment".into(),
        repl_imports: true,
      },
    )?
    .0;

    self
      .evaluate_expression(&format!(
        "'use strict'; void 0;\n{}",
        transpiled_src
      ))
      .await
  }

  async fn evaluate_expression(
    &mut self,
    expression: &str,
  ) -> Result<Value, AnyError> {
    self
      .post_message_with_event_loop(
        "Runtime.evaluate",
        Some(json!({
          "expression": expression,
          "contextId": self.context_id,
          "replMode": true,
        })),
      )
      .await
  }
}

async fn read_line_and_poll(
  repl_session: &mut ReplSession,
  message_rx: &mut Receiver<(String, Option<Value>)>,
  response_tx: &UnboundedSender<Result<Value, AnyError>>,
  editor: ReplEditor,
) -> Result<String, ReadlineError> {
  let mut line_fut = tokio::task::spawn_blocking(move || editor.readline());
  let mut poll_worker = true;

  loop {
    tokio::select! {
      result = &mut line_fut => {
        return result.unwrap();
      }
      result = message_rx.recv() => {
        if let Some((method, params)) = result {
          let result = repl_session
            .post_message_with_event_loop(&method, params)
            .await;
          response_tx.send(result).unwrap();
        }

        poll_worker = true;
      },
      _ = repl_session.run_event_loop(), if poll_worker => {
        poll_worker = false;
      }
    }
  }
}

pub async fn run(
  ps: &ProcState,
  worker: MainWorker,
  maybe_eval: Option<String>,
) -> Result<(), AnyError> {
  let mut repl_session = ReplSession::initialize(worker).await?;
  let (message_tx, mut message_rx) = channel(1);
  let (response_tx, response_rx) = unbounded_channel();

  let helper = EditorHelper {
    context_id: repl_session.context_id,
    message_tx,
    response_rx: RefCell::new(response_rx),
  };

  let history_file_path = ps.dir.root.join("deno_history.txt");
  let editor = ReplEditor::new(helper, history_file_path);

  if let Some(eval) = maybe_eval {
    let output = repl_session.evaluate_line_and_get_output(&eval).await?;
    // only output errors
    if let EvaluationOutput::Error(error_text) = output {
      println!("error in --eval flag. {}", error_text);
    }
  }

  println!("Deno {}", crate::version::deno());
  println!("exit using ctrl+d or close()");

  loop {
    let line = read_line_and_poll(
      &mut repl_session,
      &mut message_rx,
      &response_tx,
      editor.clone(),
    )
    .await;
    match line {
      Ok(line) => {
        let output = repl_session.evaluate_line_and_get_output(&line).await?;

        // We check for close and break here instead of making it a loop condition to get
        // consistent behavior in when the user evaluates a call to close().
        if repl_session.is_closing().await? {
          break;
        }

        println!("{}", output);

        editor.add_history_entry(line);
      }
      Err(ReadlineError::Interrupted) => {
        println!("exit using ctrl+d or close()");
        continue;
      }
      Err(ReadlineError::Eof) => {
        break;
      }
      Err(err) => {
        println!("Error: {:?}", err);
        break;
      }
    }
  }

  editor.save_history()?;

  Ok(())
}
