// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_core::anyhow::Context;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::located_script_name;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_core::v8;
use deno_path_util::resolve_url_or_path;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_io::StdioPipe;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_terminal::colors;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::CliFactory;
use crate::args::Flags;
use crate::args::JupyterFlags;
use crate::cdp;
use crate::ops;
use crate::ops::jupyter::IopubMessage;
use crate::ops::jupyter::JupyterEvaluateError;
use crate::ops::jupyter::JupyterEvaluateOutcome;
use crate::ops::jupyter::JupyterReplRequest;
use crate::ops::jupyter::KernelConnectionInfo;
use crate::ops::jupyter::KernelInputState;
use crate::ops::jupyter::KernelIopubReceiver;
use crate::ops::jupyter::KernelIsolateHandle;
use crate::ops::jupyter::KernelReplSender;
use crate::ops::jupyter::PendingInputRequest;
use crate::tools::repl;
use crate::tools::test::TestEventWorkerSender;
use crate::tools::test::create_single_test_event_channel;

mod install;

pub async fn kernel(
  flags: Arc<Flags>,
  jupyter_flags: JupyterFlags,
) -> Result<(), AnyError> {
  log::info!(
    "{} \"deno jupyter\" is unstable and might change in the future.",
    colors::yellow("Warning"),
  );

  if !jupyter_flags.install && !jupyter_flags.kernel {
    install::status(jupyter_flags.name.as_deref())?;
    return Ok(());
  }

  if jupyter_flags.install {
    install::install(
      jupyter_flags.name.as_deref(),
      jupyter_flags.display.as_deref(),
      jupyter_flags.force,
    )?;
    return Ok(());
  }

  let connection_filepath = jupyter_flags.conn_file.unwrap();

  let conn_file =
    std::fs::read_to_string(&connection_filepath).with_context(|| {
      format!("Couldn't read connection file: {:?}", connection_filepath)
    })?;
  // Validate JSON
  let _: serde_json::Value =
    serde_json::from_str(&conn_file).with_context(|| {
      format!(
        "Connection file is not valid JSON: {:?}",
        connection_filepath
      )
    })?;

  let factory = CliFactory::from_flags(flags);
  let cli_options = factory.cli_options()?;
  let permissions =
    PermissionsContainer::allow_all(factory.permission_desc_parser()?.clone());
  let npm_installer = factory.npm_installer_if_managed().await?.cloned();
  let compiler_options_resolver_arc =
    factory.compiler_options_resolver()?.clone();
  let resolver = factory.resolver().await?.clone();
  // Wrap in `Arc` so the kernel thread and the REPL background thread can
  // each create workers from the same factory without forcing `Clone` onto
  // `CliMainWorkerFactory` itself.
  let worker_factory =
    Arc::new(factory.create_cli_main_worker_factory().await?);
  let cli_options_arc = factory.cli_options()?.clone();

  // --- Channel setup -------------------------------------------------
  // REPL requests: ZMQ kernel → REPL thread
  let (repl_req_tx, repl_req_rx) =
    mpsc::unbounded_channel::<JupyterReplRequest>();
  // IoPub messages: REPL thread → ZMQ kernel
  let (iopub_tx, iopub_rx) = mpsc::unbounded_channel::<IopubMessage>();
  // input_request originating from user code (REPL thread → ZMQ kernel)
  let (input_tx, input_rx) = mpsc::unbounded_channel::<PendingInputRequest>();
  // REPL isolate handle: REPL thread → main thread
  let (isolate_handle_tx, isolate_handle_rx) =
    oneshot::channel::<v8::IsolateHandle>();

  // --- Spawn the REPL on a background OS thread ---------------------
  let repl_worker_factory = Arc::clone(&worker_factory);
  let repl_main_module = resolve_url_or_path(
    "./$deno$jupyter_repl.mts",
    cli_options_arc.initial_cwd(),
  )
  .unwrap();
  let repl_main_module2 = repl_main_module.clone();
  let repl_permissions = permissions.clone();
  let repl_iopub_tx = iopub_tx.clone();
  let repl_input_tx = input_tx;

  let repl_thread = std::thread::spawn(move || {
    let fut = async move {
      let (worker, test_event_receiver) = create_single_test_event_channel();
      let TestEventWorkerSender {
        sender: test_event_sender,
        stdout,
        stderr,
      } = worker;

      let mut worker = repl_worker_factory
        .create_custom_worker(
          WorkerExecutionMode::Jupyter,
          repl_main_module2.clone(),
          vec![],
          vec![],
          repl_permissions,
          vec![
            ops::jupyter::deno_jupyter_repl::init(repl_iopub_tx, repl_input_tx),
            ops::testing::deno_test::init(test_event_sender),
          ],
          Stdio {
            stdin: StdioPipe::inherit(),
            stdout: StdioPipe::file(stdout),
            stderr: StdioPipe::file(stderr),
          },
          None,
        )
        .await?;

      worker.setup_repl().await?;
      worker.execute_script_static(
        located_script_name!(),
        "Deno[Deno.internal].enableJupyter();",
      )?;
      let worker = worker.into_main_worker();

      let mut repl_session = repl::ReplSession::initialize(
        &cli_options_arc,
        npm_installer,
        resolver,
        &compiler_options_resolver_arc,
        worker,
        repl_main_module2,
        test_event_receiver,
      )
      .await?;

      // Send the isolate handle back to the main thread so it can interrupt us.
      let handle = repl_session
        .worker
        .js_runtime
        .v8_isolate()
        .thread_safe_handle();
      let _ = isolate_handle_tx.send(handle);

      // Service REPL requests until channel closes.
      let mut session = JupyterReplSession {
        repl_session,
        rx: repl_req_rx,
      };
      session.start().await;

      Ok::<(), AnyError>(())
    };
    deno_runtime::tokio_util::create_and_run_current_thread(fut)
  });

  // Wait for the REPL to be ready.
  let isolate_handle = isolate_handle_rx
    .await
    .map_err(|_| anyhow!("REPL thread failed to start"))?;

  // --- Create the ZMQ kernel worker on the main thread ---------------
  let kernel_main_module = resolve_url_or_path(
    "./$deno$jupyter_kernel.mts",
    cli_options.initial_cwd(),
  )
  .unwrap();

  let (worker2, _) = create_single_test_event_channel();
  let TestEventWorkerSender {
    sender: _test_sender2,
    stdout: stdout2,
    stderr: stderr2,
  } = worker2;

  let cwd_url =
    Url::from_directory_path(cli_options.initial_cwd()).map_err(|_| {
      anyhow!(
        "Unable to construct URL from the path of cwd: {}",
        cli_options.initial_cwd().to_string_lossy(),
      )
    })?;
  let _ = cwd_url; // used later for test reporter if needed

  let mut kernel_worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Jupyter,
      kernel_main_module,
      vec![],
      vec![],
      permissions,
      vec![ops::jupyter::deno_jupyter_kernel::init()],
      Stdio {
        stdin: StdioPipe::inherit(),
        stdout: StdioPipe::file(stdout2),
        stderr: StdioPipe::file(stderr2),
      },
      None,
    )
    .await?;

  // Populate op_state for the kernel worker.
  {
    let op_state_rc = kernel_worker.op_state();
    let mut op_state = op_state_rc.borrow_mut();
    op_state.put(KernelReplSender { tx: repl_req_tx });
    op_state.put(KernelIopubReceiver {
      rx: tokio::sync::Mutex::new(iopub_rx),
    });
    op_state.put(KernelInputState {
      rx: tokio::sync::Mutex::new(input_rx),
      pending_responder: std::sync::Mutex::new(None),
    });
    op_state.put(KernelIsolateHandle {
      handle: isolate_handle,
    });
    op_state.put(KernelConnectionInfo { json: conn_file });
  }

  // Bootstrap the JS ZMQ kernel then run the event loop.
  kernel_worker.execute_script_static(
    located_script_name!(),
    "Deno[Deno.internal].startJupyterKernel();",
  )?;
  let mut kernel_main = kernel_worker.into_main_worker();
  kernel_main.run_event_loop(false).await?;

  // Wait for the REPL thread to finish.
  match repl_thread.join() {
    Ok(Ok(())) => {}
    Ok(Err(e)) => bail!("REPL thread error: {}", e),
    Err(_) => bail!("REPL thread panicked"),
  }

  Ok(())
}

