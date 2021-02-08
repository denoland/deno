// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;

type WebGPUQueue = super::WebGPUDevice;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueSubmitArgs {
  instance_rid: u32,
  queue_rid: u32,
  command_buffers: Vec<u32>,
}

pub fn op_webgpu_queue_submit(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: QueueSubmitArgs = serde_json::from_value(args)?;

  let queue_resource = state
    .resource_table
    .get::<WebGPUQueue>(args.queue_rid)
    .ok_or_else(bad_resource_id)?;
  let queue = queue_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let mut ids = vec![];

  for rid in args.command_buffers {
    let buffer_resource = state
      .resource_table
      .get::<super::command_encoder::WebGPUCommandBuffer>(rid)
      .ok_or_else(bad_resource_id)?;
    ids.push(buffer_resource.0);
  }

  wgc::gfx_select!(queue => instance.queue_submit(queue, &ids))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUTextureDataLayout {
  offset: Option<u64>,
  bytes_per_row: Option<u32>,
  rows_per_image: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueWriteBufferArgs {
  instance_rid: u32,
  queue_rid: u32,
  buffer: u32,
  buffer_offset: u64,
  data_offset: usize,
  size: Option<usize>,
}

pub fn op_webgpu_write_buffer(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: QueueWriteBufferArgs = serde_json::from_value(args)?;

  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.buffer)
    .ok_or_else(bad_resource_id)?;
  let buffer = buffer_resource.0;
  let queue_resource = state
    .resource_table
    .get::<WebGPUQueue>(args.queue_rid)
    .ok_or_else(bad_resource_id)?;
  let queue = queue_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let data = match args.size {
    Some(size) => &zero_copy[0][args.data_offset..(args.data_offset + size)],
    None => &zero_copy[0][args.data_offset..],
  };
  wgc::gfx_select!(queue => instance.queue_write_buffer(
    queue,
    buffer,
    args.buffer_offset,
    data
  ))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueWriteTextureArgs {
  instance_rid: u32,
  queue_rid: u32,
  destination: super::command_encoder::GPUTextureCopyView,
  data_layout: GPUTextureDataLayout,
  size: super::texture::GPUExtent3D,
}

pub fn op_webgpu_write_texture(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: QueueWriteTextureArgs = serde_json::from_value(args)?;

  let texture_resource = state
    .resource_table
    .get::<super::texture::WebGPUTexture>(args.destination.texture)
    .ok_or_else(bad_resource_id)?;
  let queue_resource = state
    .resource_table
    .get::<WebGPUQueue>(args.queue_rid)
    .ok_or_else(bad_resource_id)?;
  let queue = queue_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let destination = wgc::command::TextureCopyView {
    texture: texture_resource.0,
    mip_level: args.destination.mip_level.unwrap_or(0),
    origin: args
      .destination
      .origin
      .map_or(Default::default(), |origin| wgt::Origin3d {
        x: origin.x.unwrap_or(0),
        y: origin.y.unwrap_or(0),
        z: origin.z.unwrap_or(0),
      }),
  };
  let data_layout = wgt::TextureDataLayout {
    offset: args.data_layout.offset.unwrap_or(0),
    bytes_per_row: args.data_layout.bytes_per_row.unwrap_or(0),
    rows_per_image: args.data_layout.rows_per_image.unwrap_or(0),
  };
  wgc::gfx_select!(queue => instance.queue_write_texture(
    queue,
    &destination,
    &*zero_copy[0],
    &data_layout,
    &wgt::Extent3d {
      width: args.size.width,
      height: args.size.height,
      depth: args.size.depth,
    }
  ))?;

  Ok(json!({}))
}
