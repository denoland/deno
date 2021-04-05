// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::error::DomExceptionOperationError;
use super::error::WebGpuError;

pub(crate) struct WebGpuBuffer(pub(crate) wgpu_core::id::BufferId);
impl Resource for WebGpuBuffer {
  fn name(&self) -> Cow<str> {
    "webGPUBuffer".into()
  }
}

struct WebGpuBufferMapped(*mut u8, usize);
impl Resource for WebGpuBufferMapped {
  fn name(&self) -> Cow<str> {
    "webGPUBufferMapped".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBufferArgs {
  device_rid: ResourceId,
  label: Option<String>,
  size: u64,
  usage: u32,
  mapped_at_creation: Option<bool>,
}

pub fn op_webgpu_create_buffer(
  state: &mut OpState,
  args: CreateBufferArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let descriptor = wgpu_core::resource::BufferDescriptor {
    label: args.label.map(Cow::from),
    size: args.size,
    usage: wgpu_types::BufferUsage::from_bits(args.usage).unwrap(),
    mapped_at_creation: args.mapped_at_creation.unwrap_or(false),
  };

  gfx_put!(device => instance.device_create_buffer(
    device,
    &descriptor,
    std::marker::PhantomData
  ) => state, WebGpuBuffer)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferGetMapAsyncArgs {
  buffer_rid: ResourceId,
  device_rid: ResourceId,
  mode: u32,
  offset: u64,
  size: u64,
}

pub async fn op_webgpu_buffer_get_map_async(
  state: Rc<RefCell<OpState>>,
  args: BufferGetMapAsyncArgs,
  _bufs: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let (sender, receiver) = oneshot::channel::<Result<(), AnyError>>();

  let device;
  {
    let state_ = state.borrow();
    let instance = state_.borrow::<super::Instance>();
    let buffer_resource = state_
      .resource_table
      .get::<WebGpuBuffer>(args.buffer_rid)
      .ok_or_else(bad_resource_id)?;
    let buffer = buffer_resource.0;
    let device_resource = state_
      .resource_table
      .get::<super::WebGpuDevice>(args.device_rid)
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

    // TODO(lucacasonato): error handling
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
  buffer_rid: ResourceId,
  offset: u64,
  size: u64,
}

pub fn op_webgpu_buffer_get_mapped_range(
  state: &mut OpState,
  args: BufferGetMappedRangeArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let mut zero_copy = zero_copy.ok_or_else(null_opbuf)?;
  let instance = state.borrow::<super::Instance>();
  let buffer_resource = state
    .resource_table
    .get::<WebGpuBuffer>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = buffer_resource.0;

  let slice_pointer = gfx_select!(buffer => instance.buffer_get_mapped_range(
    buffer,
    args.offset,
    std::num::NonZeroU64::new(args.size)
  ))
  .map_err(|e| DomExceptionOperationError::new(&e.to_string()))?;

  let slice = unsafe {
    std::slice::from_raw_parts_mut(slice_pointer, args.size as usize)
  };
  zero_copy.copy_from_slice(slice);

  let rid = state
    .resource_table
    .add(WebGpuBufferMapped(slice_pointer, args.size as usize));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferUnmapArgs {
  buffer_rid: ResourceId,
  mapped_rid: ResourceId,
}

pub fn op_webgpu_buffer_unmap(
  state: &mut OpState,
  args: BufferUnmapArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let mapped_resource = state
    .resource_table
    .take::<WebGpuBufferMapped>(args.mapped_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state.borrow::<super::Instance>();
  let buffer_resource = state
    .resource_table
    .get::<WebGpuBuffer>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = buffer_resource.0;

  let slice_pointer = mapped_resource.0;
  let size = mapped_resource.1;

  if let Some(buffer) = zero_copy {
    let slice = unsafe { std::slice::from_raw_parts_mut(slice_pointer, size) };
    slice.copy_from_slice(&buffer);
  }

  let maybe_err = gfx_select!(buffer => instance.buffer_unmap(buffer)).err();

  Ok(json!({ "err": maybe_err.map(WebGpuError::from) }))
}
