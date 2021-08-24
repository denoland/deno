// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::null_opbuf;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;

use super::error::WebGpuResult;

pub(crate) struct WebGpuRenderPass(
  pub(crate) RefCell<wgpu_core::command::RenderPass>,
);
impl Resource for WebGpuRenderPass {
  fn name(&self) -> Cow<str> {
    "webGPURenderPass".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassSetViewportArgs {
  render_pass_rid: ResourceId,
  x: f32,
  y: f32,
  width: f32,
  height: f32,
  min_depth: f32,
  max_depth: f32,
}

pub fn op_webgpu_render_pass_set_viewport(
  state: &mut OpState,
  args: RenderPassSetViewportArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_viewport(
    &mut render_pass_resource.0.borrow_mut(),
    args.x,
    args.y,
    args.width,
    args.height,
    args.min_depth,
    args.max_depth,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassSetScissorRectArgs {
  render_pass_rid: ResourceId,
  x: u32,
  y: u32,
  width: u32,
  height: u32,
}

pub fn op_webgpu_render_pass_set_scissor_rect(
  state: &mut OpState,
  args: RenderPassSetScissorRectArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_scissor_rect(
    &mut render_pass_resource.0.borrow_mut(),
    args.x,
    args.y,
    args.width,
    args.height,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuColor {
  pub r: f64,
  pub g: f64,
  pub b: f64,
  pub a: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassSetBlendConstantArgs {
  render_pass_rid: ResourceId,
  color: GpuColor,
}

pub fn op_webgpu_render_pass_set_blend_constant(
  state: &mut OpState,
  args: RenderPassSetBlendConstantArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_blend_constant(
    &mut render_pass_resource.0.borrow_mut(),
    &wgpu_types::Color {
      r: args.color.r,
      g: args.color.g,
      b: args.color.b,
      a: args.color.a,
    },
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassSetStencilReferenceArgs {
  render_pass_rid: ResourceId,
  reference: u32,
}

pub fn op_webgpu_render_pass_set_stencil_reference(
  state: &mut OpState,
  args: RenderPassSetStencilReferenceArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_stencil_reference(
    &mut render_pass_resource.0.borrow_mut(),
    args.reference,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassBeginPipelineStatisticsQueryArgs {
  render_pass_rid: ResourceId,
  query_set: u32,
  query_index: u32,
}

pub fn op_webgpu_render_pass_begin_pipeline_statistics_query(
  state: &mut OpState,
  args: RenderPassBeginPipelineStatisticsQueryArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(args.query_set)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_begin_pipeline_statistics_query(
    &mut render_pass_resource.0.borrow_mut(),
    query_set_resource.0,
    args.query_index,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassEndPipelineStatisticsQueryArgs {
  render_pass_rid: ResourceId,
}

pub fn op_webgpu_render_pass_end_pipeline_statistics_query(
  state: &mut OpState,
  args: RenderPassEndPipelineStatisticsQueryArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_end_pipeline_statistics_query(
    &mut render_pass_resource.0.borrow_mut(),
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassWriteTimestampArgs {
  render_pass_rid: ResourceId,
  query_set: u32,
  query_index: u32,
}

pub fn op_webgpu_render_pass_write_timestamp(
  state: &mut OpState,
  args: RenderPassWriteTimestampArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(args.query_set)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_write_timestamp(
    &mut render_pass_resource.0.borrow_mut(),
    query_set_resource.0,
    args.query_index,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassExecuteBundlesArgs {
  render_pass_rid: ResourceId,
  bundles: Vec<u32>,
}

pub fn op_webgpu_render_pass_execute_bundles(
  state: &mut OpState,
  args: RenderPassExecuteBundlesArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let mut render_bundle_ids = vec![];

  for rid in &args.bundles {
    let render_bundle_resource =
      state
        .resource_table
        .get::<super::bundle::WebGpuRenderBundle>(*rid)?;
    render_bundle_ids.push(render_bundle_resource.0);
  }

  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  unsafe {
    wgpu_core::command::render_ffi::wgpu_render_pass_execute_bundles(
      &mut render_pass_resource.0.borrow_mut(),
      render_bundle_ids.as_ptr(),
      args.bundles.len(),
    );
  }

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassEndPassArgs {
  command_encoder_rid: ResourceId,
  render_pass_rid: ResourceId,
}

pub fn op_webgpu_render_pass_end_pass(
  state: &mut OpState,
  args: RenderPassEndPassArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .get::<super::command_encoder::WebGpuCommandEncoder>(
    args.command_encoder_rid,
  )?;
  let command_encoder = command_encoder_resource.0;
  let render_pass_resource = state
    .resource_table
    .take::<WebGpuRenderPass>(args.render_pass_rid)?;
  let render_pass = &render_pass_resource.0.borrow();
  let instance = state.borrow::<super::Instance>();

  gfx_ok!(command_encoder => instance.command_encoder_run_render_pass(command_encoder, render_pass))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassSetBindGroupArgs {
  render_pass_rid: ResourceId,
  index: u32,
  bind_group: u32,
  dynamic_offsets_data: Option<Vec<u32>>,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
}

pub fn op_webgpu_render_pass_set_bind_group(
  state: &mut OpState,
  args: RenderPassSetBindGroupArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<WebGpuResult, AnyError> {
  let bind_group_resource =
    state
      .resource_table
      .get::<super::binding::WebGpuBindGroup>(args.bind_group)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  // I know this might look like it can be easily deduplicated, but it can not
  // be due to the lifetime of the args.dynamic_offsets_data slice. Because we
  // need to use a raw pointer here the slice can be freed before the pointer
  // is used in wgpu_render_pass_set_bind_group. See
  // https://matrix.to/#/!XFRnMvAfptAHthwBCx:matrix.org/$HgrlhD-Me1DwsGb8UdMu2Hqubgks8s7ILwWRwigOUAg
  match args.dynamic_offsets_data {
    Some(data) => unsafe {
      wgpu_core::command::render_ffi::wgpu_render_pass_set_bind_group(
        &mut render_pass_resource.0.borrow_mut(),
        args.index,
        bind_group_resource.0,
        data.as_slice().as_ptr(),
        args.dynamic_offsets_data_length,
      );
    },
    None => {
      let zero_copy = zero_copy.ok_or_else(null_opbuf)?;
      let (prefix, data, suffix) = unsafe { zero_copy.align_to::<u32>() };
      assert!(prefix.is_empty());
      assert!(suffix.is_empty());
      unsafe {
        wgpu_core::command::render_ffi::wgpu_render_pass_set_bind_group(
          &mut render_pass_resource.0.borrow_mut(),
          args.index,
          bind_group_resource.0,
          data[args.dynamic_offsets_data_start..].as_ptr(),
          args.dynamic_offsets_data_length,
        );
      }
    }
  };

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassPushDebugGroupArgs {
  render_pass_rid: ResourceId,
  group_label: String,
}

pub fn op_webgpu_render_pass_push_debug_group(
  state: &mut OpState,
  args: RenderPassPushDebugGroupArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  unsafe {
    let label = std::ffi::CString::new(args.group_label).unwrap();
    wgpu_core::command::render_ffi::wgpu_render_pass_push_debug_group(
      &mut render_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassPopDebugGroupArgs {
  render_pass_rid: ResourceId,
}

pub fn op_webgpu_render_pass_pop_debug_group(
  state: &mut OpState,
  args: RenderPassPopDebugGroupArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_pop_debug_group(
    &mut render_pass_resource.0.borrow_mut(),
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassInsertDebugMarkerArgs {
  render_pass_rid: ResourceId,
  marker_label: String,
}

pub fn op_webgpu_render_pass_insert_debug_marker(
  state: &mut OpState,
  args: RenderPassInsertDebugMarkerArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  unsafe {
    let label = std::ffi::CString::new(args.marker_label).unwrap();
    wgpu_core::command::render_ffi::wgpu_render_pass_insert_debug_marker(
      &mut render_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassSetPipelineArgs {
  render_pass_rid: ResourceId,
  pipeline: u32,
}

pub fn op_webgpu_render_pass_set_pipeline(
  state: &mut OpState,
  args: RenderPassSetPipelineArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pipeline_resource =
    state
      .resource_table
      .get::<super::pipeline::WebGpuRenderPipeline>(args.pipeline)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_pipeline(
    &mut render_pass_resource.0.borrow_mut(),
    render_pipeline_resource.0,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassSetIndexBufferArgs {
  render_pass_rid: ResourceId,
  buffer: u32,
  index_format: String,
  offset: u64,
  size: Option<u64>,
}

pub fn op_webgpu_render_pass_set_index_buffer(
  state: &mut OpState,
  args: RenderPassSetIndexBufferArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.buffer)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  let size = if let Some(size) = args.size {
    Some(
      std::num::NonZeroU64::new(size)
        .ok_or_else(|| type_error("size must be larger than 0"))?,
    )
  } else {
    None
  };

  render_pass_resource.0.borrow_mut().set_index_buffer(
    buffer_resource.0,
    super::pipeline::serialize_index_format(args.index_format),
    args.offset,
    size,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassSetVertexBufferArgs {
  render_pass_rid: ResourceId,
  slot: u32,
  buffer: u32,
  offset: u64,
  size: Option<u64>,
}

pub fn op_webgpu_render_pass_set_vertex_buffer(
  state: &mut OpState,
  args: RenderPassSetVertexBufferArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.buffer)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  let size = if let Some(size) = args.size {
    Some(
      std::num::NonZeroU64::new(size)
        .ok_or_else(|| type_error("size must be larger than 0"))?,
    )
  } else {
    None
  };

  wgpu_core::command::render_ffi::wgpu_render_pass_set_vertex_buffer(
    &mut render_pass_resource.0.borrow_mut(),
    args.slot,
    buffer_resource.0,
    args.offset,
    size,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassDrawArgs {
  render_pass_rid: ResourceId,
  vertex_count: u32,
  instance_count: u32,
  first_vertex: u32,
  first_instance: u32,
}

pub fn op_webgpu_render_pass_draw(
  state: &mut OpState,
  args: RenderPassDrawArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_draw(
    &mut render_pass_resource.0.borrow_mut(),
    args.vertex_count,
    args.instance_count,
    args.first_vertex,
    args.first_instance,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassDrawIndexedArgs {
  render_pass_rid: ResourceId,
  index_count: u32,
  instance_count: u32,
  first_index: u32,
  base_vertex: i32,
  first_instance: u32,
}

pub fn op_webgpu_render_pass_draw_indexed(
  state: &mut OpState,
  args: RenderPassDrawIndexedArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_draw_indexed(
    &mut render_pass_resource.0.borrow_mut(),
    args.index_count,
    args.instance_count,
    args.first_index,
    args.base_vertex,
    args.first_instance,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassDrawIndirectArgs {
  render_pass_rid: ResourceId,
  indirect_buffer: u32,
  indirect_offset: u64,
}

pub fn op_webgpu_render_pass_draw_indirect(
  state: &mut OpState,
  args: RenderPassDrawIndirectArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.indirect_buffer)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_draw_indirect(
    &mut render_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    args.indirect_offset,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPassDrawIndexedIndirectArgs {
  render_pass_rid: ResourceId,
  indirect_buffer: u32,
  indirect_offset: u64,
}

pub fn op_webgpu_render_pass_draw_indexed_indirect(
  state: &mut OpState,
  args: RenderPassDrawIndexedIndirectArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.indirect_buffer)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(args.render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_draw_indexed_indirect(
    &mut render_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    args.indirect_offset,
  );

  Ok(WebGpuResult::empty())
}
