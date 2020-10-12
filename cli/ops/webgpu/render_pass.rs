// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_viewport(
    render_pass,
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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_scissor_rect(
    render_pass,
    args.x,
    args.y,
    args.width,
    args.height,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassSetBlendColorArgs {
  render_pass_rid: u32,
  color: (), // TODO: mixed types
}

pub fn op_webgpu_render_pass_set_blend_color(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassSetBlendColorArgs = serde_json::from_value(args)?;

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_blend_color(
    render_pass,
    &wgt::Color { // TODO
      r: 0.0,
      g: 0.0,
      b: 0.0,
      a: 0.0,
    }
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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_stencil_reference(
    render_pass,
    args.reference,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPassExecuteBundlesArgs {
  render_pass_rid: u32,
  bundles: [u32],
}

pub fn op_webgpu_render_pass_execute_bundles(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPassExecuteBundlesArgs = serde_json::from_value(args)?;

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgc::command::render_ffi::wgpu_render_pass_execute_bundles(
      render_pass,
      // TODO
    );
  }
  wgc::command::render_ffi::end_pass

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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgc::command::render_ffi::wgpu_render_pass_push_debug_group(
      render_pass,
      std::ffi::CString::new(args.group_label).unwrap().as_ptr(),
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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_pop_debug_group(render_pass);

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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgc::command::render_ffi::wgpu_render_pass_insert_debug_marker(
      render_pass,
      std::ffi::CString::new(args.marker_label).unwrap().as_ptr(),
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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_set_pipeline(
    render_pass,
    *state
      .resource_table
      .get_mut::<wgc::id::RenderPipelineId>(args.pipeline)
      .ok_or_else(bad_resource_id)?,
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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_draw(
    render_pass,
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

  let render_pass = state
    .resource_table
    .get_mut::<wgc::command::RenderPass>(args.render_pass_rid)
    .ok_or_else(bad_resource_id)?;

  wgc::command::render_ffi::wgpu_render_pass_draw_indexed(
    render_pass,
    args.index_count,
    args.instance_count,
    args.first_index,
    args.base_vertex,
    args.first_instance,
  );

  Ok(json!({}))
}
