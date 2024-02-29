// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// This file is forked/ported from <https://github.com/evcxr/evcxr>
// Copyright 2020 The Evcxr Authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::cdp;
use crate::tools::repl;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use zeromq::SocketRecv;
use zeromq::SocketSend;

use super::jupyter_msg::Connection;
use super::jupyter_msg::JupyterMessage;
use super::ConnectionSpec;

pub enum StdioMsg {
  Stdout(String),
  Stderr(String),
}

pub struct JupyterServer {
  execution_count: usize,
  last_execution_request: Rc<RefCell<Option<JupyterMessage>>>,
  // This is Arc<Mutex<>>, so we don't hold RefCell borrows across await
  // points.
  iopub_socket: Arc<Mutex<Connection<zeromq::PubSocket>>>,
  repl_session: repl::ReplSession,
}

impl JupyterServer {
  pub async fn start(
    spec: ConnectionSpec,
    mut stdio_rx: mpsc::UnboundedReceiver<StdioMsg>,
    mut repl_session: repl::ReplSession,
  ) -> Result<(), AnyError> {
    let mut heartbeat =
      bind_socket::<zeromq::RepSocket>(&spec, spec.hb_port).await?;
    let shell_socket =
      bind_socket::<zeromq::RouterSocket>(&spec, spec.shell_port).await?;
    let control_socket =
      bind_socket::<zeromq::RouterSocket>(&spec, spec.control_port).await?;
    let _stdin_socket =
      bind_socket::<zeromq::RouterSocket>(&spec, spec.stdin_port).await?;
    let iopub_socket =
      bind_socket::<zeromq::PubSocket>(&spec, spec.iopub_port).await?;
    let iopub_socket = Arc::new(Mutex::new(iopub_socket));
    let last_execution_request = Rc::new(RefCell::new(None));

    // Store `iopub_socket` in the op state so it's accessible to the runtime API.
    {
      let op_state_rc = repl_session.worker.js_runtime.op_state();
      let mut op_state = op_state_rc.borrow_mut();
      op_state.put(iopub_socket.clone());
      op_state.put(last_execution_request.clone());
    }

    let cancel_handle = CancelHandle::new_rc();
    let cancel_handle2 = CancelHandle::new_rc();

    let mut server = Self {
      execution_count: 0,
      iopub_socket: iopub_socket.clone(),
      last_execution_request: last_execution_request.clone(),
      repl_session,
    };

    let handle1 = deno_core::unsync::spawn(async move {
      if let Err(err) = Self::handle_heartbeat(&mut heartbeat).await {
        eprintln!("Heartbeat error: {}", err);
      }
    });

    let handle2 = deno_core::unsync::spawn(async move {
      if let Err(err) =
        Self::handle_control(control_socket, cancel_handle2).await
      {
        eprintln!("Control error: {}", err);
      }
    });

    let handle3 = deno_core::unsync::spawn(async move {
      if let Err(err) = server.handle_shell(shell_socket).await {
        eprintln!("Shell error: {}", err);
      }
    });

    let handle4 = deno_core::unsync::spawn(async move {
      while let Some(stdio_msg) = stdio_rx.recv().await {
        Self::handle_stdio_msg(
          iopub_socket.clone(),
          last_execution_request.clone(),
          stdio_msg,
        )
        .await;
      }
    });

    let join_fut =
      futures::future::try_join_all(vec![handle1, handle2, handle3, handle4]);

    if let Ok(result) = join_fut.or_cancel(cancel_handle).await {
      result?;
    }

    Ok(())
  }

  async fn handle_stdio_msg<S: zeromq::SocketSend>(
    iopub_socket: Arc<Mutex<Connection<S>>>,
    last_execution_request: Rc<RefCell<Option<JupyterMessage>>>,
    stdio_msg: StdioMsg,
  ) {
    let maybe_exec_result = last_execution_request.borrow().clone();
    if let Some(exec_request) = maybe_exec_result {
      let (name, text) = match stdio_msg {
        StdioMsg::Stdout(text) => ("stdout", text),
        StdioMsg::Stderr(text) => ("stderr", text),
      };

      let result = exec_request
        .new_message("stream")
        .with_content(json!({
            "name": name,
            "text": text
        }))
        .send(&mut *iopub_socket.lock().await)
        .await;

      if let Err(err) = result {
        eprintln!("Output {} error: {}", name, err);
      }
    }
  }

