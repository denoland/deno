// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// This file is forked/ported from <https://github.com/evcxr/evcxr>
// Copyright 2020 The Evcxr Authors. MIT license.

use bytes::Bytes;
use data_encoding::HEXLOWER;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use ring::hmac;
use std::fmt;
use uuid::Uuid;

use crate::util::time::utc_now;

pub(crate) struct Connection<S> {
  pub(crate) socket: S,
  /// Will be None if our key was empty (digest authentication disabled).
  pub(crate) mac: Option<hmac::Key>,
}

impl<S: zeromq::Socket> Connection<S> {
  pub(crate) fn new(socket: S, key: &str) -> Self {
    let mac = if key.is_empty() {
      None
    } else {
      Some(hmac::Key::new(hmac::HMAC_SHA256, key.as_bytes()))
    };
    Connection { socket, mac }
  }
}

struct RawMessage {
  zmq_identities: Vec<Bytes>,
  jparts: Vec<Bytes>,
}

impl RawMessage {
  pub(crate) async fn read<S: zeromq::SocketRecv>(
    connection: &mut Connection<S>,
  ) -> Result<RawMessage, AnyError> {
    Self::from_multipart(connection.socket.recv().await?, connection)
  }

  pub(crate) fn from_multipart<S>(
    multipart: zeromq::ZmqMessage,
    connection: &Connection<S>,
  ) -> Result<RawMessage, AnyError> {
    let delimiter_index = multipart
      .iter()
      .position(|part| &part[..] == DELIMITER)
      .ok_or_else(|| anyhow!("Missing delimiter"))?;
    let mut parts = multipart.into_vec();
    let jparts: Vec<_> = parts.drain(delimiter_index + 2..).collect();
    let expected_hmac = parts.pop().unwrap();
    // Remove delimiter, so that what's left is just the identities.
    parts.pop();
    let zmq_identities = parts;

    let raw_message = RawMessage {
      zmq_identities,
      jparts,
    };

    if let Some(key) = &connection.mac {
      let sig = HEXLOWER.decode(&expected_hmac)?;
      let mut msg = Vec::new();
      for part in &raw_message.jparts {
        msg.extend(part);
      }

      if let Err(err) = hmac::verify(key, msg.as_ref(), sig.as_ref()) {
        bail!("{}", err);
      }
    }

    Ok(raw_message)
  }

  async fn send<S: zeromq::SocketSend>(
    self,
    connection: &mut Connection<S>,
  ) -> Result<(), AnyError> {
    let hmac = if let Some(key) = &connection.mac {
      let ctx = self.digest(key);
      let tag = ctx.sign();
      HEXLOWER.encode(tag.as_ref())
    } else {
      String::new()
    };
    let mut parts: Vec<bytes::Bytes> = Vec::new();
    for part in &self.zmq_identities {
      parts.push(part.to_vec().into());
    }
    parts.push(DELIMITER.into());
    parts.push(hmac.as_bytes().to_vec().into());
    for part in &self.jparts {
      parts.push(part.to_vec().into());
    }
    // ZmqMessage::try_from only fails if parts is empty, which it never
    // will be here.
    let message = zeromq::ZmqMessage::try_from(parts).unwrap();
    connection.socket.send(message).await?;
    Ok(())
  }

  fn digest(&self, mac: &hmac::Key) -> hmac::Context {
    let mut hmac_ctx = hmac::Context::with_key(mac);
    for part in &self.jparts {
      hmac_ctx.update(part);
    }
    hmac_ctx
  }
}

#[derive(Clone)]
pub(crate) struct JupyterMessage {
  zmq_identities: Vec<Bytes>,
  header: serde_json::Value,
  parent_header: serde_json::Value,
  metadata: serde_json::Value,
  content: serde_json::Value,
  buffers: Vec<Bytes>,
}

const DELIMITER: &[u8] = b"<IDS|MSG>";

impl JupyterMessage {
  pub(crate) async fn read<S: zeromq::SocketRecv>(
    connection: &mut Connection<S>,
  ) -> Result<JupyterMessage, AnyError> {
    Self::from_raw_message(RawMessage::read(connection).await?)
  }

