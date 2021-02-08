// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{serde_json, RcRef, ZeroCopyBuf};
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;

pub(crate) struct WebGPURenderPass(
  pub(crate) RefCell<wgc::command::RenderPass>,
);
impl Resource for WebGPURenderPass {
  fn name(&self) -> Cow<str> {
    "webGPURenderPass".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetViewportArgs {
  render_pass_rid: u32,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
  min_depth: f32,
  max_depth: f32,
}

pub fn op_webgpu_render_pass_set_viewport(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetViewportArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_viewport(
    &mut render_pass_resource.0.borrow_mut(),
    args.x,
    args.y,
    args.width,
    args.height,
    args.min_depth,
    args.max_depth,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetScissorRectArgs {
  render_pass_rid: u32,
  x: u32,
  y: u32,
  width: u32,
  height: u32,
}

pub fn op_webgpu_render_pass_set_scissor_rect(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetScissorRectArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_scissor_rect(
    &mut render_pass_resource.0.borrow_mut(),
    args.x,
    args.y,
    args.width,
    args.height,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GPUColor {
  pub r: f64,
  pub g: f64,
  pub b: f64,
  pub a: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetBlendColorArgs {
  render_pass_rid: u32,
  color: GPUColor,
}

pub fn op_webgpu_render_pass_set_blend_color(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetBlendColorArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_blend_color(
    &mut render_pass_resource.0.borrow_mut(),
    &wgt::Color {
      r: args.color.r,
      g: args.color.g,
      b: args.color.b,
      a: args.color.a,
    },
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetStencilReferenceArgs {
  render_pass_rid: u32,
  reference: u32,
}

pub fn op_webgpu_render_pass_set_stencil_reference(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetStencilReferenceArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_stencil_reference(
    &mut render_pass_resource.0.borrow_mut(),
    args.reference,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassExecuteBundlesArgs {
  render_pass_rid: u32,
  bundles: Vec<u32>,
}

pub fn op_webgpu_render_pass_execute_bundles(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassExecuteBundlesArgs = serde_json::from_value(args)?;

  let mut render_bundle_ids = vec![];

  for rid in &args.bundles {
    let render_bundle_resource = state
      .resource_table
      .get::<super::bundle::WebGPURenderBundle>(*rid)
      .ok_or_else(bad_resource_id)?;
    render_bundle_ids.push(render_bundle_resource.0);
  }

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgc::command::render_ffi::wgpu_render_pass_execute_bundles(
      &mut render_pass_resource.0.borrow_mut(),
      render_bundle_ids.as_ptr(),
      args.bundles.len(),
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassEndPassArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  render_pass_rid: u32,
}

pub fn op_webgpu_render_pass_end_pass(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassEndPassArgs = serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<super::command_encoder::WebGPUCommandEncoder>(
      args.command_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();
  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;
  let render_pass = &render_pass_resource.0.borrow();
  wgc::gfx_select!(command_encoder => instance.command_encoder_run_render_pass(command_encoder, render_pass))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetBindGroupArgs {
  render_pass_rid: u32,
  index: u32,
  bind_group: u32,
  dynamic_offsets_data: Option<Vec<u32>>,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
}

pub fn op_webgpu_render_pass_set_bind_group(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetBindGroupArgs = serde_json::from_value(args)?;

  let bind_group_resource = state
    .resource_table
    .get::<super::binding::WebGPUBindGroup>(args.bind_group)
    .ok_or_else(bad_resource_id)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgc::command::render_ffi::wgpu_render_pass_set_bind_group(
      &mut render_pass_resource.0.borrow_mut(),
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
struct RenderPassPushDebugGroupArgs {
  render_pass_rid: u32,
  group_label: String,
}

pub fn op_webgpu_render_pass_push_debug_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassPushDebugGroupArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.group_label).unwrap();
    wgc::command::render_ffi::wgpu_render_pass_push_debug_group(
      &mut render_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassPopDebugGroupArgs {
  render_pass_rid: u32,
}

pub fn op_webgpu_render_pass_pop_debug_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassPopDebugGroupArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_pop_debug_group(
    &mut render_pass_resource.0.borrow_mut(),
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassInsertDebugMarkerArgs {
  render_pass_rid: u32,
  marker_label: String,
}

pub fn op_webgpu_render_pass_insert_debug_marker(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassInsertDebugMarkerArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.marker_label).unwrap();
    wgc::command::render_ffi::wgpu_render_pass_insert_debug_marker(
      &mut render_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetPipelineArgs {
  render_pass_rid: u32,
  pipeline: u32,
}

pub fn op_webgpu_render_pass_set_pipeline(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetPipelineArgs = serde_json::from_value(args)?;

  let render_pipeline_resource = state
    .resource_table
    .get::<super::pipeline::WebGPURenderPipeline>(args.pipeline)
    .ok_or_else(bad_resource_id)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_pipeline(
    &mut render_pass_resource.0.borrow_mut(),
    render_pipeline_resource.0,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetIndexBufferArgs {
  render_pass_rid: u32,
  buffer: u32,
  _index_format: String, // wgpu#978
  offset: u64,
  size: u64,
}

pub fn op_webgpu_render_pass_set_index_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetIndexBufferArgs = serde_json::from_value(args)?;

  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.buffer)
    .ok_or_else(bad_resource_id)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_index_buffer(
    &mut render_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    args.offset,
    std::num::NonZeroU64::new(args.size),
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetVertexBufferArgs {
  render_pass_rid: u32,
  slot: u32,
  buffer: u32,
  offset: u64,
  size: u64,
}

pub fn op_webgpu_render_pass_set_vertex_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetVertexBufferArgs = serde_json::from_value(args)?;

  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.buffer)
    .ok_or_else(bad_resource_id)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_vertex_buffer(
    &mut render_pass_resource.0.borrow_mut(),
    args.slot,
    buffer_resource.0,
    args.offset,
    std::num::NonZeroU64::new(args.size),
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassDrawArgs {
  render_pass_rid: u32,
  vertex_count: u32,
  instance_count: u32,
  first_vertex: u32,
  first_instance: u32,
}

pub fn op_webgpu_render_pass_draw(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassDrawArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_draw(
    &mut render_pass_resource.0.borrow_mut(),
    args.vertex_count,
    args.instance_count,
    args.first_vertex,
    args.first_instance,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassDrawIndexedArgs {
  render_pass_rid: u32,
  index_count: u32,
  instance_count: u32,
  first_index: u32,
  base_vertex: i32,
  first_instance: u32,
}

pub fn op_webgpu_render_pass_draw_indexed(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassDrawIndexedArgs = serde_json::from_value(args)?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_draw_indexed(
    &mut render_pass_resource.0.borrow_mut(),
    args.index_count,
    args.instance_count,
    args.first_index,
    args.base_vertex,
    args.first_instance,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassDrawIndirectArgs {
  render_pass_rid: u32,
  indirect_buffer: u32,
  indirect_offset: u64,
}

pub fn op_webgpu_render_pass_draw_indirect(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassDrawIndirectArgs = serde_json::from_value(args)?;

  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.indirect_buffer)
    .ok_or_else(bad_resource_id)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_draw_indirect(
    &mut render_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    args.indirect_offset,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassDrawIndexedIndirectArgs {
  render_pass_rid: u32,
  indirect_buffer: u32,
  indirect_offset: u64,
}

pub fn op_webgpu_render_pass_draw_indexed_indirect(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassDrawIndexedIndirectArgs = serde_json::from_value(args)?;

  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.indirect_buffer)
    .ok_or_else(bad_resource_id)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGPURenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_draw_indexed_indirect(
    &mut render_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    args.indirect_offset,
  );

  Ok(json!({}))
}