  async fn handle_heartbeat(
    connection: &mut Connection<zeromq::RepSocket>,
  ) -> Result<(), AnyError> {
    loop {
      connection.socket.recv().await?;
      connection
        .socket
        .send(zeromq::ZmqMessage::from(b"ping".to_vec()))
        .await?;
    }
  }

  async fn handle_control(
    mut connection: Connection<zeromq::RouterSocket>,
    cancel_handle: Rc<CancelHandle>,
  ) -> Result<(), AnyError> {
    loop {
      let msg = JupyterMessage::read(&mut connection).await?;
      match msg.message_type() {
        "kernel_info_request" => {
          msg
            .new_reply()
            .with_content(kernel_info())
            .send(&mut connection)
            .await?;
        }
        "shutdown_request" => {
          cancel_handle.cancel();
        }
        "interrupt_request" => {
          eprintln!("Interrupt request currently not supported");
        }
        _ => {
          eprintln!(
            "Unrecognized control message type: {}",
            msg.message_type()
          );
        }
      }
    }
  }

  async fn handle_shell(
    &mut self,
    mut connection: Connection<zeromq::RouterSocket>,
  ) -> Result<(), AnyError> {
    loop {
      let msg = JupyterMessage::read(&mut connection).await?;
      self.handle_shell_message(msg, &mut connection).await?;
    }
  }

