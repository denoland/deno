// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// This file is forked/ported from <https://github.com/evcxr/evcxr>
// Copyright 2020 The Evcxr Authors. MIT license.

// NOTE(bartlomieju): unfortunately it appears that clippy is broken
// and can't allow a single line ignore for `await_holding_lock`.
#![allow(clippy::await_holding_lock)]

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::cdp;
use crate::tools::repl;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::parking_lot::Mutex;
use deno_core::serde_json;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use jupyter_runtime::messaging;
use jupyter_runtime::AsChildOf;
use jupyter_runtime::ConnectionInfo;
use jupyter_runtime::JupyterMessage;
use jupyter_runtime::JupyterMessageContent;
use jupyter_runtime::KernelControlConnection;
use jupyter_runtime::KernelIoPubConnection;
use jupyter_runtime::KernelShellConnection;
use jupyter_runtime::ReplyError;
use jupyter_runtime::ReplyStatus;
use jupyter_runtime::StreamContent;

use super::JupyterReplProxy;

pub struct JupyterServer {
  execution_count: usize,
  last_execution_request: Arc<Mutex<Option<JupyterMessage>>>,
  iopub_connection: Arc<Mutex<KernelIoPubConnection>>,
  repl_session_proxy: JupyterReplProxy,
}

pub struct StdinConnectionProxy {
  pub tx: mpsc::UnboundedSender<JupyterMessage>,
  pub rx: mpsc::UnboundedReceiver<JupyterMessage>,
}

pub struct StartupData {
  pub iopub_connection: Arc<Mutex<KernelIoPubConnection>>,
  pub stdin_connection_proxy: Arc<Mutex<StdinConnectionProxy>>,
  pub last_execution_request: Arc<Mutex<Option<JupyterMessage>>>,
}

impl JupyterServer {
  pub async fn start(
    connection_info: ConnectionInfo,
    mut stdio_rx: mpsc::UnboundedReceiver<StreamContent>,
    repl_session_proxy: JupyterReplProxy,
    setup_tx: oneshot::Sender<StartupData>,
  ) -> Result<(), AnyError> {
    let mut heartbeat =
      connection_info.create_kernel_heartbeat_connection().await?;
    let shell_connection =
      connection_info.create_kernel_shell_connection().await?;
    let control_connection =
      connection_info.create_kernel_control_connection().await?;
    let mut stdin_connection =
      connection_info.create_kernel_stdin_connection().await?;
    let iopub_connection =
      connection_info.create_kernel_iopub_connection().await?;

    let iopub_connection = Arc::new(Mutex::new(iopub_connection));
    let last_execution_request = Arc::new(Mutex::new(None));

    let (stdin_tx1, mut stdin_rx1) =
      mpsc::unbounded_channel::<JupyterMessage>();
    let (stdin_tx2, stdin_rx2) = mpsc::unbounded_channel::<JupyterMessage>();

    let stdin_connection_proxy = Arc::new(Mutex::new(StdinConnectionProxy {
      tx: stdin_tx1,
      rx: stdin_rx2,
    }));

    let Ok(()) = setup_tx.send(StartupData {
      iopub_connection: iopub_connection.clone(),
      last_execution_request: last_execution_request.clone(),
      stdin_connection_proxy,
    }) else {
      bail!("Failed to send startup data");
    };

    let cancel_handle = CancelHandle::new_rc();

    let mut server = Self {
      execution_count: 0,
      iopub_connection: iopub_connection.clone(),
      last_execution_request: last_execution_request.clone(),
      repl_session_proxy,
    };

    let stdin_fut = deno_core::unsync::spawn(async move {
      loop {
        let Some(msg) = stdin_rx1.recv().await else {
          return;
        };
        let Ok(()) = stdin_connection.send(msg).await else {
          return;
        };

        let Ok(msg) = stdin_connection.read().await else {
          return;
        };
        let Ok(()) = stdin_tx2.send(msg) else {
          return;
        };
      }
    });

    let hearbeat_fut = deno_core::unsync::spawn(async move {
      loop {
        if let Err(err) = heartbeat.single_heartbeat().await {
          log::error!(
            "Heartbeat error: {}\nBacktrace:\n{}",
            err,
            err.backtrace()
          );
        }
      }
    });

    let control_fut = deno_core::unsync::spawn({
      let cancel_handle = cancel_handle.clone();
      async move {
        if let Err(err) =
          Self::handle_control(control_connection, cancel_handle).await
        {
          log::error!(
            "Control error: {}\nBacktrace:\n{}",
            err,
            err.backtrace()
          );
        }
      }
    });

    let shell_fut = deno_core::unsync::spawn(async move {
      if let Err(err) = server.handle_shell(shell_connection).await {
        log::error!("Shell error: {}\nBacktrace:\n{}", err, err.backtrace());
      }
    });

    let stdio_fut = deno_core::unsync::spawn(async move {
      while let Some(stdio_msg) = stdio_rx.recv().await {
        Self::handle_stdio_msg(
          iopub_connection.clone(),
          last_execution_request.clone(),
          stdio_msg,
        )
        .await;
      }
    });

    let repl_session_fut = deno_core::unsync::spawn(async move {});

    let join_fut = futures::future::try_join_all(vec![
      hearbeat_fut,
      control_fut,
      shell_fut,
      stdio_fut,
      repl_session_fut,
      stdin_fut,
    ]);

    if let Ok(result) = join_fut.or_cancel(cancel_handle).await {
      result?;
    }

    Ok(())
  }

