// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use crate::Instance;

use super::error::WebGpuError;

pub(crate) struct WebGpuComputePass {
  pub instance: Rc<Instance>,
  pub command_encoder: Rc<wgpu_core::id::CommandEncoderId>,
  pub compute_pass: RefCell<wgpu_core::command::ComputePass>,
}
impl Resource for WebGpuComputePass {
  fn name(&self) -> Cow<str> {
    "webGPUComputePass".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassSetPipelineArgs {
  compute_pass_rid: ResourceId,
  pipeline: u32,
}

pub fn op_webgpu_compute_pass_set_pipeline(
  state: &mut OpState,
  args: ComputePassSetPipelineArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pipeline_resource = state
    .resource_table
    .get::<super::pipeline::WebGpuComputePipeline>(args.pipeline)
    .ok_or_else(bad_resource_id)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_set_pipeline(
    &mut compute_pass_resource.compute_pass.borrow_mut(),
    *compute_pipeline_resource.compute_pipeline,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassDispatchArgs {
  compute_pass_rid: ResourceId,
  x: u32,
  y: u32,
  z: u32,
}

pub fn op_webgpu_compute_pass_dispatch(
  state: &mut OpState,
  args: ComputePassDispatchArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_dispatch(
    &mut compute_pass_resource.compute_pass.borrow_mut(),
    args.x,
    args.y,
    args.z,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassDispatchIndirectArgs {
  compute_pass_rid: ResourceId,
  indirect_buffer: u32,
  indirect_offset: u64,
}

pub fn op_webgpu_compute_pass_dispatch_indirect(
  state: &mut OpState,
  args: ComputePassDispatchIndirectArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.indirect_buffer)
    .ok_or_else(bad_resource_id)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_dispatch_indirect(
    &mut compute_pass_resource.compute_pass.borrow_mut(),
    *buffer_resource.buffer,
    args.indirect_offset,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassBeginPipelineStatisticsQueryArgs {
  compute_pass_rid: ResourceId,
  query_set: u32,
  query_index: u32,
}

pub fn op_webgpu_compute_pass_begin_pipeline_statistics_query(
  state: &mut OpState,
  args: ComputePassBeginPipelineStatisticsQueryArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(args.query_set)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_begin_pipeline_statistics_query(
      &mut compute_pass_resource.compute_pass.borrow_mut(),
      *query_set_resource.query_set,
      args.query_index,
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassEndPipelineStatisticsQueryArgs {
  compute_pass_rid: ResourceId,
}

pub fn op_webgpu_compute_pass_end_pipeline_statistics_query(
  state: &mut OpState,
  args: ComputePassEndPipelineStatisticsQueryArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_end_pipeline_statistics_query(
      &mut compute_pass_resource.compute_pass.borrow_mut(),
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassWriteTimestampArgs {
  compute_pass_rid: ResourceId,
  query_set: u32,
  query_index: u32,
}

pub fn op_webgpu_compute_pass_write_timestamp(
  state: &mut OpState,
  args: ComputePassWriteTimestampArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(args.query_set)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgpu_core::command::compute_ffi::wgpu_compute_pass_write_timestamp(
      &mut compute_pass_resource.compute_pass.borrow_mut(),
      *query_set_resource.query_set,
      args.query_index,
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassEndPassArgs {
  compute_pass_rid: ResourceId,
}

pub fn op_webgpu_compute_pass_end_pass(
  state: &mut OpState,
  args: ComputePassEndPassArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .take::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = compute_pass_resource.instance.clone();
  let command_encoder = compute_pass_resource.command_encoder.clone();

  let maybe_err =
    gfx_select!(command_encoder => instance.command_encoder_run_compute_pass(
      *command_encoder,
      &compute_pass_resource.compute_pass.borrow()
    ))
    .err();

  Ok(json!({ "err": maybe_err.map(WebGpuError::from) }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassSetBindGroupArgs {
  compute_pass_rid: ResourceId,
  index: u32,
  bind_group: u32,
  dynamic_offsets_data: Option<Vec<u32>>,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
}

pub fn op_webgpu_compute_pass_set_bind_group(
  state: &mut OpState,
  args: ComputePassSetBindGroupArgs,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let bind_group_resource = state
    .resource_table
    .get::<super::binding::WebGpuBindGroup>(args.bind_group)
    .ok_or_else(bad_resource_id)?;
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  // I know this might look like it can be easily deduplicated, but it can not
  // be due to the lifetime of the args.dynamic_offsets_data slice. Because we
  // need to use a raw pointer here the slice can be freed before the pointer
  // is used in wgpu_render_pass_set_bind_group. See
  // https://matrix.to/#/!XFRnMvAfptAHthwBCx:matrix.org/$HgrlhD-Me1DwsGb8UdMu2Hqubgks8s7ILwWRwigOUAg
  match args.dynamic_offsets_data {
    Some(data) => unsafe {
      wgpu_core::command::compute_ffi::wgpu_compute_pass_set_bind_group(
        &mut compute_pass_resource.compute_pass.borrow_mut(),
        args.index,
        *bind_group_resource.bind_group,
        data.as_ptr(),
        args.dynamic_offsets_data_length,
      )
    },
    None => {
      let (prefix, data, suffix) = unsafe { zero_copy[0].align_to::<u32>() };
      assert!(prefix.is_empty());
      assert!(suffix.is_empty());
      unsafe {
        wgpu_core::command::compute_ffi::wgpu_compute_pass_set_bind_group(
          &mut compute_pass_resource.compute_pass.borrow_mut(),
          args.index,
          *bind_group_resource.bind_group,
          data[args.dynamic_offsets_data_start..].as_ptr(),
          args.dynamic_offsets_data_length,
        )
      }
    }
  };

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassPushDebugGroupArgs {
  compute_pass_rid: ResourceId,
  group_label: String,
}

pub fn op_webgpu_compute_pass_push_debug_group(
  state: &mut OpState,
  args: ComputePassPushDebugGroupArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.group_label).unwrap();
    wgpu_core::command::compute_ffi::wgpu_compute_pass_push_debug_group(
      &mut compute_pass_resource.compute_pass.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassPopDebugGroupArgs {
  compute_pass_rid: ResourceId,
}

pub fn op_webgpu_compute_pass_pop_debug_group(
  state: &mut OpState,
  args: ComputePassPopDebugGroupArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::compute_ffi::wgpu_compute_pass_pop_debug_group(
    &mut compute_pass_resource.compute_pass.borrow_mut(),
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePassInsertDebugMarkerArgs {
  compute_pass_rid: ResourceId,
  marker_label: String,
}

pub fn op_webgpu_compute_pass_insert_debug_marker(
  state: &mut OpState,
  args: ComputePassInsertDebugMarkerArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let compute_pass_resource = state
    .resource_table
    .get::<WebGpuComputePass>(args.compute_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.marker_label).unwrap();
    wgpu_core::command::compute_ffi::wgpu_compute_pass_insert_debug_marker(
      &mut compute_pass_resource.compute_pass.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(json!({}))
}
