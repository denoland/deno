// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
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

pub(crate) struct WebGpuRenderBundle(
  pub(crate) super::Instance,
  pub(crate) wgpu_core::id::RenderBundleId,
);
impl Resource for WebGpuRenderBundle {
  fn name(&self) -> Cow<str> {
    "webGPURenderBundle".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.render_bundle_drop(self.1));
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRenderBundleEncoderArgs {
  device_rid: ResourceId,
  label: String,
  color_formats: Vec<Option<wgpu_types::TextureFormat>>,
  depth_stencil_format: Option<wgpu_types::TextureFormat>,
  sample_count: u32,
  depth_read_only: bool,
  stencil_read_only: bool,
}

#[op2]
#[serde]
pub fn op_webgpu_create_render_bundle_encoder(
  state: &mut OpState,
  #[serde] args: CreateRenderBundleEncoderArgs,
) -> Result<WebGpuResult, AnyError> {
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.1;

  let depth_stencil = args.depth_stencil_format.map(|format| {
    wgpu_types::RenderBundleDepthStencil {
      format,
      depth_read_only: args.depth_read_only,
      stencil_read_only: args.stencil_read_only,
    }
  });

  let descriptor = wgpu_core::command::RenderBundleEncoderDescriptor {
    label: Some(Cow::Owned(args.label)),
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

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_finish(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
  #[string] label: Cow<str>,
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
      label: Some(label),
    },
    None
  ) => state, WebGpuRenderBundle)
}

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_set_bind_group(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
  index: u32,
  #[smi] bind_group: ResourceId,
  #[buffer] dynamic_offsets_data: &[u32],
  #[number] dynamic_offsets_data_start: usize,
  #[number] dynamic_offsets_data_length: usize,
) -> Result<WebGpuResult, AnyError> {
  let bind_group_resource =
    state
      .resource_table
      .get::<super::binding::WebGpuBindGroup>(bind_group)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;

  let start = dynamic_offsets_data_start;
  let len = dynamic_offsets_data_length;

  // Assert that length and start are both in bounds
  assert!(start <= dynamic_offsets_data.len());
  assert!(len <= dynamic_offsets_data.len() - start);

  let dynamic_offsets_data = &dynamic_offsets_data[start..start + len];

  // SAFETY: the raw pointer and length are of the same slice, and that slice
  // lives longer than the below function invocation.
  unsafe {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
      &mut render_bundle_encoder_resource.0.borrow_mut(),
      index,
      bind_group_resource.1,
      dynamic_offsets_data.as_ptr(),
      dynamic_offsets_data.len(),
    );
  }

  Ok(WebGpuResult::empty())
}

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_push_debug_group(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
  #[string] group_label: &str,
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

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_pop_debug_group(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
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

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_insert_debug_marker(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
  #[string] marker_label: &str,
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

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_set_pipeline(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
  #[smi] pipeline: ResourceId,
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
    render_pipeline_resource.1,
  );

  Ok(WebGpuResult::empty())
}

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_set_index_buffer(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
  #[smi] buffer: ResourceId,
  #[serde] index_format: wgpu_types::IndexFormat,
  #[number] offset: u64,
  #[number] size: u64,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;
  let size = Some(
    std::num::NonZeroU64::new(size)
      .ok_or_else(|| type_error("size must be larger than 0"))?,
  );

  render_bundle_encoder_resource
    .0
    .borrow_mut()
    .set_index_buffer(buffer_resource.1, index_format, offset, size);

  Ok(WebGpuResult::empty())
}

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_set_vertex_buffer(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
  slot: u32,
  #[smi] buffer: ResourceId,
  #[number] offset: u64,
  #[number] size: Option<u64>,
) -> Result<WebGpuResult, AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer)?;
  let render_bundle_encoder_resource =
    state
      .resource_table
      .get::<WebGpuRenderBundleEncoder>(render_bundle_encoder_rid)?;
  let size = if let Some(size) = size {
    Some(
      std::num::NonZeroU64::new(size)
        .ok_or_else(|| type_error("size must be larger than 0"))?,
    )
  } else {
    None
  };

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_vertex_buffer(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    slot,
    buffer_resource.1,
    offset,
    size,
  );

  Ok(WebGpuResult::empty())
}

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_draw(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
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

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_draw_indexed(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
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

#[op2]
#[serde]
pub fn op_webgpu_render_bundle_encoder_draw_indirect(
  state: &mut OpState,
  #[smi] render_bundle_encoder_rid: ResourceId,
  #[smi] indirect_buffer: ResourceId,
  #[number] indirect_offset: u64,
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
    buffer_resource.1,
    indirect_offset,
  );

  Ok(WebGpuResult::empty())
}
