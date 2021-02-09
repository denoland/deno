// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{serde_json, ZeroCopyBuf};
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

pub(crate) struct WebGPUTexture(pub(crate) wgpu_core::id::TextureId);
impl Resource for WebGPUTexture {
  fn name(&self) -> Cow<str> {
    "webGPUTexture".into()
  }
}

pub(crate) struct WebGPUTextureView(pub(crate) wgpu_core::id::TextureViewId);
impl Resource for WebGPUTextureView {
  fn name(&self) -> Cow<str> {
    "webGPUTextureView".into()
  }
}

pub fn serialize_texture_format(
  format: &str,
) -> Result<wgpu_types::TextureFormat, AnyError> {
  Ok(match format {
    // 8-bit formats
    "r8unorm" => wgpu_types::TextureFormat::R8Unorm,
    "r8snorm" => wgpu_types::TextureFormat::R8Snorm,
    "r8uint" => wgpu_types::TextureFormat::R8Uint,
    "r8sint" => wgpu_types::TextureFormat::R8Sint,

    // 16-bit formats
    "r16uint" => wgpu_types::TextureFormat::R16Uint,
    "r16sint" => wgpu_types::TextureFormat::R16Sint,
    "r16float" => wgpu_types::TextureFormat::R16Float,
    "rg8unorm" => wgpu_types::TextureFormat::Rg8Unorm,
    "rg8snorm" => wgpu_types::TextureFormat::Rg8Snorm,
    "rg8uint" => wgpu_types::TextureFormat::Rg8Uint,
    "rg8sint" => wgpu_types::TextureFormat::Rg8Sint,

    // 32-bit formats
    "r32uint" => wgpu_types::TextureFormat::R32Uint,
    "r32sint" => wgpu_types::TextureFormat::R32Sint,
    "r32float" => wgpu_types::TextureFormat::R32Float,
    "rg16uint" => wgpu_types::TextureFormat::Rg16Uint,
    "rg16sint" => wgpu_types::TextureFormat::Rg16Sint,
    "rg16float" => wgpu_types::TextureFormat::Rg16Float,
    "rgba8unorm" => wgpu_types::TextureFormat::Rgba8Unorm,
    "rgba8unorm-srgb" => wgpu_types::TextureFormat::Rgba8UnormSrgb,
    "rgba8snorm" => wgpu_types::TextureFormat::Rgba8Snorm,
    "rgba8uint" => wgpu_types::TextureFormat::Rgba8Uint,
    "rgba8sint" => wgpu_types::TextureFormat::Rgba8Sint,
    "bgra8unorm" => wgpu_types::TextureFormat::Bgra8Unorm,
    "bgra8unorm-srgb" => wgpu_types::TextureFormat::Bgra8UnormSrgb,
    // Packed 32-bit formats
    "rgb9e5ufloat" => return Err(not_supported()), // wgpu#967
    "rgb10a2unorm" => wgpu_types::TextureFormat::Rgb10a2Unorm,
    "rg11b10ufloat" => wgpu_types::TextureFormat::Rg11b10Float,

    // 64-bit formats
    "rg32uint" => wgpu_types::TextureFormat::Rg32Uint,
    "rg32sint" => wgpu_types::TextureFormat::Rg32Sint,
    "rg32float" => wgpu_types::TextureFormat::Rg32Float,
    "rgba16uint" => wgpu_types::TextureFormat::Rgba16Uint,
    "rgba16sint" => wgpu_types::TextureFormat::Rgba16Sint,
    "rgba16float" => wgpu_types::TextureFormat::Rgba16Float,

    // 128-bit formats
    "rgba32uint" => wgpu_types::TextureFormat::Rgba32Uint,
    "rgba32sint" => wgpu_types::TextureFormat::Rgba32Sint,
    "rgba32float" => wgpu_types::TextureFormat::Rgba32Float,

    // Depth and stencil formats
    "stencil8" => return Err(not_supported()), // wgpu#967
    "depth16unorm" => return Err(not_supported()), // wgpu#967
    "depth24plus" => wgpu_types::TextureFormat::Depth24Plus,
    "depth24plus-stencil8" => wgpu_types::TextureFormat::Depth24PlusStencil8,
    "depth32float" => wgpu_types::TextureFormat::Depth32Float,

    // BC compressed formats usable if "texture-compression-bc" is both
    // supported by the device/user agent and enabled in requestDevice.
    "bc1-rgba-unorm" => wgpu_types::TextureFormat::Bc1RgbaUnorm,
    "bc1-rgba-unorm-srgb" => wgpu_types::TextureFormat::Bc1RgbaUnormSrgb,
    "bc2-rgba-unorm" => wgpu_types::TextureFormat::Bc2RgbaUnorm,
    "bc2-rgba-unorm-srgb" => wgpu_types::TextureFormat::Bc2RgbaUnormSrgb,
    "bc3-rgba-unorm" => wgpu_types::TextureFormat::Bc3RgbaUnorm,
    "bc3-rgba-unorm-srgb" => wgpu_types::TextureFormat::Bc3RgbaUnormSrgb,
    "bc4-r-unorm" => wgpu_types::TextureFormat::Bc4RUnorm,
    "bc4-r-snorm" => wgpu_types::TextureFormat::Bc4RSnorm,
    "bc5-rg-unorm" => wgpu_types::TextureFormat::Bc5RgUnorm,
    "bc5-rg-snorm" => wgpu_types::TextureFormat::Bc5RgSnorm,
    "bc6h-rgb-ufloat" => wgpu_types::TextureFormat::Bc6hRgbUfloat,
    "bc6h-rgb-float" => wgpu_types::TextureFormat::Bc6hRgbSfloat, // wgpu#967
    "bc7-rgba-unorm" => wgpu_types::TextureFormat::Bc7RgbaUnorm,
    "bc7-rgba-unorm-srgb" => wgpu_types::TextureFormat::Bc7RgbaUnormSrgb,

    // "depth24unorm-stencil8" extension
    "depth24unorm-stencil8" => return Err(not_supported()), // wgpu#967

    // "depth32float-stencil8" extension
    "depth32float-stencil8" => return Err(not_supported()), // wgpu#967
    _ => unreachable!(),
  })
}

