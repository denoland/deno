// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use super::error::WebGpuResult;
use super::texture::serialize_texture_format;

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
  color_formats: Vec<String>,
  depth_stencil_format: Option<String>,
  sample_count: Option<u32>,
}

pub fn op_webgpu_create_render_bundle_encoder(
  state: &mut OpState,
  args: CreateRenderBundleEncoderArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<WebGpuResult, AnyError> {
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let mut color_formats = vec![];

  for format in &args.color_formats {
    color_formats.push(serialize_texture_format(format)?);
  }

  let descriptor = wgpu_core::command::RenderBundleEncoderDescriptor {
    label: args.label.map(Cow::from),
    color_formats: Cow::from(color_formats),
    depth_stencil_format: args
      .depth_stencil_format
      .map(|s| serialize_texture_format(&s))
      .transpose()?,
    sample_count: args.sample_count.unwrap_or(1),
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

pub fn op_webgpu_render_bundle_encoder_finish(
  state: &mut OpState,
  args: RenderBundleEncoderFinishArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<WebGpuResult, AnyError> {
  let render_bundle_encoder_resource = state
    .resource_table
    .take::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder = Rc::try_unwrap(render_bundle_encoder_resource)
    .ok()
    .expect("unwrapping render_bundle_encoder_resource should succeed")
    .0
    .into_inner();
  let instance = state.borrow::<super::Instance>();

  let (render_bundle, maybe_err) = gfx_select!(render_bundle_encoder.parent() => instance.render_bundle_encoder_finish(
    render_bundle_encoder,
    &wgpu_core::command::RenderBundleDescriptor {
      label: args.label.map(Cow::from),
    },
    std::marker::PhantomData
  ));

  let rid = state.resource_table.add(WebGpuRenderBundle(render_bundle));

  Ok(WebGpuResult::rid_err(rid, maybe_err))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderSetBindGroupArgs {
  render_bundle_encoder_rid: ResourceId,
  index: u32,
  bind_group: u32,
  dynamic_offsets_data: Option<Vec<u32>>,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
}

pub fn op_webgpu_render_bundle_encoder_set_bind_group(
  state: &mut OpState,
  args: RenderBundleEncoderSetBindGroupArgs,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let zero_copy = zero_copy.ok_or_else(null_opbuf)?;

  let bind_group_resource = state
    .resource_table
    .get::<super::binding::WebGpuBindGroup>(args.bind_group)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  // I know this might look like it can be easily deduplicated, but it can not
  // be due to the lifetime of the args.dynamic_offsets_data slice. Because we
  // need to use a raw pointer here the slice can be freed before the pointer
  // is used in wgpu_render_pass_set_bind_group. See
  // https://matrix.to/#/!XFRnMvAfptAHthwBCx:matrix.org/$HgrlhD-Me1DwsGb8UdMu2Hqubgks8s7ILwWRwigOUAg
  match args.dynamic_offsets_data {
    Some(data) => unsafe {
      wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
        &mut render_bundle_encoder_resource.0.borrow_mut(),
        args.index,
        bind_group_resource.0,
        data.as_slice().as_ptr(),
        args.dynamic_offsets_data_length,
      );
    },
    None => {
      let (prefix, data, suffix) = unsafe { zero_copy.align_to::<u32>() };
      assert!(prefix.is_empty());
      assert!(suffix.is_empty());
      unsafe {
        wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
          &mut render_bundle_encoder_resource.0.borrow_mut(),
          args.index,
          bind_group_resource.0,
          data[args.dynamic_offsets_data_start..].as_ptr(),
          args.dynamic_offsets_data_length,
        );
      }
    }
  };

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderPushDebugGroupArgs {
  render_bundle_encoder_rid: ResourceId,
  group_label: String,
}

pub fn op_webgpu_render_bundle_encoder_push_debug_group(
  state: &mut OpState,
  args: RenderBundleEncoderPushDebugGroupArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.group_label).unwrap();
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_push_debug_group(
      &mut render_bundle_encoder_resource.0.borrow_mut(),
      label.as_ptr(),
    );
  }

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderPopDebugGroupArgs {
  render_bundle_encoder_rid: ResourceId,
}

pub fn op_webgpu_render_bundle_encoder_pop_debug_group(
  state: &mut OpState,
  args: RenderBundleEncoderPopDebugGroupArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_pop_debug_group(
      &mut render_bundle_encoder_resource.0.borrow_mut(),
    );
  }

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderInsertDebugMarkerArgs {
  render_bundle_encoder_rid: ResourceId,
  marker_label: String,
}

pub fn op_webgpu_render_bundle_encoder_insert_debug_marker(
  state: &mut OpState,
  args: RenderBundleEncoderInsertDebugMarkerArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.marker_label).unwrap();
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_insert_debug_marker(
      &mut render_bundle_encoder_resource.0.borrow_mut(),
      label.as_ptr(),
    );
  }

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderSetPipelineArgs {
  render_bundle_encoder_rid: ResourceId,
  pipeline: u32,
}

pub fn op_webgpu_render_bundle_encoder_set_pipeline(
  state: &mut OpState,
  args: RenderBundleEncoderSetPipelineArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let render_pipeline_resource = state
    .resource_table
    .get::<super::pipeline::WebGpuRenderPipeline>(args.pipeline)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_pipeline(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    render_pipeline_resource.0,
  );

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderSetIndexBufferArgs {
  render_bundle_encoder_rid: ResourceId,
  buffer: u32,
  index_format: String,
  offset: u64,
  size: u64,
}

pub fn op_webgpu_render_bundle_encoder_set_index_buffer(
  state: &mut OpState,
  args: RenderBundleEncoderSetIndexBufferArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.buffer)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  render_bundle_encoder_resource
    .0
    .borrow_mut()
    .set_index_buffer(
      buffer_resource.0,
      super::pipeline::serialize_index_format(args.index_format),
      args.offset,
      std::num::NonZeroU64::new(args.size),
    );

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderSetVertexBufferArgs {
  render_bundle_encoder_rid: ResourceId,
  slot: u32,
  buffer: u32,
  offset: u64,
  size: u64,
}

pub fn op_webgpu_render_bundle_encoder_set_vertex_buffer(
  state: &mut OpState,
  args: RenderBundleEncoderSetVertexBufferArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.buffer)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_vertex_buffer(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    args.slot,
    buffer_resource.0,
    args.offset,
    std::num::NonZeroU64::new(args.size),
  );

  Ok(())
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

pub fn op_webgpu_render_bundle_encoder_draw(
  state: &mut OpState,
  args: RenderBundleEncoderDrawArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    args.vertex_count,
    args.instance_count,
    args.first_vertex,
    args.first_instance,
  );

  Ok(())
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

pub fn op_webgpu_render_bundle_encoder_draw_indexed(
  state: &mut OpState,
  args: RenderBundleEncoderDrawIndexedArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indexed(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    args.index_count,
    args.instance_count,
    args.first_index,
    args.base_vertex,
    args.first_instance,
  );

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderBundleEncoderDrawIndirectArgs {
  render_bundle_encoder_rid: ResourceId,
  indirect_buffer: u32,
  indirect_offset: u64,
}

pub fn op_webgpu_render_bundle_encoder_draw_indirect(
  state: &mut OpState,
  args: RenderBundleEncoderDrawIndirectArgs,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.indirect_buffer)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder_resource = state
    .resource_table
    .get::<WebGpuRenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indirect(
    &mut render_bundle_encoder_resource.0.borrow_mut(),
    buffer_resource.0,
    args.indirect_offset,
  );

  Ok(())
}
