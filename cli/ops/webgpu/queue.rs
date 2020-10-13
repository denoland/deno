// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::super::reg_json_sync(rt, "op_webgpu_queue_submit", op_webgpu_queue_submit);
  super::super::reg_json_sync(rt, "op_webgpu_write_buffer", op_webgpu_write_buffer);
  super::super::reg_json_sync(rt, "op_webgpu_write_texture", op_webgpu_write_texture);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueSubmitArgs {
  instance_rid: u32,
  queue_rid: u32,
  command_buffers: [u32],
}

pub fn op_webgpu_queue_submit(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: QueueSubmitArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let queue = state
    .resource_table
    .get_mut::<wgc::id::QueueId>(args.queue_rid)
    .ok_or_else(bad_resource_id)?;

  instance.queue_submit(
    *queue,
    &args
      .command_buffers
      .iter()
      .map(|rid| {
        *state
          .resource_table
          .get_mut::<wgc::id::CommandBufferId>(*rid)
          .ok_or_else(bad_resource_id)?
      })
      .collect::<[wgc::id::CommandBufferId]>(),
  )?;

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
  data_offset: u64,
  size: Option<u64>,
}

pub fn op_webgpu_write_buffer(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: QueueWriteBufferArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let queue = state
    .resource_table
    .get_mut::<wgc::id::QueueId>(args.queue_rid)
    .ok_or_else(bad_resource_id)?;

  instance.queue_write_buffer(
    *queue,
    *state
      .resource_table
      .get_mut::<wgc::id::BufferId>(args.buffer)
      .ok_or_else(bad_resource_id)?,
    args.buffer_offset,
    &zero_copy[0][if let Some(size) = args.size {
      args.data_offset..(args.data_offset + size)
    } else {
      args.data_offset..
    }],
  )?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueWriteTextureArgs {
  instance_rid: u32,
  queue_rid: u32,
  destination: super::command_encoder::GPUTextureCopyView,
  data_layout: GPUTextureDataLayout,
  size: (), // TODO: mixed types
}

pub fn op_webgpu_write_texture(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: QueueWriteTextureArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let queue = state
    .resource_table
    .get_mut::<wgc::id::QueueId>(args.queue_rid)
    .ok_or_else(bad_resource_id)?;

  instance.queue_write_texture(
    *queue,
    &wgc::command::TextureCopyView {
      texture: *state
        .resource_table
        .get_mut::<wgc::id::TextureId>(args.destination.texture)
        .ok_or_else(bad_resource_id)?,
      mip_level: args.destination.mip_level.unwrap_or(0),
      origin: Default::default(),
    },
    &*zero_copy[0],
    &wgt::TextureDataLayout {
      offset: args.data_layout.offset.unwrap_or(0),
      bytes_per_row: args.data_layout.bytes_per_row,
      rows_per_image: args.data_layout.rows_per_image,
    },
    &wgt::Extent3d {
      width: 0,
      height: 0,
      depth: 0,
    },
  )?;

  Ok(json!({}))
}