// ------------------------------------------------------------------
// REPL session wrapper running on the background thread
// ------------------------------------------------------------------

struct JupyterReplSession {
  repl_session: repl::ReplSession,
  rx: mpsc::UnboundedReceiver<JupyterReplRequest>,
}

impl JupyterReplSession {
  async fn start(&mut self) {
    let mut poll_worker = true;
    loop {
      tokio::select! {
        biased;

        maybe_req = self.rx.recv() => {
          let Some(req) = maybe_req else { break; };
          if self.handle_request(req).await.is_err() {
            break;
          }
          poll_worker = true;
        }
        _ = self.repl_session.run_event_loop(), if poll_worker => {
          poll_worker = false;
        }
      }
    }
  }

  async fn handle_request(
    &mut self,
    req: JupyterReplRequest,
  ) -> Result<(), AnyError> {
    match req {
      JupyterReplRequest::Evaluate { line, resp_tx } => {
        // Clear any pending terminate flag from a previous interrupt.
        self
          .repl_session
          .worker
          .js_runtime
          .v8_isolate()
          .cancel_terminate_execution();

        self.repl_session.track_source_maps = true;

        let result = self
          .repl_session
          .evaluate_line_with_object_wrapping(&line)
          .await;
        // Drop the request flag so a follow-up call without it (eg. an
        // implicit retry path) doesn't accidentally enable source-map
        // tracking on a non-Jupyter call.
        self.repl_session.track_source_maps = false;

        let outcome = match result {
          Ok(r) => {
            if let Some(exception_details) = r.value.exception_details.as_ref()
            {
              let error = self.build_evaluate_error(exception_details).await;
              JupyterEvaluateOutcome {
                value: None,
                error: Some(error),
              }
            } else {
              let value = serde_json::to_value(&r.value).ok();
              JupyterEvaluateOutcome { value, error: None }
            }
          }
          Err(err) => JupyterEvaluateOutcome {
            value: None,
            error: Some(JupyterEvaluateError {
              ename: "Error".to_string(),
              evalue: err.to_string(),
              traceback: vec![err.to_string()],
            }),
          },
        };

        let _ = resp_tx.send(outcome);
      }
      JupyterReplRequest::GetProperties { object_id, resp_tx } => {
        let result = self
          .repl_session
          .post_message_with_event_loop(
            "Runtime.getProperties",
            Some(cdp::GetPropertiesArgs {
              object_id,
              own_properties: None,
              accessor_properties_only: None,
              generate_preview: None,
              non_indexed_properties_only: Some(true),
            }),
          )
          .await;
        let _ = resp_tx.send(Some(result));
      }
      JupyterReplRequest::GlobalLexicalScopeNames { resp_tx } => {
        let result = self
          .repl_session
          .post_message_with_event_loop(
            "Runtime.globalLexicalScopeNames",
            Some(cdp::GlobalLexicalScopeNamesArgs {
              execution_context_id: Some(self.repl_session.context_id),
            }),
          )
          .await;
        let _ = resp_tx.send(result);
      }
      JupyterReplRequest::CallFunctionOnArgs {
        function_declaration,
        args,
        resp_tx,
      } => {
        let result = self
          .repl_session
          .call_function_on_args(function_declaration, &args)
          .await;
        let _ = resp_tx
          .send(result.map(|r| serde_json::to_value(r).unwrap_or_default()));
      }
      JupyterReplRequest::CallFunctionOn {
        arg0,
        arg1,
        resp_tx,
      } => {
        let response = self
          .repl_session
          .post_message_with_event_loop(
            "Runtime.callFunctionOn",
            Some(serde_json::json!({
              "functionDeclaration": r#"async function (execution_count, result) {
                await Deno[Deno.internal].jupyter.broadcastResult(execution_count, result);
              }"#,
              "arguments": [arg0, arg1],
              "executionContextId": self.repl_session.context_id,
              "awaitPromise": true,
            })),
          )
          .await;
        let json: Option<serde_json::Value> =
          serde_json::from_value(response).ok();
        let _ = resp_tx.send(json);
      }
    }
    Ok(())
  }

  /// Build the source-map-remapped `{ename, evalue, traceback}` Jupyter
  /// expects from a CDP `ExceptionDetails`. When the exception is a real
  /// `Error` instance we ask the inspector to JSON-encode its
  /// `name`/`message`/`stack` (so non-enumerable properties are picked up)
  /// and rewrite the stack via `ReplSession::apply_source_map_to_stack`.
  /// For non-`Error` throws we fall back to the CDP text and the exception
  /// description.
  async fn build_evaluate_error(
    &mut self,
    exception_details: &cdp::ExceptionDetails,
  ) -> JupyterEvaluateError {
    if let Some(exception) = exception_details.exception.as_ref() {
      let extract_fn = r#"function (object) {
        if (object instanceof Error) {
          const name = "name" in object ? String(object.name) : "Error";
          const message = "message" in object ? String(object.message) : "";
          const stack = "stack" in object ? String(object.stack) : "";
          return JSON.stringify({ name, message, stack });
        }
        return JSON.stringify({
          name: "",
          message: String(object),
          stack: "",
        });
      }"#;

      match self
        .repl_session
        .call_function_on_args(
          extract_fn.to_string(),
          std::slice::from_ref(exception),
        )
        .await
      {
        Ok(resp) => {
          if let Some(json_str) =
            resp.result.value.as_ref().and_then(|v| v.as_str())
            && let Ok(parsed) =
              serde_json::from_str::<serde_json::Value>(json_str)
          {
            let name = parsed
              .get("name")
              .and_then(|v| v.as_str())
              .filter(|s| !s.is_empty())
              .unwrap_or("Error")
              .to_string();
            let message = parsed
              .get("message")
              .and_then(|v| v.as_str())
              .unwrap_or("")
              .to_string();
            let stack_str = parsed
              .get("stack")
              .and_then(|v| v.as_str())
              .unwrap_or("")
              .to_string();
            let mapped =
              self.repl_session.apply_source_map_to_stack(&stack_str);
            let traceback = if mapped.is_empty() {
              if message.is_empty() {
                vec![name.clone()]
              } else {
                vec![format!("{name}: {message}")]
              }
            } else {
              mapped.split('\n').map(|s| s.to_string()).collect()
            };
            return JupyterEvaluateError {
              ename: name,
              evalue: message,
              traceback,
            };
          }
          // call_function_on succeeded but the inspector returned an
          // unexpected shape — drop through to the textual fallback below.
        }
        Err(_) => {
          // Inspector call failed — fall through to textual fallback.
        }
      }
    }

    // Non-`Error` throw, or extraction failed: build a minimal traceback
    // from the CDP exception text.
    let (ename, evalue) = exception_details.get_message_and_description();
    let traceback = if evalue.is_empty() {
      vec![ename.clone()]
    } else {
      vec![format!("{ename}: {evalue}")]
    };
    JupyterEvaluateError {
      ename,
      evalue,
      traceback,
    }
  }
}
