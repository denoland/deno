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

pub(crate) struct WebGPUCommandEncoder(pub(crate) wgpu_core::id::CommandEncoderId);
impl Resource for WebGPUCommandEncoder {
  fn name(&self) -> Cow<str> {
    "webGPUCommandEncoder".into()
  }
}

pub(crate) struct WebGPUCommandBuffer(pub(crate) wgpu_core::id::CommandBufferId);
impl Resource for WebGPUCommandBuffer {
  fn name(&self) -> Cow<str> {
    "webGPUCommandBuffer".into()
  }
}

fn serialize_store_op(store_op: String) -> wgpu_core::command::StoreOp {
  match store_op.as_str() {
    "store" => wgpu_core::command::StoreOp::Store,
    "clear" => wgpu_core::command::StoreOp::Clear,
    _ => unreachable!(),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCommandEncoderArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  _measure_execution_time: Option<bool>, // waiting for wgpu to add measure_execution_time
}

pub fn op_webgpu_create_command_encoder(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateCommandEncoderArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  let descriptor = wgpu_types::CommandEncoderDescriptor {
    label: args.label.map(Cow::Owned),
  };

  // TODO
  let (command_encoder, _) = gfx_select!(device => instance.device_create_command_encoder(
    device,
    &descriptor,
    std::marker::PhantomData
  ));

  let rid = state
    .resource_table
    .add(WebGPUCommandEncoder(command_encoder));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPURenderPassColorAttachment {
  view: u32,
  resolve_target: Option<u32>,
  load_op: String,
  load_value: Option<super::render_pass::GPUColor>,
  store_op: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPURenderPassDepthStencilAttachment {
  view: u32,
  depth_load_op: String,
  depth_load_value: Option<f32>,
  depth_store_op: String,
  depth_read_only: Option<bool>,
  stencil_load_op: String,
  stencil_load_value: Option<u32>,
  stencil_store_op: String,
  stencil_read_only: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderBeginRenderPassArgs {
  command_encoder_rid: u32,
  label: Option<String>,
  color_attachments: Vec<GPURenderPassColorAttachment>,
  depth_stencil_attachment: Option<GPURenderPassDepthStencilAttachment>,
  _occlusion_query_set: u32, // TODO wgpu#721
}

pub fn op_webgpu_command_encoder_begin_render_pass(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderBeginRenderPassArgs = serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  let mut color_attachments = vec![];

  for color_attachment in args.color_attachments {
    let texture_view_resource = state
      .resource_table
      .get::<super::texture::WebGPUTextureView>(color_attachment.view)
      .ok_or_else(bad_resource_id)?;

    let attachment = wgpu_core::command::ColorAttachmentDescriptor {
      attachment: texture_view_resource.0,
      resolve_target: color_attachment
        .resolve_target
        .map(|rid| {
          state
            .resource_table
            .get::<super::texture::WebGPUTextureView>(rid)
            .ok_or_else(bad_resource_id)
        })
        .transpose()?
        .map(|texture| texture.0),
      channel: match color_attachment.load_op.as_str() {
        "load" => wgpu_core::command::PassChannel {
          load_op: wgpu_core::command::LoadOp::Load,
          store_op: color_attachment
            .store_op
            .map_or(wgpu_core::command::StoreOp::Store, serialize_store_op),
          clear_value: Default::default(),
          read_only: false,
        },
        "clear" => {
          let color = color_attachment.load_value.unwrap();
          wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Clear,
            store_op: color_attachment
              .store_op
              .map_or(wgpu_core::command::StoreOp::Store, serialize_store_op),
            clear_value: wgpu_types::Color {
              r: color.r,
              g: color.g,
              b: color.b,
              a: color.a,
            },
            read_only: false,
          }
        }
        _ => unreachable!(),
      },
    };

    color_attachments.push(attachment)
  }

  let mut depth_stencil_attachment = None;

  if let Some(attachment) = args.depth_stencil_attachment {
    let texture_view_resource = state
      .resource_table
      .get::<super::texture::WebGPUTextureView>(attachment.view)
      .ok_or_else(bad_resource_id)?;

    depth_stencil_attachment =
      Some(wgpu_core::command::DepthStencilAttachmentDescriptor {
        attachment: texture_view_resource.0,
        depth: match attachment.depth_load_op.as_str() {
          "load" => wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Load,
            store_op: serialize_store_op(attachment.depth_store_op),
            clear_value: 0.0,
            read_only: attachment.depth_read_only.unwrap_or(false),
          },
          "clear" => wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Clear,
            store_op: serialize_store_op(attachment.depth_store_op),
            clear_value: attachment.depth_load_value.unwrap(),
            read_only: attachment.depth_read_only.unwrap_or(false),
          },
          _ => unreachable!(),
        },
        stencil: match attachment.stencil_load_op.as_str() {
          "load" => wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Load,
            store_op: serialize_store_op(attachment.stencil_store_op),
            clear_value: 0,
            read_only: attachment.stencil_read_only.unwrap_or(false),
          },
          "clear" => wgpu_core::command::PassChannel {
            load_op: wgpu_core::command::LoadOp::Clear,
            store_op: serialize_store_op(attachment.stencil_store_op),
            clear_value: attachment.stencil_load_value.unwrap(),
            read_only: attachment.stencil_read_only.unwrap_or(false),
          },
          _ => unreachable!(),
        },
      });
  }

  let descriptor = wgpu_core::command::RenderPassDescriptor {
    label: args.label.map(Cow::Owned),
    color_attachments: Cow::Owned(color_attachments),
    depth_stencil_attachment: depth_stencil_attachment.as_ref(),
  };

  let render_pass =
    wgpu_core::command::RenderPass::new(command_encoder_resource.0, &descriptor);

  let rid = state
    .resource_table
    .add(super::render_pass::WebGPURenderPass(RefCell::new(
      render_pass,
    )));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderBeginComputePassArgs {
  command_encoder_rid: u32,
  label: Option<String>,
}

pub fn op_webgpu_command_encoder_begin_compute_pass(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderBeginComputePassArgs = serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  let descriptor = wgpu_core::command::ComputePassDescriptor {
    label: args.label.map(Cow::Owned),
  };

  let compute_pass =
    wgpu_core::command::ComputePass::new(command_encoder_resource.0, &descriptor);

  let rid = state
    .resource_table
    .add(super::compute_pass::WebGPUComputePass(RefCell::new(
      compute_pass,
    )));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderCopyBufferToBufferArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  source: u32,
  source_offset: u64,
  destination: u32,
  destination_offset: u64,
  size: u64,
}

pub fn op_webgpu_command_encoder_copy_buffer_to_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderCopyBufferToBufferArgs =
    serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let source_buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.source)
    .ok_or_else(bad_resource_id)?;
  let source_buffer = source_buffer_resource.0;
  let destination_buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.destination)
    .ok_or_else(bad_resource_id)?;
  let destination_buffer = destination_buffer_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  gfx_select!(command_encoder => instance.command_encoder_copy_buffer_to_buffer(
    command_encoder,
    source_buffer,
    args.source_offset,
    destination_buffer,
    args.destination_offset,
    args.size
  ))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GPUImageCopyBuffer {
  buffer: u32,
  offset: Option<u64>,
  bytes_per_row: Option<u32>,
  rows_per_image: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GPUOrigin3D {
  pub x: Option<u32>,
  pub y: Option<u32>,
  pub z: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GPUImageCopyTexture {
  pub texture: u32,
  pub mip_level: Option<u32>,
  pub origin: Option<GPUOrigin3D>,
  pub aspect: Option<String>, // TODO
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderCopyBufferToTextureArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  source: GPUImageCopyBuffer,
  destination: GPUImageCopyTexture,
  copy_size: super::texture::GPUExtent3D,
}

pub fn op_webgpu_command_encoder_copy_buffer_to_texture(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderCopyBufferToTextureArgs =
    serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let source_buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.source.buffer)
    .ok_or_else(bad_resource_id)?;
  let destination_texture_resource = state
    .resource_table
    .get::<super::texture::WebGPUTexture>(args.destination.texture)
    .ok_or_else(bad_resource_id)?;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  let source = wgpu_core::command::BufferCopyView {
    buffer: source_buffer_resource.0,
    layout: wgpu_types::TextureDataLayout {
      offset: args.source.offset.unwrap_or(0),
      bytes_per_row: args.source.bytes_per_row.unwrap_or(0),
      rows_per_image: args.source.rows_per_image.unwrap_or(0),
    },
  };
  let destination = wgpu_core::command::TextureCopyView {
    texture: destination_texture_resource.0,
    mip_level: args.destination.mip_level.unwrap_or(0),
    origin: args
      .destination
      .origin
      .map_or(Default::default(), |origin| wgpu_types::Origin3d {
        x: origin.x.unwrap_or(0),
        y: origin.y.unwrap_or(0),
        z: origin.z.unwrap_or(0),
      }),
  };
  gfx_select!(command_encoder => instance.command_encoder_copy_buffer_to_texture(
    command_encoder,
    &source,
    &destination,
    &wgpu_types::Extent3d {
      width: args.copy_size.width.unwrap_or(1),
      height: args.copy_size.height.unwrap_or(1),
      depth: args.copy_size.depth.unwrap_or(1),
    }
  ))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderCopyTextureToBufferArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  source: GPUImageCopyTexture,
  destination: GPUImageCopyBuffer,
  copy_size: super::texture::GPUExtent3D,
}

pub fn op_webgpu_command_encoder_copy_texture_to_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderCopyTextureToBufferArgs =
    serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let source_texture_resource = state
    .resource_table
    .get::<super::texture::WebGPUTexture>(args.source.texture)
    .ok_or_else(bad_resource_id)?;
  let destination_buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGPUBuffer>(args.destination.buffer)
    .ok_or_else(bad_resource_id)?;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  let source = wgpu_core::command::TextureCopyView {
    texture: source_texture_resource.0,
    mip_level: args.source.mip_level.unwrap_or(0),
    origin: args.source.origin.map_or(Default::default(), |origin| {
      wgpu_types::Origin3d {
        x: origin.x.unwrap_or(0),
        y: origin.y.unwrap_or(0),
        z: origin.z.unwrap_or(0),
      }
    }),
  };
  let destination = wgpu_core::command::BufferCopyView {
    buffer: destination_buffer_resource.0,
    layout: wgpu_types::TextureDataLayout {
      offset: args.destination.offset.unwrap_or(0),
      bytes_per_row: args.destination.bytes_per_row.unwrap_or(0),
      rows_per_image: args.destination.rows_per_image.unwrap_or(0),
    },
  };
  gfx_select!(command_encoder => instance.command_encoder_copy_texture_to_buffer(
    command_encoder,
    &source,
    &destination,
    &wgpu_types::Extent3d {
      width: args.copy_size.width.unwrap_or(1),
      height: args.copy_size.height.unwrap_or(1),
      depth: args.copy_size.depth.unwrap_or(1),
    }
  ))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderCopyTextureToTextureArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  source: GPUImageCopyTexture,
  destination: GPUImageCopyTexture,
  copy_size: super::texture::GPUExtent3D,
}

