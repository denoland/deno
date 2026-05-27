// Copyright 2018-2026 the Deno authors. MIT license.

use std::path::PathBuf;

use aws_lc_rs::hmac;
use bytes::Bytes;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::Value;
use jupyter_protocol::ConnectionInfo;
use jupyter_protocol::JupyterMessage;
use jupyter_protocol::JupyterMessageContent;
use jupyter_protocol::messaging::Header;
use zeromq::Socket as _;
use zeromq::SocketRecv as _;
use zeromq::SocketSend as _;

pub struct Connection<S> {
  socket: S,
  mac: Option<hmac::Key>,
  session_id: String,
}

pub type KernelIoPubConnection = Connection<zeromq::PubSocket>;
pub type KernelShellConnection = Connection<zeromq::RouterSocket>;
pub type KernelControlConnection = Connection<zeromq::RouterSocket>;
pub type KernelStdinConnection = Connection<zeromq::RouterSocket>;

pub struct KernelHeartbeatConnection {
  socket: zeromq::RepSocket,
}

impl<S: zeromq::Socket> Connection<S> {
  fn new(socket: S, key: &str, session_id: &str) -> Self {
    let mac = if key.is_empty() {
      None
    } else {
      Some(hmac::Key::new(hmac::HMAC_SHA256, key.as_bytes()))
    };

    Self {
      socket,
      mac,
      session_id: session_id.to_string(),
    }
  }
}

impl<S: zeromq::SocketSend> Connection<S> {
  pub async fn send(
    &mut self,
    message: JupyterMessage,
  ) -> Result<(), AnyError> {
    let message = message.with_session(&self.session_id);
    let raw_message = RawMessage::from_jupyter_message(message)?;
    let zmq_message = raw_message.into_zmq_message(&self.mac)?;
    self.socket.send(zmq_message).await?;
    Ok(())
  }
}

impl<S: zeromq::SocketRecv> Connection<S> {
  pub async fn read(&mut self) -> Result<JupyterMessage, AnyError> {
    let raw_message =
      RawMessage::from_multipart(self.socket.recv().await?, &self.mac)?;
    raw_message.into_jupyter_message()
  }
}

impl KernelHeartbeatConnection {
  pub async fn single_heartbeat(&mut self) -> Result<(), AnyError> {
    let _msg = self.socket.recv().await?;
    self
      .socket
      .send(zeromq::ZmqMessage::from(b"pong".to_vec()))
      .await?;
    Ok(())
  }
}

#[derive(Debug)]
struct RawMessage {
  zmq_identities: Vec<Bytes>,
  jparts: Vec<Bytes>,
}

const DELIMITER: &[u8] = b"<IDS|MSG>";

impl RawMessage {
  fn from_multipart(
    multipart: zeromq::ZmqMessage,
    key: &Option<hmac::Key>,
  ) -> Result<Self, AnyError> {
    let delimiter_index = multipart
      .iter()
      .position(|part| &part[..] == DELIMITER)
      .ok_or_else(|| anyhow!("Missing delimiter"))?;
    let mut parts = multipart.into_vec();

    let jparts: Vec<_> = parts.drain(delimiter_index + 2..).collect();
    let expected_hmac = parts.pop().ok_or_else(|| anyhow!("Missing hmac"))?;
    parts.pop();

    let raw_message = Self {
      zmq_identities: parts,
      jparts,
    };

    if let Some(key) = key {
      let mut sig = vec![0; expected_hmac.len() / 2];
      faster_hex::hex_decode(&expected_hmac, &mut sig)?;
      let mut msg = Vec::new();
      for part in &raw_message.jparts[..4] {
        msg.extend(part);
      }

      hmac::verify(key, msg.as_ref(), sig.as_ref())
        .map_err(|_| anyhow!("HMAC verification failed"))?;
    }

    Ok(raw_message)
  }

  fn hmac(&self, key: &Option<hmac::Key>) -> String {
    if let Some(key) = key {
      let mut hmac_ctx = hmac::Context::with_key(key);
      for part in self.jparts.iter().take(4) {
        hmac_ctx.update(part);
      }
      faster_hex::hex_string(hmac_ctx.sign().as_ref())
    } else {
      String::new()
    }
  }

