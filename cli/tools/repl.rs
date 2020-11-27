// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::ast;
use crate::colors;
use crate::inspector::InspectorSession;
use crate::media_type::MediaType;
use crate::program_state::ProgramState;
use crate::worker::MainWorker;
use crate::worker::Worker;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::validate::ValidationContext;
use rustyline::validate::ValidationResult;
use rustyline::validate::Validator;
use rustyline::Context;
use rustyline::Editor;
use rustyline_derive::{Helper, Hinter};
use std::borrow::Cow;
use std::ops::Range;
use std::sync::mpsc::channel;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::sync::Mutex;
use swc_common::comments::{Comment, CommentKind, SingleThreadedComments};
use swc_common::input::StringInput;
use swc_common::{BytePos, Span};
use swc_ecmascript::parser::lexer::Lexer;
use swc_ecmascript::parser::token::{Token, Word};

// Provides helpers to the editor like validation for multi-line edits, completion candidates for
// tab completion.
#[derive(Helper, Hinter)]
struct Helper {
  context_id: u64,
  message_tx: SyncSender<(String, Option<Value>)>,
  response_rx: Receiver<Result<Value, AnyError>>,
  highlighter: LineHighlighter,
}

impl Helper {
  fn post_message(
    &self,
    method: &str,
    params: Option<Value>,
  ) -> Result<Value, AnyError> {
    self.message_tx.send((method.to_string(), params))?;
    self.response_rx.recv()?
  }
}

fn is_word_boundary(c: char) -> bool {
  if c == '.' {
    false
  } else {
    char::is_ascii_whitespace(&c) || char::is_ascii_punctuation(&c)
  }
}

impl Completer for Helper {
  type Candidate = String;

  fn complete(
    &self,
    line: &str,
    pos: usize,
    _ctx: &Context<'_>,
  ) -> Result<(usize, Vec<String>), ReadlineError> {
    let start = line[..pos].rfind(is_word_boundary).map_or_else(|| 0, |i| i);
    let end = line[pos..]
      .rfind(is_word_boundary)
      .map_or_else(|| pos, |i| pos + i);

    let word = &line[start..end];
    let word = word.strip_prefix(is_word_boundary).unwrap_or(word);
    let word = word.strip_suffix(is_word_boundary).unwrap_or(word);

    let fallback = format!(".{}", word);

    let (prefix, suffix) = match word.rfind('.') {
      Some(index) => word.split_at(index),
      None => ("globalThis", fallback.as_str()),
    };

    let evaluate_response = self
      .post_message(
        "Runtime.evaluate",
        Some(json!({
          "contextId": self.context_id,
          "expression": prefix,
          "throwOnSideEffect": true,
          "timeout": 200,
        })),
      )
      .unwrap();

    if evaluate_response.get("exceptionDetails").is_some() {
      let candidates = Vec::new();
      return Ok((pos, candidates));
    }

    if let Some(result) = evaluate_response.get("result") {
      if let Some(object_id) = result.get("objectId") {
        let get_properties_response = self
          .post_message(
            "Runtime.getProperties",
            Some(json!({
              "objectId": object_id,
            })),
          )
          .unwrap();

        if let Some(result) = get_properties_response.get("result") {
          let candidates = result
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.get("name").unwrap().as_str().unwrap().to_string())
            .filter(|r| r.starts_with(&suffix[1..]))
            .collect();

          return Ok((pos - (suffix.len() - 1), candidates));
        }
      }
    }

    Ok((pos, Vec::new()))
  }
}

impl Validator for Helper {
  fn validate(
    &self,
    ctx: &mut ValidationContext,
  ) -> Result<ValidationResult, ReadlineError> {
    let lexer = Lexer::new(
      ast::get_syntax(&MediaType::JavaScript),
      ast::TARGET,
      StringInput::new(
        ctx.input(),
        BytePos(0),
        BytePos(ctx.input().len() as u32),
      ),
      None,
    );

    let mut stack: Vec<Token> = Vec::new();
    let mut in_template = false;

    for item in lexer {
      match item.token {
        Token::BackQuote => in_template = !in_template,
        Token::LParen
        | Token::LBracket
        | Token::LBrace
        | Token::DollarLBrace => stack.push(item.token),
        Token::RParen | Token::RBracket | Token::RBrace => {
          match (stack.pop(), item.token) {
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
        _ => {}
      }
    }

    if !stack.is_empty() || in_template {
      return Ok(ValidationResult::Incomplete);
    }

    Ok(ValidationResult::Valid(None))
  }
}

impl Highlighter for Helper {
  fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
    hint.into()
  }

  fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
    self.highlighter.highlight(line, pos)
  }

  fn highlight_candidate<'c>(
    &self,
    candidate: &'c str,
    _completion: rustyline::CompletionType,
  ) -> Cow<'c, str> {
    self.highlighter.highlight(candidate, 0)
  }

  fn highlight_char(&self, line: &str, _: usize) -> bool {
    !line.is_empty()
  }
}

struct LineHighlighter;

