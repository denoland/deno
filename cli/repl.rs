// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::global_state::GlobalState;
use crate::inspector::InspectorSession;
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
    method: String,
    params: Option<Value>,
  ) -> Result<Value, AnyError> {
    self.message_tx.send((method, params))?;
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
    if line.is_empty() {
      let response = self
        .post_message(
          "Runtime.globalLexicalScopeNames".to_string(),
          Some(json!({
            "executionContextId": self.context_id,
          })),
        )
        .unwrap();

      let candidates = response
        .get("names")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_string())
        .collect();

      return Ok((pos, candidates));
    }

    if let Some(index) = line.rfind('.') {
      let (lhs, rhs) = line.split_at(index);

      let evaluate_response = self
        .post_message(
          "Runtime.evaluate".to_string(),
          Some(json!({
              "contextId": self.context_id,
              "expression": lhs,
              "throwOnSideEffect": true,
              "timeout": 200,
          })),
        )
        .unwrap();

      if let Some(result) = evaluate_response.get("result") {
        if let Some(object_id) = result.get("objectId") {
          let get_properties_response = self
            .post_message(
              "Runtime.getProperties".to_string(),
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
              .filter(|r| r.starts_with(&rhs[1..]))
              .collect();

            return Ok((lhs.len() + 1, candidates));
          }
        }
      }
    }

    if let Some(index) = line.rfind('[') {
      let (lhs, rhs) = line.split_at(index);

      let evaluate_response = self
        .post_message(
          "Runtime.evaluate".to_string(),
          Some(json!({
              "contextId": self.context_id,
              "expression": lhs,
              "throwOnSideEffect": true,
              "timeout": 200,
          })),
        )
        .unwrap();

      if let Some(result) = evaluate_response.get("result") {
        if let Some(object_id) = result.get("objectId") {
          let get_properties_response = self
            .post_message(
              "Runtime.getProperties".to_string(),
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
              .map(|r| {
                format!(
                  "\"{}\"",
                  r.get("name").unwrap().as_str().unwrap().to_string()
                )
              })
              .filter(|r| r.starts_with(&rhs[1..]))
              .collect();

            return Ok((lhs.len() + 1, candidates));
          }
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

pub async fn run(
  global_state: &GlobalState,
  mut session: Box<InspectorSession>,
) -> Result<(), AnyError> {
  // Our inspector is unable to default to the default context id so we have to specify it here.
  let context_id: u32 = 1;

  let history_file = global_state.dir.root.join("deno_history.txt");

  session
    .post_message("Runtime.enable".to_string(), None)
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

  session
    .post_message(
      "Runtime.evaluate".to_string(),
      Some(json!({
        "expression": prelude,
        "contextId": context_id,
      })),
    )
    .await?;

  loop {
    let readline_result;
    let editor2 = editor.clone();
    let readline = tokio::task::spawn_blocking(move || {
      editor2.lock().unwrap().readline("> ")
    });

    tokio::pin!(readline);

    loop {
      for (method, params) in message_rx.try_iter() {
        response_tx.send(session.post_message(method, params).await)?;
      }

      // We use a delay to not spin at 100% idle and let let the outer loop get a chance to poll
      // the event loop.
      // TODO(caspervonb) poll the event loop directly instead.
      let delay =
        tokio::time::delay_for(tokio::time::Duration::from_millis(10));
      tokio::pin!(delay);

      tokio::select! {
        _ = &mut delay => {
        }

        result = &mut readline => {
          readline_result = result.unwrap();
          break;
        }
      }
    }

    match readline_result {
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

        let evaluate_response = session
          .post_message(
            "Runtime.evaluate".to_string(),
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
            session
              .post_message(
                "Runtime.evaluate".to_string(),
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

        let is_closing = session
          .post_message(
            "Runtime.evaluate".to_string(),
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
          session
                  .post_message(
                    "Runtime.callFunctionOn".to_string(),
                    Some(json!({
                      "executionContextId": context_id,
                      "functionDeclaration": "function (object) { Deno[Deno.internal].lastThrownError = object; }",
                      "arguments": [
                        evaluate_result,
                      ],
                    }))).await?;
        } else {
          session
                  .post_message(
                    "Runtime.callFunctionOn".to_string(),
                    Some(json!({
                      "executionContextId": context_id,
                      "functionDeclaration": "function (object) { Deno[Deno.internal].lastEvalResult = object; }",
                      "arguments": [
                        evaluate_result,
                      ],
                    }))).await?;
        }

        // TODO(caspervonb) we should investigate using previews here but to keep things
        // consistent with the previous implementation we just get the preview result from
        // Deno.inspectArgs.
        let inspect_response = session
                .post_message(
                  "Runtime.callFunctionOn".to_string(),
                  Some(json!({
                    "executionContextId": context_id,
                    "functionDeclaration": "function (object) { return Deno[Deno.internal].inspectArgs(['%o', object], { colors: true}); }",
                    "arguments": [
                      evaluate_result,
                    ],
                  }))).await?;

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
