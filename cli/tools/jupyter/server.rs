// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// This file is forked/ported from <https://github.com/evcxr/evcxr>
// Copyright 2020 The Evcxr Authors. MIT license.

use std::path::Path;

use crate::args::Flags;
use crate::args::JupyterFlags;
use crate::tools::repl;
use crate::util::logger;
use crate::CliFactory;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::mpsc;
use deno_core::futures::StreamExt;
use deno_core::op;
use deno_core::resolve_url_or_path;
use deno_core::serde::Deserialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::Op;
use deno_core::OpState;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsContainer;
use ring::hmac;
use zeromq::SocketRecv;
use zeromq::SocketSend;

use super::jupyter_msg::Connection;
use super::jupyter_msg::JupyterMessage;
use super::ConnectionSpec;

struct JupyterServer {
  execution_count: usize,
  iopub_socket: Connection<zeromq::PubSocket>,
  repl_session: repl::ReplSession,
}

impl JupyterServer {
  async fn start(
    connection_filepath: &Path,
    stdio_rx: mpsc::UnboundedReceiver<()>,
    repl_session: repl::ReplSession,
  ) -> Result<(), AnyError> {
    let conn_file =
      std::fs::read_to_string(connection_filepath).with_context(|| {
        format!("Couldn't read connection file: {:?}", connection_filepath)
      })?;
    let spec: ConnectionSpec =
      serde_json::from_str(&conn_file).with_context(|| {
        format!(
          "Connection file is not a valid JSON: {:?}",
          connection_filepath
        )
      })?;

    let mut heartbeat =
      bind_socket::<zeromq::RepSocket>(&spec, spec.hb_port).await?;
    let shell_socket =
      bind_socket::<zeromq::RouterSocket>(&spec, spec.shell_port).await?;
    let control_socket =
      bind_socket::<zeromq::RouterSocket>(&spec, spec.control_port).await?;
    let stdin_socket =
      bind_socket::<zeromq::RouterSocket>(&spec, spec.stdin_port).await?;
    let iopub_socket =
      bind_socket::<zeromq::PubSocket>(&spec, spec.iopub_port).await?;

    let mut server = Self {
      execution_count: 0,
      iopub_socket,
      repl_session,
    };

    deno_core::unsync::spawn(async move {
      if let Err(err) = Self::handle_heartbeat(&mut heartbeat).await {
        eprintln!("Heartbeat error: {}", err);
      }
    });

    deno_core::unsync::spawn(async move {
      if let Err(err) = Self::handle_control(control_socket).await {
        eprintln!("Control error: {}", err);
      }
    });

    deno_core::unsync::spawn(async move {
      if let Err(err) = server.handle_shell(shell_socket).await {
        eprintln!("Shell error: {}", err);
      }
    });

    deno_core::unsync::spawn(async move {
      if let Err(err) = Self::handle_execution_requests().await {
        eprintln!("Execution error: {}", err);
      }
    });
    todo!()
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
        "shutdown_request" => todo!(),
        "interrupt_request" => todo!(),
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
      .send(&mut self.iopub_socket)
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
      "execute_request" => todo!(),
      "comm_open" => {
        msg
          .comm_close_message()
          .send(&mut self.iopub_socket)
          .await?;
      }
      "complete_request" => todo!(),
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
      .send(&mut self.iopub_socket)
      .await?;
    Ok(())
  }

  async fn handle_execution_requests() -> Result<(), AnyError> {
    todo!();
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
      "file_extension": ".ts"
    },
    "help_links": [{
      "text": "Visit Deno manual",
      "url": "https://deno.land/manual"
    }],
    "banner": "Welcome to Deno kernel",
  })
}
