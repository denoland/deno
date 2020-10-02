// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::global_state::GlobalState;
use crate::inspector::InspectorSession;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use rustyline::error::ReadlineError;
use rustyline::validate::MatchingBracketValidator;
use rustyline::validate::ValidationContext;
use rustyline::validate::ValidationResult;
use rustyline::validate::Validator;
use rustyline::Editor;
use rustyline_derive::{Completer, Helper, Highlighter, Hinter};
use std::sync::Arc;
use std::sync::Mutex;

// Provides syntax specific helpers to the editor like validation for multi-line edits.
#[derive(Completer, Helper, Highlighter, Hinter)]
struct Helper {
  validator: MatchingBracketValidator,
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

  let helper = Helper {
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
    let editor2 = editor.clone();
    let line = tokio::task::spawn_blocking(move || {
      editor2.lock().unwrap().readline("> ")
    })
    .await?;

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
              "functionDeclaration": "function (object) { return Deno[Deno.internal].inspectArgs(['%o', object]); }",
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
