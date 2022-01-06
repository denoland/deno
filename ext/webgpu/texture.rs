// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::not_supported;
use deno_core::error::AnyError;
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
#[serde(rename_all = "kebab-case")]
pub enum GpuTextureFormat {
  // 8-bit formats
  #[serde(rename = "r8unorm")]
  R8Unorm,
  #[serde(rename = "r8snorm")]
  R8Snorm,
  #[serde(rename = "r8uint")]
  R8Uint,
  #[serde(rename = "r8sint")]
  R8Sint,

  // 16-bit formats
  #[serde(rename = "r16uint")]
  R16Uint,
  #[serde(rename = "r16sint")]
  R16Sint,
  #[serde(rename = "r16float")]
  R16Float,
  #[serde(rename = "rg8unorm")]
  Rg8Unorm,
  #[serde(rename = "rg8snorm")]
  Rg8Snorm,
  #[serde(rename = "rg8uint")]
  Rg8Uint,
  #[serde(rename = "rg8sint")]
  Rg8Sint,

  // 32-bit formats
  #[serde(rename = "r32uint")]
  R32Uint,
  #[serde(rename = "r32sint")]
  R32Sint,
  #[serde(rename = "r32float")]
  R32Float,
  #[serde(rename = "rg16uint")]
  Rg16Uint,
  #[serde(rename = "rg16sint")]
  Rg16Sint,
  #[serde(rename = "rg16float")]
  Rg16Float,
  #[serde(rename = "rgba8unorm")]
  Rgba8Unorm,
  #[serde(rename = "rgba8unorm-srgb")]
  Rgba8UnormSrgb,
  #[serde(rename = "rgba8snorm")]
  Rgba8Snorm,
  #[serde(rename = "rgba8uint")]
  Rgba8Uint,
  #[serde(rename = "rgba8sint")]
  Rgba8Sint,
  #[serde(rename = "bgra8unorm")]
  Bgra8Unorm,
  #[serde(rename = "bgra8unorm-srgb")]
  Bgra8UnormSrgb,
  // Packed 32-bit formats
  #[serde(rename = "rgb9e5ufloat")]
  RgB9E5UFloat,
  #[serde(rename = "rgb10a2unorm")]
  Rgb10a2Unorm,
  #[serde(rename = "rg11b10ufloat")]
  Rg11b10Float,

  // 64-bit formats
  #[serde(rename = "rg32uint")]
  Rg32Uint,
  #[serde(rename = "rg32sint")]
  Rg32Sint,
  #[serde(rename = "rg32float")]
  Rg32Float,
  #[serde(rename = "rgba16uint")]
  Rgba16Uint,
  #[serde(rename = "rgba16sint")]
  Rgba16Sint,
  #[serde(rename = "rgba16float")]
  Rgba16Float,

  // 128-bit formats
  #[serde(rename = "rgba32uint")]
  Rgba32Uint,
  #[serde(rename = "rgba32sint")]
  Rgba32Sint,
  #[serde(rename = "rgba32float")]
  Rgba32Float,

  // Depth and stencil formats
  #[serde(rename = "stencil8")]
  Stencil8,
  #[serde(rename = "depth16unorm")]
  Depth16Unorm,
  #[serde(rename = "depth24plus")]
  Depth24Plus,
  #[serde(rename = "depth24plus-stencil8")]
  Depth24PlusStencil8,
  #[serde(rename = "depth32float")]
  Depth32Float,

