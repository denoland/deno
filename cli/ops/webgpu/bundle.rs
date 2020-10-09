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
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRenderBundleEncoderArgs {
  rid: u32,
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
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let render_bundle_encoder =
    device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
      label: args.label.map(|label| &label),
      color_formats: &args
        .color_formats
        .iter()
        .map(|format| serialize_texture_format(format.clone())?)
        .collect::<[wgpu::TextureFormat]>(),
      depth_stencil_format: args
        .depth_stencil_format
        .map(|format| serialize_texture_format(format)?),
      sample_count: args.sample_count.unwrap_or(1),
    });

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
  rid: u32,
  label: Option<String>,
}

pub fn op_webgpu_render_bundle_encoder_finish(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderBundleEncoderFinishArgs = serde_json::from_value(args)?;

  let render_bundle_encoder = state
    .resource_table
    .get_mut::<wgpu::RenderBundleEncoder>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let render_bundle =
    render_bundle_encoder.finish(&wgpu::RenderBundleDescriptor {
      label: args.label.map(|label| &label),
    });

  let rid = state
    .resource_table
    .add("webGPURenderBundle", Box::new(render_bundle));

  Ok(json!({
    "rid": rid,
  }))
}
