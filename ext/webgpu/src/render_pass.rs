// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
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

#[op]
pub fn op_webgpu_render_pass_set_viewport(
  state: &mut OpState,
  args: RenderPassSetViewportArgs,
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

#[op]
pub fn op_webgpu_render_pass_set_scissor_rect(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  x: u32,
  y: u32,
  width: u32,
  height: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_scissor_rect(
    &mut render_pass_resource.0.borrow_mut(),
    x,
    y,
    width,
    height,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_set_blend_constant(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  color: wgpu_types::Color,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_blend_constant(
    &mut render_pass_resource.0.borrow_mut(),
    &color,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_set_stencil_reference(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  reference: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_stencil_reference(
    &mut render_pass_resource.0.borrow_mut(),
    reference,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_begin_pipeline_statistics_query(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  query_set: u32,
  query_index: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(query_set)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_begin_pipeline_statistics_query(
        &mut render_pass_resource.0.borrow_mut(),
        query_set_resource.0,
        query_index,
    );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_end_pipeline_statistics_query(
  state: &mut OpState,
  render_pass_rid: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_end_pipeline_statistics_query(
        &mut render_pass_resource.0.borrow_mut(),
    );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_write_timestamp(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  query_set: u32,
  query_index: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(query_set)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_write_timestamp(
    &mut render_pass_resource.0.borrow_mut(),
    query_set_resource.0,
    query_index,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_execute_bundles(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  bundles: Vec<u32>,
) -> Result<WebGpuResult, AnyError> {
  let bundles = bundles
    .iter()
    .map(|rid| {
      let render_bundle_resource =
        state
          .resource_table
          .get::<super::bundle::WebGpuRenderBundle>(*rid)?;
      Ok(render_bundle_resource.0)
    })
    .collect::<Result<Vec<_>, AnyError>>()?;

  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  // SAFETY: the raw pointer and length are of the same slice, and that slice
  // lives longer than the below function invocation.
  unsafe {
    wgpu_core::command::render_ffi::wgpu_render_pass_execute_bundles(
      &mut render_pass_resource.0.borrow_mut(),
      bundles.as_ptr(),
      bundles.len(),
    );
  }

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_end(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  render_pass_rid: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .get::<super::command_encoder::WebGpuCommandEncoder>(
    command_encoder_rid,
  )?;
  let command_encoder = command_encoder_resource.0;
  let render_pass_resource = state
    .resource_table
    .take::<WebGpuRenderPass>(render_pass_rid)?;
  let render_pass = &render_pass_resource.0.borrow();
  let instance = state.borrow::<super::Instance>();

  gfx_ok!(command_encoder => instance.command_encoder_run_render_pass(command_encoder, render_pass))
}

#[op]
pub fn op_webgpu_render_pass_set_bind_group(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  index: u32,
  bind_group: u32,
  dynamic_offsets_data: ZeroCopyBuf,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
) -> Result<WebGpuResult, AnyError> {
  let bind_group_resource =
    state
      .resource_table
      .get::<super::binding::WebGpuBindGroup>(bind_group)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  // Align the data
  assert_eq!(dynamic_offsets_data_start % std::mem::size_of::<u32>(), 0);
  let (prefix, dynamic_offsets_data, suffix) =
  // SAFETY: A u8 to u32 cast is safe because we asserted that the length is a
  // multiple of 4.
    unsafe { dynamic_offsets_data.align_to::<u32>() };
  assert!(prefix.is_empty());
  assert!(suffix.is_empty());

  let start = dynamic_offsets_data_start;
  let len = dynamic_offsets_data_length;

  // Assert that length and start are both in bounds
  assert!(start <= dynamic_offsets_data.len());
  assert!(len <= dynamic_offsets_data.len() - start);

  let dynamic_offsets_data: &[u32] = &dynamic_offsets_data[start..start + len];

  // SAFETY: the raw pointer and length are of the same slice, and that slice
  // lives longer than the below function invocation.
  unsafe {
    wgpu_core::command::render_ffi::wgpu_render_pass_set_bind_group(
      &mut render_pass_resource.0.borrow_mut(),
      index,
      bind_group_resource.0,
      dynamic_offsets_data.as_ptr(),
      dynamic_offsets_data.len(),
    );
  }

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_push_debug_group(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  group_label: String,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  let label = std::ffi::CString::new(group_label).unwrap();
  // SAFETY: the string the raw pointer points to lives longer than the below
  // function invocation.
  unsafe {
    wgpu_core::command::render_ffi::wgpu_render_pass_push_debug_group(
      &mut render_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_pop_debug_group(
  state: &mut OpState,
  render_pass_rid: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_pop_debug_group(
    &mut render_pass_resource.0.borrow_mut(),
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_insert_debug_marker(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  marker_label: String,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  let label = std::ffi::CString::new(marker_label).unwrap();
  // SAFETY: the string the raw pointer points to lives longer than the below
  // function invocation.
  unsafe {
    wgpu_core::command::render_ffi::wgpu_render_pass_insert_debug_marker(
      &mut render_pass_resource.0.borrow_mut(),
      label.as_ptr(),
      0, // wgpu#975
    );
  }

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_set_pipeline(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  pipeline: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_pipeline_resource =
    state
      .resource_table
      .get::<super::pipeline::WebGpuRenderPipeline>(pipeline)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_set_pipeline(
    &mut render_pass_resource.0.borrow_mut(),
    render_pipeline_resource.0,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_set_index_buffer(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  buffer: u32,
  index_format: wgpu_types::IndexFormat,
  offset: u64,
  size: Option<u64>,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  let size = if let Some(size) = size {
    Some(
      std::num::NonZeroU64::new(size)
        .ok_or_else(|| type_error("size must be larger than 0"))?,
    )
  } else {
    None
  };

  render_pass_resource.0.borrow_mut().set_index_buffer(
    buffer_resource.0,
    index_format,
    offset,
    size,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_set_vertex_buffer(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  slot: u32,
  buffer: u32,
  offset: u64,
  size: Option<u64>,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  let size = if let Some(size) = size {
    Some(
      std::num::NonZeroU64::new(size)
        .ok_or_else(|| type_error("size must be larger than 0"))?,
    )
  } else {
    None
  };

  wgpu_core::command::render_ffi::wgpu_render_pass_set_vertex_buffer(
    &mut render_pass_resource.0.borrow_mut(),
    slot,
    buffer_resource.0,
    offset,
    size,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_draw(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  vertex_count: u32,
  instance_count: u32,
  first_vertex: u32,
  first_instance: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_draw(
    &mut render_pass_resource.0.borrow_mut(),
    vertex_count,
    instance_count,
    first_vertex,
    first_instance,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_draw_indexed(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  index_count: u32,
  instance_count: u32,
  first_index: u32,
  base_vertex: i32,
  first_instance: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_draw_indexed(
    &mut render_pass_resource.0.borrow_mut(),
    index_count,
    instance_count,
    first_index,
    base_vertex,
    first_instance,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_draw_indirect(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  indirect_buffer: u32,
  indirect_offset: u64,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(indirect_buffer)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_draw_indirect(
    &mut render_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    indirect_offset,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_pass_draw_indexed_indirect(
  state: &mut OpState,
  render_pass_rid: ResourceId,
  indirect_buffer: u32,
  indirect_offset: u64,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(indirect_buffer)?;
  let render_pass_resource = state
    .resource_table
    .get::<WebGpuRenderPass>(render_pass_rid)?;

  wgpu_core::command::render_ffi::wgpu_render_pass_draw_indexed_indirect(
    &mut render_pass_resource.0.borrow_mut(),
    buffer_resource.0,
    indirect_offset,
  );

  Ok(WebGpuResult::empty())
}