  // BC compressed formats usable if "texture-compression-bc" is both
  // supported by the device/user agent and enabled in requestDevice.
  #[serde(rename = "bc1-rgba-unorm")]
  Bc1RgbaUnorm,
  #[serde(rename = "bc1-rgba-unorm-srgb")]
  Bc1RgbaUnormSrgb,
  #[serde(rename = "bc2-rgba-unorm")]
  Bc2RgbaUnorm,
  #[serde(rename = "bc2-rgba-unorm-srgb")]
  Bc2RgbaUnormSrgb,
  #[serde(rename = "bc3-rgba-unorm")]
  Bc3RgbaUnorm,
  #[serde(rename = "bc3-rgba-unorm-srgb")]
  Bc3RgbaUnormSrgb,
  #[serde(rename = "bc4-r-unorm")]
  Bc4RUnorm,
  #[serde(rename = "bc4-r-snorm")]
  Bc4RSnorm,
  #[serde(rename = "bc5-rg-unorm")]
  Bc5RgUnorm,
  #[serde(rename = "bc5-rg-snorm")]
  Bc5RgSnorm,
  #[serde(rename = "bc6h-rgb-ufloat")]
  Bc6hRgbUfloat,
  #[serde(rename = "bc6h-rgb-float")]
  Bc6HRgbFloat,
  #[serde(rename = "bc7-rgba-unorm")]
  Bc7RgbaUnorm,
  #[serde(rename = "bc7-rgba-unorm-srgb")]
  Bc7RgbaUnormSrgb,

  // "depth24unorm-stencil8" feature
  #[serde(rename = "depth24unorm-stencil8")]
  Depth24UnormStencil8,

  // "depth32float-stencil8" feature
  #[serde(rename = "depth32float-stencil8")]
  Depth32FloatStencil8,
}

impl TryFrom<GpuTextureFormat> for wgpu_types::TextureFormat {
  type Error = AnyError;