impl LineHighlighter {
  fn new() -> Self {
    Self
  }
}

fn flatten_comments(
  comments: SingleThreadedComments,
) -> impl Iterator<Item = Comment> {
  let (leading, trailing) = comments.take_all();
  let mut comments = (*leading).clone().into_inner();
  comments.extend((*trailing).clone().into_inner());
  comments.into_iter().flat_map(|el| el.1)
}

impl Highlighter for LineHighlighter {
  fn highlight<'l>(&self, line: &'l str, _: usize) -> Cow<'l, str> {
    let comments = SingleThreadedComments::default();
    let lexer = Lexer::new(
      ast::get_syntax(&MediaType::JavaScript),
      ast::TARGET,
      StringInput::new(line, BytePos(0), BytePos(line.len() as u32)),
      Some(&comments),
    );

    let mut out_line = String::from(line);

    // Adding color adds more bytes to the string,
    // so an offset is needed to stop spans falling out of sync.
    let mut offset = 0;

    enum ItemKind {
      Token(Token),
      Comment(CommentKind),
    }

    struct Item {
      span: Range<usize>,
      kind: ItemKind,
    }

    fn span_to_range(span: Span) -> Range<usize> {
      span.lo.0 as usize..span.hi.0 as usize
    }

    let mut items: Vec<Item> = Vec::new();
    items.extend(lexer.map(|token| Item {
      span: span_to_range(token.span),
      kind: ItemKind::Token(token.token),
    }));
    items.extend(flatten_comments(comments).map(|comment| Item {
      span: span_to_range(comment.span),
      kind: ItemKind::Comment(comment.kind),
    }));
    items.sort_by_key(|item| item.span.start);

    for item in items {
      let before_len = out_line.len();
      out_line.replace_range(
        item.span.start + offset..item.span.end + offset,
        &match item.kind {
          ItemKind::Token(token) => match token {
            Token::Str { .. } | Token::Template { .. } | Token::BackQuote => {
              colors::green(&line[item.span]).to_string()
            }
            Token::Regex(_, _) => colors::red(&line[item.span]).to_string(),
            Token::Num(_) => colors::yellow(&line[item.span]).to_string(),
            Token::Word(word) => match word {
              Word::True | Word::False | Word::Null => {
                colors::yellow(&line[item.span]).to_string()
              }
              Word::Keyword(_) => colors::cyan(&line[item.span]).to_string(),
              Word::Ident(ident) => {
                if ident == *"undefined" {
                  colors::gray(&line[item.span]).to_string()
                } else if ident == *"Infinity" || ident == *"NaN" {
                  colors::yellow(&line[item.span]).to_string()
                } else {
                  line[item.span].to_string()
                }
              }
            },
            _ => line[item.span].to_string(),
          },
          ItemKind::Comment(_) => colors::gray(&line[item.span]).to_string(),
        },
      );
      offset += out_line.len() - before_len;
    }

    out_line.into()
  }
}

async fn post_message_and_poll(
  worker: &mut Worker,
  session: &mut InspectorSession,
  method: &str,
  params: Option<Value>,
) -> Result<Value, AnyError> {
  let response = session.post_message(method, params);
  tokio::pin!(response);

  loop {
    tokio::select! {
      result = &mut response => {
        return result
      }

      _ = worker.run_event_loop() => {
        // A zero delay is long enough to yield the thread in order to prevent the loop from
        // running hot for messages that are taking longer to resolve like for example an
        // evaluation of top level await.
        tokio::time::delay_for(tokio::time::Duration::from_millis(0)).await;
      }
    }
  }
}

