// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::args::Flags;
use crate::args::ReplFlags;
use crate::colors;
use crate::proc_state::ProcState;
use crate::worker::create_main_worker;
use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use rustyline::error::ReadlineError;

mod cdp;
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
    .fetch(&specifier, PermissionsContainer::allow_all())
    .await?;

  Ok((*file.source).to_string())
}

pub async fn run(flags: Flags, repl_flags: ReplFlags) -> Result<i32, AnyError> {
  let main_module = resolve_url_or_path("./$deno$repl.ts").unwrap();
  let ps = ProcState::build(flags).await?;
  let mut worker = create_main_worker(
    &ps,
    main_module.clone(),
    PermissionsContainer::new(Permissions::from_options(
      &ps.options.permissions_options(),
    )?),
  )
  .await?;
  worker.setup_repl().await?;
  let worker = worker.into_main_worker();
  let mut repl_session = ReplSession::initialize(ps.clone(), worker).await?;
  let mut rustyline_channel = rustyline_channel();

  let helper = EditorHelper {
    context_id: repl_session.context_id,
    sync_sender: rustyline_channel.0,
  };

  let history_file_path = ps.dir.repl_history_file_path();
  let editor = ReplEditor::new(helper, history_file_path)?;

  if let Some(eval_files) = repl_flags.eval_files {
    for eval_file in eval_files {
      match read_eval_file(&ps, &eval_file).await {
        Ok(eval_source) => {
          let output = repl_session
            .evaluate_line_and_get_output(&eval_source)
            .await;
          // only output errors
          if let EvaluationOutput::Error(error_text) = output {
            println!(
              "Error in --eval-file file \"{}\": {}",
              eval_file, error_text
            );
          }
        }
        Err(e) => {
          println!("Error in --eval-file file \"{}\": {}", eval_file, e);
        }
      }
    }
  }

  if let Some(eval) = repl_flags.eval {
    let output = repl_session.evaluate_line_and_get_output(&eval).await;
    // only output errors
    if let EvaluationOutput::Error(error_text) = output {
      println!("Error in --eval flag: {}", error_text);
    }
  }

  // Doing this manually, instead of using `log::info!` because these messages
  // are supposed to go to stdout, not stderr.
  if !ps.options.is_quiet() {
    println!("Deno {}", crate::version::deno());
    println!("exit using ctrl+d, ctrl+c, or close()");
    if repl_flags.is_default_command {
      println!(
        "{}",
        colors::yellow("REPL is running with all permissions allowed.")
      );
      println!("To specify permissions, run `deno repl` with allow flags.")
    }
  }

  loop {
    let line = read_line_and_poll(
      &mut repl_session,
      &mut rustyline_channel.1,
      editor.clone(),
    )
    .await;
    match line {
      Ok(line) => {
        editor.set_should_exit_on_interrupt(false);
        editor.update_history(line.clone());
        let output = repl_session.evaluate_line_and_get_output(&line).await;

        // We check for close and break here instead of making it a loop condition to get
        // consistent behavior in when the user evaluates a call to close().
        if repl_session.closing().await? {
          break;
        }

        println!("{}", output);
      }
      Err(ReadlineError::Interrupted) => {
        if editor.should_exit_on_interrupt() {
          break;
        }
        editor.set_should_exit_on_interrupt(true);
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

  Ok(repl_session.worker.exit_code())
}
