// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{serde_json, ZeroCopyBuf};
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;

pub(crate) struct WebGPUComputePass(
  pub(crate) RefCell<wgpu_core::command::ComputePass>,
);
impl Resource for WebGPUComputePass {
  fn name(&self) -> Cow<str> {
    "webGPUComputePass".into()
  }
}

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

  let compute_pipeline_resource = state
    .resource_table
    .get::<super::pipeline::WebGPUComputePipeline>(args.pipeline)
    .ok_or_else(bad_resource_id)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_set_pipeline(
    &mut compute_pass_resource.0.borrow_mut(),
    compute_pipeline_resource.0,
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

  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_dispatch(
    &mut compute_pass_resource.0.borrow_mut(),
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

  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.indirect_buffer)
    .ok_or_else(bad_resource_id)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_dispatch_indirect(
    &mut compute_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    args.indirect_offset,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassBeginPipelineStatisticsQueryArgs {
  compute_pass_rid: u32,
  query_set: u32,
  query_index: u32,
}

pub fn op_webgpu_compute_pass_begin_pipeline_statistics_query(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassBeginPipelineStatisticsQueryArgs =
    serde_json::from_value(args)?;

  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGPUQuerySet>(args.query_set)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_begin_pipeline_statistics_query(
      &mut compute_pass_resource.0.borrow_mut(),
      query_set_resource.0,
      args.query_index,
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassEndPipelineStatisticsQueryArgs {
  compute_pass_rid: u32,
}

pub fn op_webgpu_compute_pass_end_pipeline_statistics_query(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassEndPipelineStatisticsQueryArgs =
    serde_json::from_value(args)?;

  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_end_pipeline_statistics_query(
      &mut compute_pass_resource.0.borrow_mut(),
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassWriteTimestampArgs {
  compute_pass_rid: u32,
  query_set: u32,
  query_index: u32,
}

pub fn op_webgpu_compute_pass_write_timestamp(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassWriteTimestampArgs = serde_json::from_value(args)?;

  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGPUQuerySet>(args.query_set)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_write_timestamp(
      &mut compute_pass_resource.0.borrow_mut(),
      query_set_resource.0,
      args.query_index,
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePassEndPassArgs {
  command_encoder_rid: u32,
  compute_pass_rid: u32,
}

pub fn op_webgpu_compute_pass_end_pass(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePassEndPassArgs = serde_json::from_value(args)?;

  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<super::command_encoder::WebGPUCommandEncoder>(
      args.command_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;
  let compute_pass = &compute_pass_resource.0.borrow();

  gfx_select!(command_encoder => instance.command_encoder_run_compute_pass(
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

  let bind_group_resource = state
    .resource_table
    .get::<super::binding::WebGPUBindGroup>(args.bind_group)
    .ok_or_else(bad_resource_id)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_set_bind_group(
      &mut compute_pass_resource.0.borrow_mut(),
      args.index,
      bind_group_resource.0,
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

  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.group_label).unwrap();
    wgpu_core::command::compute_ffi::wgpu_compute_pass_push_debug_group(
      &mut compute_pass_resource.0.borrow_mut(),
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

  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_pop_debug_group(
    &mut compute_pass_resource.0.borrow_mut(),
  );

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

  let compute_pass_resource = state
    .resource_table
    .get::<WebGPUComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.marker_label).unwrap();
    wgpu_core::command::compute_ffi::wgpu_compute_pass_insert_debug_marker(
      &mut compute_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(json!({}))
}
