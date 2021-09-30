// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::{
  include_js_files, op_async, AsyncRefCell, RcRef, Resource, ResourceId,
};
use deno_core::{op_sync, ZeroCopyBuf};
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio_serial::SerialPort;
use tokio_serial::SerialPortBuilderExt;

pub fn init<WP: WebSerialPermissions + 'static>() -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/webserial",
      "01_webserial.js",
    ))
    .ops(vec![
      (
        "op_webserial_get_ports",
        op_sync(op_webserial_get_ports::<WP>),
      ),
      ("op_webserial_open_port", op_sync(op_webserial_open_port)),
      ("op_webserial_read", op_async(op_webserial_read)),
      ("op_webserial_write", op_async(op_webserial_write)),
      ("op_webserial_get_info", op_sync(op_webserial_get_info)),
      (
        "op_webserial_set_signals",
        op_async(op_webserial_set_signals),
      ),
      (
        "op_webserial_get_signals",
        op_async(op_webserial_get_signals),
      ),
    ])
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_webserial.d.ts")
}

pub trait WebSerialPermissions {
  fn check_port(&mut self, _port: &str) -> Result<(), AnyError>;
}

pub struct NoWebSerialPermissions;

impl WebSerialPermissions for NoWebSerialPermissions {
  fn check_port(&mut self, _port: &str) -> Result<(), AnyError> {
    Ok(())
  }
}

pub struct SerialPortResource(AsyncRefCell<tokio_serial::SerialStream>);

impl Resource for SerialPortResource {
  fn name(&self) -> Cow<str> {
    "serialPort".into()
  }
}

impl SerialPortResource {
  pub async fn read(
    self: &Rc<Self>,
    buf: &mut [u8],
  ) -> Result<usize, AnyError> {
    let mut serial_port = RcRef::map(self, |r| &r.0).borrow_mut().await;
    Ok(serial_port.read(buf).await?)
  }

  pub async fn write(self: &Rc<Self>, buf: &[u8]) -> Result<usize, AnyError> {
    let mut serial_port = RcRef::map(self, |r| &r.0).borrow_mut().await;
    Ok(serial_port.write(buf).await?)
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialPortInfo {
  name: String,
  info: Option<(u16, u16)>,
}

pub fn op_webserial_get_ports<WP>(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<Vec<SerialPortInfo>, AnyError>
where
  WP: WebSerialPermissions + 'static,
{
  let state = state.borrow_mut::<WP>();
  let ports = tokio_serial::available_ports()?
    .into_iter()
    .filter(|info| state.check_port(&info.port_name).is_ok())
    .map(|info| SerialPortInfo {
      name: info.port_name,
      info: match info.port_type {
        serialport::SerialPortType::UsbPort(info) => Some((info.vid, info.pid)),
        _ => None,
      },
    })
    .collect::<Vec<SerialPortInfo>>();
  Ok(ports)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPortArgs {
  baud_rate: u32,
  data_bits: u8,
  stop_bits: u8,
  parity: String,
  flow_control: String,
}

pub fn op_webserial_open_port(
  state: &mut OpState,
  name: String,
  args: OpenPortArgs,
) -> Result<ResourceId, AnyError> {
  let serial_port = tokio_serial::new(&name, args.baud_rate)
    .data_bits(match args.data_bits {
      7 => tokio_serial::DataBits::Seven,
      8 => tokio_serial::DataBits::Eight,
      _ => unreachable!(),
    })
    .stop_bits(match args.stop_bits {
      1 => tokio_serial::StopBits::One,
      2 => tokio_serial::StopBits::Two,
      _ => unreachable!(),
    })
    .parity(match args.parity.as_str() {
      "none" => tokio_serial::Parity::None,
      "even" => tokio_serial::Parity::Even,
      "odd" => tokio_serial::Parity::Odd,
      _ => unreachable!(),
    })
    .flow_control(match args.flow_control.as_str() {
      "none" => tokio_serial::FlowControl::None,
      "hardware" => tokio_serial::FlowControl::Hardware,
      _ => unreachable!(),
    })
    .open_native_async()?;

  let rid = state
    .resource_table
    .add(SerialPortResource(AsyncRefCell::new(serial_port)));

  Ok(rid)
}

pub async fn op_webserial_read(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  mut buf: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<SerialPortResource>(rid)?;
  let mut serial_port = RcRef::map(resource, |r| &r.0).borrow_mut().await;
  serial_port.read_exact(buf.as_mut()).await?;
  Ok(())
}

pub async fn op_webserial_write(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  buf: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<SerialPortResource>(rid)?;
  let mut serial_port = RcRef::map(resource, |r| &r.0).borrow_mut().await;
  serial_port.write_all(&buf).await?;
  Ok(())
}

pub fn op_webserial_get_info(
  _state: &mut OpState,
  name: String,
  _: (),
) -> Result<Option<(u16, u16)>, AnyError> {
  let info = tokio_serial::available_ports()?
    .iter()
    .find(|port| port.port_name == name)
    .and_then(|port| match &port.port_type {
      serialport::SerialPortType::UsbPort(info) => Some((info.vid, info.pid)),
      _ => None,
    });
  Ok(info)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSignalsArgs {
  data_terminal_ready: Option<bool>,
  request_to_send: Option<bool>,
  r#break: Option<bool>,
}

pub async fn op_webserial_set_signals(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  args: SetSignalsArgs,
) -> Result<(), AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<SerialPortResource>(rid)?;
  let mut serial_port = RcRef::map(resource, |r| &r.0).borrow_mut().await;

  if let Some(data_terminal_ready) = args.data_terminal_ready {
    serial_port.write_data_terminal_ready(data_terminal_ready)?;
  }
  if let Some(request_to_send) = args.request_to_send {
    serial_port.write_request_to_send(request_to_send)?;
  }
  if let Some(r#break) = args.r#break {
    if r#break {
      serial_port.set_break()?;
    } else {
      serial_port.clear_break()?;
    }
  }

  Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSignals {
  data_carrier_detect: bool,
  clear_to_send: bool,
  ring_indicator: bool,
  data_set_ready: bool,
}

pub async fn op_webserial_get_signals(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _: (),
) -> Result<GetSignals, AnyError> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get::<SerialPortResource>(rid)?;
  let mut serial_port = RcRef::map(resource, |r| &r.0).borrow_mut().await;

  Ok(GetSignals {
    data_carrier_detect: serial_port.read_carrier_detect()?,
    clear_to_send: serial_port.read_clear_to_send()?,
    ring_indicator: serial_port.read_ring_indicator()?,
    data_set_ready: serial_port.read_data_set_ready()?,
  })
}
