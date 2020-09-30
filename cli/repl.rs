// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::global_state::GlobalState;
use crate::inspector::InspectorSession;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use rustyline::error::ReadlineError;
use rustyline::Editor;

pub async fn run(
  _global_state: &GlobalState,
  mut session: Box<InspectorSession>,
) -> Result<(), AnyError> {
  // Our inspector is unable to default to the default context id so we have to specify it here.
  let context_id: u32 = 1;

  session
    .post_message("Runtime.enable".to_string(), None)
    .await?;

  let mut editor = Editor::<()>::new();

  println!("Deno {}", crate::version::DENO);
  println!("exit using ctrl+d or close()");

  loop {
    let line = editor.readline("> ");
    match line {
      Ok(line) => {
        let evaluate_response = session
          .post_message(
            "Runtime.evaluate".to_string(),
            Some(json!({
                "expression": line,
                "contextId": context_id,
                // TODO(caspervonb) set repl mode to true to enable const redeclarations and top
                // level await
                "replMode": false,
            })),
          )
          .await?;

        let evaluate_result = evaluate_response.get("result").unwrap();

        // TODO(caspervonb) we should investigate using previews here but to keep things
        // consistent with the previous implementation we just get the preview result from
        // Deno.inspectArgs.
        let inspect_response = session.post_message("Runtime.callFunctionOn".to_string(), Some(json!({
                "executionContextId": context_id,
                "functionDeclaration": "function (object) { return Deno[Deno.internal].inspectArgs(['%o', object]); }",
                "arguments": [
                    evaluate_result,
                ],
            }))).await?;

        let inspect_result = inspect_response.get("result").unwrap();
        println!("{}", inspect_result.get("value").unwrap().as_str().unwrap());

        editor.add_history_entry(line.as_str());
      }
      Err(ReadlineError::Interrupted) => {
        println!("ctrl-c");
        break;
      }
      Err(ReadlineError::Eof) => {
        println!("ctrl-d");
        break;
      }
      Err(err) => {
        println!("Error: {:?}", err);
        break;
      }
    }
  }

  // TODO(caspervonb) save history file

  Ok(())
}
