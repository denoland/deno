// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use deno_core::{BufVec, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) struct WebGPUBuffer(pub(crate) wgc::id::BufferId);
impl Resource for WebGPUBuffer {
  fn name(&self) -> Cow<str> {
    "webGPUBuffer".into()
  }
}

struct WebGPUBufferMapped(RefCell<Vec<u8>>);
impl Resource for WebGPUBufferMapped {
  fn name(&self) -> Cow<str> {
    "webGPUBufferMapped".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBufferArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  size: u64,
  usage: u32,
  mapped_at_creation: Option<bool>,
}

pub fn op_webgpu_create_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateBufferArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let ref instance = instance_resource.0;

  let descriptor = wgc::resource::BufferDescriptor {
    label: args.label.map(Cow::Owned),
    size: args.size,
    usage: wgt::BufferUsage::from_bits(args.usage).unwrap(),
    mapped_at_creation: args.mapped_at_creation.unwrap_or(false),
  };
  let buffer = wgc::gfx_select!(device => instance.device_create_buffer(
    device,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state.resource_table.add(WebGPUBuffer(buffer));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferGetMapAsyncArgs {
  instance_rid: u32,
  buffer_rid: u32,
  mode: u32,
  offset: u64,
  size: u64,
}

pub async fn op_webgpu_buffer_get_map_async(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: BufferGetMapAsyncArgs = serde_json::from_value(args)?;

  let state = state.borrow_mut();
  let buffer_resource = state
    .resource_table
    .get::<WebGPUBuffer>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = buffer_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let ref instance = instance_resource.0;

  let (sender, receiver) = oneshot::channel::<Result<(), AnyError>>();

  let boxed_sender = Box::new(sender);
  let sender_ptr = Box::into_raw(boxed_sender) as *mut u8;

  extern "C" fn buffer_map_future_wrapper(
    status: wgc::resource::BufferMapAsyncStatus,
    user_data: *mut u8,
  ) {
    let sender_ptr = user_data as *mut oneshot::Sender<Result<(), AnyError>>;
    let boxed_sender = unsafe { Box::from_raw(sender_ptr) };
    boxed_sender
      .send(match status {
        wgc::resource::BufferMapAsyncStatus::Success => Ok(()),
        _ => unreachable!(), // TODO
      })
      .unwrap();
  }

  wgc::gfx_select!(buffer => instance.buffer_map_async(
    buffer,
    args.offset..(args.offset + args.size),
    wgc::resource::BufferMapOperation {
      host: match args.mode {
        1 => wgc::device::HostMap::Read,
        2 => wgc::device::HostMap::Write,
        _ => unreachable!(),
      },
      callback: buffer_map_future_wrapper,
      user_data: sender_ptr,
    }
  ))?;

  receiver.await??;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferGetMappedRangeArgs {
  instance_rid: u32,
  buffer_rid: u32,
  offset: u64,
  size: u64,
}

pub fn op_webgpu_buffer_get_mapped_range(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: BufferGetMappedRangeArgs = serde_json::from_value(args)?;

  let buffer_resource = state
    .resource_table
    .get::<WebGPUBuffer>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = buffer_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let ref instance = instance_resource.0;

  let slice_pointer = wgc::gfx_select!(buffer => instance.buffer_get_mapped_range(
    buffer,
    args.offset,
    std::num::NonZeroU64::new(args.size)
  ))?;

  let slice = unsafe {
    std::slice::from_raw_parts_mut(slice_pointer, args.size as usize)
  };
  zero_copy[0].copy_from_slice(slice);

  let rid = state
    .resource_table
    .add(WebGPUBufferMapped(RefCell::new(slice.to_vec())));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferUnmapArgs {
  instance_rid: u32,
  buffer_rid: u32,
  mapped_rid: u32,
}

pub fn op_webgpu_buffer_unmap(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: BufferUnmapArgs = serde_json::from_value(args)?;

  let buffer_resource = state
    .resource_table
    .get::<WebGPUBuffer>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = buffer_resource.0;
  let mapped_resource = state
    .resource_table
    .get::<WebGPUBufferMapped>(args.mapped_rid)
    .ok_or_else(bad_resource_id)?;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let ref instance = instance_resource.0;

  mapped_resource
    .0
    .borrow_mut()
    .copy_from_slice(&zero_copy[0]);

  wgc::gfx_select!(buffer => instance.buffer_unmap(buffer))?;

  Ok(json!({}))
}
