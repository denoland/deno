// Copyright 2018-2025 the Deno authors. MIT license.

use std::io;
use std::io::Write;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::unsync::spawn_blocking;
use deno_lib::version::DENO_VERSION_INFO;
use deno_runtime::WorkerExecutionMode;
use rustyline::error::ReadlineError;
use tokio_util::sync::CancellationToken;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::ReplFlags;
use crate::cdp;
use crate::colors;
use crate::factory::CliFactory;
use crate::file_fetcher::CliFileFetcher;
use crate::file_fetcher::TextDecodedFile;

mod channel;
mod editor;
mod session;

use channel::RustylineSyncMessage;
use channel::RustylineSyncMessageHandler;
use channel::RustylineSyncResponse;
use channel::rustyline_channel;
use editor::EditorHelper;
use editor::ReplEditor;
pub use session::EvaluationOutput;
pub use session::ReplSession;
pub use session::TsEvaluateResponse;

use super::test::create_single_test_event_channel;

struct Repl {
  session: ReplSession,
  editor: ReplEditor,
  message_handler: RustylineSyncMessageHandler,
}

#[allow(clippy::print_stdout)]
impl Repl {
  async fn run(&mut self) -> Result<(), AnyError> {
    loop {
      let line = read_line_and_poll(
        &mut self.session,
        &mut self.message_handler,
        self.editor.clone(),
      )
      .await;
      match line {
        Ok(line) => {
          self.editor.set_should_exit_on_interrupt(false);
          self.editor.update_history(line.clone());
          let output = self.session.evaluate_line_and_get_output(&line).await;

          // We check for close and break here instead of making it a loop condition to get
          // consistent behavior in when the user evaluates a call to close().
          match self.session.closing().await {
            Ok(closing) if closing => break,
            Ok(_) => {}
            Err(err) => {
              println!("Error: {:?}", err)
            }
          };

          println!("{}", output);
        }
        Err(ReadlineError::Interrupted) => {
          if self.editor.should_exit_on_interrupt() {
            break;
          }
          self.editor.set_should_exit_on_interrupt(true);
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

    Ok(())
  }
}

#[allow(clippy::print_stdout)]
async fn read_line_and_poll(
  repl_session: &mut ReplSession,
  message_handler: &mut RustylineSyncMessageHandler,
  editor: ReplEditor,
) -> Result<String, ReadlineError> {
  let mut line_fut = spawn_blocking(move || editor.readline());
  let mut poll_worker = true;
  let notifications_rc = repl_session.notifications.clone();
  let mut notifications = notifications_rc.lock().await;

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
            let result = repl_session.language_server.completions(
              &line_text,
              position,
              CancellationToken::new(),
            ).await;
            message_handler.send(RustylineSyncResponse::LspCompletions(result)).unwrap();
          }
          None => {}, // channel closed
        }

        poll_worker = true;
      }
      message = notifications.next() => {
        if let Some(message) = message {
          let notification: cdp::Notification = serde_json::from_value(message).unwrap();
          if notification.method == "Runtime.exceptionThrown" {
            let exception_thrown: cdp::ExceptionThrown = serde_json::from_value(notification.params).unwrap();
            let (message, description) = exception_thrown.exception_details.get_message_and_description();
            println!("{} {}", message, description);
          }
        }
      }
      _ = repl_session.run_event_loop(), if poll_worker => {
        poll_worker = false;
      }
    }
  }
}

async fn read_eval_file(
  cli_options: &CliOptions,
  file_fetcher: &CliFileFetcher,
  eval_file: &str,
) -> Result<Arc<str>, AnyError> {
  let specifier =
    deno_path_util::resolve_url_or_path(eval_file, cli_options.initial_cwd())?;

  let file = file_fetcher.fetch_bypass_permissions(&specifier).await?;

  Ok(TextDecodedFile::decode(file)?.source)
}

