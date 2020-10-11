// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBufferArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  size: u64,
  usage: (), // TODO
  mapped_at_creation: Option<bool>,
}

pub fn op_webgpu_create_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateBufferArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let device = state
    .resource_table
    .get_mut::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;

  let buffer = instance.device_create_buffer(
    *device,
    &wgc::resource::BufferDescriptor {
      label: args.label.map(|label| Cow::Borrowed(&label)),
      size: args.size,
      usage: (),
      mapped_at_creation: args.mapped_at_creation.unwrap_or(false),
    },
    (), // TODO
  )?;

  let rid = state.resource_table.add("webGPUBuffer", Box::new(buffer));

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
}

pub async fn op_webgpu_buffer_get_map_async(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: BufferGetMapAsyncArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = state
    .resource_table
    .get_mut::<wgc::id::BufferId>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;

  instance.buffer_map_async(
    *buffer,
    // TODO
  )?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferGetMappedRangeArgs {
  instance_rid: u32,
  buffer_rid: u32,
  offset: u64,
  size: Option<u64>,
}

pub fn op_webgpu_buffer_get_mapped_range(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: BufferGetMappedRangeArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = state
    .resource_table
    .get_mut::<wgc::id::BufferId>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;

  let slice = instance.buffer_get_mapped_range(
    *buffer,
    args.offset,
    args.size, // TODO
  )?; // TODO

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferUnmapArgs {
  instance_rid: u32,
  buffer_rid: u32,
}

pub fn op_webgpu_buffer_unmap(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: BufferUnmapArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let buffer = state
    .resource_table
    .get_mut::<wgc::id::BufferId>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;

  instance.buffer_unmap(*buffer)?;

  Ok(json!({}))
}
