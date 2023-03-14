// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::num::NonZeroU32;

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use serde::Deserialize;

use super::error::WebGpuResult;

type WebGpuQueue = super::WebGpuDevice;

#[op]
pub fn op_webgpu_queue_submit(
  state: &mut OpState,
  queue_rid: ResourceId,
  command_buffers: Vec<ResourceId>,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let queue_resource = state.resource_table.get::<WebGpuQueue>(queue_rid)?;
  let queue = queue_resource.0;

  let ids = command_buffers
    .iter()
    .map(|rid| {
      let buffer_resource =
        state
          .resource_table
          .get::<super::command_encoder::WebGpuCommandBuffer>(*rid)?;
      Ok(buffer_resource.0)
    })
    .collect::<Result<Vec<_>, AnyError>>()?;

  let maybe_err =
    gfx_select!(queue => instance.queue_submit(queue, &ids)).err();

  for rid in command_buffers {
    state.resource_table.close(rid)?;
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
      bytes_per_row: NonZeroU32::new(layout.bytes_per_row.unwrap_or(0)),
      rows_per_image: NonZeroU32::new(layout.rows_per_image.unwrap_or(0)),
    }
  }
}

#[op]
pub fn op_webgpu_write_buffer(
  state: &mut OpState,
  queue_rid: ResourceId,
  buffer: ResourceId,
  buffer_offset: u64,
  data_offset: usize,
  size: Option<usize>,
  buf: ZeroCopyBuf,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer)?;
  let buffer = buffer_resource.0;
  let queue_resource = state.resource_table.get::<WebGpuQueue>(queue_rid)?;
  let queue = queue_resource.0;

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

#[op]
pub fn op_webgpu_write_texture(
  state: &mut OpState,
  queue_rid: ResourceId,
  destination: super::command_encoder::GpuImageCopyTexture,
  data_layout: GpuImageDataLayout,
  size: wgpu_types::Extent3d,
  buf: ZeroCopyBuf,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let texture_resource = state
    .resource_table
    .get::<super::texture::WebGpuTexture>(destination.texture)?;
  let queue_resource = state.resource_table.get::<WebGpuQueue>(queue_rid)?;
  let queue = queue_resource.0;

  let destination = wgpu_core::command::ImageCopyTexture {
    texture: texture_resource.0,
    mip_level: destination.mip_level,
    origin: destination.origin,
    aspect: destination.aspect,
  };
  let data_layout = data_layout.into();

  gfx_ok!(queue => instance.queue_write_texture(
    queue,
    &destination,
    &*buf,
    &data_layout,
    &size
  ))
}