  fn into_zmq_message(
    self,
    key: &Option<hmac::Key>,
  ) -> Result<zeromq::ZmqMessage, AnyError> {
    let hmac = self.hmac(key);
    let mut parts: Vec<Bytes> = Vec::new();
    for part in &self.zmq_identities {
      parts.push(part.to_vec().into());
    }
    parts.push(DELIMITER.into());
    parts.push(hmac.as_bytes().to_vec().into());
    for part in &self.jparts {
      parts.push(part.to_vec().into());
    }
    zeromq::ZmqMessage::try_from(parts)
      .map_err(|err| anyhow!("ZMQ message error: {err}"))
  }

  fn from_jupyter_message(
    jupyter_message: JupyterMessage,
  ) -> Result<Self, AnyError> {
    let mut jparts: Vec<Bytes> = vec![
      serde_json::to_vec(&jupyter_message.header)?.into(),
      if let Some(parent_header) = jupyter_message.parent_header.as_ref() {
        serde_json::to_vec(parent_header)?.into()
      } else {
        serde_json::to_vec(&serde_json::Map::new())?.into()
      },
      serde_json::to_vec(&jupyter_message.metadata)?.into(),
      serde_json::to_vec(&jupyter_message.content)?.into(),
    ];
    jparts.extend_from_slice(&jupyter_message.buffers);
    Ok(Self {
      zmq_identities: jupyter_message.zmq_identities.clone(),
      jparts,
    })
  }

  fn into_jupyter_message(self) -> Result<JupyterMessage, AnyError> {
    if self.jparts.len() < 4 {
      bail!("Insufficient message parts {}", self.jparts.len());
    }

    let header: Header = serde_json::from_slice(&self.jparts[0])?;
    let content: Value = serde_json::from_slice(&self.jparts[3])?;
    let content =
      JupyterMessageContent::from_type_and_content(&header.msg_type, content)
        .map_err(|err| {
        anyhow!(
          "Error deserializing content for msg_type `{}`: {}",
          header.msg_type,
          err
        )
      })?;

    Ok(JupyterMessage {
      zmq_identities: self.zmq_identities,
      header,
      parent_header: serde_json::from_slice(&self.jparts[1]).ok(),
      metadata: serde_json::from_slice(&self.jparts[2])?,
      content,
      buffers: if self.jparts.len() > 4 {
        self.jparts[4..].to_vec()
      } else {
        vec![]
      },
      channel: None,
    })
  }
}

pub async fn create_kernel_iopub_connection(
  connection_info: &ConnectionInfo,
  session_id: &str,
) -> Result<KernelIoPubConnection, AnyError> {
  let mut socket = zeromq::PubSocket::new();
  socket.bind(&connection_info.iopub_url()).await?;
  Ok(Connection::new(socket, &connection_info.key, session_id))
}

pub async fn create_kernel_shell_connection(
  connection_info: &ConnectionInfo,
  session_id: &str,
) -> Result<KernelShellConnection, AnyError> {
  let mut socket = zeromq::RouterSocket::new();
  socket.bind(&connection_info.shell_url()).await?;
  Ok(Connection::new(socket, &connection_info.key, session_id))
}

pub async fn create_kernel_control_connection(
  connection_info: &ConnectionInfo,
  session_id: &str,
) -> Result<KernelControlConnection, AnyError> {
  let mut socket = zeromq::RouterSocket::new();
  socket.bind(&connection_info.control_url()).await?;
  Ok(Connection::new(socket, &connection_info.key, session_id))
}

pub async fn create_kernel_stdin_connection(
  connection_info: &ConnectionInfo,
  session_id: &str,
) -> Result<KernelStdinConnection, AnyError> {
  let mut socket = zeromq::RouterSocket::new();
  socket.bind(&connection_info.stdin_url()).await?;
  Ok(Connection::new(socket, &connection_info.key, session_id))
}

pub async fn create_kernel_heartbeat_connection(
  connection_info: &ConnectionInfo,
) -> Result<KernelHeartbeatConnection, AnyError> {
  let mut socket = zeromq::RepSocket::new();
  socket.bind(&connection_info.hb_url()).await?;
  Ok(KernelHeartbeatConnection { socket })
}

