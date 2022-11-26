// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::proc_state::ProcState;
use crate::worker::create_main_worker_with_extensions;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
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

async fn read_eval_file(
  ps: &ProcState,
  eval_file: &str,
) -> Result<String, AnyError> {
  let specifier = deno_core::resolve_url_or_path(eval_file)?;

  let file = ps
    .file_fetcher
    .fetch(&specifier, &mut Permissions::allow_all())
    .await?;

  Ok((*file.source).to_string())
}

#[derive(Clone)]
struct ReplState {
  needs_reload: bool,
  needs_save: bool,
  maybe_session_filename: Option<String>,
}

#[op]
pub fn op_repl_reload(state: &mut OpState) {
  let repl_state = state.borrow_mut::<ReplState>();
  repl_state.needs_reload = true;
}

#[op]
pub fn op_repl_save(
  state: &mut OpState,
  maybe_session_filename: Option<String>,
) {
  let repl_state = state.borrow_mut::<ReplState>();
  repl_state.needs_save = true;
  repl_state.maybe_session_filename = maybe_session_filename;
}

async fn create_repl_session(
  ps: &ProcState,
  module_url: ModuleSpecifier,
  maybe_eval_files: Option<Vec<String>>,
  maybe_eval: Option<String>,
) -> Result<ReplSession, AnyError> {
  let extension = Extension::builder()
    .ops(vec![op_repl_reload::decl(), op_repl_save::decl()])
    .state(move |state| {
      state.put(ReplState {
        needs_reload: false,
        needs_save: false,
        maybe_session_filename: None,
      });
      Ok(())
    })
    .build();

  let mut worker = create_main_worker_with_extensions(
    ps,
    module_url.clone(),
    Permissions::from_options(&ps.options.permissions_options())?,
    vec![extension],
  )
  .await?;
  worker.setup_repl().await?;
  let worker = worker.into_main_worker();
  let mut repl_session = ReplSession::initialize(worker).await?;

  if let Some(eval_files) = maybe_eval_files {
    for eval_file in eval_files {
      match read_eval_file(ps, &eval_file).await {
        Ok(eval_source) => {
          let output = repl_session
            .evaluate_line_and_get_output(&eval_source)
            .await?;
          // only output errors
          if let EvaluationOutput::Error(error_text) = output {
            println!("error in --eval-file file {}. {}", eval_file, error_text);
          }
        }
        Err(e) => {
          println!("error in --eval-file file {}. {}", eval_file, e);
        }
      }
    }
  }

  if let Some(eval) = maybe_eval {
    let output = repl_session.evaluate_line_and_get_output(&eval).await?;
    // only output errors
    if let EvaluationOutput::Error(error_text) = output {
      println!("error in --eval flag. {}", error_text);
    }
  }

  Ok(repl_session)
}

fn save_session_to_file(
  session_history: &[String],
  maybe_filename: Option<String>,
) -> Result<(), AnyError> {
  // TODO(bartlomieju): make date shorter
  let filename = maybe_filename.unwrap_or_else(|| {
    format!("./repl-{}.ts", chrono::Local::now().to_rfc3339())
  });
  std::fs::write(&filename, session_history.join("\n"))
    .context("Unable to save session file")?;
  println!("Saved session to {}", filename);
  Ok(())
}

pub async fn run(
  ps: &ProcState,
  module_url: ModuleSpecifier,
  maybe_eval_files: Option<Vec<String>>,
  maybe_eval: Option<String>,
) -> Result<i32, AnyError> {
  let mut repl_session = create_repl_session(
    ps,
    module_url.clone(),
    maybe_eval_files.clone(),
    maybe_eval.clone(),
  )
  .await?;
  let mut rustyline_channel = rustyline_channel();
  let mut should_exit_on_interrupt = false;

  // TODO(bartlomieju): add helper to update `context_id` in the helper
  let helper = EditorHelper {
    context_id: repl_session.context_id,
    sync_sender: rustyline_channel.0,
  };

  let history_file_path = ps.dir.root.join("deno_history.txt");
  let editor = ReplEditor::new(helper, history_file_path)?;

  println!("Deno {}", crate::version::deno());
  println!("Run repl.help() to see help");
  println!("Exit using ctrl+d, ctrl+c, or close()");

  let mut session_history: Vec<String> = vec![];
  loop {
    let line = read_line_and_poll(
      &mut repl_session,
      &mut rustyline_channel.1,
      editor.clone(),
    )
    .await;
    match line {
      Ok(line) => {
        should_exit_on_interrupt = false;
        editor.update_history(line.clone());

        session_history.push(line.to_string());
        let output = repl_session.evaluate_line_and_get_output(&line).await?;

        // We check for close and break here instead of making it a loop condition to get
        // consistent behavior in when the user evaluates a call to close().
        if repl_session.closing().await? {
          break;
        }

        println!("{}", output);

        {
          let op_state = repl_session.worker.js_runtime.op_state();
          let repl_state = {
            let op_state = op_state.borrow();
            op_state.borrow::<ReplState>().clone()
          };
          if repl_state.needs_reload {
            drop(op_state);
            repl_session = create_repl_session(
              ps,
              module_url.clone(),
              maybe_eval_files.clone(),
              maybe_eval.clone(),
            )
            .await?;
            println!("Started a new REPL session. Global scope is now clean.");
          } else if repl_state.needs_save {
            let mut op_state = op_state.borrow_mut();
            op_state.put(ReplState {
              needs_reload: false,
              needs_save: false,
              maybe_session_filename: None,
            });
            save_session_to_file(
              &session_history,
              repl_state.maybe_session_filename,
            )?;
          }
        }
      }
      Err(ReadlineError::Interrupted) => {
        if should_exit_on_interrupt {
          break;
        }
        should_exit_on_interrupt = true;
        println!("press ctrl+c again to exit");
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

  Ok(repl_session.worker.get_exit_code())
}