  async fn handle_shell_message(
    &mut self,
    msg: JupyterMessage,
    connection: &mut Connection<zeromq::RouterSocket>,
  ) -> Result<(), AnyError> {
    msg
      .new_message("status")
      .with_content(json!({"execution_state": "busy"}))
      .send(&mut *self.iopub_socket.lock().await)
      .await?;

    match msg.message_type() {
      "kernel_info_request" => {
        msg
          .new_reply()
          .with_content(kernel_info())
          .send(connection)
          .await?;
      }
      "is_complete_request" => {
        msg
          .new_reply()
          .with_content(json!({"status": "complete"}))
          .send(connection)
          .await?;
      }
      "execute_request" => {
        self
          .handle_execution_request(msg.clone(), connection)
          .await?;
      }
      "comm_open" => {
        msg
          .comm_close_message()
          .send(&mut *self.iopub_socket.lock().await)
          .await?;
      }
      "complete_request" => {
        let user_code = msg.code();
        let cursor_pos = msg.cursor_pos();

        let lsp_completions = self
          .repl_session
          .language_server
          .completions(user_code, cursor_pos)
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

          msg
            .new_reply()
            .with_content(json!({
              "status": "ok",
              "matches": matches,
              "cursor_start": cursor_start,
              "cursor_end": cursor_end,
              "metadata": {},
            }))
            .send(connection)
            .await?;
        } else {
          let expr = get_expr_from_line_at_pos(user_code, cursor_pos);
          // check if the expression is in the form `obj.prop`
          let (completions, cursor_start) = if let Some(index) = expr.rfind('.')
          {
            let sub_expr = &expr[..index];
            let prop_name = &expr[index + 1..];
            let candidates =
              get_expression_property_names(&mut self.repl_session, sub_expr)
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
              &mut self.repl_session,
              "globalThis",
            )
            .await
            .into_iter()
            .chain(get_global_lexical_scope_names(&mut self.repl_session).await)
            .filter(|n| n.starts_with(expr) && n != &*repl::REPL_INTERNALS_NAME)
            .collect::<Vec<_>>();

            // sort and remove duplicates
            candidates.sort();
            candidates.dedup(); // make sure to sort first

            (candidates, cursor_pos - expr.len())
          };
          msg
            .new_reply()
            .with_content(json!({
              "status": "ok",
              "matches": completions,
              "cursor_start": cursor_start,
              "cursor_end": cursor_pos,
              "metadata": {},
            }))
            .send(connection)
            .await?;
        }
      }
      "comm_msg" | "comm_info_request" | "history_request" => {
        // We don't handle these messages
      }
      _ => {
        eprintln!("Unrecognized shell message type: {}", msg.message_type());
      }
    }

    msg
      .new_message("status")
      .with_content(json!({"execution_state": "idle"}))
      .send(&mut *self.iopub_socket.lock().await)
      .await?;
    Ok(())
  }

  async fn handle_execution_request(
    &mut self,
    msg: JupyterMessage,
    connection: &mut Connection<zeromq::RouterSocket>,
  ) -> Result<(), AnyError> {
    self.execution_count += 1;
    *self.last_execution_request.borrow_mut() = Some(msg.clone());

    msg
      .new_message("execute_input")
      .with_content(json!({
          "execution_count": self.execution_count,
          "code": msg.code()
      }))
      .send(&mut *self.iopub_socket.lock().await)
      .await?;

    let result = self
      .repl_session
      .evaluate_line_with_object_wrapping(msg.code())
      .await;

    let evaluate_response = match result {
      Ok(eval_response) => eval_response,
      Err(err) => {
        msg
          .new_message("error")
          .with_content(json!({
            "ename": err.to_string(),
            "evalue": " ", // Fake value, otherwise old Jupyter frontends don't show the error
            "traceback": [],
          }))
          .send(&mut *self.iopub_socket.lock().await)
          .await?;
        msg
          .new_reply()
          .with_content(json!({
            "status": "error",
            "execution_count": self.execution_count,
          }))
          .send(connection)
          .await?;
        return Ok(());
      }
    };

    let cdp::EvaluateResponse {
      result,
      exception_details,
    } = evaluate_response.value;

    if exception_details.is_none() {
      publish_result(&mut self.repl_session, &result, self.execution_count)
        .await?;

      msg
        .new_reply()
        .with_content(json!({
            "status": "ok",
            "execution_count": self.execution_count,
        }))
        .send(connection)
        .await?;
      // Let's sleep here for a few ms, so we give a chance to the task that is
      // handling stdout and stderr streams to receive and flush the content.
      // Otherwise, executing multiple cells one-by-one might lead to output
      // from various cells be grouped together in another cell result.
      tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    } else if let Some(exception_details) = exception_details {
      // Determine the exception value and name
      let (name, message, stack) =
        if let Some(exception) = exception_details.exception {
          let result = self
            .repl_session
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
              &[exception],
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
                eprintln!("Unexpected result while parsing JSON {str}");
                ("".into(), "".into(), "".into())
              }
            }
            _ => {
              eprintln!("Unexpected result while parsing exception {result:?}");
              ("".into(), "".into(), "".into())
            }
          }
        } else {
          eprintln!("Unexpectedly missing exception {exception_details:?}");
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

      msg
        .new_message("error")
        .with_content(json!({
          "ename": ename,
          "evalue": evalue,
          "traceback": traceback,
        }))
        .send(&mut *self.iopub_socket.lock().await)
        .await?;
      msg
        .new_reply()
        .with_content(json!({
          "status": "error",
          "execution_count": self.execution_count,
        }))
        .send(connection)
        .await?;
    }

    Ok(())
  }
}

async fn bind_socket<S: zeromq::Socket>(
  config: &ConnectionSpec,
  port: u32,
) -> Result<Connection<S>, AnyError> {
  let endpoint = format!("{}://{}:{}", config.transport, config.ip, port);
  let mut socket = S::new();
  socket.bind(&endpoint).await?;
  Ok(Connection::new(socket, &config.key))
}

fn kernel_info() -> serde_json::Value {
  json!({
    "status": "ok",
    "protocol_version": "5.3",
    "implementation_version": crate::version::deno(),
    "implementation": "Deno kernel",
    "language_info": {
      "name": "typescript",
      "version": crate::version::TYPESCRIPT,
      "mimetype": "text/x.typescript",
      "file_extension": ".ts",
      "pygments_lexer": "typescript",
      "nb_converter": "script"
    },
    "help_links": [{
      "text": "Visit Deno manual",
      "url": "https://deno.land/manual"
    }],
    "banner": "Welcome to Deno kernel",
  })
}