pub fn user_data_dir() -> Result<PathBuf, AnyError> {
  #[cfg(target_os = "macos")]
  {
    Ok(home_dir()?.join("Library").join("Jupyter"))
  }
  #[cfg(windows)]
  {
    Ok(
      std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("Failed to get APPDATA directory"))?
        .join("jupyter"),
    )
  }
  #[cfg(not(any(target_os = "macos", windows)))]
  {
    Ok(
      std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home_dir()?.join(".local").join("share"))
        .join("jupyter"),
    )
  }
}

#[cfg(not(windows))]
fn home_dir() -> Result<PathBuf, AnyError> {
  #[allow(clippy::disallowed_types, reason = "setup code")]
  sys_traits::EnvHomeDir::env_home_dir(&sys_traits::impls::RealSys)
    .ok_or_else(|| anyhow!("Failed to get home directory"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use bytes::Bytes;
  use deno_core::serde_json::json;
  use jupyter_protocol::KernelInfoRequest;

  fn mac_key(key: &str) -> Option<hmac::Key> {
    Some(hmac::Key::new(hmac::HMAC_SHA256, key.as_bytes()))
  }

  fn kernel_info_message() -> JupyterMessage {
    JupyterMessage::new(KernelInfoRequest {}, None)
      .with_metadata(json!({ "request": "kernel-info" }))
      .with_zmq_identities(vec![Bytes::from_static(b"client-id")])
      .with_buffers(vec![
        Bytes::from_static(b"buffer-one"),
        Bytes::from_static(b"buffer-two"),
      ])
  }

  #[test]
  fn raw_message_signed_roundtrip_preserves_identities_and_buffers() {
    let key = mac_key("secret");
    let raw_message =
      RawMessage::from_jupyter_message(kernel_info_message()).unwrap();
    let zmq_message = raw_message.into_zmq_message(&key).unwrap();

    let raw_message = RawMessage::from_multipart(zmq_message, &key).unwrap();
    let message = raw_message.into_jupyter_message().unwrap();

    assert_eq!(
      message.zmq_identities,
      vec![Bytes::from_static(b"client-id")]
    );
    assert_eq!(message.metadata, json!({ "request": "kernel-info" }),);
    assert_eq!(
      message.buffers,
      vec![
        Bytes::from_static(b"buffer-one"),
        Bytes::from_static(b"buffer-two"),
      ]
    );
    assert_eq!(message.header.msg_type, "kernel_info_request");
  }

  #[test]
  fn raw_message_rejects_bad_hmac() {
    let key = mac_key("secret");
    let raw_message =
      RawMessage::from_jupyter_message(kernel_info_message()).unwrap();
    let zmq_message = raw_message.into_zmq_message(&key).unwrap();
    let mut parts = zmq_message.into_vec();
    parts[3] = Bytes::from_static(br#"{"tampered":true}"#);
    let zmq_message = zeromq::ZmqMessage::try_from(parts).unwrap();

    let err = RawMessage::from_multipart(zmq_message, &key).unwrap_err();

    assert!(err.to_string().contains("HMAC verification failed"));
  }

  #[test]
  fn raw_message_empty_key_writes_empty_hmac_and_skips_verification() {
    let raw_message =
      RawMessage::from_jupyter_message(kernel_info_message()).unwrap();
    let zmq_message = raw_message.into_zmq_message(&None).unwrap();
    let mut parts = zmq_message.into_vec();

    assert_eq!(&parts[2][..], b"");
    parts[5] = Bytes::from_static(br#"{"tampered":true}"#);
    let zmq_message = zeromq::ZmqMessage::try_from(parts).unwrap();

    let raw_message = RawMessage::from_multipart(zmq_message, &None).unwrap();
    let message = raw_message.into_jupyter_message().unwrap();

    assert_eq!(
      message.zmq_identities,
      vec![Bytes::from_static(b"client-id")]
    );
    assert_eq!(
      message.buffers,
      vec![
        Bytes::from_static(b"buffer-one"),
        Bytes::from_static(b"buffer-two"),
      ]
    );
    assert_eq!(message.metadata, json!({ "tampered": true }));
  }
}
