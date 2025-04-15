// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_error::JsErrorBox;
use wgpu_types::AstcBlock;
use wgpu_types::AstcChannel;
use wgpu_types::Extent3d;
use wgpu_types::TextureAspect;
use wgpu_types::TextureDimension;
use wgpu_types::TextureFormat;
use wgpu_types::TextureViewDimension;

use crate::Instance;

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUTextureDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub size: super::webidl::GPUExtent3D,
  #[webidl(default = 1)]
  #[options(enforce_range = true)]
  pub mip_level_count: u32,
  #[webidl(default = 1)]
  #[options(enforce_range = true)]
  pub sample_count: u32,
  #[webidl(default = GPUTextureDimension::D2)]
  pub dimension: GPUTextureDimension,
  pub format: GPUTextureFormat,
  #[options(enforce_range = true)]
  pub usage: u32,
  #[webidl(default = vec![])]
  pub view_formats: Vec<GPUTextureFormat>,
}

pub struct GPUTexture {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub id: wgpu_core::id::TextureId,
  pub device_id: wgpu_core::id::DeviceId,
  pub queue_id: wgpu_core::id::QueueId,

  pub label: String,

  pub size: Extent3d,
  pub mip_level_count: u32,
  pub sample_count: u32,
  pub dimension: GPUTextureDimension,
  pub format: GPUTextureFormat,
  pub usage: u32,
}

impl Drop for GPUTexture {
  fn drop(&mut self) {
    self.instance.texture_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUTexture {
  const NAME: &'static str = "GPUTexture";
}

impl GarbageCollected for GPUTexture {}

#[op2]
impl GPUTexture {
  #[getter]
  #[string]
  fn label(&self) -> String {
    self.label.clone()
  }
  #[setter]
  #[string]
  fn label(&self, #[webidl] _label: String) {
    // TODO(@crowlKats): no-op, needs wpgu to implement changing the label
  }

  #[getter]
  fn width(&self) -> u32 {
    self.size.width
  }
  #[getter]
  fn height(&self) -> u32 {
    self.size.height
  }
  #[getter]
  fn depth_or_array_layers(&self) -> u32 {
    self.size.depth_or_array_layers
  }
  #[getter]
  fn mip_level_count(&self) -> u32 {
    self.mip_level_count
  }
  #[getter]
  fn sample_count(&self) -> u32 {
    self.sample_count
  }
  #[getter]
  #[string]
  fn dimension(&self) -> &'static str {
    self.dimension.as_str()
  }
  #[getter]
  #[string]
  fn format(&self) -> &'static str {
    self.format.as_str()
  }
  #[getter]
  fn usage(&self) -> u32 {
    self.usage
  }
  #[fast]
  fn destroy(&self) -> Result<(), JsErrorBox> {
    self
      .instance
      .texture_destroy(self.id)
      .map_err(|e| JsErrorBox::generic(e.to_string()))
  }

  #[cppgc]
  fn create_view(
    &self,
    #[webidl] descriptor: GPUTextureViewDescriptor,
  ) -> Result<GPUTextureView, JsErrorBox> {
    let wgpu_descriptor = wgpu_core::resource::TextureViewDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      format: descriptor.format.map(Into::into),
      dimension: descriptor.dimension.map(Into::into),
      usage: Some(
        wgpu_types::TextureUsages::from_bits(descriptor.usage)
          .ok_or_else(|| JsErrorBox::type_error("usage is not valid"))?,
      ),
      range: wgpu_types::ImageSubresourceRange {
        aspect: descriptor.aspect.into(),
        base_mip_level: descriptor.base_mip_level,
        mip_level_count: descriptor.mip_level_count,
        base_array_layer: descriptor.base_array_layer,
        array_layer_count: descriptor.array_layer_count,
      },
    };

    let (id, err) =
      self
        .instance
        .texture_create_view(self.id, &wgpu_descriptor, None);

    self.error_handler.push_error(err);

