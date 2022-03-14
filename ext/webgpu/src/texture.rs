// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ResourceId;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

use super::error::WebGpuResult;
pub(crate) struct WebGpuTexture(pub(crate) wgpu_core::id::TextureId);
impl Resource for WebGpuTexture {
  fn name(&self) -> Cow<str> {
    "webGPUTexture".into()
  }
}

pub(crate) struct WebGpuTextureView(pub(crate) wgpu_core::id::TextureViewId);
impl Resource for WebGpuTextureView {
  fn name(&self) -> Cow<str> {
    "webGPUTextureView".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTextureArgs {
  device_rid: ResourceId,
  label: Option<String>,
  size: wgpu_types::Extent3d,
  mip_level_count: u32,
  sample_count: u32,
  dimension: wgpu_types::TextureDimension,
  format: wgpu_types::TextureFormat,
  usage: u32,
}

#[op]
pub fn op_webgpu_create_texture(
  state: &mut OpState,
  args: CreateTextureArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let descriptor = wgpu_core::resource::TextureDescriptor {
    label: args.label.map(Cow::from),
    size: args.size,
    mip_level_count: args.mip_level_count,
    sample_count: args.sample_count,
    dimension: args.dimension,
    format: args.format,
    usage: wgpu_types::TextureUsages::from_bits_truncate(args.usage),
  };

  gfx_put!(device => instance.device_create_texture(
    device,
    &descriptor,
    std::marker::PhantomData
  ) => state, WebGpuTexture)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTextureViewArgs {
  texture_rid: ResourceId,
  label: Option<String>,
  format: Option<wgpu_types::TextureFormat>,
  dimension: Option<wgpu_types::TextureViewDimension>,
  aspect: wgpu_types::TextureAspect,
  base_mip_level: u32,
  mip_level_count: Option<u32>,
  base_array_layer: u32,
  array_layer_count: Option<u32>,
}

#[op]
pub fn op_webgpu_create_texture_view(
  state: &mut OpState,
  args: CreateTextureViewArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let texture_resource = state
    .resource_table
    .get::<WebGpuTexture>(args.texture_rid)?;
  let texture = texture_resource.0;

  let descriptor = wgpu_core::resource::TextureViewDescriptor {
    label: args.label.map(Cow::from),
    format: args.format,
    dimension: args.dimension,
    range: wgpu_types::ImageSubresourceRange {
      aspect: args.aspect,
      base_mip_level: args.base_mip_level,
      mip_level_count: std::num::NonZeroU32::new(
        args.mip_level_count.unwrap_or(0),
      ),
      base_array_layer: args.base_array_layer,
      array_layer_count: std::num::NonZeroU32::new(
        args.array_layer_count.unwrap_or(0),
      ),
    },
  };

  gfx_put!(texture => instance.texture_create_view(
    texture,
    &descriptor,
    std::marker::PhantomData
  ) => state, WebGpuTextureView)
}
