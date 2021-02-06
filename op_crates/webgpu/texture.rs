// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{serde_json, ZeroCopyBuf};
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

pub(crate) struct WebGPUTexture(pub(crate) wgc::id::TextureId);
impl Resource for WebGPUTexture {
  fn name(&self) -> Cow<str> {
    "webGPUTexture".into()
  }
}

pub(crate) struct WebGPUTextureView(pub(crate) wgc::id::TextureViewId);
impl Resource for WebGPUTextureView {
  fn name(&self) -> Cow<str> {
    "webGPUTextureView".into()
  }
}

pub fn serialize_texture_format(
  format: String,
) -> Result<wgt::TextureFormat, AnyError> {
  Ok(match format.as_str() {
    // 8-bit formats
    "r8unorm" => wgt::TextureFormat::R8Unorm,
    "r8snorm" => wgt::TextureFormat::R8Snorm,
    "r8uint" => wgt::TextureFormat::R8Uint,
    "r8sint" => wgt::TextureFormat::R8Sint,

    // 16-bit formats
    "r16uint" => wgt::TextureFormat::R16Uint,
    "r16sint" => wgt::TextureFormat::R16Sint,
    "r16float" => wgt::TextureFormat::R16Float,
    "rg8unorm" => wgt::TextureFormat::Rg8Unorm,
    "rg8snorm" => wgt::TextureFormat::Rg8Snorm,
    "rg8uint" => wgt::TextureFormat::Rg8Uint,
    "rg8sint" => wgt::TextureFormat::Rg8Sint,

    // 32-bit formats
    "r32uint" => wgt::TextureFormat::R32Uint,
    "r32sint" => wgt::TextureFormat::R32Sint,
    "r32float" => wgt::TextureFormat::R32Float,
    "rg16uint" => wgt::TextureFormat::Rg16Uint,
    "rg16sint" => wgt::TextureFormat::Rg16Sint,
    "rg16float" => wgt::TextureFormat::Rg16Float,
    "rgba8unorm" => wgt::TextureFormat::Rgba8Unorm,
    "rgba8unorm-srgb" => wgt::TextureFormat::Rgba8UnormSrgb,
    "rgba8snorm" => wgt::TextureFormat::Rgba8Snorm,
    "rgba8uint" => wgt::TextureFormat::Rgba8Uint,
    "rgba8sint" => wgt::TextureFormat::Rgba8Sint,
    "bgra8unorm" => wgt::TextureFormat::Bgra8Unorm,
    "bgra8unorm-srgb" => wgt::TextureFormat::Bgra8UnormSrgb,
    // Packed 32-bit formats
    "rgb9e5ufloat" => return Err(not_supported()), // wgpu#967
    "rgb10a2unorm" => wgt::TextureFormat::Rgb10a2Unorm,
    "rg11b10ufloat" => wgt::TextureFormat::Rg11b10Float,

    // 64-bit formats
    "rg32uint" => wgt::TextureFormat::Rg32Uint,
    "rg32sint" => wgt::TextureFormat::Rg32Sint,
    "rg32float" => wgt::TextureFormat::Rg32Float,
    "rgba16uint" => wgt::TextureFormat::Rgba16Uint,
    "rgba16sint" => wgt::TextureFormat::Rgba16Sint,
    "rgba16float" => wgt::TextureFormat::Rgba16Float,

    // 128-bit formats
    "rgba32uint" => wgt::TextureFormat::Rgba32Uint,
    "rgba32sint" => wgt::TextureFormat::Rgba32Sint,
    "rgba32float" => wgt::TextureFormat::Rgba32Float,

    // Depth and stencil formats
    "stencil8" => return Err(not_supported()), // wgpu#967
    "depth16unorm" => return Err(not_supported()), // wgpu#967
    "depth24plus" => wgt::TextureFormat::Depth24Plus,
    "depth24plus-stencil8" => wgt::TextureFormat::Depth24PlusStencil8,
    "depth32float" => wgt::TextureFormat::Depth32Float,

    // BC compressed formats usable if "texture-compression-bc" is both
    // supported by the device/user agent and enabled in requestDevice.
    "bc1-rgba-unorm" => wgt::TextureFormat::Bc1RgbaUnorm,
    "bc1-rgba-unorm-srgb" => wgt::TextureFormat::Bc1RgbaUnormSrgb,
    "bc2-rgba-unorm" => wgt::TextureFormat::Bc2RgbaUnorm,
    "bc2-rgba-unorm-srgb" => wgt::TextureFormat::Bc2RgbaUnormSrgb,
    "bc3-rgba-unorm" => wgt::TextureFormat::Bc3RgbaUnorm,
    "bc3-rgba-unorm-srgb" => wgt::TextureFormat::Bc3RgbaUnormSrgb,
    "bc4-r-unorm" => wgt::TextureFormat::Bc4RUnorm,
    "bc4-r-snorm" => wgt::TextureFormat::Bc4RSnorm,
    "bc5-rg-unorm" => wgt::TextureFormat::Bc5RgUnorm,
    "bc5-rg-snorm" => wgt::TextureFormat::Bc5RgSnorm,
    "bc6h-rgb-ufloat" => wgt::TextureFormat::Bc6hRgbUfloat,
    "bc6h-rgb-float" => wgt::TextureFormat::Bc6hRgbSfloat, // wgpu#967
    "bc7-rgba-unorm" => wgt::TextureFormat::Bc7RgbaUnorm,
    "bc7-rgba-unorm-srgb" => wgt::TextureFormat::Bc7RgbaUnormSrgb,

    // "depth24unorm-stencil8" extension
    "depth24unorm-stencil8" => return Err(not_supported()), // wgpu#967

    // "depth32float-stencil8" extension
    "depth32float-stencil8" => return Err(not_supported()), // wgpu#967
    _ => unreachable!(),
  })
}

