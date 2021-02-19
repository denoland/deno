// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use deno_core::{BufVec, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub(crate) struct WebGPUBuffer(pub(crate) wgpu_core::id::BufferId);
impl Resource for WebGPUBuffer {
  fn name(&self) -> Cow<str> {
    "webGPUBuffer".into()
  }
}

struct WebGPUBufferMapped(*mut u8, usize);
impl Resource for WebGPUBufferMapped {
  fn name(&self) -> Cow<str> {
    "webGPUBufferMapped".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBufferArgs {
  device_rid: u32,
  label: Option<String>,
  size: u64,
  usage: u32,
  mapped_at_creation: Option<bool>,
}

pub fn op_webgpu_create_buffer(
  state: &mut OpState,
  args: CreateBufferArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let descriptor = wgpu_core::resource::BufferDescriptor {
    label: args.label.map(Cow::from),
    size: args.size,
    usage: wgpu_types::BufferUsage::from_bits(args.usage).unwrap(),
    mapped_at_creation: args.mapped_at_creation.unwrap_or(false),
  };

  let buffer = gfx_select_err!(device => instance.device_create_buffer(
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
pub struct BufferGetMapAsyncArgs {
  buffer_rid: u32,
  device_rid: u32,
  mode: u32,
  offset: u64,
  size: u64,
}

pub async fn op_webgpu_buffer_get_map_async(
  state: Rc<RefCell<OpState>>,
  args: BufferGetMapAsyncArgs,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let (sender, receiver) = oneshot::channel::<Result<(), AnyError>>();

  let device;
  {
    let state_ = state.borrow();
    let instance = state_.borrow::<super::Instance>();
    let buffer_resource = state_
      .resource_table
      .get::<WebGPUBuffer>(args.buffer_rid)
      .ok_or_else(bad_resource_id)?;
    let buffer = buffer_resource.0;
    let device_resource = state_
      .resource_table
      .get::<super::WebGPUDevice>(args.device_rid)
      .ok_or_else(bad_resource_id)?;
    device = device_resource.0;

    let boxed_sender = Box::new(sender);
    let sender_ptr = Box::into_raw(boxed_sender) as *mut u8;

    extern "C" fn buffer_map_future_wrapper(
      status: wgpu_core::resource::BufferMapAsyncStatus,
      user_data: *mut u8,
    ) {
      let sender_ptr = user_data as *mut oneshot::Sender<Result<(), AnyError>>;
      let boxed_sender = unsafe { Box::from_raw(sender_ptr) };
      boxed_sender
        .send(match status {
          wgpu_core::resource::BufferMapAsyncStatus::Success => Ok(()),
          _ => unreachable!(), // TODO
        })
        .unwrap();
    }

    gfx_select!(buffer => instance.buffer_map_async(
      buffer,
      args.offset..(args.offset + args.size),
      wgpu_core::resource::BufferMapOperation {
        host: match args.mode {
          1 => wgpu_core::device::HostMap::Read,
          2 => wgpu_core::device::HostMap::Write,
          _ => unreachable!(),
        },
        callback: buffer_map_future_wrapper,
        user_data: sender_ptr,
      }
    ))?;
  }

  let done = Rc::new(RefCell::new(false));
  let done_ = done.clone();
  let device_poll_fut = async move {
    while !*done.borrow() {
      {
        let state = state.borrow();
        let instance = state.borrow::<super::Instance>();
        gfx_select!(device => instance.device_poll(device, false)).unwrap()
      }
      tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok::<(), AnyError>(())
  };

  let receiver_fut = async move {
    receiver.await??;
    let mut done = done_.borrow_mut();
    *done = true;
    Ok::<(), AnyError>(())
  };

  tokio::try_join!(device_poll_fut, receiver_fut)?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferGetMappedRangeArgs {
  buffer_rid: u32,
  offset: u64,
  size: u64,
}

pub fn op_webgpu_buffer_get_mapped_range(
  state: &mut OpState,
  args: BufferGetMappedRangeArgs,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let buffer_resource = state
    .resource_table
    .get::<WebGPUBuffer>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = buffer_resource.0;

  let slice_pointer = gfx_select!(buffer => instance.buffer_get_mapped_range(
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
    .add(WebGPUBufferMapped(slice_pointer, args.size as usize));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferUnmapArgs {
  buffer_rid: u32,
  mapped_rid: u32,
}

pub fn op_webgpu_buffer_unmap(
  state: &mut OpState,
  args: BufferUnmapArgs,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let mapped_resource = state
    .resource_table
    .take::<WebGPUBufferMapped>(args.mapped_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state.borrow::<super::Instance>();
  let buffer_resource = state
    .resource_table
    .get::<WebGPUBuffer>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = buffer_resource.0;

  let slice_pointer = mapped_resource.0;
  let size = mapped_resource.1;

  if let Some(buffer) = zero_copy.get(0) {
    let slice = unsafe { std::slice::from_raw_parts_mut(slice_pointer, size) };
    slice.copy_from_slice(&buffer);
  }

  gfx_select!(buffer => instance.buffer_unmap(buffer))?;

  Ok(json!({}))
}
