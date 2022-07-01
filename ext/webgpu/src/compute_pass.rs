// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;

use super::error::WebGpuResult;

pub(crate) struct WebGpuComputePass(
  pub(crate) RefCell<wgpu_core::command::ComputePass>,
);
impl Resource for WebGpuComputePass {
  fn name(&self) -> Cow<str> {
    "webGPUComputePass".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassSetPipelineArgs {
  compute_pass_rid: ResourceId,
  pipeline: ResourceId,
}

#[op]
pub fn op_webgpu_compute_pass_set_pipeline(
  state: &mut OpState,
  args: ComputePassSetPipelineArgs,
) -> Result<WebGpuResult, AnyError> {
  let compute_pipeline_resource =
    state
      .resource_table
      .get::<super::pipeline::WebGpuComputePipeline>(args.pipeline)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_set_pipeline(
    &mut compute_pass_resource.0.borrow_mut(),
    compute_pipeline_resource.0,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassDispatchArgs {
  compute_pass_rid: ResourceId,
  x: u32,
  y: u32,
  z: u32,
}

#[op]
pub fn op_webgpu_compute_pass_dispatch(
  state: &mut OpState,
  args: ComputePassDispatchArgs,
) -> Result<WebGpuResult, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_dispatch(
    &mut compute_pass_resource.0.borrow_mut(),
    args.x,
    args.y,
    args.z,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassDispatchIndirectArgs {
  compute_pass_rid: ResourceId,
  indirect_buffer: ResourceId,
  indirect_offset: u64,
}

#[op]
pub fn op_webgpu_compute_pass_dispatch_indirect(
  state: &mut OpState,
  args: ComputePassDispatchIndirectArgs,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.indirect_buffer)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_dispatch_indirect(
    &mut compute_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    args.indirect_offset,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassBeginPipelineStatisticsQueryArgs {
  compute_pass_rid: ResourceId,
  query_set: ResourceId,
  query_index: u32,
}

#[op]
pub fn op_webgpu_compute_pass_begin_pipeline_statistics_query(
  state: &mut OpState,
  args: ComputePassBeginPipelineStatisticsQueryArgs,
) -> Result<WebGpuResult, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(args.query_set)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_begin_pipeline_statistics_query(
        &mut compute_pass_resource.0.borrow_mut(),
        query_set_resource.0,
        args.query_index,
    );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassEndPipelineStatisticsQueryArgs {
  compute_pass_rid: ResourceId,
}

#[op]
pub fn op_webgpu_compute_pass_end_pipeline_statistics_query(
  state: &mut OpState,
  args: ComputePassEndPipelineStatisticsQueryArgs,
) -> Result<WebGpuResult, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_end_pipeline_statistics_query(
        &mut compute_pass_resource.0.borrow_mut(),
    );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassWriteTimestampArgs {
  compute_pass_rid: ResourceId,
  query_set: ResourceId,
  query_index: u32,
}

#[op]
pub fn op_webgpu_compute_pass_write_timestamp(
  state: &mut OpState,
  args: ComputePassWriteTimestampArgs,
) -> Result<WebGpuResult, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(args.query_set)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_write_timestamp(
    &mut compute_pass_resource.0.borrow_mut(),
    query_set_resource.0,
    args.query_index,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassEndPassArgs {
  command_encoder_rid: ResourceId,
  compute_pass_rid: ResourceId,
}

#[op]
pub fn op_webgpu_compute_pass_end_pass(
  state: &mut OpState,
  args: ComputePassEndPassArgs,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .get::<super::command_encoder::WebGpuCommandEncoder>(
    args.command_encoder_rid,
  )?;
  let command_encoder = command_encoder_resource.0;
  let compute_pass_resource = state
    .resource_table
    .take::<WebGpuComputePass>(args.compute_pass_rid)?;
  let compute_pass = &compute_pass_resource.0.borrow();
  let instance = state.borrow::<super::Instance>();

  gfx_ok!(command_encoder => instance.command_encoder_run_compute_pass(
    command_encoder,
    compute_pass
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassSetBindGroupArgs {
  compute_pass_rid: ResourceId,
  index: u32,
  bind_group: ResourceId,
  dynamic_offsets_data: ZeroCopyBuf,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
}

#[op]
pub fn op_webgpu_compute_pass_set_bind_group(
  state: &mut OpState,
  args: ComputePassSetBindGroupArgs,
) -> Result<WebGpuResult, AnyError> {
  let bind_group_resource =
    state
      .resource_table
      .get::<super::binding::WebGpuBindGroup>(args.bind_group)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;

  // Align the data
  assert!(args.dynamic_offsets_data_start % std::mem::size_of::<u32>() == 0);
  let (prefix, dynamic_offsets_data, suffix) =
    // SAFETY: A u8 to u32 cast is safe because we asserted that the length is a
    // multiple of 4.
    unsafe { args.dynamic_offsets_data.align_to::<u32>() };
  assert!(prefix.is_empty());
  assert!(suffix.is_empty());

  let start = args.dynamic_offsets_data_start;
  let len = args.dynamic_offsets_data_length;

  // Assert that length and start are both in bounds
  assert!(start <= dynamic_offsets_data.len());
  assert!(len <= dynamic_offsets_data.len() - start);

  let dynamic_offsets_data: &[u32] = &dynamic_offsets_data[start..start + len];

  // SAFETY: the raw pointer and length are of the same slice, and that slice
  // lives longer than the below function invocation.
  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_set_bind_group(
      &mut compute_pass_resource.0.borrow_mut(),
      args.index,
      bind_group_resource.0,
      dynamic_offsets_data.as_ptr(),
      dynamic_offsets_data.len(),
    );
  }

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassPushDebugGroupArgs {
  compute_pass_rid: ResourceId,
  group_label: String,
}

#[op]
pub fn op_webgpu_compute_pass_push_debug_group(
  state: &mut OpState,
  args: ComputePassPushDebugGroupArgs,
) -> Result<WebGpuResult, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;

  let label = std::ffi::CString::new(args.group_label).unwrap();
  // SAFETY: the string the raw pointer points to lives longer than the below
  // function invocation.
  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_push_debug_group(
      &mut compute_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassPopDebugGroupArgs {
  compute_pass_rid: ResourceId,
}

#[op]
pub fn op_webgpu_compute_pass_pop_debug_group(
  state: &mut OpState,
  args: ComputePassPopDebugGroupArgs,
) -> Result<WebGpuResult, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_pop_debug_group(
    &mut compute_pass_resource.0.borrow_mut(),
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassInsertDebugMarkerArgs {
  compute_pass_rid: ResourceId,
  marker_label: String,
}

#[op]
pub fn op_webgpu_compute_pass_insert_debug_marker(
  state: &mut OpState,
  args: ComputePassInsertDebugMarkerArgs,
) -> Result<WebGpuResult, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)?;

  let label = std::ffi::CString::new(args.marker_label).unwrap();
  // SAFETY: the string the raw pointer points to lives longer than the below
  // function invocation.
  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_insert_debug_marker(
      &mut compute_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(WebGpuResult::empty())
}
