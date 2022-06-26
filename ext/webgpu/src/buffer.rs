// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::op;

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
use super::error::WebGpuResult;

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
  mapped_at_creation: bool,
}

#[op]
pub fn op_webgpu_create_buffer(
  state: &mut OpState,
  args: CreateBufferArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let descriptor = wgpu_core::resource::BufferDescriptor {
    label: args.label.map(Cow::from),
    size: args.size,
    usage: wgpu_types::BufferUsages::from_bits(args.usage)
      .ok_or_else(|| type_error("usage is not valid"))?,
    mapped_at_creation: args.mapped_at_creation,
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

#[op]
pub async fn op_webgpu_buffer_get_map_async(
  state: Rc<RefCell<OpState>>,
  args: BufferGetMapAsyncArgs,
) -> Result<WebGpuResult, AnyError> {
  let (sender, receiver) = oneshot::channel::<Result<(), AnyError>>();

  let device;
  {
    let state_ = state.borrow();
    let instance = state_.borrow::<super::Instance>();
    let buffer_resource =
      state_.resource_table.get::<WebGpuBuffer>(args.buffer_rid)?;
    let buffer = buffer_resource.0;
    let device_resource = state_
      .resource_table
      .get::<super::WebGpuDevice>(args.device_rid)?;
    device = device_resource.0;

    let boxed_sender = Box::new(sender);
    let sender_ptr = Box::into_raw(boxed_sender) as *mut u8;

    extern "C" fn buffer_map_future_wrapper(
      status: wgpu_core::resource::BufferMapAsyncStatus,
      user_data: *mut u8,
    ) {
      let sender_ptr = user_data as *mut oneshot::Sender<Result<(), AnyError>>;
      // TODO(bartlomieju):
      #[allow(clippy::undocumented_unsafe_blocks)]
      let boxed_sender = unsafe { Box::from_raw(sender_ptr) };
      boxed_sender
        .send(match status {
          wgpu_core::resource::BufferMapAsyncStatus::Success => Ok(()),
          _ => unreachable!(), // TODO
        })
        .unwrap();
    }

    // TODO(lucacasonato): error handling
    let maybe_err = gfx_select!(buffer => instance.buffer_map_async(
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
    ))
    .err();

    if maybe_err.is_some() {
      return Ok(WebGpuResult::maybe_err(maybe_err));
    }
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

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferGetMappedRangeArgs {
  buffer_rid: ResourceId,
  offset: u64,
  size: Option<u64>,
}

#[op]
pub fn op_webgpu_buffer_get_mapped_range(
  state: &mut OpState,
  args: BufferGetMappedRangeArgs,
  mut zero_copy: ZeroCopyBuf,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let buffer_resource =
    state.resource_table.get::<WebGpuBuffer>(args.buffer_rid)?;
  let buffer = buffer_resource.0;

  let (slice_pointer, range_size) =
    gfx_select!(buffer => instance.buffer_get_mapped_range(
      buffer,
      args.offset,
      args.size
    ))
    .map_err(|e| DomExceptionOperationError::new(&e.to_string()))?;

  // TODO(bartlomieju):
  #[allow(clippy::undocumented_unsafe_blocks)]
  let slice = unsafe {
    std::slice::from_raw_parts_mut(slice_pointer, range_size as usize)
  };
  zero_copy.copy_from_slice(slice);

  let rid = state
    .resource_table
    .add(WebGpuBufferMapped(slice_pointer, range_size as usize));

  Ok(WebGpuResult::rid(rid))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferUnmapArgs {
  buffer_rid: ResourceId,
  mapped_rid: ResourceId,
}

#[op]
pub fn op_webgpu_buffer_unmap(
  state: &mut OpState,
  args: BufferUnmapArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<WebGpuResult, AnyError> {
  let mapped_resource = state
    .resource_table
    .take::<WebGpuBufferMapped>(args.mapped_rid)?;
  let instance = state.borrow::<super::Instance>();
  let buffer_resource =
    state.resource_table.get::<WebGpuBuffer>(args.buffer_rid)?;
  let buffer = buffer_resource.0;

  let slice_pointer = mapped_resource.0;
  let size = mapped_resource.1;

  if let Some(buffer) = zero_copy {
    // TODO(bartlomieju):
    #[allow(clippy::undocumented_unsafe_blocks)]
    let slice = unsafe { std::slice::from_raw_parts_mut(slice_pointer, size) };
    slice.copy_from_slice(&buffer);
  }

  gfx_ok!(buffer => instance.buffer_unmap(buffer))
}