    Ok(GPUTextureView {
      instance: self.instance.clone(),
      id,
      label: descriptor.label,
    })
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct GPUTextureViewDescriptor {
  #[webidl(default = String::new())]
  label: String,

  format: Option<GPUTextureFormat>,
  dimension: Option<GPUTextureViewDimension>,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  usage: u32,
  #[webidl(default = GPUTextureAspect::All)]
  aspect: GPUTextureAspect,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  base_mip_level: u32,
  #[options(enforce_range = true)]
  mip_level_count: Option<u32>,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  base_array_layer: u32,
  #[options(enforce_range = true)]
  array_layer_count: Option<u32>,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUTextureViewDimension {
  #[webidl(rename = "1d")]
  D1,
  #[webidl(rename = "2d")]
  D2,
  #[webidl(rename = "2d-array")]
  D2Array,
  #[webidl(rename = "cube")]
  Cube,
  #[webidl(rename = "cube-array")]
  CubeArray,
  #[webidl(rename = "3d")]
  D3,
}

impl From<GPUTextureViewDimension> for TextureViewDimension {
  fn from(value: GPUTextureViewDimension) -> Self {
    match value {
      GPUTextureViewDimension::D1 => Self::D1,
      GPUTextureViewDimension::D2 => Self::D2,
      GPUTextureViewDimension::D3 => Self::D3,
      GPUTextureViewDimension::D2Array => Self::D2Array,
      GPUTextureViewDimension::Cube => Self::Cube,
      GPUTextureViewDimension::CubeArray => Self::CubeArray,
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub enum GPUTextureAspect {
  All,
  StencilOnly,
  DepthOnly,
}

impl From<GPUTextureAspect> for TextureAspect {
  fn from(value: GPUTextureAspect) -> Self {
    match value {
      GPUTextureAspect::All => Self::All,
      GPUTextureAspect::StencilOnly => Self::StencilOnly,
      GPUTextureAspect::DepthOnly => Self::DepthOnly,
    }
  }
}

pub struct GPUTextureView {
  pub instance: Instance,
  pub id: wgpu_core::id::TextureViewId,
  pub label: String,
}

impl Drop for GPUTextureView {
  fn drop(&mut self) {
    let _ = self.instance.texture_view_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUTextureView {
  const NAME: &'static str = "GPUTextureView";
}

impl GarbageCollected for GPUTextureView {}
// TODO(@crowlKats): weakref in texture for view

#[op2]
impl GPUTextureView {
  #[getter]
  #[string]
  fn label(&self) -> String {
    self.label.clone()
  }
  #[setter]
  #[string]
  fn label(&self, #[webidl] _label: String) {
    // TODO(@crowlKats): no-op, needs wpgu to implement changing the label
  }
}

#[derive(WebIDL, Clone)]
#[webidl(enum)]
pub enum GPUTextureDimension {
  #[webidl(rename = "1d")]
  D1,
  #[webidl(rename = "2d")]
  D2,
  #[webidl(rename = "3d")]
  D3,
}

impl From<GPUTextureDimension> for TextureDimension {
  fn from(value: GPUTextureDimension) -> Self {
    match value {
      GPUTextureDimension::D1 => Self::D1,
      GPUTextureDimension::D2 => Self::D2,
      GPUTextureDimension::D3 => Self::D3,
    }
  }
}

#[derive(WebIDL, Clone)]
#[webidl(enum)]
pub enum GPUTextureFormat {
  #[webidl(rename = "r8unorm")]
  R8unorm,
  #[webidl(rename = "r8snorm")]
  R8snorm,
  #[webidl(rename = "r8uint")]
  R8uint,
  #[webidl(rename = "r8sint")]
  R8sint,
  #[webidl(rename = "r16uint")]
  R16uint,
  #[webidl(rename = "r16sint")]
  R16sint,
  #[webidl(rename = "r16float")]
  R16float,
  #[webidl(rename = "rg8unorm")]
  Rg8unorm,
  #[webidl(rename = "rg8snorm")]
  Rg8snorm,
  #[webidl(rename = "rg8uint")]
  Rg8uint,
  #[webidl(rename = "rg8sint")]
  Rg8sint,
  #[webidl(rename = "r32uint")]
  R32uint,
  #[webidl(rename = "r32sint")]
  R32sint,
  #[webidl(rename = "r32float")]
  R32float,
  #[webidl(rename = "rg16uint")]
  Rg16uint,
  #[webidl(rename = "rg16sint")]
  Rg16sint,
  #[webidl(rename = "rg16float")]
  Rg16float,
  #[webidl(rename = "rgba8unorm")]
  Rgba8unorm,
  #[webidl(rename = "rgba8unorm-srgb")]
  Rgba8unormSrgb,
  #[webidl(rename = "rgba8snorm")]
  Rgba8snorm,
  #[webidl(rename = "rgba8uint")]
  Rgba8uint,
  #[webidl(rename = "rgba8sint")]
  Rgba8sint,
  #[webidl(rename = "bgra8unorm")]
  Bgra8unorm,
  #[webidl(rename = "bgra8unorm-srgb")]
  Bgra8unormSrgb,
  #[webidl(rename = "rgb9e5ufloat")]
  Rgb9e5ufloat,
  #[webidl(rename = "rgb10a2uint")]
  Rgb10a2uint,
  #[webidl(rename = "rgb10a2unorm")]
  Rgb10a2unorm,
  #[webidl(rename = "rg11b10ufloat")]
  Rg11b10ufloat,
  #[webidl(rename = "rg32uint")]
  Rg32uint,
  #[webidl(rename = "rg32sint")]
  Rg32sint,
  #[webidl(rename = "rg32float")]
  Rg32float,
  #[webidl(rename = "rgba16uint")]
  Rgba16uint,
  #[webidl(rename = "rgba16sint")]
  Rgba16sint,
  #[webidl(rename = "rgba16float")]
  Rgba16float,
  #[webidl(rename = "rgba32uint")]
  Rgba32uint,
  #[webidl(rename = "rgba32sint")]
  Rgba32sint,
  #[webidl(rename = "rgba32float")]
  Rgba32float,
  #[webidl(rename = "stencil8")]
  Stencil8,
  #[webidl(rename = "depth16unorm")]
  Depth16unorm,
  #[webidl(rename = "depth24plus")]
  Depth24plus,
  #[webidl(rename = "depth24plus-stencil8")]
  Depth24plusStencil8,
  #[webidl(rename = "depth32float")]
  Depth32float,
  #[webidl(rename = "depth32float-stencil8")]
  Depth32floatStencil8,
  #[webidl(rename = "bc1-rgba-unorm")]
  Bc1RgbaUnorm,
  #[webidl(rename = "bc1-rgba-unorm-srgb")]
  Bc1RgbaUnormSrgb,
  #[webidl(rename = "bc2-rgba-unorm")]
  Bc2RgbaUnorm,
  #[webidl(rename = "bc2-rgba-unorm-srgb")]
  Bc2RgbaUnormSrgb,
  #[webidl(rename = "bc3-rgba-unorm")]
  Bc3RgbaUnorm,
  #[webidl(rename = "bc3-rgba-unorm-srgb")]
  Bc3RgbaUnormSrgb,
  #[webidl(rename = "bc4-r-unorm")]
  Bc4RUnorm,
  #[webidl(rename = "bc4-r-snorm")]
  Bc4RSnorm,
  #[webidl(rename = "bc5-rg-unorm")]
  Bc5RgUnorm,
  #[webidl(rename = "bc5-rg-snorm")]
  Bc5RgSnorm,
  #[webidl(rename = "bc6h-rgb-ufloat")]
  Bc6hRgbUfloat,
  #[webidl(rename = "bc6h-rgb-float")]
  Bc6hRgbFloat,
  #[webidl(rename = "bc7-rgba-unorm")]
  Bc7RgbaUnorm,
  #[webidl(rename = "bc7-rgba-unorm-srgb")]
  Bc7RgbaUnormSrgb,
  #[webidl(rename = "etc2-rgb8unorm")]
  Etc2Rgb8unorm,
  #[webidl(rename = "etc2-rgb8unorm-srgb")]
  Etc2Rgb8unormSrgb,
  #[webidl(rename = "etc2-rgb8a1unorm")]
  Etc2Rgb8a1unorm,
  #[webidl(rename = "etc2-rgb8a1unorm-srgb")]
  Etc2Rgb8a1unormSrgb,
  #[webidl(rename = "etc2-rgba8unorm")]
  Etc2Rgba8unorm,
  #[webidl(rename = "etc2-rgba8unorm-srgb")]
  Etc2Rgba8unormSrgb,
  #[webidl(rename = "eac-r11unorm")]
  EacR11unorm,
  #[webidl(rename = "eac-r11snorm")]
  EacR11snorm,
  #[webidl(rename = "eac-rg11unorm")]
  EacRg11unorm,
  #[webidl(rename = "eac-rg11snorm")]
  EacRg11snorm,
  #[webidl(rename = "astc-4x4-unorm")]
  Astc4x4Unorm,
  #[webidl(rename = "astc-4x4-unorm-srgb")]
  Astc4x4UnormSrgb,
  #[webidl(rename = "astc-5x4-unorm")]
  Astc5x4Unorm,
  #[webidl(rename = "astc-5x4-unorm-srgb")]
  Astc5x4UnormSrgb,
  #[webidl(rename = "astc-5x5-unorm")]
  Astc5x5Unorm,
  #[webidl(rename = "astc-5x5-unorm-srgb")]
  Astc5x5UnormSrgb,
  #[webidl(rename = "astc-6x5-unorm")]
  Astc6x5Unorm,
  #[webidl(rename = "astc-6x5-unorm-srgb")]
  Astc6x5UnormSrgb,
  #[webidl(rename = "astc-6x6-unorm")]
  Astc6x6Unorm,
  #[webidl(rename = "astc-6x6-unorm-srgb")]
  Astc6x6UnormSrgb,
  #[webidl(rename = "astc-8x5-unorm")]
  Astc8x5Unorm,
  #[webidl(rename = "astc-8x5-unorm-srgb")]
  Astc8x5UnormSrgb,
  #[webidl(rename = "astc-8x6-unorm")]
  Astc8x6Unorm,
  #[webidl(rename = "astc-8x6-unorm-srgb")]
  Astc8x6UnormSrgb,
  #[webidl(rename = "astc-8x8-unorm")]
  Astc8x8Unorm,
  #[webidl(rename = "astc-8x8-unorm-srgb")]
  Astc8x8UnormSrgb,
  #[webidl(rename = "astc-10x5-unorm")]
  Astc10x5Unorm,
  #[webidl(rename = "astc-10x5-unorm-srgb")]
  Astc10x5UnormSrgb,
  #[webidl(rename = "astc-10x6-unorm")]
  Astc10x6Unorm,
  #[webidl(rename = "astc-10x6-unorm-srgb")]
  Astc10x6UnormSrgb,
  #[webidl(rename = "astc-10x8-unorm")]
  Astc10x8Unorm,
  #[webidl(rename = "astc-10x8-unorm-srgb")]
  Astc10x8UnormSrgb,
  #[webidl(rename = "astc-10x10-unorm")]
  Astc10x10Unorm,
  #[webidl(rename = "astc-10x10-unorm-srgb")]
  Astc10x10UnormSrgb,
  #[webidl(rename = "astc-12x10-unorm")]
  Astc12x10Unorm,
  #[webidl(rename = "astc-12x10-unorm-srgb")]
  Astc12x10UnormSrgb,
  #[webidl(rename = "astc-12x12-unorm")]
  Astc12x12Unorm,
  #[webidl(rename = "astc-12x12-unorm-srgb")]
  Astc12x12UnormSrgb,
}

impl From<GPUTextureFormat> for TextureFormat {
  fn from(value: GPUTextureFormat) -> Self {
    match value {
      GPUTextureFormat::R8unorm => Self::R8Unorm,
      GPUTextureFormat::R8snorm => Self::R8Snorm,
      GPUTextureFormat::R8uint => Self::R8Uint,
      GPUTextureFormat::R8sint => Self::R8Sint,
      GPUTextureFormat::R16uint => Self::R16Uint,
      GPUTextureFormat::R16sint => Self::R16Sint,
      GPUTextureFormat::R16float => Self::R16Float,
      GPUTextureFormat::Rg8unorm => Self::Rg8Unorm,
      GPUTextureFormat::Rg8snorm => Self::Rg8Snorm,
      GPUTextureFormat::Rg8uint => Self::Rg8Uint,
      GPUTextureFormat::Rg8sint => Self::Rg8Sint,
      GPUTextureFormat::R32uint => Self::R32Uint,
      GPUTextureFormat::R32sint => Self::R32Sint,
      GPUTextureFormat::R32float => Self::R32Float,
      GPUTextureFormat::Rg16uint => Self::Rg16Uint,
      GPUTextureFormat::Rg16sint => Self::Rg16Sint,
      GPUTextureFormat::Rg16float => Self::Rg16Float,
      GPUTextureFormat::Rgba8unorm => Self::Rgba8Unorm,
      GPUTextureFormat::Rgba8unormSrgb => Self::Rgba8UnormSrgb,
      GPUTextureFormat::Rgba8snorm => Self::Rgba8Snorm,
      GPUTextureFormat::Rgba8uint => Self::Rgba8Uint,
      GPUTextureFormat::Rgba8sint => Self::Rgba8Sint,
      GPUTextureFormat::Bgra8unorm => Self::Bgra8Unorm,
      GPUTextureFormat::Bgra8unormSrgb => Self::Bgra8UnormSrgb,
      GPUTextureFormat::Rgb9e5ufloat => Self::Rgb9e5Ufloat,
      GPUTextureFormat::Rgb10a2uint => Self::Rgb10a2Uint,
      GPUTextureFormat::Rgb10a2unorm => Self::Rgb10a2Unorm,
      GPUTextureFormat::Rg11b10ufloat => Self::Rg11b10Ufloat,
      GPUTextureFormat::Rg32uint => Self::Rg32Uint,
      GPUTextureFormat::Rg32sint => Self::Rg32Sint,
      GPUTextureFormat::Rg32float => Self::Rg32Float,
      GPUTextureFormat::Rgba16uint => Self::Rgba16Uint,
      GPUTextureFormat::Rgba16sint => Self::Rgba16Sint,
      GPUTextureFormat::Rgba16float => Self::Rgba16Float,
      GPUTextureFormat::Rgba32uint => Self::Rgba32Uint,
      GPUTextureFormat::Rgba32sint => Self::Rgba32Sint,
      GPUTextureFormat::Rgba32float => Self::Rgba32Float,
      GPUTextureFormat::Stencil8 => Self::Stencil8,
      GPUTextureFormat::Depth16unorm => Self::Depth16Unorm,
      GPUTextureFormat::Depth24plus => Self::Depth24Plus,
      GPUTextureFormat::Depth24plusStencil8 => Self::Depth24PlusStencil8,
      GPUTextureFormat::Depth32float => Self::Depth32Float,
      GPUTextureFormat::Depth32floatStencil8 => Self::Depth32FloatStencil8,
      GPUTextureFormat::Bc1RgbaUnorm => Self::Bc1RgbaUnorm,
      GPUTextureFormat::Bc1RgbaUnormSrgb => Self::Bc1RgbaUnormSrgb,
      GPUTextureFormat::Bc2RgbaUnorm => Self::Bc2RgbaUnorm,
      GPUTextureFormat::Bc2RgbaUnormSrgb => Self::Bc2RgbaUnormSrgb,
      GPUTextureFormat::Bc3RgbaUnorm => Self::Bc3RgbaUnorm,
      GPUTextureFormat::Bc3RgbaUnormSrgb => Self::Bc3RgbaUnormSrgb,
      GPUTextureFormat::Bc4RUnorm => Self::Bc4RUnorm,
      GPUTextureFormat::Bc4RSnorm => Self::Bc4RSnorm,
      GPUTextureFormat::Bc5RgUnorm => Self::Bc5RgUnorm,
      GPUTextureFormat::Bc5RgSnorm => Self::Bc5RgSnorm,
      GPUTextureFormat::Bc6hRgbUfloat => Self::Bc6hRgbUfloat,
      GPUTextureFormat::Bc6hRgbFloat => Self::Bc6hRgbFloat,
      GPUTextureFormat::Bc7RgbaUnorm => Self::Bc7RgbaUnorm,
      GPUTextureFormat::Bc7RgbaUnormSrgb => Self::Bc7RgbaUnormSrgb,
      GPUTextureFormat::Etc2Rgb8unorm => Self::Etc2Rgb8Unorm,
      GPUTextureFormat::Etc2Rgb8unormSrgb => Self::Etc2Rgb8UnormSrgb,
      GPUTextureFormat::Etc2Rgb8a1unorm => Self::Etc2Rgb8A1Unorm,
      GPUTextureFormat::Etc2Rgb8a1unormSrgb => Self::Etc2Rgb8A1UnormSrgb,
      GPUTextureFormat::Etc2Rgba8unorm => Self::Etc2Rgba8Unorm,
      GPUTextureFormat::Etc2Rgba8unormSrgb => Self::Etc2Rgba8UnormSrgb,
      GPUTextureFormat::EacR11unorm => Self::EacR11Unorm,
      GPUTextureFormat::EacR11snorm => Self::EacR11Snorm,
      GPUTextureFormat::EacRg11unorm => Self::EacRg11Unorm,
      GPUTextureFormat::EacRg11snorm => Self::EacRg11Snorm,
      GPUTextureFormat::Astc4x4Unorm => Self::Astc {
        block: AstcBlock::B4x4,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc4x4UnormSrgb => Self::Astc {
        block: AstcBlock::B5x4,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc5x4Unorm => Self::Astc {
        block: AstcBlock::B5x4,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc5x4UnormSrgb => Self::Astc {
        block: AstcBlock::B5x4,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc5x5Unorm => Self::Astc {
        block: AstcBlock::B5x5,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc5x5UnormSrgb => Self::Astc {
        block: AstcBlock::B5x5,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc6x5Unorm => Self::Astc {
        block: AstcBlock::B6x5,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc6x5UnormSrgb => Self::Astc {
        block: AstcBlock::B6x5,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc6x6Unorm => Self::Astc {
        block: AstcBlock::B6x6,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc6x6UnormSrgb => Self::Astc {
        block: AstcBlock::B6x6,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc8x5Unorm => Self::Astc {
        block: AstcBlock::B8x5,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc8x5UnormSrgb => Self::Astc {
        block: AstcBlock::B8x5,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc8x6Unorm => Self::Astc {
        block: AstcBlock::B8x6,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc8x6UnormSrgb => Self::Astc {
        block: AstcBlock::B8x6,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc8x8Unorm => Self::Astc {
        block: AstcBlock::B8x8,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc8x8UnormSrgb => Self::Astc {
        block: AstcBlock::B8x8,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc10x5Unorm => Self::Astc {
        block: AstcBlock::B10x5,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc10x5UnormSrgb => Self::Astc {
        block: AstcBlock::B10x5,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc10x6Unorm => Self::Astc {
        block: AstcBlock::B10x6,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc10x6UnormSrgb => Self::Astc {
        block: AstcBlock::B10x6,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc10x8Unorm => Self::Astc {
        block: AstcBlock::B10x8,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc10x8UnormSrgb => Self::Astc {
        block: AstcBlock::B10x8,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc10x10Unorm => Self::Astc {
        block: AstcBlock::B10x10,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc10x10UnormSrgb => Self::Astc {
        block: AstcBlock::B10x10,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc12x10Unorm => Self::Astc {
        block: AstcBlock::B12x10,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc12x10UnormSrgb => Self::Astc {
        block: AstcBlock::B12x10,
        channel: AstcChannel::UnormSrgb,
      },
      GPUTextureFormat::Astc12x12Unorm => Self::Astc {
        block: AstcBlock::B12x12,
        channel: AstcChannel::Unorm,
      },
      GPUTextureFormat::Astc12x12UnormSrgb => Self::Astc {
        block: AstcBlock::B12x12,
        channel: AstcChannel::UnormSrgb,
      },
    }
  }
}
