// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// TODO(bartlomieju): remove me
#![allow(unused)]

use crate::flags::Flags;
use crate::flags::JupyterFlags;
use crate::tools::repl::EvaluationOutput;
use crate::tools::repl::ReplSession;
use data_encoding::HEXLOWER;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_runtime::worker::MainWorker;
use ring::hmac;
use std::collections::HashMap;
use std::env::current_exe;
use std::time::Duration;
use tempfile::TempDir;
use tokio::join;
use tokio::time::sleep;
use zeromq::prelude::*;
use zeromq::ZmqMessage;

use super::hmac_verify;
use super::ReplyMessage;
use super::RequestMessage;
use super::SideEffectMessage;

pub struct PubComm {
  conn_str: String,
  session_id: String,
  hmac_key: hmac::Key,
  socket: zeromq::PubSocket,
}

// TODO(apowers313) connect and send look like traits shared with DealerComm
impl PubComm {
  pub fn new(
    conn_str: String,
    session_id: String,
    hmac_key: hmac::Key,
  ) -> Self {
    println!("iopub connection: {}", conn_str);
    Self {
      conn_str,
      session_id,
      hmac_key,
      socket: zeromq::PubSocket::new(),
    }
  }

  pub async fn connect(&mut self) -> Result<(), AnyError> {
    self.socket.bind(&self.conn_str).await?;

    Ok(())
  }

  pub async fn send(&mut self, msg: SideEffectMessage) -> Result<(), AnyError> {
    let zmq_msg = msg.serialize(&self.hmac_key);
    println!(">>> ZMQ SENDING: {:#?}", zmq_msg);
    self.socket.send(zmq_msg).await?;
    Ok(())
  }
}

pub struct DealerComm {
  name: String,
  conn_str: String,
  session_id: String,
  hmac_key: hmac::Key,
  socket: zeromq::DealerSocket,
}

impl DealerComm {
  pub fn new(
    name: &str,
    conn_str: String,
    session_id: String,
    hmac_key: hmac::Key,
  ) -> Self {
    println!("dealer '{}' connection: {}", name, conn_str);
    Self {
      name: name.to_string(),
      conn_str,
      session_id,
      hmac_key,
      socket: zeromq::DealerSocket::new(),
    }
  }

  pub async fn connect(&mut self) -> Result<(), AnyError> {
    self.socket.bind(&self.conn_str).await?;

    Ok(())
  }

  pub async fn recv(&mut self) -> Result<RequestMessage, AnyError> {
    let zmq_msg = self.socket.recv().await?;
    println!("<<< ZMQ RECEIVING: {:#?}", zmq_msg);

    hmac_verify(
      &self.hmac_key,
      zmq_msg.get(1).unwrap(),
      zmq_msg.get(2).unwrap(),
      zmq_msg.get(3).unwrap(),
      zmq_msg.get(4).unwrap(),
      zmq_msg.get(5).unwrap(),
    )?;

    let jup_msg = RequestMessage::try_from(zmq_msg)?;

    Ok(jup_msg)
  }

  pub async fn send(&mut self, msg: ReplyMessage) -> Result<(), AnyError> {
    let zmq_msg = msg.serialize(&self.hmac_key);
    println!(">>> ZMQ SENDING: {:#?}", zmq_msg);
    self.socket.send(zmq_msg).await?;
    Ok(())
  }
}

// TODO(apowers313) this is the heartbeat loop now
pub async fn create_zmq_reply(
  name: &str,
  conn_str: &str,
) -> Result<(), AnyError> {
  println!("reply '{}' connection string: {}", name, conn_str);

  let mut sock = zeromq::RepSocket::new(); // TODO(apowers313) exact same as dealer, refactor
  sock.monitor();
  sock.bind(conn_str).await?;

  loop {
    let msg = sock.recv().await?;
    println!("*** '{}' got packet!", name);
  }
}
