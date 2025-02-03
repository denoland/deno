// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::rc::Rc;

use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;

use super::error::WebGpuResult;
use crate::command_encoder::WebGpuCommandBuffer;
use crate::Instance;

pub struct WebGpuQueue(pub Instance, pub wgpu_core::id::QueueId);
impl Resource for WebGpuQueue {
  fn name(&self) -> Cow<str> {
    "webGPUQueue".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.queue_drop(self.1));
  }
}

#[op2]
#[serde]
pub fn op_webgpu_queue_submit(
  state: &mut OpState,
  #[smi] queue_rid: ResourceId,
  #[serde] command_buffers: Vec<ResourceId>,
) -> Result<WebGpuResult, ResourceError> {
  let instance = state.borrow::<Instance>();
  let queue_resource = state.resource_table.get::<WebGpuQueue>(queue_rid)?;
  let queue = queue_resource.1;

  let ids = command_buffers
    .iter()
    .map(|rid| {
      let buffer_resource =
        state.resource_table.get::<WebGpuCommandBuffer>(*rid)?;
      let mut id = buffer_resource.1.borrow_mut();
      Ok(id.take().unwrap())
    })
    .collect::<Result<Vec<_>, ResourceError>>()?;

  let maybe_err =
    gfx_select!(queue => instance.queue_submit(queue, &ids)).err();

  for rid in command_buffers {
    let resource = state.resource_table.take::<WebGpuCommandBuffer>(rid)?;
    resource.close();
  }

  Ok(WebGpuResult::maybe_err(maybe_err))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuImageDataLayout {
  offset: u64,
  bytes_per_row: Option<u32>,
  rows_per_image: Option<u32>,
}

impl From<GpuImageDataLayout> for wgpu_types::ImageDataLayout {
  fn from(layout: GpuImageDataLayout) -> Self {
    wgpu_types::ImageDataLayout {
      offset: layout.offset,
      bytes_per_row: layout.bytes_per_row,
      rows_per_image: layout.rows_per_image,
    }
  }
}

#[op2]
#[serde]
pub fn op_webgpu_write_buffer(
  state: &mut OpState,
  #[smi] queue_rid: ResourceId,
  #[smi] buffer: ResourceId,
  #[number] buffer_offset: u64,
  #[number] data_offset: usize,
  #[number] size: Option<usize>,
  #[buffer] buf: &[u8],
) -> Result<WebGpuResult, ResourceError> {
  let instance = state.borrow::<Instance>();
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer)?;
  let buffer = buffer_resource.1;
  let queue_resource = state.resource_table.get::<WebGpuQueue>(queue_rid)?;
  let queue = queue_resource.1;

  let data = match size {
    Some(size) => &buf[data_offset..(data_offset + size)],
    None => &buf[data_offset..],
  };
  let maybe_err = gfx_select!(queue => instance.queue_write_buffer(
    queue,
    buffer,
    buffer_offset,
    data
  ))
  .err();

  Ok(WebGpuResult::maybe_err(maybe_err))
}

#[op2]
#[serde]
pub fn op_webgpu_write_texture(
  state: &mut OpState,
  #[smi] queue_rid: ResourceId,
  #[serde] destination: super::command_encoder::GpuImageCopyTexture,
  #[serde] data_layout: GpuImageDataLayout,
  #[serde] size: wgpu_types::Extent3d,
  #[buffer] buf: &[u8],
) -> Result<WebGpuResult, ResourceError> {
  let instance = state.borrow::<Instance>();
  let texture_resource = state
    .resource_table
    .get::<super::texture::WebGpuTexture>(destination.texture)?;
  let queue_resource = state.resource_table.get::<WebGpuQueue>(queue_rid)?;
  let queue = queue_resource.1;

  let destination = wgpu_core::command::ImageCopyTexture {
    texture: texture_resource.id,
    mip_level: destination.mip_level,
    origin: destination.origin,
    aspect: destination.aspect,
  };
  let data_layout = data_layout.into();

  gfx_ok!(queue => instance.queue_write_texture(
    queue,
    &destination,
    buf,
    &data_layout,
    &size
  ))
}