async fn read_line_and_poll(
  worker: &mut Worker,
  session: &mut InspectorSession,
  message_rx: &Receiver<(String, Option<Value>)>,
  response_tx: &Sender<Result<Value, AnyError>>,
  editor: Arc<Mutex<Editor<Helper>>>,
) -> Result<String, ReadlineError> {
  let mut line =
    tokio::task::spawn_blocking(move || editor.lock().unwrap().readline("> "));

  let mut poll_worker = true;

  loop {
    for (method, params) in message_rx.try_iter() {
      response_tx
        .send(session.post_message(&method, params).await)
        .unwrap();
    }

    // Because an inspector websocket client may choose to connect at anytime when we have an
    // inspector server we need to keep polling the worker to pick up new connections.
    let mut timeout =
      tokio::time::delay_for(tokio::time::Duration::from_millis(100));

    tokio::select! {
      result = &mut line => {
        return result.unwrap();
      }
      _ = worker.run_event_loop(), if poll_worker => {
        poll_worker = false;
      }
      _ = &mut timeout => {
        poll_worker = true
      }
    }
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

async fn inject_prelude(
  worker: &mut MainWorker,
  session: &mut InspectorSession,
  context_id: u64,
) -> Result<(), AnyError> {
  post_message_and_poll(
    worker,
    session,
    "Runtime.evaluate",
    Some(json!({
      "expression": PRELUDE,
      "contextId": context_id,
    })),
  )
  .await?;

  Ok(())
}

pub async fn is_closing(
  worker: &mut MainWorker,
  session: &mut InspectorSession,
  context_id: u64,
) -> Result<bool, AnyError> {
  let closed = post_message_and_poll(
    worker,
    session,
    "Runtime.evaluate",
    Some(json!({
      "expression": "(globalThis.closed)",
      "contextId": context_id,
    })),
  )
  .await?
  .get("result")
  .unwrap()
  .get("value")
  .unwrap()
  .as_bool()
  .unwrap();

  Ok(closed)
}

pub async fn run(
  program_state: &ProgramState,
  mut worker: MainWorker,
) -> Result<(), AnyError> {
  let mut session = worker.create_inspector_session();

  let history_file = program_state.dir.root.join("deno_history.txt");

  post_message_and_poll(&mut *worker, &mut session, "Runtime.enable", None)
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

  let (message_tx, message_rx) = sync_channel(1);
  let (response_tx, response_rx) = channel();

  let helper = Helper {
    context_id,
    message_tx,
    response_rx,
    highlighter: LineHighlighter::new(),
  };

  let editor = Arc::new(Mutex::new(Editor::new()));

  editor.lock().unwrap().set_helper(Some(helper));

  editor
    .lock()
    .unwrap()
    .load_history(history_file.to_str().unwrap())
    .unwrap_or(());

  println!("Deno {}", crate::version::DENO);
  println!("exit using ctrl+d or close()");

  inject_prelude(&mut worker, &mut session, context_id).await?;

  while !is_closing(&mut worker, &mut session, context_id).await? {
    let line = read_line_and_poll(
      &mut *worker,
      &mut session,
      &message_rx,
      &response_tx,
      editor.clone(),
    )
    .await;
    match line {
      Ok(line) => {
        // It is a bit unexpected that { "foo": "bar" } is interpreted as a block
        // statement rather than an object literal so we interpret it as an expression statement
        // to match the behavior found in a typical prompt including browser developer tools.
        let wrapped_line = if line.trim_start().starts_with('{')
          && !line.trim_end().ends_with(';')
        {
          format!("({})", &line)
        } else {
          line.clone()
        };

        let evaluate_response = post_message_and_poll(
          &mut *worker,
          &mut session,
          "Runtime.evaluate",
          Some(json!({
            "expression": format!("'use strict'; void 0;\n{}", &wrapped_line),
            "contextId": context_id,
            "replMode": true,
          })),
        )
        .await?;

        // If that fails, we retry it without wrapping in parens letting the error bubble up to the
        // user if it is still an error.
        let evaluate_response =
          if evaluate_response.get("exceptionDetails").is_some()
            && wrapped_line != line
          {
            post_message_and_poll(
              &mut *worker,
              &mut session,
              "Runtime.evaluate",
              Some(json!({
                "expression": format!("'use strict'; void 0;\n{}", &line),
                "contextId": context_id,
                "replMode": true,
              })),
            )
            .await?
          } else {
            evaluate_response
          };

        let evaluate_result = evaluate_response.get("result").unwrap();
        let evaluate_exception_details =
          evaluate_response.get("exceptionDetails");

        if evaluate_exception_details.is_some() {
          post_message_and_poll(
                    &mut *worker,
                    &mut session,
                    "Runtime.callFunctionOn",
                    Some(json!({
                      "executionContextId": context_id,
                      "functionDeclaration": "function (object) { Deno[Deno.internal].lastThrownError = object; }",
                      "arguments": [
                        evaluate_result,
                      ],
                    })),
                  ).await?;
        } else {
          post_message_and_poll(
                    &mut *worker,
                    &mut session,
                    "Runtime.callFunctionOn",
                    Some(json!({
                      "executionContextId": context_id,
                      "functionDeclaration": "function (object) { Deno[Deno.internal].lastEvalResult = object; }",
                      "arguments": [
                        evaluate_result,
                      ],
                    })),
                  ).await?;
        }

        // TODO(caspervonb) we should investigate using previews here but to keep things
        // consistent with the previous implementation we just get the preview result from
        // Deno.inspectArgs.
        let inspect_response =
          post_message_and_poll(
            &mut *worker,
            &mut session,
            "Runtime.callFunctionOn",
            Some(json!({
              "executionContextId": context_id,
              "functionDeclaration": "function (object) { return Deno[Deno.internal].inspectArgs(['%o', object], { colors: !Deno.noColor }); }",
              "arguments": [
                evaluate_result,
              ],
            })),
          ).await?;

        let inspect_result = inspect_response.get("result").unwrap();

        let value = inspect_result.get("value").unwrap().as_str().unwrap();
        let output = match evaluate_exception_details {
          Some(_) => format!("Uncaught {}", value),
          None => value.to_string(),
        };

        println!("{}", output);

        editor.lock().unwrap().add_history_entry(line.as_str());
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

  std::fs::create_dir_all(history_file.parent().unwrap())?;
  editor
    .lock()
    .unwrap()
    .save_history(history_file.to_str().unwrap())?;

  Ok(())
}
