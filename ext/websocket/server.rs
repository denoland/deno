// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::MessageKind;
use crate::SendValue;
use crate::Upgraded;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::AsyncRefCell;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::StringOrBuffer;
use deno_core::ZeroCopyBuf;
use std::borrow::Cow;
use std::cell::RefCell;
use std::pin::Pin;
use std::rc::Rc;

use fastwebsockets::CloseCode;
use fastwebsockets::FragmentCollector;
use fastwebsockets::Frame;
use fastwebsockets::OpCode;
use fastwebsockets::WebSocket;

pub struct ServerWebSocket {
  ws: AsyncRefCell<FragmentCollector<Pin<Box<dyn Upgraded>>>>,
}

impl Resource for ServerWebSocket {
  fn name(&self) -> Cow<str> {
    "serverWebSocket".into()
  }
}
pub async fn ws_create_server_stream(
  state: &Rc<RefCell<OpState>>,
  transport: Pin<Box<dyn Upgraded>>,
) -> Result<ResourceId, AnyError> {
  let mut ws = WebSocket::after_handshake(transport);
  ws.set_writev(false);
  ws.set_auto_close(true);
  ws.set_auto_pong(true);

  let ws_resource = ServerWebSocket {
    ws: AsyncRefCell::new(FragmentCollector::new(ws)),
  };

  let resource_table = &mut state.borrow_mut().resource_table;
  let rid = resource_table.add(ws_resource);
  Ok(rid)
}

#[op]
pub async fn op_server_ws_send_binary(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  data: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;

  let mut ws = RcRef::map(&resource, |r| &r.ws).borrow_mut().await;
  ws.write_frame(Frame::new(true, OpCode::Binary, None, data.to_vec()))
    .await
    .map_err(|err| type_error(err.to_string()))?;
  Ok(())
}

#[op]
pub async fn op_server_ws_send_text(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  data: String,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  let mut ws = RcRef::map(&resource, |r| &r.ws).borrow_mut().await;
  ws.write_frame(Frame::new(true, OpCode::Text, None, data.into_bytes()))
    .await
    .map_err(|err| type_error(err.to_string()))?;
  Ok(())
}

#[op]
pub async fn op_server_ws_send(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  value: SendValue,
) -> Result<(), AnyError> {
  let msg = match value {
    SendValue::Text(text) => {
      Frame::new(true, OpCode::Text, None, text.into_bytes())
    }
    SendValue::Binary(buf) => {
      Frame::new(true, OpCode::Binary, None, buf.to_vec())
    }
    SendValue::Pong => Frame::new(true, OpCode::Pong, None, vec![]),
    SendValue::Ping => Frame::new(true, OpCode::Ping, None, vec![]),
  };

  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  let mut ws = RcRef::map(&resource, |r| &r.ws).borrow_mut().await;

  ws.write_frame(msg)
    .await
    .map_err(|err| type_error(err.to_string()))?;
  Ok(())
}

#[op(deferred)]
pub async fn op_server_ws_close(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  code: Option<u16>,
  reason: Option<String>,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  let mut ws = RcRef::map(&resource, |r| &r.ws).borrow_mut().await;
  let frame = reason
    .map(|reason| Frame::close(code.unwrap_or(1005), reason.as_bytes()))
    .unwrap_or_else(|| Frame::close_raw(vec![]));
  ws.write_frame(frame)
    .await
    .map_err(|err| type_error(err.to_string()))?;
  Ok(())
}

#[op(deferred)]
pub async fn op_server_ws_next_event(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<(u16, StringOrBuffer), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<ServerWebSocket>(rid)?;
  let mut ws = RcRef::map(&resource, |r| &r.ws).borrow_mut().await;
  let val = match ws.read_frame().await {
    Ok(val) => val,
    Err(err) => {
      return Ok((
        MessageKind::Error as u16,
        StringOrBuffer::String(err.to_string()),
      ))
    }
  };

  let res = match val.opcode {
    OpCode::Text => (
      MessageKind::Text as u16,
      StringOrBuffer::String(String::from_utf8(val.payload).unwrap()),
    ),
    OpCode::Binary => (
      MessageKind::Binary as u16,
      StringOrBuffer::Buffer(val.payload.into()),
    ),
    OpCode::Close => {
      if val.payload.len() < 2 {
        return Ok((1005, StringOrBuffer::String("".to_string())));
      }

      let close_code =
        CloseCode::from(u16::from_be_bytes([val.payload[0], val.payload[1]]));
      let reason = String::from_utf8(val.payload[2..].to_vec()).unwrap();
      (close_code.into(), StringOrBuffer::String(reason))
    }
    OpCode::Ping => (
      MessageKind::Ping as u16,
      StringOrBuffer::Buffer(vec![].into()),
    ),
    OpCode::Pong => (
      MessageKind::Pong as u16,
      StringOrBuffer::Buffer(vec![].into()),
    ),
    OpCode::Continuation => {
      return Err(type_error("Unexpected continuation frame"))
    }
  };
  Ok(res)
}