  async fn handle_stdio_msg(
    iopub_connection: Arc<Mutex<KernelIoPubConnection>>,
    last_execution_request: Arc<Mutex<Option<JupyterMessage>>>,
    stdio_msg: StreamContent,
  ) {
    let maybe_exec_result = last_execution_request.lock().clone();
    let Some(exec_request) = maybe_exec_result else {
      return;
    };

    let result = iopub_connection
      .lock()
      .send(stdio_msg.as_child_of(&exec_request))
      .await;

    if let Err(err) = result {
      log::error!("Output error: {}", err);
    }
  }

  async fn handle_control(
    mut connection: KernelControlConnection,
    cancel_handle: Rc<CancelHandle>,
  ) -> Result<(), AnyError> {
    loop {
      let msg = connection.read().await?;

      match msg.content {
        JupyterMessageContent::KernelInfoRequest(_) => {
          // normally kernel info is sent from the shell channel
          // however, some frontends will send it on the control channel
          // and it's no harm to send a kernel info reply on control
          connection.send(kernel_info().as_child_of(&msg)).await?;
        }
        JupyterMessageContent::ShutdownRequest(_) => {
          cancel_handle.cancel();
        }
        JupyterMessageContent::InterruptRequest(_) => {
          log::error!("Interrupt request currently not supported");
        }
        JupyterMessageContent::DebugRequest(_) => {
          log::error!("Debug request currently not supported");
          // See https://jupyter-client.readthedocs.io/en/latest/messaging.html#debug-request
          // and https://microsoft.github.io/debug-adapter-protocol/
        }
        _ => {
          log::error!(
            "Unrecognized control message type: {}",
            msg.message_type()
          );
        }
      }
    }
  }

  async fn handle_shell(
    &mut self,
    mut connection: KernelShellConnection,
  ) -> Result<(), AnyError> {
    loop {
      let msg = connection.read().await?;
      self.handle_shell_message(msg, &mut connection).await?;
    }
  }

