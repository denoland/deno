// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// This file is forked/ported from <https://github.com/evcxr/evcxr>
// Copyright 2020 The Evcxr Authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::tools::repl;
use crate::tools::repl::cdp;
use deno_core::error::AnyError;
use deno_core::futures;
use deno_core::futures::channel::mpsc;
use deno_core::futures::future::Either;
use deno_core::futures::FutureExt;
use deno_core::futures::SinkExt;
use deno_core::futures::StreamExt;
use deno_core::serde_json;
use deno_core::serde_json::json;
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
    repl_session: repl::ReplSession,
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

    let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded();

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
      if let Err(err) = Self::handle_control(control_socket, shutdown_tx).await
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
      while let Some(stdio_msg) = stdio_rx.next().await {
        if let Some(exec_request) = last_execution_request.borrow().clone() {
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
    });

    let shutdown_fut = async move {
      let _ = shutdown_rx.next().await;
    }
    .boxed_local();
    let join_fut =
      futures::future::try_join_all(vec![handle1, handle2, handle3, handle4]);
    if let Either::Left((join_fut, _)) =
      futures::future::select(join_fut, shutdown_fut).await
    {
      join_fut?;
    };

    Ok(())
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
    mut shutdown_tx: mpsc::UnboundedSender<()>,
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
          let _ = shutdown_tx.send(()).await;
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

    let evaluate_response = self
      .repl_session
      .evaluate_line_with_object_wrapping(msg.code())
      .await?;

    let repl::cdp::EvaluateResponse {
      result,
      exception_details,
    } = evaluate_response.value;

    if exception_details.is_none() {
      let output =
        get_jupyter_display_or_eval_value(&mut self.repl_session, &result)
          .await?;
      msg
        .new_message("execute_result")
        .with_content(json!({
            "execution_count": self.execution_count,
            "data": output,
            "metadata": {},
        }))
        .send(&mut *self.iopub_socket.lock().await)
        .await?;
      msg
        .new_reply()
        .with_content(json!({
            "status": "ok",
            "execution_count": self.execution_count,
        }))
        .send(connection)
        .await?;
    } else {
      let exception_details = exception_details.unwrap();
      let name = if let Some(exception) = exception_details.exception {
        if let Some(description) = exception.description {
          description
        } else if let Some(value) = exception.value {
          value.to_string()
        } else {
          "undefined".to_string()
        }
      } else {
        "Unknown exception".to_string()
      };

      // TODO: fill all the fields
      msg
        .new_message("error")
        .with_content(json!({
          "ename": name,
          "evalue": "",
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
      // TODO(bartlomieju):
      // "nb_converter":
    },
    "help_links": [{
      "text": "Visit Deno manual",
      "url": "https://deno.land/manual"
    }],
    "banner": "Welcome to Deno kernel",
  })
}

async fn get_jupyter_display_or_eval_value(
  session: &mut repl::ReplSession,
  evaluate_result: &cdp::RemoteObject,
) -> Result<HashMap<String, serde_json::Value>, AnyError> {
  let mut data = HashMap::default();
  let response = session
    .call_function_on_args(
      r#"function (object) {{
        return object[Symbol.for("Jupyter.display")]();
      }}"#
        .to_string(),
      &[evaluate_result.clone()],
    )
    .await?;

  if response.exception_details.is_none() {
    let object_id = response.result.object_id.unwrap();

    if let Some(get_properties_response) = session
      .post_message_with_event_loop(
        "Runtime.getProperties",
        Some(cdp::GetPropertiesArgs {
          object_id,
          own_properties: Some(true),
          accessor_properties_only: None,
          generate_preview: None,
          non_indexed_properties_only: Some(true),
        }),
      )
      .await
      .ok()
    {
      let get_properties_response: cdp::GetPropertiesResponse =
        serde_json::from_value(get_properties_response).unwrap();

      for prop in get_properties_response.result.into_iter() {
        if let Some(value) = &prop.value {
          data.insert(
            prop.name.to_string(),
            value
              .value
              .clone()
              .unwrap_or_else(|| serde_json::Value::Null),
          );
        }
      }

      if !data.is_empty() {
        return Ok(data);
      }
    }
  }

  let response = session
    .call_function_on_args(
      format!(
        r#"function (object) {{
        try {{
          return {0}.inspectArgs(["%o", object], {{ colors: !{0}.noColor }});
        }} catch (err) {{
          return {0}.inspectArgs(["%o", err]);
        }}
      }}"#,
        *repl::REPL_INTERNALS_NAME
      ),
      &[evaluate_result.clone()],
    )
    .await?;
  let value = response.result.value.unwrap();
  data.insert("text/plain".to_string(), value);
  Ok(data)
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
