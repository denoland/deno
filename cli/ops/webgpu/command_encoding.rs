// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCommandEncoderArgs {
  rid: u32,
  label: Option<String>,
  measure_execution_time: Option<bool>, // TODO
}

pub fn op_webgpu_create_command_encoder(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateCommandEncoderArgs = serde_json::from_value(args)?;

  let device = state
    .resource_table
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let command_encoder =
    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
      label: args.label.map(|label| &label),
    });

  let rid = state
    .resource_table
    .add("webGPUCommandEncoder", Box::new(command_encoder));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPURenderPassColorAttachmentDescriptor {
  attachment: u32,
  resolve_target: Option<u32>,
  load_value: (), // TODO
  store_op: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPURenderPassDepthStencilAttachmentDescriptor {
  attachment: u32,
  depth_load_value: (), // TODO
  depth_store_op: String,
  depth_read_only: Option<bool>,
  stencil_load_value: (), // TODO
  stencil_store_op: String,
  stencil_read_only: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderBeginRenderPassArgs {
  rid: u32,
  label: Option<String>, // TODO
  color_attachments: [GPURenderPassColorAttachmentDescriptor],
  depth_stencil_attachment:
    Option<GPURenderPassDepthStencilAttachmentDescriptor>,
  occlusion_query_set: (), // TODO
}

pub fn op_webgpu_command_encoder_begin_render_pass(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderBeginRenderPassArgs = serde_json::from_value(args)?;

  let command_encoder = state
    .resource_table
    .get_mut::<wgpu::CommandEncoder>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let render_pass =
    command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
      color_attachments: &args
        .color_attachments
        .iter()
        .map(|color_attachment| {
          wgpu::RenderPassColorAttachmentDescriptor {
            attachment: state
              .resource_table
              .get_mut::<wgpu::TextureView>(color_attachment.attachment)
              .ok_or_else(bad_resource_id)?,
            resolve_target: color_attachment.resolve_target.map(|rid| {
              state
                .resource_table
                .get_mut::<wgpu::TextureView>(rid)
                .ok_or_else(bad_resource_id)?
            }),
            ops: (), // TODO
          }
        })
        .collect::<[wgpu::RenderPassColorAttachmentDescriptor]>(),
      depth_stencil_attachment: args.depth_stencil_attachment.map(
        |depth_stencil_attachment| {
          wgpu::RenderPassDepthStencilAttachmentDescriptor {
            attachment: state
              .resource_table
              .get_mut::<wgpu::TextureView>(depth_stencil_attachment.attachment)
              .ok_or_else(bad_resource_id)?,
            depth_ops: None,   // TODO
            stencil_ops: None, // TODO
          }
        },
      ),
    });

  let rid = state
    .resource_table
    .add("webGPURenderPass", Box::new(render_pass));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderBeginComputePassArgs {
  rid: u32,
  label: Option<String>, // TODO
}

pub fn op_webgpu_command_encoder_begin_compute_pass(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderBeginComputePassArgs = serde_json::from_value(args)?;

  let command_encoder = state
    .resource_table
    .get_mut::<wgpu::CommandEncoder>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let compute_pass = command_encoder.begin_compute_pass();

  let rid = state
    .resource_table
    .add("webGPUComputePass", Box::new(compute_pass));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUTextureCopyView {
  texture: u32,
  mip_level: Option<u32>,
  origin: (), // TODO
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderCopyTextureToTextureArgs {
  rid: u32,
  source: GPUTextureCopyView,
  destination: GPUTextureCopyView,
  copy_size: (), // TODO
}

pub fn op_webgpu_command_encoder_copy_texture_to_texture(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderCopyTextureToTextureArgs =
    serde_json::from_value(args)?;

  let command_encoder = state
    .resource_table
    .get_mut::<wgpu::CommandEncoder>(args.rid)
    .ok_or_else(bad_resource_id)?;

  command_encoder.copy_texture_to_texture(
    wgpu::TextureCopyView {
      texture: state
        .resource_table
        .get_mut::<wgpu::Texture>(args.source.texture)
        .ok_or_else(bad_resource_id)?,
      mip_level: args.source.mip_level.unwrap_or(0),
      origin: Default::default(), // TODO
    },
    wgpu::TextureCopyView {
      texture: state
        .resource_table
        .get_mut::<wgpu::Texture>(args.destination.texture)
        .ok_or_else(bad_resource_id)?,
      mip_level: args.destination.mip_level.unwrap_or(0),
      origin: Default::default(), // TODO
    },
    wgpu::Extent3d {
      width: 0,
      height: 0,
      depth: 0,
    },
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderFinishArgs {
  rid: u32,
  label: Option<String>, // TODO
}

pub fn op_webgpu_command_encoder_finish(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderFinishArgs = serde_json::from_value(args)?;

  let command_encoder = state
    .resource_table
    .get_mut::<wgpu::CommandEncoder>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let command_buffer = command_encoder.finish();

  let rid = state
    .resource_table
    .add("webGPUCommandBuffer", Box::new(command_buffer));

  Ok(json!({
    "rid": rid,
  }))
}
