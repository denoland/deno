// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::futures::channel::oneshot;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::super::reg_json_sync(
    rt,
    "op_webgpu_create_buffer",
    op_webgpu_create_buffer,
  );
  super::super::reg_json_async(
    rt,
    "op_webgpu_buffer_get_map_async",
    op_webgpu_buffer_get_map_async,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_buffer_get_mapped_range",
    op_webgpu_buffer_get_mapped_range,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_buffer_unmap",
    op_webgpu_buffer_unmap,
  );
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

  let device = *state
    .resource_table
    .get::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  let descriptor = wgc::resource::BufferDescriptor {
    label: args.label.map(|label| Cow::Owned(label)),
    size: args.size,
    usage: wgt::BufferUsage::from_bits(args.usage).unwrap(),
    mapped_at_creation: args.mapped_at_creation.unwrap_or(false),
  };
  let buffer = wgc::gfx_select!(device => instance.device_create_buffer(
    device,
    &descriptor,
    std::marker::PhantomData
  ))?;

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
  offset: u64,
  size: u64,
}

pub async fn op_webgpu_buffer_get_map_async(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: BufferGetMapAsyncArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  let buffer = *state
    .resource_table
    .get_mut::<wgc::id::BufferId>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  let (sender, receiver) = oneshot::channel::<Result<(), AnyError>>();

  let boxed_sender = Box::new(sender);
  let sender_ptr = Box::into_raw(boxed_sender);
  let sender_ptr = unsafe {
    std::mem::transmute::<*mut oneshot::Sender<Result<(), AnyError>>, *mut u8>(
      sender_ptr,
    )
  };

  extern "C" fn buffer_map_future_wrapper(
    status: wgc::resource::BufferMapAsyncStatus,
    user_data: *mut u8,
  ) {
    let sender_ptr = unsafe {
      std::mem::transmute::<*mut u8, *mut oneshot::Sender<Result<(), AnyError>>>(
        user_data,
      )
    };
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
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: BufferGetMappedRangeArgs = serde_json::from_value(args)?;

  let buffer = *state
    .resource_table
    .get::<wgc::id::BufferId>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  let slice_pointer = wgc::gfx_select!(buffer => instance.buffer_get_mapped_range(
    buffer,
    args.offset,
    std::num::NonZeroU64::new(args.size)
  ))?;

  // TODO: use
  let _slice = unsafe {
    std::slice::from_raw_parts_mut(slice_pointer, args.size as usize)
  };

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

  let buffer = *state
    .resource_table
    .get::<wgc::id::BufferId>(args.buffer_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::gfx_select!(buffer => instance.buffer_unmap(buffer))?;

  Ok(json!({}))
}
