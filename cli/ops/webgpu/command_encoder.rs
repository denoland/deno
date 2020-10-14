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

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::super::reg_json_sync(
    rt,
    "op_webgpu_create_command_encoder",
    op_webgpu_create_command_encoder,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_begin_render_pass",
    op_webgpu_command_encoder_begin_render_pass,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_begin_compute_pass",
    op_webgpu_command_encoder_begin_compute_pass,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_buffer_to_buffer",
    op_webgpu_command_encoder_copy_buffer_to_buffer,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_buffer_to_texture",
    op_webgpu_command_encoder_copy_buffer_to_texture,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_texture_to_buffer",
    op_webgpu_command_encoder_copy_texture_to_buffer,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_texture_to_texture",
    op_webgpu_command_encoder_copy_texture_to_texture,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_push_debug_group",
    op_webgpu_command_encoder_push_debug_group,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_pop_debug_group",
    op_webgpu_command_encoder_pop_debug_group,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_insert_debug_marker",
    op_webgpu_command_encoder_insert_debug_marker,
  );
  super::super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_finish",
    op_webgpu_command_encoder_finish,
  );
}

fn serialize_store_op(store_op: String) -> wgc::command::StoreOp {
  match store_op {
    &"store" => wgc::command::StoreOp::Store,
    &"clear" => wgc::command::StoreOp::Clear,
    _ => unreachable!(),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateCommandEncoderArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  measure_execution_time: Option<bool>, // waiting for wgpu to add measure_execution_time
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
    std::marker::PhantomData,
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
  load_value: (), // TODO: mixed types
  store_op: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPURenderPassDepthStencilAttachmentDescriptor {
  attachment: u32,
  depth_load_value: (), // TODO: mixed types
  depth_store_op: String,
  depth_read_only: Option<bool>,
  stencil_load_value: (), // TODO: mixed types
  stencil_store_op: String,
  stencil_read_only: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderBeginRenderPassArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  label: Option<String>, // wgpu#974
  color_attachments: [GPURenderPassColorAttachmentDescriptor],
  depth_stencil_attachment:
    Option<GPURenderPassDepthStencilAttachmentDescriptor>,
  occlusion_query_set: u32, // wgpu#721
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
              channel: wgc::command::PassChannel {
                load_op: LoadOp::Clear, // TODO
                store_op: color_attachment
                  .store_op
                  .map_or(wgc::command::StoreOp::Store, serialize_store_op),
                clear_value: (),  // TODO
                read_only: false, // TODO
              },
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
            depth: wgc::command::PassChannel {
              load_op: LoadOp::Clear, // TODO
              store_op: serialize_store_op(
                depth_stencil_attachment.depth_store_op,
              ),
              clear_value: (),  // TODO
              read_only: false, // TODO
            },
            stencil: wgc::command::PassChannel {
              load_op: LoadOp::Clear, // TODO
              store_op: serialize_store_op(
                depth_stencil_attachment.stencil_store_op,
              ),
              clear_value: (),  // TODO
              read_only: false, // TODO
            },
          }
        },
      ),
    },
  );

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
  label: Option<String>, // wgpu#974
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

  let rid = state
    .resource_table
    .add("webGPUComputePass", Box::new(compute_pass));

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

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  instance.command_encoder_copy_buffer_to_buffer(
    *command_encoder,
    *state
      .resource_table
      .get_mut::<wgc::id::BufferId>(args.source)
      .ok_or_else(bad_resource_id)?,
    args.source_offset,
    *state
      .resource_table
      .get_mut::<wgc::id::BufferId>(args.destination)
      .ok_or_else(bad_resource_id)?,
    args.destination_offset,
    args.size,
  )?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GPUBufferCopyView {
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
pub struct GPUTextureCopyView {
  pub texture: u32,
  pub mip_level: Option<u32>,
  pub origin: Option<GPUOrigin3D>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderCopyBufferToTextureArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  source: GPUBufferCopyView,
  destination: GPUTextureCopyView,
  copy_size: super::texture::GPUExtent3D,
}

pub fn op_webgpu_command_encoder_copy_buffer_to_texture(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderCopyBufferToTextureArgs =
    serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  instance.command_encoder_copy_buffer_to_texture(
    *command_encoder,
    &wgc::command::BufferCopyView {
      buffer: *state
        .resource_table
        .get_mut::<wgc::id::BufferId>(args.source.buffer)
        .ok_or_else(bad_resource_id)?,
      layout: wgt::TextureDataLayout {
        offset: args.source.offset.unwrap_or(0),
        bytes_per_row: args.source.bytes_per_row, // TODO: default value?
        rows_per_image: args.source.rows_per_image, // TODO: default value?
      },
    },
    &wgc::command::TextureCopyView {
      texture: *state
        .resource_table
        .get_mut::<wgc::id::TextureId>(args.destination.texture)
        .ok_or_else(bad_resource_id)?,
      mip_level: args.destination.mip_level.unwrap_or(0),
      origin: args
        .destination
        .origin
        .map_or(Default::default(), |origin| wgt::Origin3d {
          x: origin.x.unwrap_or(0),
          y: origin.y.unwrap_or(0),
          z: origin.z.unwrap_or(0),
        }),
    },
    &wgt::Extent3d {
      width: args.copy_size.width,
      height: args.copy_size.height,
      depth: args.copy_size.depth,
    },
  )?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderCopyTextureToBufferArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  source: GPUTextureCopyView,
  destination: GPUBufferCopyView,
  copy_size: super::texture::GPUExtent3D,
}

pub fn op_webgpu_command_encoder_copy_texture_to_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CommandEncoderCopyTextureToBufferArgs =
    serde_json::from_value(args)?;

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  instance.command_encoder_copy_texture_to_buffer(
    *command_encoder,
    &wgc::command::TextureCopyView {
      texture: *state
        .resource_table
        .get_mut::<wgc::id::TextureId>(args.source.texture)
        .ok_or_else(bad_resource_id)?,
      mip_level: args.source.mip_level.unwrap_or(0),
      origin: args.source.origin.map_or(Default::default(), |origin| {
        wgt::Origin3d {
          x: origin.x.unwrap_or(0),
          y: origin.y.unwrap_or(0),
          z: origin.z.unwrap_or(0),
        }
      }),
    },
    &wgc::command::BufferCopyView {
      buffer: *state
        .resource_table
        .get_mut::<wgc::id::BufferId>(args.destination.buffer)
        .ok_or_else(bad_resource_id)?,
      layout: wgt::TextureDataLayout {
        offset: args.destination.offset.unwrap_or(0),
        bytes_per_row: args.destination.bytes_per_row, // TODO: default value?
        rows_per_image: args.destination.rows_per_image, // TODO: default value?
      },
    },
    &wgt::Extent3d {
      width: args.copy_size.width,
      height: args.copy_size.height,
      depth: args.copy_size.depth,
    },
  )?;

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandEncoderCopyTextureToTextureArgs {
  instance_rid: u32,
  command_encoder_rid: u32,
  source: GPUTextureCopyView,
  destination: GPUTextureCopyView,
  copy_size: super::texture::GPUExtent3D,
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
      origin: args.source.origin.map_or(Default::default(), |origin| {
        wgt::Origin3d {
          x: origin.x.unwrap_or(0),
          y: origin.y.unwrap_or(0),
          z: origin.z.unwrap_or(0),
        }
      }),
    },
    &wgc::command::TextureCopyView {
      texture: *state
        .resource_table
        .get_mut::<wgc::id::TextureId>(args.destination.texture)
        .ok_or_else(bad_resource_id)?,
      mip_level: args.destination.mip_level.unwrap_or(0),
      origin: args
        .destination
        .origin
        .map_or(Default::default(), |origin| wgt::Origin3d {
          x: origin.x.unwrap_or(0),
          y: origin.y.unwrap_or(0),
          z: origin.z.unwrap_or(0),
        }),
    },
    &wgt::Extent3d {
      width: args.copy_size.width,
      height: args.copy_size.height,
      depth: args.copy_size.depth,
    },
  )?;

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

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  instance
    .command_encoder_push_debug_group(*command_encoder, &args.group_label)?;

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

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  instance.command_encoder_pop_debug_group(*command_encoder)?;

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

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let command_encoder = state
    .resource_table
    .get_mut::<wgc::id::CommandEncoderId>(args.command_encoder_rid)
    .ok_or_else(bad_resource_id)?;

  instance.command_encoder_insert_debug_marker(
    *command_encoder,
    &args.marker_label,
  )?;

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