  fn try_from(value: GpuTextureFormat) -> Result<Self, Self::Error> {
    use wgpu_types::TextureFormat;
    match value {
      GpuTextureFormat::R8Unorm => Ok(TextureFormat::R8Unorm),
      GpuTextureFormat::R8Snorm => Ok(TextureFormat::R8Snorm),
      GpuTextureFormat::R8Uint => Ok(TextureFormat::R8Uint),
      GpuTextureFormat::R8Sint => Ok(TextureFormat::R8Sint),

      GpuTextureFormat::R16Uint => Ok(TextureFormat::R16Uint),
      GpuTextureFormat::R16Sint => Ok(TextureFormat::R16Sint),
      GpuTextureFormat::R16Float => Ok(TextureFormat::R16Float),
      GpuTextureFormat::Rg8Unorm => Ok(TextureFormat::Rg8Unorm),
      GpuTextureFormat::Rg8Snorm => Ok(TextureFormat::Rg8Snorm),
      GpuTextureFormat::Rg8Uint => Ok(TextureFormat::Rg8Uint),
      GpuTextureFormat::Rg8Sint => Ok(TextureFormat::Rg8Sint),

      GpuTextureFormat::R32Uint => Ok(TextureFormat::R32Uint),
      GpuTextureFormat::R32Sint => Ok(TextureFormat::R32Sint),
      GpuTextureFormat::R32Float => Ok(TextureFormat::R32Float),
      GpuTextureFormat::Rg16Uint => Ok(TextureFormat::Rg16Uint),
      GpuTextureFormat::Rg16Sint => Ok(TextureFormat::Rg16Sint),
      GpuTextureFormat::Rg16Float => Ok(TextureFormat::Rg16Float),
      GpuTextureFormat::Rgba8Unorm => Ok(TextureFormat::Rgba8Unorm),
      GpuTextureFormat::Rgba8UnormSrgb => Ok(TextureFormat::Rgba8UnormSrgb),
      GpuTextureFormat::Rgba8Snorm => Ok(TextureFormat::Rgba8Snorm),
      GpuTextureFormat::Rgba8Uint => Ok(TextureFormat::Rgba8Uint),
      GpuTextureFormat::Rgba8Sint => Ok(TextureFormat::Rgba8Sint),
      GpuTextureFormat::Bgra8Unorm => Ok(TextureFormat::Bgra8Unorm),
      GpuTextureFormat::Bgra8UnormSrgb => Ok(TextureFormat::Bgra8UnormSrgb),
      GpuTextureFormat::RgB9E5UFloat => Err(not_supported()), // wgpu#967
      GpuTextureFormat::Rgb10a2Unorm => Ok(TextureFormat::Rgb10a2Unorm),
      GpuTextureFormat::Rg11b10Float => Ok(TextureFormat::Rg11b10Float),

      GpuTextureFormat::Rg32Uint => Ok(TextureFormat::Rg32Uint),
      GpuTextureFormat::Rg32Sint => Ok(TextureFormat::Rg32Sint),
      GpuTextureFormat::Rg32Float => Ok(TextureFormat::Rg32Float),
      GpuTextureFormat::Rgba16Uint => Ok(TextureFormat::Rgba16Uint),
      GpuTextureFormat::Rgba16Sint => Ok(TextureFormat::Rgba16Sint),
      GpuTextureFormat::Rgba16Float => Ok(TextureFormat::Rgba16Float),

      GpuTextureFormat::Rgba32Uint => Ok(TextureFormat::Rgba32Uint),
      GpuTextureFormat::Rgba32Sint => Ok(TextureFormat::Rgba32Sint),
      GpuTextureFormat::Rgba32Float => Ok(TextureFormat::Rgba32Float),

      GpuTextureFormat::Stencil8 => Err(not_supported()), // wgpu#967
      GpuTextureFormat::Depth16Unorm => Err(not_supported()), // wgpu#967
      GpuTextureFormat::Depth24Plus => Ok(TextureFormat::Depth24Plus),
      GpuTextureFormat::Depth24PlusStencil8 => {
        Ok(TextureFormat::Depth24PlusStencil8)
      }
      GpuTextureFormat::Depth32Float => Ok(TextureFormat::Depth32Float),

      GpuTextureFormat::Bc1RgbaUnorm => Ok(TextureFormat::Bc1RgbaUnorm),
      GpuTextureFormat::Bc1RgbaUnormSrgb => Ok(TextureFormat::Bc1RgbaUnormSrgb),
      GpuTextureFormat::Bc2RgbaUnorm => Ok(TextureFormat::Bc2RgbaUnorm),
      GpuTextureFormat::Bc2RgbaUnormSrgb => Ok(TextureFormat::Bc2RgbaUnormSrgb),
      GpuTextureFormat::Bc3RgbaUnorm => Ok(TextureFormat::Bc3RgbaUnorm),
      GpuTextureFormat::Bc3RgbaUnormSrgb => Ok(TextureFormat::Bc3RgbaUnormSrgb),
      GpuTextureFormat::Bc4RUnorm => Ok(TextureFormat::Bc4RUnorm),
      GpuTextureFormat::Bc4RSnorm => Ok(TextureFormat::Bc4RSnorm),
      GpuTextureFormat::Bc5RgUnorm => Ok(TextureFormat::Bc5RgUnorm),
      GpuTextureFormat::Bc5RgSnorm => Ok(TextureFormat::Bc5RgSnorm),
      GpuTextureFormat::Bc6hRgbUfloat => Ok(TextureFormat::Bc6hRgbUfloat),
      GpuTextureFormat::Bc6HRgbFloat => Ok(TextureFormat::Bc6hRgbSfloat), // wgpu#967
      GpuTextureFormat::Bc7RgbaUnorm => Ok(TextureFormat::Bc7RgbaUnorm),
      GpuTextureFormat::Bc7RgbaUnormSrgb => Ok(TextureFormat::Bc7RgbaUnormSrgb),

      GpuTextureFormat::Depth24UnormStencil8 => Err(not_supported()), // wgpu#967,

      GpuTextureFormat::Depth32FloatStencil8 => Err(not_supported()), // wgpu#967
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuTextureViewDimension {
  #[serde(rename = "1d")]
  D1,
  #[serde(rename = "2d")]
  D2,
  #[serde(rename = "2d-array")]
  D2Array,
  #[serde(rename = "cube")]
  Cube,
  #[serde(rename = "cube-array")]
  CubeArray,
  #[serde(rename = "3d")]
  D3,
}

impl From<GpuTextureViewDimension> for wgpu_types::TextureViewDimension {
  fn from(view_dimension: GpuTextureViewDimension) -> Self {
    match view_dimension {
      GpuTextureViewDimension::D1 => wgpu_types::TextureViewDimension::D1,
      GpuTextureViewDimension::D2 => wgpu_types::TextureViewDimension::D2,
      GpuTextureViewDimension::D2Array => {
        wgpu_types::TextureViewDimension::D2Array
      }
      GpuTextureViewDimension::Cube => wgpu_types::TextureViewDimension::Cube,
      GpuTextureViewDimension::CubeArray => {
        wgpu_types::TextureViewDimension::CubeArray
      }
      GpuTextureViewDimension::D3 => wgpu_types::TextureViewDimension::D3,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuTextureDimension {
  #[serde(rename = "1d")]
  D1,
  #[serde(rename = "2d")]
  D2,
  #[serde(rename = "3d")]
  D3,
}

impl From<GpuTextureDimension> for wgpu_types::TextureDimension {
  fn from(texture_dimension: GpuTextureDimension) -> Self {
    match texture_dimension {
      GpuTextureDimension::D1 => wgpu_types::TextureDimension::D1,
      GpuTextureDimension::D2 => wgpu_types::TextureDimension::D2,
      GpuTextureDimension::D3 => wgpu_types::TextureDimension::D3,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuTextureAspect {
  All,
  StencilOnly,
  DepthOnly,
}

impl From<GpuTextureAspect> for wgpu_types::TextureAspect {
  fn from(aspect: GpuTextureAspect) -> wgpu_types::TextureAspect {
    match aspect {
      GpuTextureAspect::All => wgpu_types::TextureAspect::All,
      GpuTextureAspect::StencilOnly => wgpu_types::TextureAspect::StencilOnly,
      GpuTextureAspect::DepthOnly => wgpu_types::TextureAspect::DepthOnly,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuExtent3D {
  pub width: u32,
  pub height: u32,
  pub depth_or_array_layers: u32,
}

impl From<GpuExtent3D> for wgpu_types::Extent3d {
  fn from(extent: GpuExtent3D) -> Self {
    wgpu_types::Extent3d {
      width: extent.width,
      height: extent.height,
      depth_or_array_layers: extent.depth_or_array_layers,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTextureArgs {
  device_rid: ResourceId,
  label: Option<String>,
  size: GpuExtent3D,
  mip_level_count: u32,
  sample_count: u32,
  dimension: GpuTextureDimension,
  format: GpuTextureFormat,
  usage: u32,
}

pub fn op_webgpu_create_texture(
  state: &mut OpState,
  args: CreateTextureArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let descriptor = wgpu_core::resource::TextureDescriptor {
    label: args.label.map(Cow::from),
    size: args.size.into(),
    mip_level_count: args.mip_level_count,
    sample_count: args.sample_count,
    dimension: args.dimension.into(),
    format: args.format.try_into()?,
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
  format: Option<GpuTextureFormat>,
  dimension: Option<GpuTextureViewDimension>,
  aspect: GpuTextureAspect,
  base_mip_level: u32,
  mip_level_count: Option<u32>,
  base_array_layer: u32,
  array_layer_count: Option<u32>,
}

pub fn op_webgpu_create_texture_view(
  state: &mut OpState,
  args: CreateTextureViewArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let texture_resource = state
    .resource_table
    .get::<WebGpuTexture>(args.texture_rid)?;
  let texture = texture_resource.0;

  let descriptor = wgpu_core::resource::TextureViewDescriptor {
    label: args.label.map(Cow::from),
    format: args.format.map(|s| s.try_into()).transpose()?,
    dimension: args.dimension.map(|s| s.into()),
    range: wgpu_types::ImageSubresourceRange {
      aspect: args.aspect.into(),
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
