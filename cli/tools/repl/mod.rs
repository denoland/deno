// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::proc_state::ProcState;
use deno_core::error::AnyError;
use deno_runtime::worker::MainWorker;
use rustyline::error::ReadlineError;

mod channel;
mod editor;
mod session;

use channel::rustyline_channel;
use channel::RustylineSyncMessage;
use channel::RustylineSyncMessageHandler;
use channel::RustylineSyncResponse;
use editor::EditorHelper;
use editor::ReplEditor;
use session::EvaluationOutput;
use session::ReplSession;

async fn read_line_and_poll(
  repl_session: &mut ReplSession,
  message_handler: &mut RustylineSyncMessageHandler,
  editor: ReplEditor,
) -> Result<String, ReadlineError> {
  let mut line_fut = tokio::task::spawn_blocking(move || editor.readline());
  let mut poll_worker = true;

  loop {
    tokio::select! {
      result = &mut line_fut => {
        return result.unwrap();
      }
      result = message_handler.recv() => {
        match result {
          Some(RustylineSyncMessage::PostMessage { method, params }) => {
            let result = repl_session
              .post_message_with_event_loop(&method, params)
              .await;
            message_handler.send(RustylineSyncResponse::PostMessage(result)).unwrap();
          },
          Some(RustylineSyncMessage::LspCompletions {
            line_text,
            position,
          }) => {
            let result = repl_session.language_server.completions(&line_text, position).await;
            message_handler.send(RustylineSyncResponse::LspCompletions(result)).unwrap();
          }
          None => {}, // channel closed
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
) -> Result<i32, AnyError> {
  let mut repl_session = ReplSession::initialize(worker).await?;
  let mut rustyline_channel = rustyline_channel();

  let helper = EditorHelper {
    context_id: repl_session.context_id,
    sync_sender: rustyline_channel.0,
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
      &mut rustyline_channel.1,
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

  Ok(repl_session.worker.get_exit_code())
}
