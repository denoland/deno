// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::error::bad_resource_id;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::AsyncRefCell;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use libusb1_sys::constants::*;
use rusb::request_type;
use rusb::ConfigDescriptor;
use rusb::Context;
use rusb::Device;
use rusb::DeviceHandle;
use rusb::Interface;
use rusb::InterfaceDescriptor;
use rusb::UsbContext;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

pub use rusb; // Re-export rusb

static EP_DIR_IN: u8 = 0x80;
static EP_DIR_OUT: u8 = 0x0;

pub fn init<P: WebUsbPermissions + 'static>(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/webusb",
      "01_webusb.js",
    ))
    .ops(vec![
      ("op_webusb_open_device", op_async(op_webusb_open_device)),
      ("op_webusb_reset", op_async(op_webusb_reset)),
      ("op_webusb_close_device", op_async(op_webusb_close_device)),
      (
        "op_webusb_select_configuration",
        op_async(op_webusb_select_configuration),
      ),
      ("op_webusb_transfer_out", op_async(op_webusb_transfer_out)),
      ("op_webusb_transfer_in", op_async(op_webusb_transfer_in)),
      (
        "op_webusb_control_transfer_in",
        op_async(op_webusb_control_transfer_in),
      ),
      (
        "op_webusb_control_transfer_out",
        op_async(op_webusb_control_transfer_out),
      ),
      ("op_webusb_clear_halt", op_async(op_webusb_clear_halt)),
      (
        "op_webusb_select_alternate_interface",
        op_async(op_webusb_select_alternate_interface),
      ),
      (
        "op_webusb_release_interface",
        op_async(op_webusb_release_interface),
      ),
      (
        "op_webusb_claim_interface",
        op_async(op_webusb_claim_interface),
      ),
      (
        "op_webusb_request_device",
        op_async(op_webusb_request_device::<P>),
      ),
      (
        "op_webusb_get_devices",
        op_async(op_webusb_get_devices::<P>),
      ),
    ])
    .state(move |state| {
      state.put(Unstable(unstable));
      Ok(())
    })
    .build()
}

pub struct Unstable(pub bool);

fn check_unstable(state: &OpState, api_name: &str) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!(
      "Unstable API '{}'. The --unstable flag must be provided.",
      api_name
    );
    std::process::exit(70);
  }
}

pub trait WebUsbPermissions {
  fn check_usb(&self, device: u16) -> Result<(), AnyError>;
  fn request_device(&mut self, device: (Option<String>, u16)) -> u8;
}

pub struct NoWebUsbPermissions;

impl WebUsbPermissions for NoWebUsbPermissions {
  fn check_usb(&self, _device: u16) -> Result<(), AnyError> {
    Ok(())
  }