#[allow(clippy::print_stdout)]
pub async fn run(
  flags: Arc<Flags>,
  repl_flags: ReplFlags,
) -> Result<i32, AnyError> {
  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let main_module = cli_options.resolve_main_module()?;
  let permissions = factory.root_permissions_container()?;
  let npm_installer = factory.npm_installer_if_managed().await?.cloned();
  let resolver = factory.resolver().await?.clone();
  let file_fetcher = factory.file_fetcher()?;
  let compiler_options_resolver = factory.compiler_options_resolver()?;
  let worker_factory = factory.create_cli_main_worker_factory().await?;
  let (worker, test_event_receiver) = create_single_test_event_channel();
  let test_event_sender = worker.sender;
  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Repl,
      main_module.clone(),
      // `deno repl` doesn't support preloading modules
      vec![],
      // `deno repl` doesn't support require modules
      vec![],
      permissions.clone(),
      vec![crate::ops::testing::deno_test::init(test_event_sender)],
      Default::default(),
      None,
    )
    .await?;
  worker.setup_repl().await?;
  let worker = worker.into_main_worker();
  let session = ReplSession::initialize(
    cli_options,
    npm_installer,
    resolver,
    compiler_options_resolver,
    worker,
    main_module.clone(),
    test_event_receiver,
  )
  .await?;

  #[cfg(unix)]
  if repl_flags.json {
    return run_json(session).await;
  }

  let rustyline_channel = rustyline_channel();

  let helper = EditorHelper {
    context_id: session.context_id,
    sync_sender: rustyline_channel.0,
  };

  let history_file_path = factory
    .deno_dir()
    .ok()
    .and_then(|dir| dir.repl_history_file_path());
  let editor = ReplEditor::new(helper, history_file_path)?;

  let mut repl = Repl {
    session,
    editor,
    message_handler: rustyline_channel.1,
  };

  if let Some(eval_files) = repl_flags.eval_files {
    for eval_file in eval_files {
      match read_eval_file(cli_options, file_fetcher, &eval_file).await {
        Ok(eval_source) => {
          let output = repl
            .session
            .evaluate_line_and_get_output(&eval_source)
            .await;
          // only output errors
          if let EvaluationOutput::Error(error_text) = output {
            println!("Error in --eval-file file \"{eval_file}\": {error_text}");
          }
        }
        Err(e) => {
          println!("Error in --eval-file file \"{eval_file}\": {e}");
        }
      }
    }
  }

  if let Some(eval) = repl_flags.eval {
    let output = repl.session.evaluate_line_and_get_output(&eval).await;
    // only output errors
    if let EvaluationOutput::Error(error_text) = output {
      println!("Error in --eval flag: {error_text}");
    }
  }

  // Doing this manually, instead of using `log::info!` because these messages
  // are supposed to go to stdout, not stderr.
  // Using writeln, because println panics in certain cases
  // (eg: broken pipes - https://github.com/denoland/deno/issues/21861)
  if !cli_options.is_quiet() {
    let mut handle = io::stdout().lock();

    writeln!(handle, "Deno {}", DENO_VERSION_INFO.deno)?;
    writeln!(handle, "exit using ctrl+d, ctrl+c, or close()")?;

    if repl_flags.is_default_command {
      writeln!(
        handle,
        "{}",
        colors::yellow("REPL is running with all permissions allowed.")
      )?;
      writeln!(
        handle,
        "To specify permissions, run `deno repl` with allow flags."
      )?;
    }
  }

  repl.run().await?;

  Ok(repl.session.worker.exit_code())
}

#[cfg(unix)]
async fn run_json(mut repl_session: ReplSession) -> Result<i32, AnyError> {
  use bytes::Buf;
  use bytes::Bytes;
  use deno_runtime::deno_io::BiPipe;
  use tokio::io::AsyncReadExt;
  use tokio::io::AsyncWriteExt;
  use tokio::io::BufReader;

  #[derive(serde::Serialize, serde::Deserialize, Debug)]
  #[serde(tag = "type")]
  enum ReplMessage {
    Run { code: String, output: bool },
    RunSuccess { output: Option<String> },
    RunFailure { text: String },
    Error { error: String },
  }

  let (receiver, mut sender) = BiPipe::from_raw(3)?.split();
  let mut receiver = BufReader::new(receiver);

  loop {
    let mut line_fut = std::pin::pin!(async {
      let len = receiver.read_u32_le().await?;
      let mut buf = vec![0; len as _];
      receiver.read_exact(&mut buf).await?;
      Ok::<_, AnyError>(buf)
    });
    let mut poll_worker = true;
    let line = loop {
      tokio::select! {
        line = &mut line_fut => break line?,
        _ = repl_session.run_event_loop(), if poll_worker => {
          poll_worker = false;
          continue;
        }
      }
    };
    let command: ReplMessage = serde_json::from_slice(&line)?;

    if let ReplMessage::Run { code, output } = command {
      let result = repl_session.evaluate_line_with_object_wrapping(&code).await;

      // We check for close and break here instead of making it a loop condition to get
      // consistent behavior in when the user evaluates a call to close().
      match repl_session.closing().await {
        Ok(closing) if closing => break,
        Ok(_) => {}
        Err(err) => {
          let buf = serde_json::to_vec(&ReplMessage::Error {
            error: format!("{}", err),
          })?;
          sender
            .write_all_buf(
              &mut Bytes::from_owner((buf.len() as u32).to_le_bytes())
                .chain(Bytes::from(buf)),
            )
            .await?;
        }
      };

      match result {
        Ok(evaluate_response) => {
          let cdp::EvaluateResponse {
            result,
            exception_details,
          } = evaluate_response.value;

          let msg = if let Some(exception_details) = exception_details {
            repl_session.set_last_thrown_error(&result).await?;

            ReplMessage::RunFailure {
              text: exception_details.text,
            }
          } else {
            repl_session
              .language_server
              .commit_text(&evaluate_response.ts_code)
              .await;
            repl_session.set_last_eval_result(&result).await?;

            let output = if output {
              let response = repl_session
                .call_function_on_repl_internal_obj(
                  "function (object) { return this.String(object); }".into(),
                  &[result],
                )
                .await?;
              let output = response
                .result
                .value
                .map(|v| v.as_str().unwrap().to_string())
                .or(response.result.description)
                .unwrap_or_else(|| "something went wrong".into());
              Some(output)
            } else {
              None
            };

            ReplMessage::RunSuccess { output }
          };

          let buf = serde_json::to_vec(&msg)?;
          sender
            .write_all_buf(
              &mut Bytes::from_owner((buf.len() as u32).to_le_bytes())
                .chain(Bytes::from(buf)),
            )
            .await?;
        }
        Err(err) => {
          let buf = serde_json::to_vec(&ReplMessage::Error {
            error: format!("{}", err),
          })?;
          sender
            .write_all_buf(
              &mut Bytes::from_owner((buf.len() as u32).to_le_bytes())
                .chain(Bytes::from(buf)),
            )
            .await?;
        }
      }
    }
  }

  Ok(repl_session.worker.exit_code())
}
