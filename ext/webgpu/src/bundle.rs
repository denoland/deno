// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
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
  color_formats: Vec<Option<wgpu_types::TextureFormat>>,
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

  let depth_stencil = args.depth_stencil_format.map(|format| {
    wgpu_types::RenderBundleDepthStencil {
      format,
      depth_read_only: args.depth_read_only,
      stencil_read_only: args.stencil_read_only,
    }
  });

  let descriptor = wgpu_core::command::RenderBundleEncoderDescriptor {
    label: args.label.map(Cow::from),
    color_formats: Cow::from(args.color_formats),
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

#[op]
pub fn op_webgpu_render_bundle_encoder_finish(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  label: Option<String>,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .take::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;
  let render_bundle_encoder = Rc::try_unwrap(render_bundle_encoder_resource)
    .ok()
    .expect("unwrapping render_bundle_encoder_resource should succeed")
    .0
    .into_inner();
  let instance = state.borrow::<super::Instance>();

  gfx_put!(render_bundle_encoder.parent() => instance.render_bundle_encoder_finish(
    render_bundle_encoder,
    &wgpu_core::command::RenderBundleDescriptor {
      label: label.map(Cow::from),
    },
    std::marker::PhantomData
  ) => state, WebGpuRenderBundle)
}

#[op]
pub fn op_webgpu_render_bundle_encoder_set_bind_group(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  index: u32,
  bind_group: ResourceId,
  dynamic_offsets_data: ZeroCopyBuf,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
) -> Result<WebGpuResult, AnyError> {
  let bind_group_resource =
    state
      .resource_table
      .get::<super::binding::WebGpuBindGroup>(bind_group)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  // Align the data
  assert!(dynamic_offsets_data.len() % std::mem::size_of::<u32>() == 0);
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
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
      &mut render_bundle_encoder_resource.0.borrow_mut(),
      index,
      bind_group_resource.0,
      dynamic_offsets_data.as_ptr(),
      dynamic_offsets_data.len(),
    );
  }

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_bundle_encoder_push_debug_group(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  group_label: String,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  let label = std::ffi::CString::new(group_label).unwrap();
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

#[op]
pub fn op_webgpu_render_bundle_encoder_pop_debug_group(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_pop_debug_group(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_bundle_encoder_insert_debug_marker(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  marker_label: String,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  let label = std::ffi::CString::new(marker_label).unwrap();
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

#[op]
pub fn op_webgpu_render_bundle_encoder_set_pipeline(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  pipeline: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let render_pipeline_resource =
    state
      .resource_table
      .get::<super::pipeline::WebGpuRenderPipeline>(pipeline)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_pipeline(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    render_pipeline_resource.0,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_bundle_encoder_set_index_buffer(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  buffer: ResourceId,
  index_format: wgpu_types::IndexFormat,
  offset: u64,
  size: u64,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  render_bundle_encoder_resource
    .0
    .borrow_mut()
    .set_index_buffer(
      buffer_resource.0,
      index_format,
      offset,
      std::num::NonZeroU64::new(size),
    );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_bundle_encoder_set_vertex_buffer(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  slot: u32,
  buffer: ResourceId,
  offset: u64,
  size: u64,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_vertex_buffer(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    slot,
    buffer_resource.0,
    offset,
    std::num::NonZeroU64::new(size),
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_bundle_encoder_draw(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  vertex_count: u32,
  instance_count: u32,
  first_vertex: u32,
  first_instance: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    vertex_count,
    instance_count,
    first_vertex,
    first_instance,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_bundle_encoder_draw_indexed(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  index_count: u32,
  instance_count: u32,
  first_index: u32,
  base_vertex: i32,
  first_instance: u32,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indexed(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    index_count,
    instance_count,
    first_index,
    base_vertex,
    first_instance,
  );

  Ok(WebGpuResult::empty())
}

#[op]
pub fn op_webgpu_render_bundle_encoder_draw_indirect(
  state: &mut OpState,
  render_bundle_encoder_rid: ResourceId,
  indirect_buffer: ResourceId,
  indirect_offset: u64,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(indirect_buffer)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indirect(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    buffer_resource.0,
    indirect_offset,
  );

  Ok(WebGpuResult::empty())
}