  fn request_device(&mut self, _device: (Option<String>, u16)) -> u8 {
    0
  }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbConfiguration {
  // Index of String Descriptor describing this configuration.
  configuration_name: Option<String>,
  // The configuration number. Should corresspond to bConfigurationValue (https://www.beyondlogic.org/usbnutshell/usb5.shtml#ConfigurationDescriptors)
  configuration_value: u8,
  interfaces: Vec<UsbInterface>,
}

impl UsbConfiguration {
  pub fn from(
    config_descriptor: ConfigDescriptor,
    handle: &DeviceHandle<Context>,
  ) -> Result<Self, AnyError> {
    Ok(UsbConfiguration {
      configuration_name: match config_descriptor.description_string_index() {
        None => None,
        Some(idx) => Some(handle.read_string_descriptor_ascii(idx)?),
      },
      configuration_value: config_descriptor.number(),
      interfaces: config_descriptor
        .interfaces()
        .map(|i| UsbInterface::from(i, &handle))
        .collect::<Vec<UsbInterface>>(),
    })
  }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbInterface {
  interface_number: u8,
  alternate: UsbAlternateInterface,
  alternates: Vec<UsbAlternateInterface>,
  claimed: bool,
}

impl UsbInterface {
  pub fn from(i: Interface, handle: &DeviceHandle<Context>) -> Self {
    UsbInterface {
      interface_number: i.number(),
      claimed: false,
      // By default, the alternate setting is for the interface with
      // bAlternateSetting equal to 0.
      alternate: {
        // TODO: don't panic
        let interface =
          i.descriptors().find(|d| d.setting_number() == 0).unwrap();
        UsbAlternateInterface::from(interface, &handle)
      },
      alternates: i
        .descriptors()
        .map(|interface| UsbAlternateInterface::from(interface, &handle))
        .collect(),
    }
  }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum UsbEndpointType {
  Bulk,
  Interrupt,
  Isochronous,
  Control,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbEndpoint {
  endpoint_number: u8,
  direction: Direction,
  // TODO(littledivy): Get rid of reserved `type` key somehow?
  r#type: UsbEndpointType,
  packet_size: u16,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbAlternateInterface {
  alternate_setting: u8,
  interface_class: u8,
  interface_subclass: u8,
  interface_protocol: u8,
  interface_name: Option<String>,
  endpoints: Vec<UsbEndpoint>,
}

impl UsbAlternateInterface {
  pub fn from(d: InterfaceDescriptor, handle: &DeviceHandle<Context>) -> Self {
    UsbAlternateInterface {
      alternate_setting: d.setting_number(),
      interface_class: d.class_code(),
      interface_subclass: d.sub_class_code(),
      interface_protocol: d.protocol_code(),
      interface_name: d
        .description_string_index()
        .map(|idx| handle.read_string_descriptor_ascii(idx).unwrap()),
      endpoints: d
        .endpoint_descriptors()
        .map(|e| UsbEndpoint {
          endpoint_number: e.number(),
          packet_size: e.max_packet_size(),
          direction: match e.direction() {
            rusb::Direction::In => Direction::In,
            rusb::Direction::Out => Direction::Out,
          },
          r#type: match e.transfer_type() {
            rusb::TransferType::Control => UsbEndpointType::Control,
            rusb::TransferType::Isochronous => UsbEndpointType::Isochronous,
            rusb::TransferType::Bulk => UsbEndpointType::Bulk,
            rusb::TransferType::Interrupt => UsbEndpointType::Interrupt,
          },
        })
        .collect(),
    }
  }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsbDevice {
  configurations: Vec<UsbConfiguration>,
  configuration: Option<UsbConfiguration>,
  device_class: u8,
  device_subclass: u8,
  device_protocol: u8,
  device_version_major: u8,
  device_version_minor: u8,
  device_version_subminor: u8,
  manufacturer_name: Option<String>,
  product_id: u16,
  product_name: Option<String>,
  serial_number: Option<String>,
  usb_version_major: u8,
  usb_version_minor: u8,
  usb_version_subminor: u8,
  vendor_id: u16,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenArgs {
  rid: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimInterfaceArgs {
  rid: u32,
  interface_number: u8,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectConfigurationArgs {
  rid: u32,
  configuration_value: u8,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectAlternateInterfaceArgs {
  rid: u32,
  interface_number: u8,
  alternate_setting: u8,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
  In,
  Out,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearHaltArgs {
  rid: u32,
  direction: Direction,
  endpoint_number: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferInArgs {
  rid: u32,
  length: usize,
  endpoint_number: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferOutArgs {
  rid: u32,
  endpoint_number: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebUsbRequestType {
  Standard,
  Class,
  Vendor,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WebUsbRecipient {
  Device,
  Interface,
  Endpoint,
  Other,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum WebUsbTransferStatus {
  Completed,
  TransferError,
  Timeout,
  Stall,
  Disconnect,
  Babble,
  Cancelled,
}

impl WebUsbTransferStatus {
  pub fn from_libusb_status(status: i32) -> Self {
    match status {
      LIBUSB_TRANSFER_COMPLETED => WebUsbTransferStatus::Completed,
      LIBUSB_TRANSFER_ERROR => WebUsbTransferStatus::TransferError,
      // Should never happen but no harm to keep it.
      LIBUSB_TRANSFER_TIMED_OUT => WebUsbTransferStatus::Timeout,
      LIBUSB_TRANSFER_STALL => WebUsbTransferStatus::Stall,
      LIBUSB_TRANSFER_NO_DEVICE => WebUsbTransferStatus::Disconnect,
      LIBUSB_TRANSFER_OVERFLOW => WebUsbTransferStatus::Babble,
      LIBUSB_TRANSFER_CANCELLED => WebUsbTransferStatus::Cancelled,
      // Unreachable but we'll settle for a TransferError.
      _ => WebUsbTransferStatus::TransferError,
    }
  }

  pub fn from_rusb_error(error: rusb::Error) -> Self {
    match error {
      rusb::Error::NoDevice | rusb::Error::NotFound => {
        WebUsbTransferStatus::Disconnect
      }
      rusb::Error::Busy => WebUsbTransferStatus::Stall,
      rusb::Error::Timeout => WebUsbTransferStatus::Timeout,
      rusb::Error::Overflow => WebUsbTransferStatus::Babble,
      rusb::Error::Pipe => WebUsbTransferStatus::TransferError,
      rusb::Error::NoMem => WebUsbTransferStatus::Babble,
      _ => WebUsbTransferStatus::TransferError,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupArgs {
  request_type: WebUsbRequestType,
  recipient: WebUsbRecipient,
  request: u8,
  value: u16,
  index: u16,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlTransferOutArgs {
  rid: u32,
  setup: SetupArgs,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlTransferInArgs {
  rid: u32,
  length: usize,
  setup: SetupArgs,
}

pub struct UsbResource {
  device: Device<Context>,
}

pub struct UsbHandleResource {
  handle: AsyncRefCell<DeviceHandle<Context>>,
}

impl Resource for UsbHandleResource {
  fn name(&self) -> Cow<str> {
    "usbDeviceHandle".into()
  }
}

impl Resource for UsbResource {
  fn name(&self) -> Cow<str> {
    "usbDevice".into()
  }
}

// Method to determine the transfer type from the device's
// configuration descriptor and an endpoint address.
fn transfer_type(
  cnf: ConfigDescriptor,
  addr: u8,
) -> Option<rusb::TransferType> {
  let interfaces = cnf.interfaces();
  for interface in interfaces {
    for descriptor in interface.descriptors() {
      let endpoint_desc = descriptor
        .endpoint_descriptors()
        .find(|s| s.address() == addr);
      if endpoint_desc.is_none() {
        continue;
      }
      // TODO(littledivy): Do not unwrap.
      return Some(endpoint_desc.unwrap().transfer_type());
    }
  }
  None
}

pub async fn op_webusb_open_device(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: OpenArgs = serde_json::from_value(args)?;
  let rid = args.rid;

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let handle = resource.device.open()?;
  let rid = state.borrow_mut().resource_table.add(UsbHandleResource {
    handle: AsyncRefCell::new(handle),
  });
  Ok(json!({ "rid": rid }))
}

pub async fn op_webusb_reset(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  // Note: Reusing `OpenArgs` struct here. The rid is for the device handle.
  let args: OpenArgs = serde_json::from_value(args)?;
  let rid = args.rid;

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  handle.reset()?;
  Ok(json!({}))
}

pub async fn op_webusb_close_device(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  // Note: Reusing `OpenArgs` struct here. The rid is for the device handle.
  let args: OpenArgs = serde_json::from_value(args)?;
  let rid = args.rid;

  let resource = state
    .borrow_mut()
    .resource_table
    .take::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  drop(handle);
  Ok(json!({}))
}

pub async fn op_webusb_select_configuration(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: SelectConfigurationArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let configuration_value = args.configuration_value;

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  handle.set_active_configuration(configuration_value)?;
  Ok(json!({}))
}

pub async fn op_webusb_transfer_out(
  state: Rc<RefCell<OpState>>,
  args: TransferOutArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let zero_copy = zero_copy.ok_or_else(null_opbuf)?;
  let rid = args.rid;
  let endpoint_number = args.endpoint_number;

  // Ported from the Chromium codebase.
  // https://chromium.googlesource.com/chromium/src/+/master/services/device/usb/usb_device_handle_impl.cc#789
  let endpoint_addr = EP_DIR_OUT | endpoint_number;

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;

  let cnf = handle
    .device() // -> Device<T>
    .active_config_descriptor()?; // -> ConfigDescriptor<T>

  let ttype = transfer_type(cnf, endpoint_addr);
  let data = &*zero_copy;
  match ttype {
    Some(t) => {
      let mut status = WebUsbTransferStatus::Completed;
      let bytes_written = match t {
        rusb::TransferType::Bulk => {
          match handle.write_bulk(endpoint_addr, &data, Duration::new(0, 0)) {
            Ok(bw) => bw,
            Err(err) => {
              status = WebUsbTransferStatus::from_rusb_error(err);
              0
            }
          }
        }
        rusb::TransferType::Interrupt => {
          match handle.write_interrupt(
            endpoint_addr,
            &data,
            Duration::new(0, 0),
          ) {
            Ok(bw) => bw,
            Err(err) => {
              status = WebUsbTransferStatus::from_rusb_error(err);
              0
            }
          }
        }
        _ => {
          return Ok(
            json!({ "bytes_written": 0, "status": WebUsbTransferStatus::TransferError }),
          )
        }
      };
      Ok(json!({ "bytesWritten": bytes_written, "status": status }))
    }
    None => Ok(json!({})),
  }
}

pub async fn op_webusb_transfer_in(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: TransferInArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let endpoint_number = args.endpoint_number;

  // Ported from the Chromium codebase.
  // https://chromium.googlesource.com/chromium/src/+/master/services/device/usb/usb_device_handle_impl.cc#789
  let endpoint_addr = EP_DIR_IN | endpoint_number;

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  let cnf = handle
    .device() // -> Device<T>
    .active_config_descriptor()?; // -> ConfigDescriptor<T>

  let ttype = transfer_type(cnf, endpoint_addr);

  let mut data = vec![0u8; args.length];
  match ttype {
    Some(t) => match t {
      rusb::TransferType::Bulk => {
        match handle.read_bulk(endpoint_addr, &mut data, Duration::new(0, 0)) {
          Ok(_) => {}
          Err(err) => {
            return Ok(
              json!({ "status": WebUsbTransferStatus::from_rusb_error(err), "data": data }),
            )
          }
        }
      }
      rusb::TransferType::Interrupt => {
        match handle.read_interrupt(
          endpoint_addr,
          &mut data,
          Duration::new(0, 0),
        ) {
          Ok(_) => {}
          Err(err) => {
            return Ok(
              json!({ "status": WebUsbTransferStatus::from_rusb_error(err), "data": data }),
            )
          }
        }
      }
      _ => {
        return Ok(
          json!({ "status": WebUsbTransferStatus::TransferError, "data": data }),
        )
      }
    },
    // TODO(littledivy): Is this the right status to return?
    None => {
      return Ok(
        json!({ "status": WebUsbTransferStatus::TransferError, "data": data }),
      )
    }
  };

  Ok(json!({ "status": WebUsbTransferStatus::Completed, "data": data }))
}

pub async fn op_webusb_control_transfer_in(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: ControlTransferInArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let setup = args.setup;
  let length = args.length;

  let req = match setup.request_type {
    WebUsbRequestType::Standard => rusb::RequestType::Standard,
    WebUsbRequestType::Class => rusb::RequestType::Class,
    WebUsbRequestType::Vendor => rusb::RequestType::Vendor,
  };

  let recipient = match setup.recipient {
    WebUsbRecipient::Device => rusb::Recipient::Device,
    WebUsbRecipient::Interface => rusb::Recipient::Interface,
    WebUsbRecipient::Endpoint => rusb::Recipient::Endpoint,
    WebUsbRecipient::Other => rusb::Recipient::Other,
  };

  let req_type = request_type(rusb::Direction::In, req, recipient);

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  let mut buf = vec![0u8; length];
  // http://libusb.sourceforge.net/api-1.0/group__libusb__syncio.html
  // For unlimited timeout, use value `0`.
  let data = handle.read_control(
    req_type,
    setup.request,
    setup.value,
    setup.index,
    &mut buf,
    Duration::new(0, 0),
  )?;

  Ok(json!({ "data": data }))
}

pub async fn op_webusb_control_transfer_out(
  state: Rc<RefCell<OpState>>,
  args: ControlTransferOutArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let zero_copy = zero_copy.ok_or_else(null_opbuf)?;
  let rid = args.rid;
  let setup = args.setup;

  let buf = &*zero_copy;

  let req = match setup.request_type {
    WebUsbRequestType::Standard => rusb::RequestType::Standard,
    WebUsbRequestType::Class => rusb::RequestType::Class,
    WebUsbRequestType::Vendor => rusb::RequestType::Vendor,
  };

  let recipient = match setup.recipient {
    WebUsbRecipient::Device => rusb::Recipient::Device,
    WebUsbRecipient::Interface => rusb::Recipient::Interface,
    WebUsbRecipient::Endpoint => rusb::Recipient::Endpoint,
    WebUsbRecipient::Other => rusb::Recipient::Other,
  };

  let req_type = request_type(rusb::Direction::Out, req, recipient);

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  // http://libusb.sourceforge.net/api-1.0/group__libusb__syncio.html
  // For unlimited timeout, use value `0`.
  match handle.write_control(
    req_type,
    setup.request,
    setup.value,
    setup.index,
    &buf,
    Duration::new(0, 0),
  ) {
    Ok(bytes_written) => Ok(
      json!({ "bytesWritten": bytes_written, "status": WebUsbTransferStatus::Completed }),
    ),
    Err(err) => Ok(
      json!({ "bytesWritten": 0, "status": WebUsbTransferStatus::from_rusb_error(err) }),
    ),
  }
}

pub async fn op_webusb_clear_halt(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: ClearHaltArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let direction: Direction = args.direction;

  let mut endpoint = args.endpoint_number;

  match direction {
    Direction::In => endpoint |= EP_DIR_IN,
    Direction::Out => endpoint |= EP_DIR_OUT,
  };

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  handle.clear_halt(endpoint)?;
  Ok(json!({}))
}

pub async fn op_webusb_select_alternate_interface(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: SelectAlternateInterfaceArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let interface_number = args.interface_number;
  let alternate_setting = args.alternate_setting;

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  handle.set_alternate_setting(interface_number, alternate_setting)?;
  Ok(json!({}))
}

pub async fn op_webusb_release_interface(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: ClaimInterfaceArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let interface_number = args.interface_number;

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  handle.release_interface(interface_number)?;
  Ok(json!({}))
}

pub async fn op_webusb_claim_interface(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: ClaimInterfaceArgs = serde_json::from_value(args)?;
  let rid = args.rid;
  let interface_number = args.interface_number;

  let resource = state
    .borrow()
    .resource_table
    .get::<UsbHandleResource>(rid)
    .ok_or_else(bad_resource_id)?;

  let mut handle = RcRef::map(resource, |r| &r.handle).borrow_mut().await;
  handle.claim_interface(interface_number)?;
  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsbDeviceFilter {
  pub vendor_id: Option<u16>,
  pub product_id: Option<u16>,
  pub class_code: Option<u8>,
  pub subclass_code: Option<u8>,
  pub protocol_code: Option<u8>,
  pub serial_number: Option<String>,
}

// Request for device and add it to the permission state.
pub async fn op_webusb_request_device<WP>(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError>
where
  WP: WebUsbPermissions + 'static,
{
  let args: UsbDeviceFilter = serde_json::from_value(args)?;
  let mut devices: Vec<Device<Context>> = rusb::Context::new()
    .unwrap()
    .devices()
    .unwrap()
    .iter()
    .collect();

  // Filter available devices based on the filter arguments.
  devices.retain(|device| {
    let device_descriptor = device.device_descriptor();

    if let Ok(device_descriptor) = device_descriptor {
      if let Some(vendor_id) = args.vendor_id {
        return device_descriptor.vendor_id() == vendor_id;
      }
      if let Some(product_id) = args.product_id {
        return device_descriptor.product_id() == product_id;
      }
      if let Some(class_code) = args.class_code {
        return device_descriptor.class_code() == class_code;
      }
      if let Some(subclass_code) = args.subclass_code {
        return device_descriptor.sub_class_code() == subclass_code;
      }
      if let Some(protocol_code) = args.protocol_code {
        return device_descriptor.protocol_code() == protocol_code;
      }
      if let Some(serial_number) = &args.serial_number {
        if let Ok(handle) = device.open() {
          if let Ok(srno) =
            handle.read_serial_number_string_ascii(&device_descriptor)
          {
            return &srno == serial_number;
          }
        }
      }
    }

    false
  });

  let mut state = state.borrow_mut();
  let permissions = state.borrow_mut::<WP>();
  for device in devices.iter() {
    if let Ok(device_descriptor) = device.device_descriptor() {
      if let Ok(handle) = device.open() {
        let product_name =
          handle.read_product_string_ascii(&device_descriptor).ok();
        println!(
          "{}",
          permissions
            .request_device((product_name, device_descriptor.product_id()))
        );
      }
    }
  }

  Ok(json!({}))
}

pub async fn op_webusb_get_devices<WP>(
  state: Rc<RefCell<OpState>>,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError>
where
  WP: WebUsbPermissions + 'static,
{
  let mut state = state.borrow_mut();
  check_unstable(&state, "navigator.usb.getDevices()");

  let devices = rusb::Context::new().unwrap().devices().unwrap();

  #[derive(Serialize)]
  struct Device {
    usbdevice: UsbDevice,
    rid: u32,
  }

  let mut usbdevices: Vec<Device> = vec![];
  for device in devices.iter() {
    let device_descriptor = device.device_descriptor().unwrap();

    //let permissions = state.borrow::<WP>();
    let device_class = device_descriptor.class_code();

    // Do not list hubs. Ignore them.
    if device_class == 9 {
      continue;
    }

    let config_descriptor = device.active_config_descriptor();
    let device_version = device_descriptor.device_version();
    let usb_version = device_descriptor.usb_version();

    if let Ok(handle) = device.open() {
      let configuration = match config_descriptor {
        Ok(config_descriptor) => {
          UsbConfiguration::from(config_descriptor, &handle).ok()
        }
        Err(_) => None,
      };

      let num_configurations = device_descriptor.num_configurations();
      let mut configurations: Vec<UsbConfiguration> = vec![];
      for idx in 0..num_configurations {
        if let Ok(curr_config_descriptor) = device.config_descriptor(idx) {
          configurations
            .push(UsbConfiguration::from(curr_config_descriptor, &handle)?);
        }
      }
      let manufacturer_name = handle
        .read_manufacturer_string_ascii(&device_descriptor)
        .ok();
      let product_name =
        handle.read_product_string_ascii(&device_descriptor).ok();
      let serial_number = handle
        .read_serial_number_string_ascii(&device_descriptor)
        .ok();

      let usbdevice = UsbDevice {
        configurations,
        configuration,
        device_class,
        device_subclass: device_descriptor.sub_class_code(),
        device_protocol: device_descriptor.protocol_code(),
        device_version_major: device_version.major(),
        device_version_minor: device_version.minor(),
        device_version_subminor: device_version.sub_minor(),
        product_id: device_descriptor.product_id(),
        usb_version_major: usb_version.major(),
        usb_version_minor: usb_version.minor(),
        usb_version_subminor: usb_version.sub_minor(),
        vendor_id: device_descriptor.vendor_id(),
        manufacturer_name,
        product_name,
        serial_number,
      };

      // Explicitly close the device.
      drop(handle);

      let rid = state.resource_table.add(UsbResource { device });
      usbdevices.push(Device { usbdevice, rid });
    }
  }

  Ok(json!(usbdevices))
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_webusb.d.ts")
}
