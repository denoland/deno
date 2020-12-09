// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::texture::serialize_texture_format;
use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::borrow::Cow;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRenderBundleEncoderArgs {
  device_rid: u32,
  label: Option<String>,
  color_formats: Vec<String>,
  depth_stencil_format: Option<String>,
  sample_count: Option<u32>,
}

pub fn op_webgpu_create_render_bundle_encoder(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateRenderBundleEncoderArgs = serde_json::from_value(args)?;

  let device = state
    .resource_table
    .get_mut::<wgc::id::DeviceId>(args.device_rid)
    .ok_or_else(bad_resource_id)?;

  let mut color_formats = vec![];

  for format in &args.color_formats {
    color_formats.push(serialize_texture_format(format.clone())?);
  }

  let descriptor = wgc::command::RenderBundleEncoderDescriptor {
    label: args.label.map(Cow::Owned),
    color_formats: Cow::Owned(color_formats),
    depth_stencil_format: args
      .depth_stencil_format
      .map(serialize_texture_format)
      .transpose()?,
    sample_count: args.sample_count.unwrap_or(1),
  };
  let render_bundle_encoder =
    wgc::command::RenderBundleEncoder::new(&descriptor, *device, None)?;

  let rid = state
    .resource_table
    .add("webGPURenderBundleEncoder", Box::new(render_bundle_encoder));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderFinishArgs {
  instance_rid: u32,
  render_bundle_encoder_rid: u32,
  label: Option<String>,
}

pub fn op_webgpu_render_bundle_encoder_finish(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderFinishArgs = serde_json::from_value(args)?;

  let render_bundle_encoder = *state
    .resource_table
    .remove::<wgc::command::RenderBundleEncoder>(args.render_bundle_encoder_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state
    .resource_table
    .get::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  let render_bundle = wgc::gfx_select!(render_bundle_encoder.parent() => instance.render_bundle_encoder_finish(
    render_bundle_encoder,
    &wgc::command::RenderBundleDescriptor {
      label: args.label.map(Cow::Owned),
    },
    std::marker::PhantomData
  ))?;

  let rid = state
    .resource_table
    .add("webGPURenderBundle", Box::new(render_bundle));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderSetBindGroupArgs {
  render_bundle_encoder_rid: u32,
  index: u32,
  bind_group: u32,
  dynamic_offsets_data: Option<Vec<u32>>,
  dynamic_offsets_data_start: usize,
  dynamic_offsets_data_length: usize,
}

pub fn op_webgpu_render_bundle_encoder_set_bind_group(
  state: &mut OpState,
  args: Value,
  zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderSetBindGroupArgs = serde_json::from_value(args)?;

  let bind_group_id = *state
    .resource_table
    .get::<wgc::id::BindGroupId>(args.bind_group)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgc::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
      render_bundle_encoder,
      args.index,
      bind_group_id,
      match args.dynamic_offsets_data {
        Some(data) => data.as_ptr(),
        None => {
          let (prefix, data, suffix) = zero_copy[0].align_to::<u32>();
          assert!(prefix.is_empty());
          assert!(suffix.is_empty());
          data[args.dynamic_offsets_data_start..].as_ptr()
        }
      },
      args.dynamic_offsets_data_length,
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderPushDebugGroupArgs {
  render_bundle_encoder_rid: u32,
  group_label: String,
}

pub fn op_webgpu_render_bundle_encoder_push_debug_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderPushDebugGroupArgs =
    serde_json::from_value(args)?;

  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.group_label).unwrap();
    wgc::command::bundle_ffi::wgpu_render_bundle_push_debug_group(
      render_bundle_encoder,
      label.as_ptr(),
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderPopDebugGroupArgs {
  render_bundle_encoder_rid: u32,
}

pub fn op_webgpu_render_bundle_encoder_pop_debug_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderPopDebugGroupArgs =
    serde_json::from_value(args)?;

  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  unsafe {
    wgc::command::bundle_ffi::wgpu_render_bundle_pop_debug_group(
      render_bundle_encoder,
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderInsertDebugMarkerArgs {
  render_bundle_encoder_rid: u32,
  marker_label: String,
}

pub fn op_webgpu_render_bundle_encoder_insert_debug_marker(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderInsertDebugMarkerArgs =
    serde_json::from_value(args)?;

  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  unsafe {
    let label = std::ffi::CString::new(args.marker_label).unwrap();
    wgc::command::bundle_ffi::wgpu_render_bundle_insert_debug_marker(
      render_bundle_encoder,
      label.as_ptr(),
    );
  }

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderSetPipelineArgs {
  render_bundle_encoder_rid: u32,
  pipeline: u32,
}

pub fn op_webgpu_render_bundle_encoder_set_pipeline(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderSetPipelineArgs = serde_json::from_value(args)?;

  let pipeline_id = *state
    .resource_table
    .get::<wgc::id::RenderPipelineId>(args.pipeline)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  wgc::command::bundle_ffi::wgpu_render_bundle_set_pipeline(
    render_bundle_encoder,
    pipeline_id,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderSetIndexBufferArgs {
  render_bundle_encoder_rid: u32,
  buffer: u32,
  _index_format: String, // wgpu#978
  offset: u64,
  size: u64,
}

pub fn op_webgpu_render_bundle_encoder_set_index_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderSetIndexBufferArgs =
    serde_json::from_value(args)?;

  let buffer_id = *state
    .resource_table
    .get::<wgc::id::BufferId>(args.buffer)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  wgc::command::bundle_ffi::wgpu_render_bundle_set_index_buffer(
    render_bundle_encoder,
    buffer_id,
    args.offset,
    std::num::NonZeroU64::new(args.size),
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderSetVertexBufferArgs {
  render_bundle_encoder_rid: u32,
  slot: u32,
  buffer: u32,
  offset: u64,
  size: u64,
}

pub fn op_webgpu_render_bundle_encoder_set_vertex_buffer(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderSetVertexBufferArgs =
    serde_json::from_value(args)?;

  let buffer_id = *state
    .resource_table
    .get::<wgc::id::BufferId>(args.buffer)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  wgc::command::bundle_ffi::wgpu_render_bundle_set_vertex_buffer(
    render_bundle_encoder,
    args.slot,
    buffer_id,
    args.offset,
    std::num::NonZeroU64::new(args.size),
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderDrawArgs {
  render_bundle_encoder_rid: u32,
  vertex_count: u32,
  instance_count: u32,
  first_vertex: u32,
  first_instance: u32,
}

pub fn op_webgpu_render_bundle_encoder_draw(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderDrawArgs = serde_json::from_value(args)?;

  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  wgc::command::bundle_ffi::wgpu_render_bundle_draw(
    render_bundle_encoder,
    args.vertex_count,
    args.instance_count,
    args.first_vertex,
    args.first_instance,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderDrawIndexedArgs {
  render_bundle_encoder_rid: u32,
  index_count: u32,
  instance_count: u32,
  first_index: u32,
  base_vertex: i32,
  first_instance: u32,
}

pub fn op_webgpu_render_bundle_encoder_draw_indexed(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderDrawIndexedArgs = serde_json::from_value(args)?;

  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  wgc::command::bundle_ffi::wgpu_render_bundle_draw_indexed(
    render_bundle_encoder,
    args.index_count,
    args.instance_count,
    args.first_index,
    args.base_vertex,
    args.first_instance,
  );

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderBundleEncoderDrawIndirectArgs {
  render_bundle_encoder_rid: u32,
  indirect_buffer: u32,
  indirect_offset: u64,
}

pub fn op_webgpu_render_bundle_encoder_draw_indirect(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderDrawIndirectArgs = serde_json::from_value(args)?;

  let buffer_id = *state
    .resource_table
    .get::<wgc::id::BufferId>(args.indirect_buffer)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  wgc::command::bundle_ffi::wgpu_render_bundle_draw_indirect(
    render_bundle_encoder,
    buffer_id,
    args.indirect_offset,
  );

  Ok(json!({}))
}