async fn publish_result(
  session: &mut repl::ReplSession,
  evaluate_result: &cdp::RemoteObject,
  execution_count: usize,
) -> Result<Option<HashMap<String, serde_json::Value>>, AnyError> {
  let arg0 = cdp::CallArgument {
    value: Some(serde_json::Value::Number(execution_count.into())),
    unserializable_value: None,
    object_id: None,
  };

  let arg1 = cdp::CallArgument::from(evaluate_result);

  let response = session
    .post_message_with_event_loop(
      "Runtime.callFunctionOn",
      Some(json!({
        "functionDeclaration": r#"async function (execution_count, result) {
          await Deno[Deno.internal].jupyter.broadcastResult(execution_count, result);
    }"#,
        "arguments": [arg0, arg1],
        "executionContextId": session.context_id,
        "awaitPromise": true,
      })),
    )
    .await?;

  let response: cdp::CallFunctionOnResponse = serde_json::from_value(response)?;

  if let Some(exception_details) = &response.exception_details {
    // If the object doesn't have a Jupyter.display method or it throws an
    // exception, we just ignore it and let the caller handle it.
    eprintln!("Exception encountered: {}", exception_details.text);
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
  session: &mut repl::ReplSession,
) -> Vec<String> {
  let evaluate_response = session
    .post_message_with_event_loop(
      "Runtime.globalLexicalScopeNames",
      Some(cdp::GlobalLexicalScopeNamesArgs {
        execution_context_id: Some(session.context_id),
      }),
    )
    .await
    .unwrap();
  let evaluate_response: cdp::GlobalLexicalScopeNamesResponse =
    serde_json::from_value(evaluate_response).unwrap();
  evaluate_response.names
}

// TODO(bartlomieju): dedup with repl::editor
async fn get_expression_property_names(
  session: &mut repl::ReplSession,
  expr: &str,
) -> Vec<String> {
  // try to get the properties from the expression
  if let Some(properties) = get_object_expr_properties(session, expr).await {
    return properties;
  }

  // otherwise fall back to the prototype
  let expr_type = get_expression_type(session, expr).await;
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

  get_object_expr_properties(session, object_expr)
    .await
    .unwrap_or_default()
}

// TODO(bartlomieju): dedup with repl::editor
async fn get_expression_type(
  session: &mut repl::ReplSession,
  expr: &str,
) -> Option<String> {
  evaluate_expression(session, expr)
    .await
    .map(|res| res.result.kind)
}

// TODO(bartlomieju): dedup with repl::editor
async fn get_object_expr_properties(
  session: &mut repl::ReplSession,
  object_expr: &str,
) -> Option<Vec<String>> {
  let evaluate_result = evaluate_expression(session, object_expr).await?;
  let object_id = evaluate_result.result.object_id?;

  let get_properties_response = session
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
    .await
    .ok()?;
  let get_properties_response: cdp::GetPropertiesResponse =
    serde_json::from_value(get_properties_response).ok()?;
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
  session: &mut repl::ReplSession,
  expr: &str,
) -> Option<cdp::EvaluateResponse> {
  let evaluate_response = session
    .post_message_with_event_loop(
      "Runtime.evaluate",
      Some(cdp::EvaluateArgs {
        expression: expr.to_string(),
        object_group: None,
        include_command_line_api: None,
        silent: None,
        context_id: Some(session.context_id),
        return_by_value: None,
        generate_preview: None,
        user_gesture: None,
        await_promise: None,
        throw_on_side_effect: Some(true),
        timeout: Some(200),
        disable_breaks: None,
        repl_mode: None,
        allow_unsafe_eval_blocked_by_csp: None,
        unique_context_id: None,
      }),
    )
    .await
    .ok()?;
  let evaluate_response: cdp::EvaluateResponse =
    serde_json::from_value(evaluate_response).ok()?;

  if evaluate_response.exception_details.is_some() {
    None
  } else {
    Some(evaluate_response)
  }
}