  async fn handle_shell_message(
    &mut self,
    msg: JupyterMessage,
    connection: &mut KernelShellConnection,
  ) -> Result<(), AnyError> {
    let parent = &msg.clone();

    self
      .send_iopub(messaging::Status::busy().as_child_of(parent))
      .await?;

    match msg.content {
      JupyterMessageContent::ExecuteRequest(execute_request) => {
        self
          .handle_execution_request(execute_request, parent, connection)
          .await?;
      }
      JupyterMessageContent::CompleteRequest(req) => {
        let user_code = req.code;
        let cursor_pos = req.cursor_pos;

        let lsp_completions = self
          .repl_session_proxy
          .lsp_completions(user_code.clone(), cursor_pos)
          .await;

        if !lsp_completions.is_empty() {
          let matches: Vec<String> = lsp_completions
            .iter()
            .map(|item| item.new_text.clone())
            .collect();

          let cursor_start = lsp_completions
            .first()
            .map(|item| item.range.start)
            .unwrap_or(cursor_pos);

          let cursor_end = lsp_completions
            .last()
            .map(|item| item.range.end)
            .unwrap_or(cursor_pos);

          connection
            .send(
              messaging::CompleteReply {
                matches,
                cursor_start,
                cursor_end,
                metadata: Default::default(),
                status: ReplyStatus::Ok,
                error: None,
              }
              .as_child_of(parent),
            )
            .await?;
        } else {
          let expr = get_expr_from_line_at_pos(&user_code, cursor_pos);
          // check if the expression is in the form `obj.prop`
          let (completions, cursor_start) = if let Some(index) = expr.rfind('.')
          {
            let sub_expr = &expr[..index];
            let prop_name = &expr[index + 1..];
            let candidates = get_expression_property_names(
              &mut self.repl_session_proxy,
              sub_expr,
            )
            .await
            .into_iter()
            .filter(|n| {
              !n.starts_with("Symbol(")
                && n.starts_with(prop_name)
                && n != &*repl::REPL_INTERNALS_NAME
            })
            .collect();

            (candidates, cursor_pos - prop_name.len())
          } else {
            // combine results of declarations and globalThis properties
            let mut candidates = get_expression_property_names(
              &mut self.repl_session_proxy,
              "globalThis",
            )
            .await
            .into_iter()
            .chain(
              get_global_lexical_scope_names(&mut self.repl_session_proxy)
                .await,
            )
            .filter(|n| n.starts_with(expr) && n != &*repl::REPL_INTERNALS_NAME)
            .collect::<Vec<_>>();

            // sort and remove duplicates
            candidates.sort();
            candidates.dedup(); // make sure to sort first

            (candidates, cursor_pos - expr.len())
          };

          connection
            .send(
              messaging::CompleteReply {
                matches: completions,
                cursor_start,
                cursor_end: cursor_pos,
                metadata: Default::default(),
                status: ReplyStatus::Ok,
                error: None,
              }
              .as_child_of(parent),
            )
            .await?;
        }
      }

      JupyterMessageContent::InspectRequest(_req) => {
        // TODO(bartlomieju?): implement introspection request
        // The inspect request is used to get information about an object at cursor position.
        // There are two detail levels: 0 is typically documentation, 1 is typically source code

        // The response includes a MimeBundle to render the object:
        // {
        //   "status": "ok",
        //   "found": true,
        //   "data": {
        //     "text/plain": "Plain documentation here",
        //     "text/html": "<div>Rich documentation here</div>",
        //     "application/json": {
        //       "key1": "value1",
        //       "key2": "value2"
        //     }
        //   },
        // }

        connection
          .send(
            messaging::InspectReply {
              status: ReplyStatus::Ok,
              found: false,
              data: Default::default(),
              metadata: Default::default(),
              error: None,
            }
            .as_child_of(parent),
          )
          .await?;
      }

      JupyterMessageContent::IsCompleteRequest(_) => {
        connection
          .send(messaging::IsCompleteReply::complete().as_child_of(parent))
          .await?;
      }
      JupyterMessageContent::KernelInfoRequest(_) => {
        connection.send(kernel_info().as_child_of(parent)).await?;
      }
      JupyterMessageContent::CommOpen(comm) => {
        connection
          .send(
            messaging::CommClose {
              comm_id: comm.comm_id,
              data: Default::default(),
            }
            .as_child_of(parent),
          )
          .await?;
      }
      JupyterMessageContent::HistoryRequest(_req) => {
        connection
          .send(
            messaging::HistoryReply {
              history: vec![],
              error: None,
              status: ReplyStatus::Ok,
            }
            .as_child_of(parent),
          )
          .await?;
      }
      JupyterMessageContent::InputReply(_rep) => {
        // TODO(@zph): implement input reply from https://github.com/denoland/deno/pull/23592
        // NOTE: This will belong on the stdin channel, not the shell channel
      }
      JupyterMessageContent::CommInfoRequest(_req) => {
        connection
          .send(
            messaging::CommInfoReply {
              comms: Default::default(),
              status: ReplyStatus::Ok,
              error: None,
            }
            .as_child_of(parent),
          )
          .await?;
      }
      JupyterMessageContent::CommMsg(_)
      | JupyterMessageContent::CommClose(_) => {
        // Do nothing with regular comm messages
      }
      // Any unknown message type is ignored
      _ => {
        log::error!(
          "Unrecognized shell message type: {}",
          msg.content.message_type()
        );
      }
    }

    self
      .send_iopub(messaging::Status::idle().as_child_of(parent))
      .await?;

    Ok(())
  }