pub fn op_webgpu_command_encoder_copy_texture_to_texture(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderCopyTextureToTextureArgs =
    serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let source_texture_resource = state
    .resource_table
    .get::<super::texture::WebGPUTexture>(args.source.texture)
    .ok_or_else(bad_resource_id)?;
  let destination_texture_resource = state
    .resource_table
    .get::<super::texture::WebGPUTexture>(args.destination.texture)
    .ok_or_else(bad_resource_id)?;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  let source = wgpu_core::command::TextureCopyView {
    texture: source_texture_resource.0,
    mip_level: args.source.mip_level.unwrap_or(0),
    origin: args.source.origin.map_or(Default::default(), |origin| {
      wgpu_types::Origin3d {
        x: origin.x.unwrap_or(0),
        y: origin.y.unwrap_or(0),
        z: origin.z.unwrap_or(0),
      }
    }),
  };
  let destination = wgpu_core::command::TextureCopyView {
    texture: destination_texture_resource.0,
    mip_level: args.destination.mip_level.unwrap_or(0),
    origin: args
      .destination
      .origin
      .map_or(Default::default(), |origin| wgpu_types::Origin3d {
        x: origin.x.unwrap_or(0),
        y: origin.y.unwrap_or(0),
        z: origin.z.unwrap_or(0),
      }),
  };
  gfx_select!(command_encoder => instance.command_encoder_copy_texture_to_texture(
    command_encoder,
    &source,
    &destination,
    &wgpu_types::Extent3d {
      width: args.copy_size.width.unwrap_or(1),
      height: args.copy_size.height.unwrap_or(1),
      depth: args.copy_size.depth.unwrap_or(1),
    }
  ))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderPushDebugGroupArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  group_label: String,
}

