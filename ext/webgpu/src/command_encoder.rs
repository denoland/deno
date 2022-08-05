// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::num::NonZeroU32;

use super::error::WebGpuResult;

pub(crate) struct WebGpuCommandEncoder(
  pub(crate) wgpu_core::id::CommandEncoderId,
);
impl Resource for WebGpuCommandEncoder {
  fn name(&self) -> Cow<str> {
    "webGPUCommandEncoder".into()
  }
}

pub(crate) struct WebGpuCommandBuffer(
  pub(crate) wgpu_core::id::CommandBufferId,
);
impl Resource for WebGpuCommandBuffer {
  fn name(&self) -> Cow<str> {
    "webGPUCommandBuffer".into()
  }
}

#[op]
pub fn op_webgpu_create_command_encoder(
  state: &mut OpState,
  device_rid: ResourceId,
  label: Option<String>,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.0;

  let descriptor = wgpu_types::CommandEncoderDescriptor {
    label: label.map(Cow::from),
  };

  gfx_put!(device => instance.device_create_command_encoder(
    device,
    &descriptor,
    std::marker::PhantomData
  ) => state, WebGpuCommandEncoder)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuRenderPassColorAttachment {
  view: ResourceId,
  resolve_target: Option<ResourceId>,
  clear_value: Option<wgpu_types::Color>,
  load_op: wgpu_core::command::LoadOp,
  store_op: wgpu_core::command::StoreOp,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuRenderPassDepthStencilAttachment {
  view: ResourceId,
  depth_clear_value: f32,
  depth_load_op: Option<wgpu_core::command::LoadOp>,
  depth_store_op: Option<wgpu_core::command::StoreOp>,
  depth_read_only: bool,
  stencil_clear_value: u32,
  stencil_load_op: Option<wgpu_core::command::LoadOp>,
  stencil_store_op: Option<wgpu_core::command::StoreOp>,
  stencil_read_only: bool,
}

#[op]
pub fn op_webgpu_command_encoder_begin_render_pass(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  label: Option<String>,
  color_attachments: Vec<Option<GpuRenderPassColorAttachment>>,
  depth_stencil_attachment: Option<GpuRenderPassDepthStencilAttachment>,
  _occlusion_query_set: Option<u32>, // not yet implemented
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;

  let color_attachments = color_attachments
    .into_iter()
    .map(|color_attachment| {
      let rp_at = if let Some(at) = color_attachment.as_ref() {
        let texture_view_resource =
          state
            .resource_table
            .get::<super::texture::WebGpuTextureView>(at.view)?;

        let resolve_target = at
          .resolve_target
          .map(|rid| {
            state
              .resource_table
              .get::<super::texture::WebGpuTextureView>(rid)
          })
          .transpose()?
          .map(|texture| texture.0);

        Some(wgpu_core::command::RenderPassColorAttachment {
          view: texture_view_resource.0,
          resolve_target,
          channel: wgpu_core::command::PassChannel {
            load_op: at.load_op,
            store_op: at.store_op,
            clear_value: at.clear_value.unwrap_or_default(),
            read_only: false,
          },
        })
      } else {
        None
      };
      Ok(rp_at)
    })
    .collect::<Result<Vec<_>, AnyError>>()?;

  let mut processed_depth_stencil_attachment = None;

  if let Some(attachment) = depth_stencil_attachment {
    let texture_view_resource =
      state
        .resource_table
        .get::<super::texture::WebGpuTextureView>(attachment.view)?;

    processed_depth_stencil_attachment =
      Some(wgpu_core::command::RenderPassDepthStencilAttachment {
        view: texture_view_resource.0,
        depth: wgpu_core::command::PassChannel {
          load_op: attachment
            .depth_load_op
            .unwrap_or(wgpu_core::command::LoadOp::Load),
          store_op: attachment
            .depth_store_op
            .unwrap_or(wgpu_core::command::StoreOp::Store),
          clear_value: attachment.depth_clear_value,
          read_only: attachment.depth_read_only,
        },
        stencil: wgpu_core::command::PassChannel {
          load_op: attachment
            .stencil_load_op
            .unwrap_or(wgpu_core::command::LoadOp::Load),
          store_op: attachment
            .stencil_store_op
            .unwrap_or(wgpu_core::command::StoreOp::Store),
          clear_value: attachment.stencil_clear_value,
          read_only: attachment.stencil_read_only,
        },
      });
  }

  let descriptor = wgpu_core::command::RenderPassDescriptor {
    label: label.map(Cow::from),
    color_attachments: Cow::from(color_attachments),
    depth_stencil_attachment: processed_depth_stencil_attachment.as_ref(),
  };

  let render_pass = wgpu_core::command::RenderPass::new(
    command_encoder_resource.0,
    &descriptor,
  );

  let rid = state
    .resource_table
    .add(super::render_pass::WebGpuRenderPass(RefCell::new(
      render_pass,
    )));

  Ok(WebGpuResult::rid(rid))
}

#[op]
pub fn op_webgpu_command_encoder_begin_compute_pass(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  label: Option<String>,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;

  let descriptor = wgpu_core::command::ComputePassDescriptor {
    label: label.map(Cow::from),
  };

  let compute_pass = wgpu_core::command::ComputePass::new(
    command_encoder_resource.0,
    &descriptor,
  );

  let rid = state
    .resource_table
    .add(super::compute_pass::WebGpuComputePass(RefCell::new(
      compute_pass,
    )));

  Ok(WebGpuResult::rid(rid))
}

#[op]
pub fn op_webgpu_command_encoder_copy_buffer_to_buffer(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  source: ResourceId,
  source_offset: u64,
  destination: ResourceId,
  destination_offset: u64,
  size: u64,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let source_buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(source)?;
  let source_buffer = source_buffer_resource.0;
  let destination_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(destination)?;
  let destination_buffer = destination_buffer_resource.0;

  gfx_ok!(command_encoder => instance.command_encoder_copy_buffer_to_buffer(
    command_encoder,
    source_buffer,
    source_offset,
    destination_buffer,
    destination_offset,
    size
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuImageCopyBuffer {
  buffer: ResourceId,
  offset: u64,
  bytes_per_row: Option<u32>,
  rows_per_image: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuImageCopyTexture {
  pub texture: ResourceId,
  pub mip_level: u32,
  pub origin: wgpu_types::Origin3d,
  pub aspect: wgpu_types::TextureAspect,
}

#[op]
pub fn op_webgpu_command_encoder_copy_buffer_to_texture(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  source: GpuImageCopyBuffer,
  destination: GpuImageCopyTexture,
  copy_size: wgpu_types::Extent3d,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let source_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(source.buffer)?;
  let destination_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(destination.texture)?;

  let source = wgpu_core::command::ImageCopyBuffer {
    buffer: source_buffer_resource.0,
    layout: wgpu_types::ImageDataLayout {
      offset: source.offset,
      bytes_per_row: NonZeroU32::new(source.bytes_per_row.unwrap_or(0)),
      rows_per_image: NonZeroU32::new(source.rows_per_image.unwrap_or(0)),
    },
  };
  let destination = wgpu_core::command::ImageCopyTexture {
    texture: destination_texture_resource.0,
    mip_level: destination.mip_level,
    origin: destination.origin,
    aspect: destination.aspect,
  };
  gfx_ok!(command_encoder => instance.command_encoder_copy_buffer_to_texture(
    command_encoder,
    &source,
    &destination,
    &copy_size
  ))
}

#[op]
pub fn op_webgpu_command_encoder_copy_texture_to_buffer(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  source: GpuImageCopyTexture,
  destination: GpuImageCopyBuffer,
  copy_size: wgpu_types::Extent3d,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let source_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(source.texture)?;
  let destination_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(destination.buffer)?;

  let source = wgpu_core::command::ImageCopyTexture {
    texture: source_texture_resource.0,
    mip_level: source.mip_level,
    origin: source.origin,
    aspect: source.aspect,
  };
  let destination = wgpu_core::command::ImageCopyBuffer {
    buffer: destination_buffer_resource.0,
    layout: wgpu_types::ImageDataLayout {
      offset: destination.offset,
      bytes_per_row: NonZeroU32::new(destination.bytes_per_row.unwrap_or(0)),
      rows_per_image: NonZeroU32::new(destination.rows_per_image.unwrap_or(0)),
    },
  };
  gfx_ok!(command_encoder => instance.command_encoder_copy_texture_to_buffer(
    command_encoder,
    &source,
    &destination,
    &copy_size
  ))
}

#[op]
pub fn op_webgpu_command_encoder_copy_texture_to_texture(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  source: GpuImageCopyTexture,
  destination: GpuImageCopyTexture,
  copy_size: wgpu_types::Extent3d,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let source_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(source.texture)?;
  let destination_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(destination.texture)?;

  let source = wgpu_core::command::ImageCopyTexture {
    texture: source_texture_resource.0,
    mip_level: source.mip_level,
    origin: source.origin,
    aspect: source.aspect,
  };
  let destination = wgpu_core::command::ImageCopyTexture {
    texture: destination_texture_resource.0,
    mip_level: destination.mip_level,
    origin: destination.origin,
    aspect: destination.aspect,
  };
  gfx_ok!(command_encoder => instance.command_encoder_copy_texture_to_texture(
    command_encoder,
    &source,
    &destination,
    &copy_size
  ))
}

#[op]
pub fn op_webgpu_command_encoder_clear_buffer(
  state: &mut OpState,
  command_encoder_rid: u32,
  buffer_rid: u32,
  offset: u64,
  size: u64,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let destination_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer_rid)?;

  gfx_ok!(command_encoder => instance.command_encoder_clear_buffer(
    command_encoder,
    destination_resource.0,
    offset,
    std::num::NonZeroU64::new(size)
  ))
}

#[op]
pub fn op_webgpu_command_encoder_push_debug_group(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  group_label: String,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;

  gfx_ok!(command_encoder => instance.command_encoder_push_debug_group(command_encoder, &group_label))
}

#[op]
pub fn op_webgpu_command_encoder_pop_debug_group(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;

  gfx_ok!(command_encoder => instance.command_encoder_pop_debug_group(command_encoder))
}

#[op]
pub fn op_webgpu_command_encoder_insert_debug_marker(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  marker_label: String,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;

  gfx_ok!(command_encoder => instance.command_encoder_insert_debug_marker(
    command_encoder,
    &marker_label
  ))
}

#[op]
pub fn op_webgpu_command_encoder_write_timestamp(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  query_set: ResourceId,
  query_index: u32,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(query_set)?;

  gfx_ok!(command_encoder => instance.command_encoder_write_timestamp(
    command_encoder,
    query_set_resource.0,
    query_index
  ))
}

#[op]
pub fn op_webgpu_command_encoder_resolve_query_set(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  query_set: ResourceId,
  first_query: u32,
  query_count: u32,
  destination: ResourceId,
  destination_offset: u64,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(query_set)?;
  let destination_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(destination)?;

  gfx_ok!(command_encoder => instance.command_encoder_resolve_query_set(
    command_encoder,
    query_set_resource.0,
    first_query,
    query_count,
    destination_resource.0,
    destination_offset
  ))
}

#[op]
pub fn op_webgpu_command_encoder_finish(
  state: &mut OpState,
  command_encoder_rid: ResourceId,
  label: Option<String>,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .take::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let instance = state.borrow::<super::Instance>();

  let descriptor = wgpu_types::CommandBufferDescriptor {
    label: label.map(Cow::from),
  };

  gfx_put!(command_encoder => instance.command_encoder_finish(
    command_encoder,
    &descriptor
  ) => state, WebGpuCommandBuffer)
}
