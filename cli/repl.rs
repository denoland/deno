// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::colors;
use crate::inspector::InspectorSession;
use crate::program_state::ProgramState;
use crate::worker::MainWorker;
use crate::worker::Worker;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use regex::Captures;
use regex::Regex;
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
use std::sync::mpsc::channel;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::sync::Mutex;

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
    let mut stack: Vec<char> = Vec::new();
    let mut literal: Option<char> = None;
    let mut escape: bool = false;

    for c in ctx.input().chars() {
      if escape {
        escape = false;
        continue;
      }

      if c == '\\' {
        escape = true;
        continue;
      }

      if let Some(v) = literal {
        if c == v {
          literal = None
        }

        continue;
      } else {
        literal = match c {
          '`' | '"' | '/' | '\'' => Some(c),
          _ => None,
        };
      }

      match c {
        '(' | '[' | '{' => stack.push(c),
        ')' | ']' | '}' => match (stack.pop(), c) {
          (Some('('), ')') | (Some('['), ']') | (Some('{'), '}') => {}
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
        },
        _ => {}
      }
    }

    if !stack.is_empty() || literal == Some('`') {
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

struct LineHighlighter {
  regex: Regex,
}

impl LineHighlighter {
  fn new() -> Self {
    let regex = Regex::new(
      r#"(?x)
      (?P<comment>(?:/\*[\s\S]*?\*/|//[^\n]*)) |
      (?P<string>(?:"([^"\\]|\\.)*"|'([^'\\]|\\.)*'|`([^`\\]|\\.)*`)) |
      (?P<regexp>/(?:(?:\\/|[^\n/]))*?/[gimsuy]*) |
      (?P<number>\b\d+(?:\.\d+)?(?:e[+-]?\d+)*n?\b) |
      (?P<infinity>\b(?:Infinity|NaN)\b) |
      (?P<hexnumber>\b0x[a-fA-F0-9]+\b) |
      (?P<octalnumber>\b0o[0-7]+\b) |
      (?P<binarynumber>\b0b[01]+\b) |
      (?P<boolean>\b(?:true|false)\b) |
      (?P<null>\b(?:null)\b) |
      (?P<undefined>\b(?:undefined)\b) |
      (?P<keyword>\b(?:await|async|var|let|for|if|else|in|of|class|const|function|yield|return|with|case|break|switch|import|export|new|while|do|throw|catch|this)\b) |
      "#,
      )
      .unwrap();

    Self { regex }
  }
}

impl Highlighter for LineHighlighter {
  fn highlight<'l>(&self, line: &'l str, _: usize) -> Cow<'l, str> {
    self
      .regex
      .replace_all(&line.to_string(), |caps: &Captures<'_>| {
        if let Some(cap) = caps.name("comment") {
          colors::gray(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("string") {
          colors::green(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("regexp") {
          colors::red(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("number") {
          colors::yellow(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("boolean") {
          colors::yellow(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("null") {
          colors::yellow(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("undefined") {
          colors::gray(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("keyword") {
          colors::cyan(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("infinity") {
          colors::yellow(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("classes") {
          colors::green_bold(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("hexnumber") {
          colors::yellow(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("octalnumber") {
          colors::yellow(cap.as_str()).to_string()
        } else if let Some(cap) = caps.name("binarynumber") {
          colors::yellow(cap.as_str()).to_string()
        } else {
          caps[0].to_string()
        }
      })
      .to_string()
      .into()
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
