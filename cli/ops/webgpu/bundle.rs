// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::texture::serialize_texture_format;
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
struct CreateRenderBundleEncoderArgs {
  device_rid: u32,
  label: Option<String>,
  color_formats: [String],
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

  let render_bundle_encoder = wgc::command::RenderBundleEncoder::new(
    &wgc::command::RenderBundleEncoderDescriptor {
      label: args.label.map(|label| Cow::Borrowed(&label)),
      color_formats: Cow::Owned(
        args
          .color_formats
          .iter()
          .map(|format| serialize_texture_format(format.clone())?)
          .collect::<Vec<wgt::TextureFormat>>(),
      ),
      depth_stencil_format: args
        .depth_stencil_format
        .map(|format| serialize_texture_format(format)?),
      sample_count: args.sample_count.unwrap_or(1),
    },
    *device,
    None, // TODO: check what this is
  )?;

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

  let instance = state
    .resource_table
    .get_mut::<super::WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  let render_bundle = instance.render_bundle_encoder_finish(
    render_bundle_encoder, // TODO
    &wgc::command::RenderBundleDescriptor {
      label: args.label.map(|label| Cow::Borrowed(&label)),
    },
    (), // TODO: id_in
  )?;

  let rid = state
    .resource_table
    .add("webGPURenderBundle", Box::new(render_bundle));

  Ok(json!({
    "rid": rid,
  }))
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
    wgc::command::bundle_ffi::wgpu_render_bundle_push_debug_group(
      render_bundle_encoder,
      std::ffi::CString::new(args.group_label).unwrap().as_ptr(),
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
    wgc::command::bundle_ffi::wgpu_render_bundle_insert_debug_marker(
      render_bundle_encoder,
      std::ffi::CString::new(args.marker_label).unwrap().as_ptr(),
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

  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgc::command::RenderBundleEncoder>(
      args.render_bundle_encoder_rid,
    )
    .ok_or_else(bad_resource_id)?;

  wgc::command::bundle_ffi::wgpu_render_bundle_set_pipeline(
    render_bundle_encoder,
    *state
      .resource_table
      .get_mut::<wgc::id::RenderPipelineId>(args.pipeline)
      .ok_or_else(bad_resource_id)?,
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
    render_pass,
    args.index_count,
    args.instance_count,
    args.first_index,
    args.base_vertex,
    args.first_instance,
  );

  Ok(json!({}))
}
