// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::WebGpuQuerySet;
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

pub(crate) struct WebGpuCommandEncoder(
  pub(crate) super::Instance,
  pub(crate) wgpu_core::id::CommandEncoderId, // TODO: should maybe be option?
);
impl Resource for WebGpuCommandEncoder {
  fn name(&self) -> Cow<str> {
    "webGPUCommandEncoder".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.command_encoder_drop(self.1));
  }
}

pub(crate) struct WebGpuCommandBuffer(
  pub(crate) super::Instance,
  pub(crate) RefCell<Option<wgpu_core::id::CommandBufferId>>,
);
impl Resource for WebGpuCommandBuffer {
  fn name(&self) -> Cow<str> {
    "webGPUCommandBuffer".into()
  }

  fn close(self: Rc<Self>) {
    if let Some(id) = *self.1.borrow() {
      gfx_select!(id => self.0.command_buffer_drop(id));
    }
  }
}

#[op2]
#[serde]
pub fn op_webgpu_create_command_encoder(
  state: &mut OpState,
  #[smi] device_rid: ResourceId,
  #[string] label: Cow<str>,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.1;

  let descriptor = wgpu_types::CommandEncoderDescriptor { label: Some(label) };

  gfx_put!(device => instance.device_create_command_encoder(
    device,
    &descriptor,
    None
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GPURenderPassTimestampWrites {
  query_set: ResourceId,
  beginning_of_pass_write_index: Option<u32>,
  end_of_pass_write_index: Option<u32>,
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_begin_render_pass(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[string] label: Cow<str>,
  #[serde] color_attachments: Vec<Option<GpuRenderPassColorAttachment>>,
  #[serde] depth_stencil_attachment: Option<
    GpuRenderPassDepthStencilAttachment,
  >,
  #[smi] occlusion_query_set: Option<ResourceId>,
  #[serde] timestamp_writes: Option<GPURenderPassTimestampWrites>,
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
          .map(|texture| texture.1);

        Some(wgpu_core::command::RenderPassColorAttachment {
          view: texture_view_resource.1,
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
        view: texture_view_resource.1,
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

  let timestamp_writes = if let Some(timestamp_writes) = timestamp_writes {
    let query_set_resource = state
      .resource_table
      .get::<WebGpuQuerySet>(timestamp_writes.query_set)?;
    let query_set = query_set_resource.1;

    Some(wgpu_core::command::RenderPassTimestampWrites {
      query_set,
      beginning_of_pass_write_index: timestamp_writes
        .beginning_of_pass_write_index,
      end_of_pass_write_index: timestamp_writes.end_of_pass_write_index,
    })
  } else {
    None
  };

  let occlusion_query_set_resource = occlusion_query_set
    .map(|rid| state.resource_table.get::<WebGpuQuerySet>(rid))
    .transpose()?
    .map(|query_set| query_set.1);

  let descriptor = wgpu_core::command::RenderPassDescriptor {
    label: Some(label),
    color_attachments: Cow::from(color_attachments),
    depth_stencil_attachment: processed_depth_stencil_attachment.as_ref(),
    timestamp_writes: timestamp_writes.as_ref(),
    occlusion_query_set: occlusion_query_set_resource,
  };

  let render_pass = wgpu_core::command::RenderPass::new(
    command_encoder_resource.1,
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
pub struct GPUComputePassTimestampWrites {
  query_set: ResourceId,
  beginning_of_pass_write_index: Option<u32>,
  end_of_pass_write_index: Option<u32>,
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_begin_compute_pass(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[string] label: Cow<str>,
  #[serde] timestamp_writes: Option<GPUComputePassTimestampWrites>,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;

  let timestamp_writes = if let Some(timestamp_writes) = timestamp_writes {
    let query_set_resource = state
      .resource_table
      .get::<WebGpuQuerySet>(timestamp_writes.query_set)?;
    let query_set = query_set_resource.1;

    Some(wgpu_core::command::ComputePassTimestampWrites {
      query_set,
      beginning_of_pass_write_index: timestamp_writes
        .beginning_of_pass_write_index,
      end_of_pass_write_index: timestamp_writes.end_of_pass_write_index,
    })
  } else {
    None
  };

  let descriptor = wgpu_core::command::ComputePassDescriptor {
    label: Some(label),
    timestamp_writes: timestamp_writes.as_ref(),
  };

  let compute_pass = wgpu_core::command::ComputePass::new(
    command_encoder_resource.1,
    &descriptor,
  );

  let rid = state
    .resource_table
    .add(super::compute_pass::WebGpuComputePass(RefCell::new(
      compute_pass,
    )));

  Ok(WebGpuResult::rid(rid))
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_copy_buffer_to_buffer(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[smi] source: ResourceId,
  #[number] source_offset: u64,
  #[smi] destination: ResourceId,
  #[number] destination_offset: u64,
  #[number] size: u64,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;
  let source_buffer_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(source)?;
  let source_buffer = source_buffer_resource.1;
  let destination_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(destination)?;
  let destination_buffer = destination_buffer_resource.1;

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

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_copy_buffer_to_texture(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[serde] source: GpuImageCopyBuffer,
  #[serde] destination: GpuImageCopyTexture,
  #[serde] copy_size: wgpu_types::Extent3d,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;
  let source_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(source.buffer)?;
  let destination_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(destination.texture)?;

  let source = wgpu_core::command::ImageCopyBuffer {
    buffer: source_buffer_resource.1,
    layout: wgpu_types::ImageDataLayout {
      offset: source.offset,
      bytes_per_row: source.bytes_per_row,
      rows_per_image: source.rows_per_image,
    },
  };
  let destination = wgpu_core::command::ImageCopyTexture {
    texture: destination_texture_resource.id,
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

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_copy_texture_to_buffer(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[serde] source: GpuImageCopyTexture,
  #[serde] destination: GpuImageCopyBuffer,
  #[serde] copy_size: wgpu_types::Extent3d,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;
  let source_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(source.texture)?;
  let destination_buffer_resource =
    state
      .resource_table
      .get::<super::buffer::WebGpuBuffer>(destination.buffer)?;

  let source = wgpu_core::command::ImageCopyTexture {
    texture: source_texture_resource.id,
    mip_level: source.mip_level,
    origin: source.origin,
    aspect: source.aspect,
  };
  let destination = wgpu_core::command::ImageCopyBuffer {
    buffer: destination_buffer_resource.1,
    layout: wgpu_types::ImageDataLayout {
      offset: destination.offset,
      bytes_per_row: destination.bytes_per_row,
      rows_per_image: destination.rows_per_image,
    },
  };
  gfx_ok!(command_encoder => instance.command_encoder_copy_texture_to_buffer(
    command_encoder,
    &source,
    &destination,
    &copy_size
  ))
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_copy_texture_to_texture(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[serde] source: GpuImageCopyTexture,
  #[serde] destination: GpuImageCopyTexture,
  #[serde] copy_size: wgpu_types::Extent3d,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;
  let source_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(source.texture)?;
  let destination_texture_resource =
    state
      .resource_table
      .get::<super::texture::WebGpuTexture>(destination.texture)?;

  let source = wgpu_core::command::ImageCopyTexture {
    texture: source_texture_resource.id,
    mip_level: source.mip_level,
    origin: source.origin,
    aspect: source.aspect,
  };
  let destination = wgpu_core::command::ImageCopyTexture {
    texture: destination_texture_resource.id,
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

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_clear_buffer(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[smi] buffer_rid: ResourceId,
  #[number] offset: u64,
  #[number] size: u64,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;
  let destination_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(buffer_rid)?;

  gfx_ok!(command_encoder => instance.command_encoder_clear_buffer(
    command_encoder,
    destination_resource.1,
    offset,
    Some(size)
  ))
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_push_debug_group(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[string] group_label: &str,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;

  gfx_ok!(command_encoder => instance.command_encoder_push_debug_group(command_encoder, group_label))
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_pop_debug_group(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;

  gfx_ok!(command_encoder => instance.command_encoder_pop_debug_group(command_encoder))
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_insert_debug_marker(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[string] marker_label: &str,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;

  gfx_ok!(command_encoder => instance.command_encoder_insert_debug_marker(
    command_encoder,
    marker_label
  ))
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_write_timestamp(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[smi] query_set: ResourceId,
  query_index: u32,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(query_set)?;

  gfx_ok!(command_encoder => instance.command_encoder_write_timestamp(
    command_encoder,
    query_set_resource.1,
    query_index
  ))
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_resolve_query_set(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[smi] query_set: ResourceId,
  first_query: u32,
  query_count: u32,
  #[smi] destination: ResourceId,
  #[number] destination_offset: u64,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let command_encoder_resource = state
    .resource_table
    .get::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;
  let query_set_resource = state
    .resource_table
    .get::<super::WebGpuQuerySet>(query_set)?;
  let destination_resource = state
    .resource_table
    .get::<super::buffer::WebGpuBuffer>(destination)?;

  gfx_ok!(command_encoder => instance.command_encoder_resolve_query_set(
    command_encoder,
    query_set_resource.1,
    first_query,
    query_count,
    destination_resource.1,
    destination_offset
  ))
}

#[op2]
#[serde]
pub fn op_webgpu_command_encoder_finish(
  state: &mut OpState,
  #[smi] command_encoder_rid: ResourceId,
  #[string] label: Cow<str>,
) -> Result<WebGpuResult, AnyError> {
  let command_encoder_resource = state
    .resource_table
    .take::<WebGpuCommandEncoder>(command_encoder_rid)?;
  let command_encoder = command_encoder_resource.1;
  let instance = state.borrow::<super::Instance>();

  let descriptor = wgpu_types::CommandBufferDescriptor { label: Some(label) };

  let (val, maybe_err) = gfx_select!(command_encoder => instance.command_encoder_finish(
    command_encoder,
    &descriptor
  ));

  let rid = state.resource_table.add(WebGpuCommandBuffer(
    instance.clone(),
    RefCell::new(Some(val)),
  ));

  Ok(WebGpuResult::rid_err(rid, maybe_err))
}