  async fn handle_execution_request(
    &mut self,
    execute_request: messaging::ExecuteRequest,
    parent_message: &JupyterMessage,
    connection: &mut KernelShellConnection,
  ) -> Result<(), AnyError> {
    if !execute_request.silent && execute_request.store_history {
      self.execution_count += 1;
    }
    *self.last_execution_request.lock() = Some(parent_message.clone());

    self
      .send_iopub(
        messaging::ExecuteInput {
          execution_count: self.execution_count,
          code: execute_request.code.clone(),
        }
        .as_child_of(parent_message),
      )
      .await?;

    let result = self
      .repl_session_proxy
      .evaluate_line_with_object_wrapping(execute_request.code)
      .await;

    let evaluate_response = match result {
      Ok(eval_response) => eval_response,
      Err(err) => {
        self
          .send_iopub(
            messaging::ErrorOutput {
              ename: err.to_string(),
              evalue: err.to_string(),
              traceback: vec![],
            }
            .as_child_of(parent_message),
          )
          .await?;
        connection
          .send(
            messaging::ExecuteReply {
              execution_count: self.execution_count,
              status: ReplyStatus::Error,
              payload: Default::default(),
              user_expressions: None,
              error: None,
            }
            .as_child_of(parent_message),
          )
          .await?;
        return Ok(());
      }
    };

    let cdp::EvaluateResponse {
      result,
      exception_details,
    } = evaluate_response.value;

    if exception_details.is_none() {
      publish_result(
        &mut self.repl_session_proxy,
        &result,
        self.execution_count,
      )
      .await?;

      connection
        .send(
          messaging::ExecuteReply {
            execution_count: self.execution_count,
            status: ReplyStatus::Ok,
            user_expressions: None,
            payload: Default::default(),
            error: None,
          }
          .as_child_of(parent_message),
        )
        .await?;
      // Let's sleep here for a few ms, so we give a chance to the task that is
      // handling stdout and stderr streams to receive and flush the content.
      // Otherwise, executing multiple cells one-by-one might lead to output
      // from various cells be grouped together in another cell result.
      tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    } else if let Some(exception_details) = exception_details {
      // Determine the exception value and name
      let (name, message, stack) = if let Some(exception) =
        exception_details.exception
      {
        let result = self
          .repl_session_proxy
          .call_function_on_args(
            r#"
          function(object) {
            if (object instanceof Error) {
              const name = "name" in object ? String(object.name) : "";
              const message = "message" in object ? String(object.message) : "";
              const stack = "stack" in object ? String(object.stack) : "";
              return JSON.stringify({ name, message, stack });
            } else {
              const message = String(object);
              return JSON.stringify({ name: "", message, stack: "" });
            }
          }
        "#
            .into(),
            vec![exception],
          )
          .await?;

        match result.result.value {
          Some(serde_json::Value::String(str)) => {
            if let Ok(object) =
              serde_json::from_str::<HashMap<String, String>>(&str)
            {
              let get = |k| object.get(k).cloned().unwrap_or_default();
              (get("name"), get("message"), get("stack"))
            } else {
              log::error!("Unexpected result while parsing JSON {str}");
              ("".into(), "".into(), "".into())
            }
          }
          _ => {
            log::error!("Unexpected result while parsing exception {result:?}");
            ("".into(), "".into(), "".into())
          }
        }
      } else {
        log::error!("Unexpectedly missing exception {exception_details:?}");
        ("".into(), "".into(), "".into())
      };

      let stack = if stack.is_empty() {
        format!(
          "{}\n    at <unknown>",
          serde_json::to_string(&message).unwrap()
        )
      } else {
        stack
      };
      let traceback = format!("Stack trace:\n{stack}")
        .split('\n')
        .map(|s| s.to_owned())
        .collect::<Vec<_>>();

      let ename = if name.is_empty() {
        "Unknown error".into()
      } else {
        name
      };

      let evalue = if message.is_empty() {
        "(none)".into()
      } else {
        message
      };

      self
        .send_iopub(
          messaging::ErrorOutput {
            ename: ename.clone(),
            evalue: evalue.clone(),
            traceback: traceback.clone(),
          }
          .as_child_of(parent_message),
        )
        .await?;
      connection
        .send(
          messaging::ExecuteReply {
            execution_count: self.execution_count,
            status: ReplyStatus::Error,
            error: Some(ReplyError {
              ename,
              evalue,
              traceback,
            }),
            user_expressions: None,
            payload: Default::default(),
          }
          .as_child_of(parent_message),
        )
        .await?;
    }

    Ok(())
  }

  async fn send_iopub(
    &mut self,
    message: JupyterMessage,
  ) -> Result<(), AnyError> {
    self.iopub_connection.lock().send(message).await
  }
}

fn kernel_info() -> messaging::KernelInfoReply {
  messaging::KernelInfoReply {
    status: ReplyStatus::Ok,
    protocol_version: "5.3".to_string(),
    implementation: "Deno kernel".to_string(),
    implementation_version: crate::version::deno().to_string(),
    language_info: messaging::LanguageInfo {
      name: "typescript".to_string(),
      version: crate::version::TYPESCRIPT.to_string(),
      mimetype: "text/x.typescript".to_string(),
      file_extension: ".ts".to_string(),
      pygments_lexer: "typescript".to_string(),
      codemirror_mode: messaging::CodeMirrorMode::typescript(),
      nbconvert_exporter: "script".to_string(),
    },
    banner: "Welcome to Deno kernel".to_string(),
    help_links: vec![messaging::HelpLink {
      text: "Visit Deno manual".to_string(),
      url: "https://docs.deno.com".to_string(),
    }],
    debugger: false,
    error: None,
  }
}

