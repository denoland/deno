// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// This file is forked/ported from <https://github.com/evcxr/evcxr>
// Copyright 2020 The Evcxr Authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::tools::repl;
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

        let completions = self.repl_session.language_server.completions(user_code, cursor_pos).await;

        let matches: Vec<String> = completions
          .iter()
          .map(|item| item.new_text.clone())
          .collect();

        let cursor_start = completions
          .first()
          .map(|item| item.range.start)
          .unwrap_or(cursor_pos);

        let cursor_end = completions
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
      let output = self.repl_session.get_eval_value(&result).await?;
      msg
        .new_message("execute_result")
        .with_content(json!({
            "execution_count": self.execution_count,
            "data": {
                "text/plain": output
            },
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