  fn from_raw_message(
    raw_message: RawMessage,
  ) -> Result<JupyterMessage, AnyError> {
    if raw_message.jparts.len() < 4 {
      bail!("Insufficient message parts {}", raw_message.jparts.len());
    }

    Ok(JupyterMessage {
      zmq_identities: raw_message.zmq_identities,
      header: serde_json::from_slice(&raw_message.jparts[0])?,
      parent_header: serde_json::from_slice(&raw_message.jparts[1])?,
      metadata: serde_json::from_slice(&raw_message.jparts[2])?,
      content: serde_json::from_slice(&raw_message.jparts[3])?,
      buffers: if raw_message.jparts.len() > 4 {
        raw_message.jparts[4..].to_vec()
      } else {
        vec![]
      },
    })
  }

  pub(crate) fn message_type(&self) -> &str {
    self.header["msg_type"].as_str().unwrap_or("")
  }

  pub(crate) fn code(&self) -> &str {
    self.content["code"].as_str().unwrap_or("")
  }

  pub(crate) fn cursor_pos(&self) -> usize {
    self.content["cursor_pos"].as_u64().unwrap_or(0) as usize
  }

  pub(crate) fn comm_id(&self) -> &str {
    self.content["comm_id"].as_str().unwrap_or("")
  }

  // Creates a new child message of this message. ZMQ identities are not transferred.
  pub(crate) fn new_message(&self, msg_type: &str) -> JupyterMessage {
    let mut header = self.header.clone();
    header["msg_type"] = serde_json::Value::String(msg_type.to_owned());
    header["username"] = serde_json::Value::String("kernel".to_owned());
    header["msg_id"] = serde_json::Value::String(Uuid::new_v4().to_string());
    header["date"] = serde_json::Value::String(utc_now().to_rfc3339());

    JupyterMessage {
      zmq_identities: Vec::new(),
      header,
      parent_header: self.header.clone(),
      metadata: json!({}),
      content: json!({}),
      buffers: vec![],
    }
  }

  // Creates a reply to this message. This is a child with the message type determined
  // automatically by replacing "request" with "reply". ZMQ identities are transferred.
  pub(crate) fn new_reply(&self) -> JupyterMessage {
    let mut reply =
      self.new_message(&self.message_type().replace("_request", "_reply"));
    reply.zmq_identities = self.zmq_identities.clone();
    reply
  }

  #[must_use = "Need to send this message for it to have any effect"]
  pub(crate) fn comm_close_message(&self) -> JupyterMessage {
    self.new_message("comm_close").with_content(json!({
      "comm_id": self.comm_id()
    }))
  }

  pub(crate) fn with_content(
    mut self,
    content: serde_json::Value,
  ) -> JupyterMessage {
    self.content = content;
    self
  }

  pub(crate) fn with_metadata(
    mut self,
    metadata: serde_json::Value,
  ) -> JupyterMessage {
    self.metadata = metadata;
    self
  }

  pub(crate) fn with_buffers(mut self, buffers: Vec<Bytes>) -> JupyterMessage {
    self.buffers = buffers;
    self
  }

  pub(crate) async fn send<S: zeromq::SocketSend>(
    &self,
    connection: &mut Connection<S>,
  ) -> Result<(), AnyError> {
    // If performance is a concern, we can probably avoid the clone and to_vec calls with a bit
    // of refactoring.
    let mut jparts: Vec<Bytes> = vec![
      serde_json::to_string(&self.header)
        .unwrap()
        .as_bytes()
        .to_vec()
        .into(),
      serde_json::to_string(&self.parent_header)
        .unwrap()
        .as_bytes()
        .to_vec()
        .into(),
      serde_json::to_string(&self.metadata)
        .unwrap()
        .as_bytes()
        .to_vec()
        .into(),
      serde_json::to_string(&self.content)
        .unwrap()
        .as_bytes()
        .to_vec()
        .into(),
    ];
    jparts.extend_from_slice(&self.buffers);
    let raw_message = RawMessage {
      zmq_identities: self.zmq_identities.clone(),
      jparts,
    };
    raw_message.send(connection).await
  }
}

impl fmt::Debug for JupyterMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(
      f,
      "\nHeader: {}",
      serde_json::to_string_pretty(&self.header).unwrap()
    )?;
    writeln!(
      f,
      "Parent header: {}",
      serde_json::to_string_pretty(&self.parent_header).unwrap()
    )?;
    writeln!(
      f,
      "Metadata: {}",
      serde_json::to_string_pretty(&self.metadata).unwrap()
    )?;
    writeln!(
      f,
      "Content: {}\n",
      serde_json::to_string_pretty(&self.content).unwrap()
    )?;
    Ok(())
  }
}
