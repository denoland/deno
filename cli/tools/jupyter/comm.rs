// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use ring::hmac;
use zeromq::prelude::*;
use zeromq::util::PeerIdentity;
use zeromq::SocketOptions;

use super::hmac_verify;
use super::ConnectionSpec;
use super::ReplyMessage;
use super::RequestMessage;
use super::SideEffectMessage;

pub struct PubComm {
  conn_str: String,
  hmac_key: hmac::Key,
  socket: zeromq::PubSocket,

  // TODO(bartlomieju):
  #[allow(unused)]
  identity: String,
}

fn create_conn_str(transport: &str, ip: &str, port: u32) -> String {
  format!("{}://{}:{}", transport, ip, port)
}

// TODO(apowers313) connect and send look like traits shared with DealerComm
impl PubComm {
  pub fn new(
    spec: &ConnectionSpec,
    identity: &str,
    hmac_key: &hmac::Key,
  ) -> Self {
    let conn_str = create_conn_str(&spec.transport, &spec.ip, spec.iopub_port);
    println!("iopub connection: {}", conn_str);
    let peer_identity =
      PeerIdentity::try_from(identity.as_bytes().to_vec()).unwrap();
    let mut options = SocketOptions::default();
    options.peer_identity(peer_identity);

    Self {
      conn_str,
      identity: identity.to_string(),
      hmac_key: hmac_key.to_owned(),
      socket: zeromq::PubSocket::with_options(options),
    }
  }

  pub async fn connect(&mut self) -> Result<(), AnyError> {
    self.socket.bind(&self.conn_str).await?;

    Ok(())
  }

  pub async fn send(&mut self, msg: SideEffectMessage) -> Result<(), AnyError> {
    log::debug!("==> IoPub SENDING: {:#?}", msg);
    let zmq_msg = msg.serialize(&self.hmac_key);
    self.socket.send(zmq_msg).await?;
    Ok(())
  }
}

pub struct DealerComm {
  name: String,
  conn_str: String,
  hmac_key: hmac::Key,
  socket: zeromq::DealerSocket,

  // TODO(bartlomieju):
  #[allow(unused)]
  identity: String,
}

impl DealerComm {
  pub fn new(
    name: &str,
    conn_str: String,
    identity: &str,
    hmac_key: &hmac::Key,
  ) -> Self {
    println!("dealer '{}' connection: {}", name, conn_str);
    let peer_identity =
      PeerIdentity::try_from(identity.as_bytes().to_vec()).unwrap();
    let mut options = SocketOptions::default();
    options.peer_identity(peer_identity);

    Self {
      name: name.to_string(),
      conn_str,
      identity: identity.to_string(),
      hmac_key: hmac_key.to_owned(),
      socket: zeromq::DealerSocket::with_options(options),
    }
  }

  pub fn create_shell(
    spec: &ConnectionSpec,
    identity: &str,
    hmac_key: &hmac::Key,
  ) -> Self {
    Self::new(
      "shell",
      create_conn_str(&spec.transport, &spec.ip, spec.shell_port),
      identity,
      hmac_key,
    )
  }

  pub fn create_control(
    spec: &ConnectionSpec,
    identity: &str,
    hmac_key: &hmac::Key,
  ) -> Self {
    Self::new(
      "control",
      create_conn_str(&spec.transport, &spec.ip, spec.control_port),
      identity,
      hmac_key,
    )
  }

  pub fn create_stdin(
    spec: &ConnectionSpec,
    identity: &str,
    hmac_key: &hmac::Key,
  ) -> Self {
    Self::new(
      "stdin",
      create_conn_str(&spec.transport, &spec.ip, spec.stdin_port),
      identity,
      hmac_key,
    )
  }

  pub async fn connect(&mut self) -> Result<(), AnyError> {
    self.socket.bind(&self.conn_str).await?;

    Ok(())
  }

  pub async fn recv(&mut self) -> Result<RequestMessage, AnyError> {
    let zmq_msg = self.socket.recv().await?;

    hmac_verify(
      &self.hmac_key,
      zmq_msg.get(1).unwrap(),
      zmq_msg.get(2).unwrap(),
      zmq_msg.get(3).unwrap(),
      zmq_msg.get(4).unwrap(),
      zmq_msg.get(5).unwrap(),
    )?;

    let jup_msg = RequestMessage::try_from(zmq_msg)?;
    log::debug!("<== {} RECEIVING: {:#?}", self.name, jup_msg);
    Ok(jup_msg)
  }

  pub async fn send(&mut self, msg: ReplyMessage) -> Result<(), AnyError> {
    log::debug!("==> {} SENDING: {:#?}", self.name, msg);
    let zmq_msg = msg.serialize(&self.hmac_key);
    self.socket.send(zmq_msg).await?;
    log::debug!("==> {} SENT", self.name);
    Ok(())
  }
}

pub struct HbComm {
  conn_str: String,
  socket: zeromq::RepSocket,
}

impl HbComm {
  pub fn new(spec: &ConnectionSpec) -> Self {
    let conn_str = create_conn_str(&spec.transport, &spec.ip, spec.hb_port);
    println!("hb connection: {}", conn_str);
    Self {
      conn_str,
      socket: zeromq::RepSocket::new(),
    }
  }

  pub async fn connect(&mut self) -> Result<(), AnyError> {
    self.socket.bind(&self.conn_str).await?;

    Ok(())
  }

  pub async fn heartbeat(&mut self) -> Result<(), AnyError> {
    let msg = self.socket.recv().await?;
    println!("<== heartbeat received");
    self.socket.send(msg).await?;
    println!("==> heartbeat sent");
    Ok(())
  }
}