pub fn serialize_dimension(dimension: String) -> wgt::TextureViewDimension {
  match dimension.as_str() {
    "1d" => wgt::TextureViewDimension::D1,
    "2d" => wgt::TextureViewDimension::D2,
    "2d-array" => wgt::TextureViewDimension::D2Array,
    "cube" => wgt::TextureViewDimension::Cube,
    "cube-array" => wgt::TextureViewDimension::CubeArray,
    "3d" => wgt::TextureViewDimension::D3,
    _ => unreachable!(),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GPUExtent3D {
  pub width: u32,
  pub height: u32,
  pub depth: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTextureArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  size: GPUExtent3D,
  mip_level_count: Option<u32>,
  sample_count: Option<u32>,
  dimension: Option<String>,
  format: String,
  usage: u32,
}

pub fn op_webgpu_create_texture(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateTextureArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let ref instance = instance_resource.0;

  let descriptor = wgc::resource::TextureDescriptor {
    label: args.label.map(Cow::Owned),
    size: wgt::Extent3d {
      width: args.size.width,
      height: args.size.height,
      depth: args.size.depth,
    },
    mip_level_count: args.mip_level_count.unwrap_or(1),
    sample_count: args.sample_count.unwrap_or(1),
    dimension: match args.dimension {
      Some(dimension) => match dimension.as_str() {
        "1d" => wgt::TextureDimension::D1,
        "2d" => wgt::TextureDimension::D2,
        "3d" => wgt::TextureDimension::D3,
        _ => unreachable!(),
      },
      None => wgt::TextureDimension::D2,
    },
    format: serialize_texture_format(args.format)?,
    usage: wgt::TextureUsage::from_bits(args.usage).unwrap(),
  };

  let texture = wgc::gfx_select!(device => instance.device_create_texture(
    device,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state.resource_table.add(WebGPUTexture(texture));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTextureViewArgs {
  instance_rid: u32,
  texture_rid: u32,
  label: Option<String>,
  format: Option<String>,
  dimension: Option<String>,
  aspect: Option<String>,
  base_mip_level: Option<u32>,
  mip_level_count: Option<u32>,
  base_array_layer: Option<u32>,
  array_layer_count: Option<u32>,
}

pub fn op_webgpu_create_texture_view(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateTextureViewArgs = serde_json::from_value(args)?;

  let texture_resource = state
    .resource_table
    .get::<WebGPUTexture>(args.texture_rid)
    .ok_or_else(bad_resource_id)?;
  let texture = texture_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let ref instance = instance_resource.0;

  let descriptor = wgc::resource::TextureViewDescriptor {
    label: args.label.map(Cow::Owned),
    format: args.format.map(serialize_texture_format).transpose()?,
    dimension: args.dimension.map(serialize_dimension),
    aspect: match args.aspect {
      Some(aspect) => match aspect.as_str() {
        "all" => wgt::TextureAspect::All,
        "stencil-only" => wgt::TextureAspect::StencilOnly,
        "depth-only" => wgt::TextureAspect::DepthOnly,
        _ => unreachable!(),
      },
      None => wgt::TextureAspect::All,
    },
    base_mip_level: args.base_mip_level.unwrap_or(0),
    level_count: std::num::NonZeroU32::new(args.mip_level_count.unwrap_or(0)),
    base_array_layer: args.base_array_layer.unwrap_or(0),
    array_layer_count: std::num::NonZeroU32::new(
      args.array_layer_count.unwrap_or(0),
    ),
  };
  let texture_view = wgc::gfx_select!(texture => instance.texture_create_view(
    texture,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state.resource_table.add(WebGPUTextureView(texture_view));

  Ok(json!({
    "rid": rid,
  }))
}