pub fn serialize_dimension(
  dimension: &str,
) -> wgpu_types::TextureViewDimension {
  match dimension {
    "1d" => wgpu_types::TextureViewDimension::D1,
    "2d" => wgpu_types::TextureViewDimension::D2,
    "2d-array" => wgpu_types::TextureViewDimension::D2Array,
    "cube" => wgpu_types::TextureViewDimension::Cube,
    "cube-array" => wgpu_types::TextureViewDimension::CubeArray,
    "3d" => wgpu_types::TextureViewDimension::D3,
    _ => unreachable!(),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GPUExtent3D {
  pub width: Option<u32>,
  pub height: Option<u32>,
  pub depth: Option<u32>,
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
  let instance = &instance_resource.0;

  let descriptor = wgpu_core::resource::TextureDescriptor {
    label: args.label.map(Cow::from),
    size: wgpu_types::Extent3d {
      width: args.size.width.unwrap_or(1),
      height: args.size.height.unwrap_or(1),
      depth: args.size.depth.unwrap_or(1),
    },
    mip_level_count: args.mip_level_count.unwrap_or(1),
    sample_count: args.sample_count.unwrap_or(1),
    dimension: match args.dimension {
      Some(dimension) => match dimension.as_str() {
        "1d" => wgpu_types::TextureDimension::D1,
        "2d" => wgpu_types::TextureDimension::D2,
        "3d" => wgpu_types::TextureDimension::D3,
        _ => unreachable!(),
      },
      None => wgpu_types::TextureDimension::D2,
    },
    format: serialize_texture_format(&args.format)?,
    usage: wgpu_types::TextureUsage::from_bits(args.usage).unwrap(),
  };

  let texture = gfx_select_err!(device => instance.device_create_texture(
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
  let instance = &instance_resource.0;

  let descriptor = wgpu_core::resource::TextureViewDescriptor {
    label: args.label.map(Cow::from),
    format: args
      .format
      .map(|s| serialize_texture_format(&s))
      .transpose()?,
    dimension: args.dimension.map(|s| serialize_dimension(&s)),
    aspect: match args.aspect {
      Some(aspect) => match aspect.as_str() {
        "all" => wgpu_types::TextureAspect::All,
        "stencil-only" => wgpu_types::TextureAspect::StencilOnly,
        "depth-only" => wgpu_types::TextureAspect::DepthOnly,
        _ => unreachable!(),
      },
      None => wgpu_types::TextureAspect::All,
    },
    base_mip_level: args.base_mip_level.unwrap_or(0),
    level_count: std::num::NonZeroU32::new(args.mip_level_count.unwrap_or(0)),
    base_array_layer: args.base_array_layer.unwrap_or(0),
    array_layer_count: std::num::NonZeroU32::new(
      args.array_layer_count.unwrap_or(0),
    ),
  };

  let texture_view = gfx_select_err!(texture => instance.texture_create_view(
    texture,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state.resource_table.add(WebGPUTextureView(texture_view));

  Ok(json!({
    "rid": rid,
  }))
}
