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
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCommandEncoderArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  measure_execution_time: Option<bool>, // TODO
}

pub fn op_webgpu_create_command_encoder(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateCommandEncoderArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let device = state
    .resource_table
    .get_mut::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;

  let command_encoder = instance.device_create_command_encoder(
    *device,
    &wgt::CommandEncoderDescriptor {
      label: args.label.map(|label| Cow::Borrowed(&label)),
    },
    (), // TODO
  )?;

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
  instance_rid: u32,
  command_encoder_rid: u32,
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

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  let render_pass = wgc::command::RenderPass::new(
    *command_encoder,
    wgc::command::RenderPassDescriptor {
      color_attachments: Cow::Owned(
        args
          .color_attachments
          .iter()
          .map(|color_attachment| {
            wgc::command::ColorAttachmentDescriptor {
              attachment: *state
                .resource_table
                .get_mut::<wgc::id::TextureViewId>(color_attachment.attachment)
                .ok_or_else(bad_resource_id)?,
              resolve_target: color_attachment.resolve_target.map(|rid| {
                *state
                  .resource_table
                  .get_mut::<wgc::id::TextureViewId>(rid)
                  .ok_or_else(bad_resource_id)?
              }),
              channel: PassChannel {}, // TODO
            }
          })
          .collect::<Vec<wgc::command::ColorAttachmentDescriptor>>(),
      ),
      depth_stencil_attachment: args.depth_stencil_attachment.map(
        |depth_stencil_attachment| {
          &wgc::command::DepthStencilAttachmentDescriptor {
            attachment: *state
              .resource_table
              .get_mut::<wgc::id::TextureViewId>(
                depth_stencil_attachment.attachment,
              )
              .ok_or_else(bad_resource_id)?,
            depth: PassChannel {},   // TODO
            stencil: PassChannel {}, // TODO
          }
        },
      ),
    },
  );

  instance.command_encoder_run_render_pass(*command_encoder, &render_pass)?;

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
  instance_rid: u32,
  command_encoder_rid: u32,
  label: Option<String>, // TODO
}

pub fn op_webgpu_command_encoder_begin_compute_pass(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderBeginComputePassArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  let compute_pass = wgc::command::ComputePass::new(*command_encoder);

  instance.command_encoder_run_compute_pass(*command_encoder, &compute_pass);

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
  instance_rid: u32,
  command_encoder_rid: u32,
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

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  instance.command_encoder_copy_texture_to_texture(
    *command_encoder,
    &wgc::command::TextureCopyView {
      texture: *state
        .resource_table
        .get_mut::<wgc::id::TextureId>(args.source.texture)
        .ok_or_else(bad_resource_id)?,
      mip_level: args.source.mip_level.unwrap_or(0),
      origin: Default::default(), // TODO
    },
    &wgc::command::TextureCopyView {
      texture: *state
        .resource_table
        .get_mut::<wgc::id::TextureId>(args.destination.texture)
        .ok_or_else(bad_resource_id)?,
      mip_level: args.destination.mip_level.unwrap_or(0),
      origin: Default::default(), // TODO
    },
    &wgt::Extent3d {
      // TODO
      width: 0,
      height: 0,
      depth: 0,
    },
  )?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderFinishArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  label: Option<String>, // TODO
}

pub fn op_webgpu_command_encoder_finish(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderFinishArgs = serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  let command_buffer = instance.command_encoder_finish(
    *command_encoder,
    &wgt::CommandBufferDescriptor {
      label: args.label.map(|label| Cow::Borrowed(&label)),
    },
  )?;

  let rid = state
    .resource_table
    .add("webGPUCommandBuffer", Box::new(command_buffer));

  Ok(json!({
    "rid": rid,
  }))
}
