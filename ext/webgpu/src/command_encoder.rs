// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ResourceId;
use deno_core::{OpState, Resource};
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommandEncoderArgs {
  device_rid: ResourceId,
  label: Option<String>,
  _measure_execution_time: Option<bool>, // not yet implemented
}

#[op]
pub fn op_webgpu_create_command_encoder(
  state: &mut OpState,
  args: CreateCommandEncoderArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let descriptor = wgpu_types::CommandEncoderDescriptor {
    label: args.label.map(Cow::from),
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
  load_op: GpuLoadOp<wgpu_types::Color>,
  store_op: wgpu_core::command::StoreOp,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum GpuLoadOp<T> {
  Load,
  Clear(T),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuRenderPassDepthStencilAttachment {
  view: ResourceId,
  depth_load_op: GpuLoadOp<f32>,
  depth_store_op: wgpu_core::command::StoreOp,
  depth_read_only: bool,
  stencil_load_op: GpuLoadOp<u32>,
  stencil_store_op: wgpu_core::command::StoreOp,
  stencil_read_only: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderBeginRenderPassArgs {
  command_encoder_rid: ResourceId,
  label: Option<String>,
  color_attachments: Vec<GpuRenderPassColorAttachment>,
  depth_stencil_attachment: Option<GpuRenderPassDepthStencilAttachment>,
  _occlusion_query_set: Option<u32>, // not yet implemented
}

#[op]
pub fn op_webgpu_command_encoder_begin_render_pass(
  state: &mut OpState,
  args: CommandEncoderBeginRenderPassArgs,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;

  let mut color_attachments = vec![];

  for color_attachment in args.color_attachments {
    let texture_view_resource =
      state
        .resource_table
        .get::<super::texture::WebGpuTextureView>(color_attachment.view)?;

    let resolve_target = color_attachment
      .resolve_target
      .map(|rid| {
        state
          .resource_table
          .get::<super::texture::WebGpuTextureView>(rid)
      })
      .transpose()?
      .map(|texture| texture.0);

    let attachment = wgpu_core::command::RenderPassColorAttachment {
      view: texture_view_resource.0,
      resolve_target,
      channel: match color_attachment.load_op {
        GpuLoadOp::Load => wgpu_core::command::PassChannel {
          load_op: wgpu_core::command::LoadOp::Load,
          store_op: color_attachment.store_op,
          clear_value: Default::default(),
          read_only: false,
        },
        GpuLoadOp::Clear(color) => wgpu_core::command::PassChannel {
          load_op: wgpu_core::command::LoadOp::Clear,
          store_op: color_attachment.store_op,
          clear_value: color,
          read_only: false,
        },
      },
    };

    color_attachments.push(attachment)
  }

  let mut depth_stencil_attachment = None;

  if let Some(attachment) = args.depth_stencil_attachment {
    let texture_view_resource =
      state
        .resource_table
        .get::<super::texture::WebGpuTextureView>(attachment.view)?;

    depth_stencil_attachment =
      Some(wgpu_core::command::RenderPassDepthStencilAttachment {
        view: texture_view_resource.0,
        depth: match attachment.depth_load_op {
          GpuLoadOp::Load => wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Load,
            store_op: attachment.depth_store_op,
            clear_value: 0.0,
            read_only: attachment.depth_read_only,
          },
          GpuLoadOp::Clear(value) => wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Clear,
            store_op: attachment.depth_store_op,
            clear_value: value,
            read_only: attachment.depth_read_only,
          },
        },
        stencil: match attachment.stencil_load_op {
          GpuLoadOp::Load => wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Load,
            store_op: attachment.stencil_store_op,
            clear_value: 0,
            read_only: attachment.stencil_read_only,
          },
          GpuLoadOp::Clear(value) => wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Clear,
            store_op: attachment.stencil_store_op,
            clear_value: value,
            read_only: attachment.stencil_read_only,
          },
        },
      });
  }

  let descriptor = wgpu_core::command::RenderPassDescriptor {
    label: args.label.map(Cow::from),
    color_attachments: Cow::from(color_attachments),
    depth_stencil_attachment: depth_stencil_attachment.as_ref(),
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderBeginComputePassArgs {
  command_encoder_rid: ResourceId,
  label: Option<String>,
}

#[op]
pub fn op_webgpu_command_encoder_begin_compute_pass(
  state: &mut OpState,
  args: CommandEncoderBeginComputePassArgs,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;

  let descriptor = wgpu_core::command::ComputePassDescriptor {
    label: args.label.map(Cow::from),
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderCopyBufferToBufferArgs {
  command_encoder_rid: ResourceId,
  source: ResourceId,
  source_offset: u64,
  destination: ResourceId,
  destination_offset: u64,
  size: u64,
}

#[op]
pub fn op_webgpu_command_encoder_copy_buffer_to_buffer(
  state: &mut OpState,
  args: CommandEncoderCopyBufferToBufferArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let source_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(args.source)?;
  let source_buffer = source_buffer_resource.0;
  let destination_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(args.destination)?;
  let destination_buffer = destination_buffer_resource.0;

  gfx_ok!(command_encoder => instance.command_encoder_copy_buffer_to_buffer(
    command_encoder,
    source_buffer,
    args.source_offset,
    destination_buffer,
    args.destination_offset,
    args.size
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderCopyBufferToTextureArgs {
  command_encoder_rid: ResourceId,
  source: GpuImageCopyBuffer,
  destination: GpuImageCopyTexture,
  copy_size: wgpu_types::Extent3d,
}

#[op]
pub fn op_webgpu_command_encoder_copy_buffer_to_texture(
  state: &mut OpState,
  args: CommandEncoderCopyBufferToTextureArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let source_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(args.source.buffer)?;
  let destination_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(args.destination.texture)?;

  let source = wgpu_core::command::ImageCopyBuffer {
    buffer: source_buffer_resource.0,
    layout: wgpu_types::ImageDataLayout {
      offset: args.source.offset,
      bytes_per_row: NonZeroU32::new(args.source.bytes_per_row.unwrap_or(0)),
      rows_per_image: NonZeroU32::new(args.source.rows_per_image.unwrap_or(0)),
    },
  };
  let destination = wgpu_core::command::ImageCopyTexture {
    texture: destination_texture_resource.0,
    mip_level: args.destination.mip_level,
    origin: args.destination.origin,
    aspect: args.destination.aspect,
  };
  gfx_ok!(command_encoder => instance.command_encoder_copy_buffer_to_texture(
    command_encoder,
    &source,
    &destination,
    &args.copy_size
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderCopyTextureToBufferArgs {
  command_encoder_rid: ResourceId,
  source: GpuImageCopyTexture,
  destination: GpuImageCopyBuffer,
  copy_size: wgpu_types::Extent3d,
}

#[op]
pub fn op_webgpu_command_encoder_copy_texture_to_buffer(
  state: &mut OpState,
  args: CommandEncoderCopyTextureToBufferArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let source_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(args.source.texture)?;
  let destination_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(args.destination.buffer)?;

  let source = wgpu_core::command::ImageCopyTexture {
    texture: source_texture_resource.0,
    mip_level: args.source.mip_level,
    origin: args.source.origin,
    aspect: args.source.aspect,
  };
  let destination = wgpu_core::command::ImageCopyBuffer {
    buffer: destination_buffer_resource.0,
    layout: wgpu_types::ImageDataLayout {
      offset: args.destination.offset,
      bytes_per_row: NonZeroU32::new(
        args.destination.bytes_per_row.unwrap_or(0),
      ),
      rows_per_image: NonZeroU32::new(
        args.destination.rows_per_image.unwrap_or(0),
      ),
    },
  };
  gfx_ok!(command_encoder => instance.command_encoder_copy_texture_to_buffer(
    command_encoder,
    &source,
    &destination,
    &args.copy_size
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderCopyTextureToTextureArgs {
  command_encoder_rid: ResourceId,
  source: GpuImageCopyTexture,
  destination: GpuImageCopyTexture,
  copy_size: wgpu_types::Extent3d,
}

#[op]
pub fn op_webgpu_command_encoder_copy_texture_to_texture(
  state: &mut OpState,
  args: CommandEncoderCopyTextureToTextureArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let source_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(args.source.texture)?;
  let destination_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(args.destination.texture)?;

  let source = wgpu_core::command::ImageCopyTexture {
    texture: source_texture_resource.0,
    mip_level: args.source.mip_level,
    origin: args.source.origin,
    aspect: args.source.aspect,
  };
  let destination = wgpu_core::command::ImageCopyTexture {
    texture: destination_texture_resource.0,
    mip_level: args.destination.mip_level,
    origin: args.destination.origin,
    aspect: args.destination.aspect,
  };
  gfx_ok!(command_encoder => instance.command_encoder_copy_texture_to_texture(
    command_encoder,
    &source,
    &destination,
    &args.copy_size
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderClearBufferArgs {
  command_encoder_rid: u32,
  destination_rid: u32,
  destination_offset: u64,
  size: u64,
}

#[op]
pub fn op_webgpu_command_encoder_clear_buffer(
  state: &mut OpState,
  args: CommandEncoderClearBufferArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let destination_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.destination_rid)?;

  gfx_ok!(command_encoder => instance.command_encoder_clear_buffer(
    command_encoder,
    destination_resource.0,
    args.destination_offset,
    std::num::NonZeroU64::new(args.size)
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderPushDebugGroupArgs {
  command_encoder_rid: ResourceId,
  group_label: String,
}

#[op]
pub fn op_webgpu_command_encoder_push_debug_group(
  state: &mut OpState,
  args: CommandEncoderPushDebugGroupArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;

  gfx_ok!(command_encoder => instance
    .command_encoder_push_debug_group(command_encoder, &args.group_label))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderPopDebugGroupArgs {
  command_encoder_rid: ResourceId,
}

#[op]
pub fn op_webgpu_command_encoder_pop_debug_group(
  state: &mut OpState,
  args: CommandEncoderPopDebugGroupArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;

  gfx_ok!(command_encoder => instance.command_encoder_pop_debug_group(command_encoder))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderInsertDebugMarkerArgs {
  command_encoder_rid: ResourceId,
  marker_label: String,
}

#[op]
pub fn op_webgpu_command_encoder_insert_debug_marker(
  state: &mut OpState,
  args: CommandEncoderInsertDebugMarkerArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;

  gfx_ok!(command_encoder => instance.command_encoder_insert_debug_marker(
    command_encoder,
    &args.marker_label
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderWriteTimestampArgs {
  command_encoder_rid: ResourceId,
  query_set: ResourceId,
  query_index: u32,
}

#[op]
pub fn op_webgpu_command_encoder_write_timestamp(
  state: &mut OpState,
  args: CommandEncoderWriteTimestampArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(args.query_set)?;

  gfx_ok!(command_encoder => instance.command_encoder_write_timestamp(
    command_encoder,
    query_set_resource.0,
    args.query_index
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderResolveQuerySetArgs {
  command_encoder_rid: ResourceId,
  query_set: ResourceId,
  first_query: u32,
  query_count: u32,
  destination: ResourceId,
  destination_offset: u64,
}

#[op]
pub fn op_webgpu_command_encoder_resolve_query_set(
  state: &mut OpState,
  args: CommandEncoderResolveQuerySetArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(args.query_set)?;
  let destination_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(args.destination)?;

  gfx_ok!(command_encoder => instance.command_encoder_resolve_query_set(
    command_encoder,
    query_set_resource.0,
    args.first_query,
    args.query_count,
    destination_resource.0,
    args.destination_offset
  ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEncoderFinishArgs {
  command_encoder_rid: ResourceId,
  label: Option<String>,
}

#[op]
pub fn op_webgpu_command_encoder_finish(
  state: &mut OpState,
  args: CommandEncoderFinishArgs,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .take::<WebGpuCommandEncoder>(args.command_encoder_rid)?;
  let command_encoder = command_encoder_resource.0;
  let instance = state.borrow::<super::Instance>();

  let descriptor = wgpu_types::CommandBufferDescriptor {
    label: args.label.map(Cow::from),
  };

  gfx_put!(command_encoder => instance.command_encoder_finish(
    command_encoder,
    &descriptor
  ) => state, WebGpuCommandBuffer)
}