pub fn op_webgpu_command_encoder_push_debug_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderPushDebugGroupArgs = serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  gfx_select!(command_encoder => instance
    .command_encoder_push_debug_group(command_encoder, &args.group_label))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderPopDebugGroupArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
}

pub fn op_webgpu_command_encoder_pop_debug_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderPopDebugGroupArgs = serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  gfx_select!(command_encoder => instance.command_encoder_pop_debug_group(command_encoder))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderInsertDebugMarkerArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  marker_label: String,
}

pub fn op_webgpu_command_encoder_insert_debug_marker(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderInsertDebugMarkerArgs = serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  gfx_select!(command_encoder => instance.command_encoder_insert_debug_marker(
    command_encoder,
    &args.marker_label
  ))?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderFinishArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  label: Option<String>,
}

pub fn op_webgpu_command_encoder_finish(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderFinishArgs = serde_json::from_value(args)?;

  let command_encoder_resource = state
    .resource_table
    .get::<WebGPUCommandEncoder>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = command_encoder_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  let descriptor = wgpu_types::CommandBufferDescriptor {
    label: args.label.map(Cow::Owned),
  };

  // TODO
  let (command_buffer, _) = gfx_select!(command_encoder => instance.command_encoder_finish(
    command_encoder,
    &descriptor
  ));

  let rid = state
    .resource_table
    .add(WebGPUCommandBuffer(command_buffer));

  Ok(json!({
    "rid": rid,
  }))
}
