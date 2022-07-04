// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use super::error::WebGpuResult;

struct WebGpuRenderBundleEncoder(
  RefCell<wgpu_core::command::RenderBundleEncoder>,
);
impl Resource for WebGpuRenderBundleEncoder {
  fn name(&self) -> Cow<str> {
    "webGPURenderBundleEncoder".into()
  }
}

pub(crate) struct WebGpuRenderBundle(pub(crate) wgpu_core::id::RenderBundleId);
impl Resource for WebGpuRenderBundle {
  fn name(&self) -> Cow<str> {
    "webGPURenderBundle".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRenderBundleEncoderArgs {
  device_rid: ResourceId,
  label: Option<String>,
  color_formats: Vec<wgpu_types::TextureFormat>,
  depth_stencil_format: Option<wgpu_types::TextureFormat>,
  sample_count: u32,
  depth_read_only: bool,
  stencil_read_only: bool,
}

#[op]
pub fn op_webgpu_create_render_bundle_encoder(
  state: &mut OpState,
  args: CreateRenderBundleEncoderArgs,
) -> Result<WebGpuResult, AnyError> {
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let mut color_formats = vec![];

  for format in args.color_formats {
    color_formats.push(format);
  }

  let depth_stencil = if let Some(format) = args.depth_stencil_format {
    Some(wgpu_types::RenderBundleDepthStencil {
      format,
      depth_read_only: args.depth_read_only,
      stencil_read_only: args.stencil_read_only,
    })
  } else {
    None
  };

  let descriptor = wgpu_core::command::RenderBundleEncoderDescriptor {
    label: args.label.map(Cow::from),
    color_formats: Cow::from(color_formats),
    sample_count: args.sample_count,
    depth_stencil,
    multiview: None,
  };

  let res =
    wgpu_core::command::RenderBundleEncoder::new(&descriptor, device, None);
  let (render_bundle_encoder, maybe_err) = match res {
    Ok(encoder) => (encoder, None),
    Err(e) => (
      wgpu_core::command::RenderBundleEncoder::dummy(device),
      Some(e),
    ),
  };

  let rid = state
    .resource_table
    .add(WebGpuRenderBundleEncoder(RefCell::new(
      render_bundle_encoder,
    )));

  Ok(WebGpuResult::rid_err(rid, maybe_err))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderFinishArgs {
  render_bundle_encoder_rid: ResourceId,
  label: Option<String>,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_finish(
  state: &mut OpState,
  args: RenderBundleEncoderFinishArgs,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .take::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;
  let render_bundle_encoder = Rc::try_unwrap(render_bundle_encoder_resource)
    .ok()
    .expect("unwrapping render_bundle_encoder_resource should succeed")
    .0
    .into_inner();
  let instance = state.borrow::<super::Instance>();

  gfx_put!(render_bundle_encoder.parent() => instance.render_bundle_encoder_finish(
    render_bundle_encoder,
    &wgpu_core::command::RenderBundleDescriptor {
      label: args.label.map(Cow::from),
    },
    std::marker::PhantomData
  ) => state, WebGpuRenderBundle)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderSetBindGroupArgs {
  render_bundle_encoder_rid: ResourceId,
  index: u32,
  bind_group: ResourceId,
  dynamic_offsets_data: ZeroCopyBuf,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_set_bind_group(
  state: &mut OpState,
  args: RenderBundleEncoderSetBindGroupArgs,
) -> Result<WebGpuResult, AnyError> {
  let bind_group_resource =
    state
      .resource_table
      .get::<super::binding::WebGpuBindGroup>(args.bind_group)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  // Align the data
  assert!(args.dynamic_offsets_data.len() % std::mem::size_of::<u32>() == 0);
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
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
      &mut render_bundle_encoder_resource.0.borrow_mut(),
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
pub struct RenderBundleEncoderPushDebugGroupArgs {
  render_bundle_encoder_rid: ResourceId,
  group_label: String,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_push_debug_group(
  state: &mut OpState,
  args: RenderBundleEncoderPushDebugGroupArgs,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  let label = std::ffi::CString::new(args.group_label).unwrap();
  // SAFETY: the string the raw pointer points to lives longer than the below
  // function invocation.
  unsafe {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_push_debug_group(
      &mut render_bundle_encoder_resource.0.borrow_mut(),
      label.as_ptr(),
    );
  }

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderPopDebugGroupArgs {
  render_bundle_encoder_rid: ResourceId,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_pop_debug_group(
  state: &mut OpState,
  args: RenderBundleEncoderPopDebugGroupArgs,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_pop_debug_group(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderInsertDebugMarkerArgs {
  render_bundle_encoder_rid: ResourceId,
  marker_label: String,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_insert_debug_marker(
  state: &mut OpState,
  args: RenderBundleEncoderInsertDebugMarkerArgs,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  let label = std::ffi::CString::new(args.marker_label).unwrap();
  // SAFETY: the string the raw pointer points to lives longer than the below
  // function invocation.
  unsafe {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_insert_debug_marker(
      &mut render_bundle_encoder_resource.0.borrow_mut(),
      label.as_ptr(),
    );
  }

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderSetPipelineArgs {
  render_bundle_encoder_rid: ResourceId,
  pipeline: ResourceId,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_set_pipeline(
  state: &mut OpState,
  args: RenderBundleEncoderSetPipelineArgs,
) -> Result<WebGpuResult, AnyError> {
  let render_pipeline_resource =
    state
      .resource_table
      .get::<super::pipeline::WebGpuRenderPipeline>(args.pipeline)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_pipeline(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    render_pipeline_resource.0,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderSetIndexBufferArgs {
  render_bundle_encoder_rid: ResourceId,
  buffer: ResourceId,
  index_format: wgpu_types::IndexFormat,
  offset: u64,
  size: u64,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_set_index_buffer(
  state: &mut OpState,
  args: RenderBundleEncoderSetIndexBufferArgs,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.buffer)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  render_bundle_encoder_resource
    .0
    .borrow_mut()
    .set_index_buffer(
      buffer_resource.0,
      args.index_format,
      args.offset,
      std::num::NonZeroU64::new(args.size),
    );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderSetVertexBufferArgs {
  render_bundle_encoder_rid: ResourceId,
  slot: u32,
  buffer: ResourceId,
  offset: u64,
  size: u64,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_set_vertex_buffer(
  state: &mut OpState,
  args: RenderBundleEncoderSetVertexBufferArgs,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.buffer)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_vertex_buffer(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    args.slot,
    buffer_resource.0,
    args.offset,
    std::num::NonZeroU64::new(args.size),
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderDrawArgs {
  render_bundle_encoder_rid: ResourceId,
  vertex_count: u32,
  instance_count: u32,
  first_vertex: u32,
  first_instance: u32,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_draw(
  state: &mut OpState,
  args: RenderBundleEncoderDrawArgs,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    args.vertex_count,
    args.instance_count,
    args.first_vertex,
    args.first_instance,
  );

  Ok(WebGpuResult::empty())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderDrawIndexedArgs {
  render_bundle_encoder_rid: ResourceId,
  index_count: u32,
  instance_count: u32,
  first_index: u32,
  base_vertex: i32,
  first_instance: u32,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_draw_indexed(
  state: &mut OpState,
  args: RenderBundleEncoderDrawIndexedArgs,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indexed(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
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
pub struct RenderBundleEncoderDrawIndirectArgs {
  render_bundle_encoder_rid: ResourceId,
  indirect_buffer: ResourceId,
  indirect_offset: u64,
}

#[op]
pub fn op_webgpu_render_bundle_encoder_draw_indirect(
  state: &mut OpState,
  args: RenderBundleEncoderDrawIndirectArgs,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.indirect_buffer)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indirect(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    buffer_resource.0,
    args.indirect_offset,
  );

  Ok(WebGpuResult::empty())
}
