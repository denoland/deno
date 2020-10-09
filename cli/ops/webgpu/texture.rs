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

pub fn serialize_texture_format(
  format: String,
) -> Result<wgpu::TextureFormat, AnyError> {
  Ok(match format {
    // 8-bit formats
    &"r8unorm" => wgpu::TextureFormat::R8Unorm,
    &"r8snorm" => wgpu::TextureFormat::R8Snorm,
    &"r8uint" => wgpu::TextureFormat::R8Uint,
    &"r8sint" => wgpu::TextureFormat::R8Sint,

    // 16-bit formats
    &"r16uint" => wgpu::TextureFormat::R16Uint,
    &"r16sint" => wgpu::TextureFormat::R16Sint,
    &"r16float" => wgpu::TextureFormat::R16Float,
    &"rg8unorm" => wgpu::TextureFormat::Rg8Unorm,
    &"rg8snorm" => wgpu::TextureFormat::Rg8Snorm,
    &"rg8uint" => wgpu::TextureFormat::Rg8Uint,
    &"rg8sint" => wgpu::TextureFormat::Rg8Sint,

    // 32-bit formats
    &"r32uint" => wgpu::TextureFormat::R32Uint,
    &"r32sint" => wgpu::TextureFormat::R32Sint,
    &"r32float" => wgpu::TextureFormat::R32Float,
    &"rg16uint" => wgpu::TextureFormat::Rg16Uint,
    &"rg16sint" => wgpu::TextureFormat::Rg16Sint,
    &"rg16float" => wgpu::TextureFormat::Rg16Float,
    &"rgba8unorm" => wgpu::TextureFormat::Rgba8Unorm,
    &"rgba8unorm-srgb" => wgpu::TextureFormat::Rgba8UnormSrgb,
    &"rgba8snorm" => wgpu::TextureFormat::Rgba8Snorm,
    &"rgba8uint" => wgpu::TextureFormat::Rgba8Uint,
    &"rgba8sint" => wgpu::TextureFormat::Rgba8Sint,
    &"bgra8unorm" => wgpu::TextureFormat::Bgra8Unorm,
    &"bgra8unorm-srgb" => wgpu::TextureFormat::Bgra8UnormSrgb,
    // Packed 32-bit formats
    &"rgb9e5ufloat" => return Err(not_supported()), // wgpu-rs#590
    &"rgb10a2unorm" => wgpu::TextureFormat::Rgb10a2Unorm,
    &"rg11b10ufloat" => wgpu::TextureFormat::Rg11b10Float,

    // 64-bit formats
    &"rg32uint" => wgpu::TextureFormat::Rg32Uint,
    &"rg32sint" => wgpu::TextureFormat::Rg32Sint,
    &"rg32float" => wgpu::TextureFormat::Rg32Float,
    &"rgba16uint" => wgpu::TextureFormat::Rgba16Uint,
    &"rgba16sint" => wgpu::TextureFormat::Rgba16Sint,
    &"rgba16float" => wgpu::TextureFormat::Rgba16Float,

    // 128-bit formats
    &"rgba32uint" => wgpu::TextureFormat::Rgba32Uint,
    &"rgba32sint" => wgpu::TextureFormat::Rgba32Sint,
    &"rgba32float" => wgpu::TextureFormat::Rgba32Float,

    // Depth and stencil formats
    &"stencil8" => return Err(not_supported()), // wgpu-rs#590
    &"depth16unorm" => return Err(not_supported()), // wgpu-rs#590
    &"depth24plus" => wgpu::TextureFormat::Depth24Plus,
    &"depth24plus-stencil8" => wgpu::TextureFormat::Depth24PlusStencil8,
    &"depth32float" => wgpu::TextureFormat::Depth32Float,

    // BC compressed formats usable if "texture-compression-bc" is both
    // supported by the device/user agent and enabled in requestDevice.
    &"bc1-rgba-unorm" => wgpu::TextureFormat::Bc1RgbaUnorm,
    &"bc1-rgba-unorm-srgb" => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
    &"bc2-rgba-unorm" => wgpu::TextureFormat::Bc2RgbaUnorm,
    &"bc2-rgba-unorm-srgb" => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
    &"bc3-rgba-unorm" => wgpu::TextureFormat::Bc3RgbaUnorm,
    &"bc3-rgba-unorm-srgb" => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
    &"bc4-r-unorm" => wgpu::TextureFormat::Bc4RUnorm,
    &"bc4-r-snorm" => wgpu::TextureFormat::Bc4RSnorm,
    &"bc5-rg-unorm" => wgpu::TextureFormat::Bc5RgUnorm,
    &"bc5-rg-snorm" => wgpu::TextureFormat::Bc5RgSnorm,
    &"bc6h-rgb-ufloat" => wgpu::TextureFormat::Bc6hRgbUfloat,
    &"bc6h-rgb-float" => wgpu::TextureFormat::Bc6hRgbSfloat, // wgpu-rs#590
    &"bc7-rgba-unorm" => wgpu::TextureFormat::Bc7RgbaUnorm,
    &"bc7-rgba-unorm-srgb" => wgpu::TextureFormat::Bc7RgbaUnormSrgb,

    // "depth24unorm-stencil8" extension
    &"depth24unorm-stencil8" => return Err(not_supported()), // wgpu-rs#590

    // "depth32float-stencil8" extension
    &"depth32float-stencil8" => return Err(not_supported()), // wgpu-rs#590
    _ => unreachable!(),
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTextureArgs {
  rid: u32,
  label: Option<String>,
  mip_level_count: Option<u32>,
  sample_count: Option<u32>,
  dimension: Option<String>,
  format: String,
  usage: (), // TODO
}

pub fn op_webgpu_create_texture(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateTextureArgs = serde_json::from_value(args)?;

  let device = state
    .resource_table
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let texture = device.create_texture(&wgpu::TextureDescriptor {
    label: args.label.map(|label| &label),
    size: Default::default(), // TODO
    mip_level_count: args.mip_level_count.unwrap_or(1),
    sample_count: args.sample_count.unwrap_or(1),
    dimension: match args.dimension {
      Some(&"1d") => wgpu::TextureDimension::D1,
      Some(&"2d") => wgpu::TextureDimension::D2,
      Some(&"3d") => wgpu::TextureDimension::D3,
      Some(_) => unreachable!(),
      None => wgpu::TextureDimension::D2,
    },
    format: serialize_texture_format(args.format)?,
    usage: (), // TODO
  });

  let rid = state.resource_table.add("webGPUTexture", Box::new(texture));

  Ok(json!({
    "rid": rid,
  }))
}

pub fn serialize_dimension(dimension: String) -> wgpu::TextureViewDimension {
  match dimension {
    &"1d" => wgpu::TextureViewDimension::D1,
    &"2d" => wgpu::TextureViewDimension::D2,
    &"2d-array" => wgpu::TextureViewDimension::D2Array,
    &"cube" => wgpu::TextureViewDimension::Cube,
    &"cube-array" => wgpu::TextureViewDimension::CubeArray,
    &"3d" => wgpu::TextureViewDimension::D3,
    _ => unreachable!(),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTextureViewArgs {
  rid: u32,
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

  let texture = state
    .resource_table
    .get_mut::<wgpu::Texture>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
    label: args.label.map(|label| &label),
    format: args.format.map(|format| serialize_texture_format(format)?),
    dimension: args
      .dimension
      .map(|dimension| serialize_dimension(dimension)),
    aspect: match args.aspect {
      Some(&"all") => wgpu::TextureAspect::All,
      Some(&"stencil-only") => wgpu::TextureAspect::StencilOnly,
      Some(&"depth-only") => wgpu::TextureAspect::DepthOnly,
      Some(_) => unreachable!(),
      None => wgpu::TextureAspect::All,
    },
    base_mip_level: args.base_mip_level.unwrap_or(0),
    level_count: args.mip_level_count, // TODO
    base_array_layer: args.base_array_layer.unwrap_or(0),
    array_layer_count: args.array_layer_count, // TODO
  });

  let rid = state
    .resource_table
    .add("webGPUTextureView", Box::new(texture_view));

  Ok(json!({
    "rid": rid,
  }))
}
