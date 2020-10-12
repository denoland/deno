// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::global_state::GlobalState;
use crate::inspector::InspectorSession;
use crate::worker::MainWorker;
use crate::worker::Worker;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use rustyline::completion::Completer;
use rustyline::error::ReadlineError;
use rustyline::validate::MatchingBracketValidator;
use rustyline::validate::ValidationContext;
use rustyline::validate::ValidationResult;
use rustyline::validate::Validator;
use rustyline::Context;
use rustyline::Editor;
use rustyline_derive::{Helper, Highlighter, Hinter};
use std::sync::mpsc::channel;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::sync::Mutex;

// Provides syntax specific helpers to the editor like validation for multi-line edits.
#[derive(Helper, Highlighter, Hinter)]
struct Helper {
  context_id: u32,
  message_tx: SyncSender<(String, Option<Value>)>,
  response_rx: Receiver<Result<Value, AnyError>>,
  validator: MatchingBracketValidator,
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

impl Completer for Helper {
  type Candidate = String;

  fn complete(
    &self,
    line: &str,
    pos: usize,
    _ctx: &Context<'_>,
  ) -> Result<(usize, Vec<String>), ReadlineError> {
    let start_slice = &line[..pos];
    let start_offset = start_slice
      .rfind(|c| c == ' ' || c == '\n' || c == '{' || c == '(')
      .map_or(0, |i| i + 1);

    let end_slice = &line[pos..];
    let end_offset = end_slice
      .rfind(|c| c == ' ' || c == '\n' || c == '}' || c == ')')
      .map_or_else(|| line.len(), |i| pos + i - 1);

    let slice = &line[start_offset..end_offset];
    let mut parts: Vec<&str> =
      slice.rsplitn(2, |c| c == '.' || c == '[').collect();

    if parts.len() == 1 {
      parts.push("(globalThis)");
    }

    let evaluate_response = self
      .post_message(
        "Runtime.evaluate",
        Some(json!({
          "contextId": self.context_id,
          "expression": parts[1],
          "throwOnSideEffect": true,
          "timeout": 200,
        })),
      )
      .unwrap();

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
            .filter(|r| r.starts_with(&parts[0]))
            .collect();

          return Ok((pos - (parts[0].len()), candidates));
        }
      }
    }

    let candidates = Vec::new();
    Ok((pos, candidates))
  }
}

impl Validator for Helper {
  fn validate(
    &self,
    ctx: &mut ValidationContext,
  ) -> Result<ValidationResult, ReadlineError> {
    self.validator.validate(ctx)
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

pub async fn run(
  global_state: &GlobalState,
  mut worker: MainWorker,
) -> Result<(), AnyError> {
  // Our inspector is unable to default to the default context id so we have to specify it here.
  let context_id: u32 = 1;

  let mut session = worker.create_inspector_session();

  let history_file = global_state.dir.root.join("deno_history.txt");

  post_message_and_poll(&mut *worker, &mut session, "Runtime.enable", None)
    .await?;

  let (message_tx, message_rx) = sync_channel(1);
  let (response_tx, response_rx) = channel();

  let helper = Helper {
    context_id,
    message_tx,
    response_rx,
    validator: MatchingBracketValidator::new(),
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

  let prelude = r#"
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

  post_message_and_poll(
    &mut *worker,
    &mut session,
    "Runtime.evaluate",
    Some(json!({
      "expression": prelude,
      "contextId": context_id,
    })),
  )
  .await?;

  loop {
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

        let is_closing = post_message_and_poll(
          &mut *worker,
          &mut session,
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

        if is_closing {
          break;
        }

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
                      "functionDeclaration": "function (object) { return Deno[Deno.internal].inspectArgs(['%o', object], { colors: true}); }",
                      "arguments": [
                        evaluate_result,
                      ],
                    })),
                  ).await?;

        let inspect_result = inspect_response.get("result").unwrap();

        match evaluate_exception_details {
          Some(_) => eprintln!(
            "Uncaught {}",
            inspect_result.get("value").unwrap().as_str().unwrap()
          ),
          None => println!(
            "{}",
            inspect_result.get("value").unwrap().as_str().unwrap()
          ),
        }

        editor.lock().unwrap().add_history_entry(line.as_str());
      }
      Err(ReadlineError::Interrupted) => {
        break;
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
