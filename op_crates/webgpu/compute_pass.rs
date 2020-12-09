// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassSetPipelineArgs {
  compute_pass_rid: u32,
  pipeline: u32,
}

pub fn op_webgpu_compute_pass_set_pipeline(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassSetPipelineArgs = serde_json::from_value(args)?;

  let pipeline_id = *state
    .resource_table
    .get_mut::<wgc::id::ComputePipelineId>(args.pipeline)
    .ok_or_else(bad_resource_id)?;
  let compute_pass = state
    .resource_table
    .get_mut::<wgc::command::ComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::compute_ffi::wgpu_compute_pass_set_pipeline(
    compute_pass,
    pipeline_id,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassDispatchArgs {
  compute_pass_rid: u32,
  x: u32,
  y: u32,
  z: u32,
}

pub fn op_webgpu_compute_pass_dispatch(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassDispatchArgs = serde_json::from_value(args)?;

  let compute_pass = state
    .resource_table
    .get_mut::<wgc::command::ComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::compute_ffi::wgpu_compute_pass_dispatch(
    compute_pass,
    args.x,
    args.y,
    args.z,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassDispatchIndirectArgs {
  compute_pass_rid: u32,
  indirect_buffer: u32,
  indirect_offset: u64,
}

pub fn op_webgpu_compute_pass_dispatch_indirect(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassDispatchIndirectArgs = serde_json::from_value(args)?;

  let buffer_id = *state
    .resource_table
    .get_mut::<wgc::id::BufferId>(args.indirect_buffer)
    .ok_or_else(bad_resource_id)?;
  let compute_pass = state
    .resource_table
    .get_mut::<wgc::command::ComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::compute_ffi::wgpu_compute_pass_dispatch_indirect(
    compute_pass,
    buffer_id,
    args.indirect_offset,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassEndPassArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  compute_pass_rid: u32,
}

pub fn op_webgpu_compute_pass_end_pass(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassEndPassArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = *state
    .resource_table
    .get::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let compute_pass = state
    .resource_table
    .get::<wgc::command::ComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::gfx_select!(command_encoder => instance.command_encoder_run_compute_pass(
    command_encoder,
    compute_pass
  ))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassSetBindGroupArgs {
  compute_pass_rid: u32,
  index: u32,
  bind_group: u32,
  dynamic_offsets_data: Option<Vec<u32>>,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
}

pub fn op_webgpu_compute_pass_set_bind_group(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassSetBindGroupArgs = serde_json::from_value(args)?;

  let bind_group_id = *state
    .resource_table
    .get::<wgc::id::BindGroupId>(args.bind_group)
    .ok_or_else(bad_resource_id)?;

  let compute_pass = state
    .resource_table
    .get_mut::<wgc::command::ComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgc::command::compute_ffi::wgpu_compute_pass_set_bind_group(
      compute_pass,
      args.index,
      bind_group_id,
      match args.dynamic_offsets_data {
        Some(data) => data.as_ptr(),
        None => {
          let (prefix, data, suffix) = zero_copy[0].align_to::<u32>();
          assert!(prefix.is_empty());
          assert!(suffix.is_empty());
          data[args.dynamic_offsets_data_start..].as_ptr()
        }
      },
      args.dynamic_offsets_data_length,
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassPushDebugGroupArgs {
  compute_pass_rid: u32,
  group_label: String,
}

pub fn op_webgpu_compute_pass_push_debug_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassPushDebugGroupArgs = serde_json::from_value(args)?;

  let compute_pass = state
    .resource_table
    .get_mut::<wgc::command::ComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.group_label).unwrap();
    wgc::command::compute_ffi::wgpu_compute_pass_push_debug_group(
      compute_pass,
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassPopDebugGroupArgs {
  compute_pass_rid: u32,
}

pub fn op_webgpu_compute_pass_pop_debug_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassPopDebugGroupArgs = serde_json::from_value(args)?;

  let compute_pass = state
    .resource_table
    .get_mut::<wgc::command::ComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::compute_ffi::wgpu_compute_pass_pop_debug_group(compute_pass);

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassInsertDebugMarkerArgs {
  compute_pass_rid: u32,
  marker_label: String,
}

pub fn op_webgpu_compute_pass_insert_debug_marker(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassInsertDebugMarkerArgs = serde_json::from_value(args)?;

  let compute_pass = state
    .resource_table
    .get_mut::<wgc::command::ComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.marker_label).unwrap();
    wgc::command::compute_ffi::wgpu_compute_pass_insert_debug_marker(
      compute_pass,
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(json!({}))
}