async fn publish_result(
  repl_session_proxy: &mut JupyterReplProxy,
  evaluate_result: &cdp::RemoteObject,
  execution_count: usize,
) -> Result<Option<HashMap<String, serde_json::Value>>, AnyError> {
  let arg0 = cdp::CallArgument {
    value: Some(serde_json::Value::Number(execution_count.into())),
    unserializable_value: None,
    object_id: None,
  };

  let arg1 = cdp::CallArgument::from(evaluate_result);

  let Some(response) = repl_session_proxy.call_function_on(arg0, arg1).await
  else {
    return Ok(None);
  };

  if let Some(exception_details) = &response.exception_details {
    // If the object doesn't have a Jupyter.display method or it throws an
    // exception, we just ignore it and let the caller handle it.
    log::error!("Exception encountered: {}", exception_details.text);
    return Ok(None);
  }

  Ok(None)
}

// TODO(bartlomieju): dedup with repl::editor
fn get_expr_from_line_at_pos(line: &str, cursor_pos: usize) -> &str {
  let start = line[..cursor_pos].rfind(is_word_boundary).unwrap_or(0);
  let end = line[cursor_pos..]
    .rfind(is_word_boundary)
    .map(|i| cursor_pos + i)
    .unwrap_or(cursor_pos);

  let word = &line[start..end];
  let word = word.strip_prefix(is_word_boundary).unwrap_or(word);
  let word = word.strip_suffix(is_word_boundary).unwrap_or(word);

  word
}

// TODO(bartlomieju): dedup with repl::editor
fn is_word_boundary(c: char) -> bool {
  if matches!(c, '.' | '_' | '$') {
    false
  } else {
    char::is_ascii_whitespace(&c) || char::is_ascii_punctuation(&c)
  }
}

// TODO(bartlomieju): dedup with repl::editor
async fn get_global_lexical_scope_names(
  repl_session_proxy: &mut JupyterReplProxy,
) -> Vec<String> {
  repl_session_proxy.global_lexical_scope_names().await.names
}

// TODO(bartlomieju): dedup with repl::editor
async fn get_expression_property_names(
  repl_session_proxy: &mut JupyterReplProxy,
  expr: &str,
) -> Vec<String> {
  // try to get the properties from the expression
  if let Some(properties) =
    get_object_expr_properties(repl_session_proxy, expr).await
  {
    return properties;
  }

  // otherwise fall back to the prototype
  let expr_type = get_expression_type(repl_session_proxy, expr).await;
  let object_expr = match expr_type.as_deref() {
    // possibilities: https://chromedevtools.github.io/devtools-protocol/v8/Runtime/#type-RemoteObject
    Some("object") => "Object.prototype",
    Some("function") => "Function.prototype",
    Some("string") => "String.prototype",
    Some("boolean") => "Boolean.prototype",
    Some("bigint") => "BigInt.prototype",
    Some("number") => "Number.prototype",
    _ => return Vec::new(), // undefined, symbol, and unhandled
  };

  get_object_expr_properties(repl_session_proxy, object_expr)
    .await
    .unwrap_or_default()
}

// TODO(bartlomieju): dedup with repl::editor
async fn get_expression_type(
  repl_session_proxy: &mut JupyterReplProxy,
  expr: &str,
) -> Option<String> {
  evaluate_expression(repl_session_proxy, expr)
    .await
    .map(|res| res.result.kind)
}

// TODO(bartlomieju): dedup with repl::editor
async fn get_object_expr_properties(
  repl_session_proxy: &mut JupyterReplProxy,
  object_expr: &str,
) -> Option<Vec<String>> {
  let evaluate_result =
    evaluate_expression(repl_session_proxy, object_expr).await?;
  let object_id = evaluate_result.result.object_id?;

  let get_properties_response =
    repl_session_proxy.get_properties(object_id.clone()).await?;
  Some(
    get_properties_response
      .result
      .into_iter()
      .map(|prop| prop.name)
      .collect(),
  )
}

// TODO(bartlomieju): dedup with repl::editor
async fn evaluate_expression(
  repl_session_proxy: &mut JupyterReplProxy,
  expr: &str,
) -> Option<cdp::EvaluateResponse> {
  let evaluate_response = repl_session_proxy.evaluate(expr.to_string()).await?;
  if evaluate_response.exception_details.is_some() {
    None
  } else {
    Some(evaluate_response)
  }
}
