// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::op;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryFrom;
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

#[op]
pub fn op_webgpu_create_buffer(
  state: &mut OpState,
  device_rid: ResourceId,
  label: Option<String>,
  size: u64,
  usage: u32,
  mapped_at_creation: bool,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.0;

  let descriptor = wgpu_core::resource::BufferDescriptor {
    label: label.map(Cow::from),
    size,
    usage: wgpu_types::BufferUsages::from_bits(usage)
      .ok_or_else(|| type_error("usage is not valid"))?,
    mapped_at_creation,
  };

  gfx_put!(device => instance.device_create_buffer(
    device,
    &descriptor,
    std::marker::PhantomData
  ) => state, WebGpuBuffer)
}

#[op]
pub async fn op_webgpu_buffer_get_map_async(
  state: Rc<RefCell<OpState>>,
  buffer_rid: ResourceId,
  device_rid: ResourceId,
  mode: u32,
  offset: u64,
  size: u64,
) -> Result<WebGpuResult, AnyError> {
  let (sender, receiver) = oneshot::channel::<Result<(), AnyError>>();

  let device;
  {
    let state_ = state.borrow();
    let instance = state_.borrow::<super::Instance>();
    let buffer_resource =
      state_.resource_table.get::<WebGpuBuffer>(buffer_rid)?;
    let buffer = buffer_resource.0;
    let device_resource = state_
      .resource_table
      .get::<super::WebGpuDevice>(device_rid)?;
    device = device_resource.0;

    let callback = Box::new(move |status| {
      sender
        .send(match status {
          wgpu_core::resource::BufferMapAsyncStatus::Success => Ok(()),
          _ => unreachable!(), // TODO
        })
        .unwrap();
    });

    // TODO(lucacasonato): error handling
    let maybe_err = gfx_select!(buffer => instance.buffer_map_async(
            buffer,
            offset..(offset + size),
            wgpu_core::resource::BufferMapOperation {
                host: match mode {
                    1 => wgpu_core::device::HostMap::Read,
                    2 => wgpu_core::device::HostMap::Write,
                    _ => unreachable!(),
                },
                callback: wgpu_core::resource::BufferMapCallback::from_rust(callback),
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
        gfx_select!(device => instance.device_poll(device, wgpu_types::Maintain::Wait)).unwrap();
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

#[op]
pub fn op_webgpu_buffer_get_mapped_range(
  state: &mut OpState,
  buffer_rid: ResourceId,
  offset: u64,
  size: Option<u64>,
  mut buf: ZeroCopyBuf,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let buffer_resource = state.resource_table.get::<WebGpuBuffer>(buffer_rid)?;
  let buffer = buffer_resource.0;

  let (slice_pointer, range_size) =
    gfx_select!(buffer => instance.buffer_get_mapped_range(
      buffer,
      offset,
      size
    ))
    .map_err(|e| DomExceptionOperationError::new(&e.to_string()))?;

  // TODO(crowlKats):
  #[allow(clippy::undocumented_unsafe_blocks)]
  let slice = unsafe {
    std::slice::from_raw_parts_mut(slice_pointer, range_size as usize)
  };
  buf.copy_from_slice(slice);

  let rid = state
    .resource_table
    .add(WebGpuBufferMapped(slice_pointer, range_size as usize));

  Ok(WebGpuResult::rid(rid))
}

#[op]
pub fn op_webgpu_buffer_unmap(
  state: &mut OpState,
  buffer_rid: ResourceId,
  mapped_rid: ResourceId,
  buf: Option<ZeroCopyBuf>,
) -> Result<WebGpuResult, AnyError> {
  let mapped_resource = state
    .resource_table
    .take::<WebGpuBufferMapped>(mapped_rid)?;
  let instance = state.borrow::<super::Instance>();
  let buffer_resource = state.resource_table.get::<WebGpuBuffer>(buffer_rid)?;
  let buffer = buffer_resource.0;

  if let Some(buf) = buf {
    // TODO(crowlKats):
    #[allow(clippy::undocumented_unsafe_blocks)]
    let slice = unsafe {
      std::slice::from_raw_parts_mut(mapped_resource.0, mapped_resource.1)
    };
    slice.copy_from_slice(&buf);
  }

  gfx_ok!(buffer => instance.buffer_unmap(buffer))
}
