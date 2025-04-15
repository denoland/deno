// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;

use deno_core::cppgc::Ptr;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::IntOptions;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;
use deno_core::WebIDL;
use deno_error::JsErrorBox;

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUExtent3DDict {
  #[options(enforce_range = true)]
  width: u32,
  #[webidl(default = 1)]
  #[options(enforce_range = true)]
  height: u32,
  #[webidl(default = 1)]
  #[options(enforce_range = true)]
  depth_or_array_layers: u32,
}

pub(crate) enum GPUExtent3D {
  Dict(GPUExtent3DDict),
  Sequence((u32, u32, u32)),
}

impl<'a> WebIdlConverter<'a> for GPUExtent3D {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_null_or_undefined() {
      return Ok(GPUExtent3D::Dict(GPUExtent3DDict::convert(
        scope,
        value,
        prefix,
        context.borrowed(),
        options,
      )?));
    }
    if let Ok(obj) = value.try_cast::<v8::Object>() {
      let iter = v8::Symbol::get_iterator(scope);
      if let Some(iter) = obj.get(scope, iter.into()) {
        if !iter.is_undefined() {
          let conv = <Vec<u32>>::convert(
            scope,
            value,
            prefix.clone(),
            context.borrowed(),
            &IntOptions {
              clamp: false,
              enforce_range: true,
            },
          )?;
          if !(conv.len() > 1 && conv.len() <= 3) {
            return Err(WebIdlError::other(prefix, context, JsErrorBox::type_error(format!("A sequence of number used as a GPUExtent3D must have between 1 and 3 elements, received {} elements", conv.len()))));
          }

          let mut iter = conv.into_iter();
          return Ok(GPUExtent3D::Sequence((
            iter.next().unwrap(),
            iter.next().unwrap_or(1),
            iter.next().unwrap_or(1),
          )));
        }
      }

      return Ok(GPUExtent3D::Dict(GPUExtent3DDict::convert(
        scope, value, prefix, context, options,
      )?));
    }

    Err(WebIdlError::new(
      prefix,
      context,
      WebIdlErrorKind::ConvertToConverterType(
        "sequence<GPUIntegerCoordinate> or GPUExtent3DDict",
      ),
    ))
  }
}

impl From<GPUExtent3D> for wgpu_types::Extent3d {
  fn from(value: GPUExtent3D) -> Self {
    match value {
      GPUExtent3D::Dict(dict) => Self {
        width: dict.width,
        height: dict.height,
        depth_or_array_layers: dict.depth_or_array_layers,
      },
      GPUExtent3D::Sequence((width, height, depth)) => Self {
        width,
        height,
        depth_or_array_layers: depth,
      },
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUOrigin3DDict {
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  x: u32,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  y: u32,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  z: u32,
}

pub(crate) enum GPUOrigin3D {
  Dict(GPUOrigin3DDict),
  Sequence((u32, u32, u32)),
}

impl Default for GPUOrigin3D {
  fn default() -> Self {
    GPUOrigin3D::Sequence((0, 0, 0))
  }
}

impl<'a> WebIdlConverter<'a> for GPUOrigin3D {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_null_or_undefined() {
      return Ok(GPUOrigin3D::Dict(GPUOrigin3DDict::convert(
        scope,
        value,
        prefix,
        context.borrowed(),
        options,
      )?));
    }
    if let Ok(obj) = value.try_cast::<v8::Object>() {
      let iter = v8::Symbol::get_iterator(scope);
      if let Some(iter) = obj.get(scope, iter.into()) {
        if !iter.is_undefined() {
          let conv = <Vec<u32>>::convert(
            scope,
            value,
            prefix.clone(),
            context.borrowed(),
            &IntOptions {
              clamp: false,
              enforce_range: true,
            },
          )?;
          if conv.len() > 3 {
            return Err(WebIdlError::other(prefix, context, JsErrorBox::type_error(format!("A sequence of number used as a GPUOrigin3D must have at most 3 elements, received {} elements", conv.len()))));
          }

          let mut iter = conv.into_iter();
          return Ok(GPUOrigin3D::Sequence((
            iter.next().unwrap_or(0),
            iter.next().unwrap_or(0),
            iter.next().unwrap_or(0),
          )));
        }
      }

      return Ok(GPUOrigin3D::Dict(GPUOrigin3DDict::convert(
        scope, value, prefix, context, options,
      )?));
    }

    Err(WebIdlError::new(
      prefix,
      context,
      WebIdlErrorKind::ConvertToConverterType(
        "sequence<GPUIntegerCoordinate> or GPUOrigin3DDict",
      ),
    ))
  }
}

impl From<GPUOrigin3D> for wgpu_types::Origin3d {
  fn from(value: GPUOrigin3D) -> Self {
    match value {
      GPUOrigin3D::Dict(dict) => Self {
        x: dict.x,
        y: dict.y,
        z: dict.z,
      },
      GPUOrigin3D::Sequence((x, y, z)) => Self { x, y, z },
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUColorDict {
  r: f64,
  g: f64,
  b: f64,
  a: f64,
}

pub(crate) enum GPUColor {
  Dict(GPUColorDict),
  Sequence((f64, f64, f64, f64)),
}

impl<'a> WebIdlConverter<'a> for GPUColor {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_null_or_undefined() {
      return Ok(GPUColor::Dict(GPUColorDict::convert(
        scope,
        value,
        prefix,
        context.borrowed(),
        options,
      )?));
    }
    if let Ok(obj) = value.try_cast::<v8::Object>() {
      let iter = v8::Symbol::get_iterator(scope);
      if let Some(iter) = obj.get(scope, iter.into()) {
        if !iter.is_undefined() {
          let conv = <Vec<f64>>::convert(
            scope,
            value,
            prefix.clone(),
            context.borrowed(),
            options,
          )?;
          if conv.len() != 4 {
            return Err(WebIdlError::other(prefix, context, JsErrorBox::type_error(format!("A sequence of number used as a GPUColor must have exactly 4 elements, received {} elements", conv.len()))));
          }

          let mut iter = conv.into_iter();
          return Ok(GPUColor::Sequence((
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
          )));
        }
      }

      return Ok(GPUColor::Dict(GPUColorDict::convert(
        scope, value, prefix, context, options,
      )?));
    }

    Err(WebIdlError::new(
      prefix,
      context,
      WebIdlErrorKind::ConvertToConverterType(
        "sequence<GPUIntegerCoordinate> or GPUOrigin3DDict",
      ),
    ))
  }
}

impl From<GPUColor> for wgpu_types::Color {
  fn from(value: GPUColor) -> Self {
    match value {
      GPUColor::Dict(dict) => Self {
        r: dict.r,
        g: dict.g,
        b: dict.b,
        a: dict.a,
      },
      GPUColor::Sequence((r, g, b, a)) => Self { r, g, b, a },
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUAutoLayoutMode {
  Auto,
}

pub(crate) enum GPUPipelineLayoutOrGPUAutoLayoutMode {
  PipelineLayout(Ptr<crate::pipeline_layout::GPUPipelineLayout>),
  AutoLayoutMode(GPUAutoLayoutMode),
}

impl From<GPUPipelineLayoutOrGPUAutoLayoutMode>
  for Option<wgpu_core::id::PipelineLayoutId>
{
  fn from(value: GPUPipelineLayoutOrGPUAutoLayoutMode) -> Self {
    match value {
      GPUPipelineLayoutOrGPUAutoLayoutMode::PipelineLayout(layout) => {
        Some(layout.id)
      }
      GPUPipelineLayoutOrGPUAutoLayoutMode::AutoLayoutMode(
        GPUAutoLayoutMode::Auto,
      ) => None,
    }
  }
}

impl<'a> WebIdlConverter<'a> for GPUPipelineLayoutOrGPUAutoLayoutMode {
  type Options = ();

  fn convert<'b>(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    if value.is_object() {
      Ok(Self::PipelineLayout(WebIdlConverter::convert(
        scope, value, prefix, context, options,
      )?))
    } else {
      Ok(Self::AutoLayoutMode(WebIdlConverter::convert(
        scope, value, prefix, context, options,
      )?))
    }
  }
}

#[derive(WebIDL, Clone, Hash, Eq, PartialEq)]
#[webidl(enum)]
pub enum GPUFeatureName {
  #[webidl(rename = "depth-clip-control")]
  DepthClipControl,
  #[webidl(rename = "timestamp-query")]
  TimestampQuery,
  #[webidl(rename = "indirect-first-instance")]
  IndirectFirstInstance,
  #[webidl(rename = "shader-f16")]
  ShaderF16,
  #[webidl(rename = "depth32float-stencil8")]
  Depth32floatStencil8,
  #[webidl(rename = "texture-compression-bc")]
  TextureCompressionBc,
  #[webidl(rename = "texture-compression-bc-sliced-3d")]
  TextureCompressionBcSliced3d,
  #[webidl(rename = "texture-compression-etc2")]
  TextureCompressionEtc2,
  #[webidl(rename = "texture-compression-astc")]
  TextureCompressionAstc,
  #[webidl(rename = "rg11b10ufloat-renderable")]
  Rg11b10ufloatRenderable,
  #[webidl(rename = "bgra8unorm-storage")]
  Bgra8unormStorage,
  #[webidl(rename = "float32-filterable")]
  Float32Filterable,
  #[webidl(rename = "dual-source-blending")]
  DualSourceBlending,
  #[webidl(rename = "subgroups")]
  Subgroups,

  // extended from spec
  #[webidl(rename = "texture-format-16-bit-norm")]
  TextureFormat16BitNorm,
  #[webidl(rename = "texture-compression-astc-hdr")]
  TextureCompressionAstcHdr,
  #[webidl(rename = "texture-adapter-specific-format-features")]
  TextureAdapterSpecificFormatFeatures,
  #[webidl(rename = "pipeline-statistics-query")]
  PipelineStatisticsQuery,
  #[webidl(rename = "timestamp-query-inside-passes")]
  TimestampQueryInsidePasses,
  #[webidl(rename = "mappable-primary-buffers")]
  MappablePrimaryBuffers,
  #[webidl(rename = "texture-binding-array")]
  TextureBindingArray,
  #[webidl(rename = "buffer-binding-array")]
  BufferBindingArray,
  #[webidl(rename = "storage-resource-binding-array")]
  StorageResourceBindingArray,
  #[webidl(
    rename = "sampled-texture-and-storage-buffer-array-non-uniform-indexing"
  )]
  SampledTextureAndStorageBufferArrayNonUniformIndexing,
  #[webidl(
    rename = "uniform-buffer-and-storage-texture-array-non-uniform-indexing"
  )]
  UniformBufferAndStorageTextureArrayNonUniformIndexing,
  #[webidl(rename = "partially-bound-binding-array")]
  PartiallyBoundBindingArray,
  #[webidl(rename = "multi-draw-indirect")]
  MultiDrawIndirect,
  #[webidl(rename = "multi-draw-indirect-count")]
  MultiDrawIndirectCount,
  #[webidl(rename = "push-constants")]
  PushConstants,
  #[webidl(rename = "address-mode-clamp-to-zero")]
  AddressModeClampToZero,
  #[webidl(rename = "address-mode-clamp-to-border")]
  AddressModeClampToBorder,
  #[webidl(rename = "polygon-mode-line")]
  PolygonModeLine,
  #[webidl(rename = "polygon-mode-point")]
  PolygonModePoint,
  #[webidl(rename = "conservative-rasterization")]
  ConservativeRasterization,
  #[webidl(rename = "vertex-writable-storage")]
  VertexWritableStorage,
  #[webidl(rename = "clear-texture")]
  ClearTexture,
  #[webidl(rename = "spirv-shader-passthrough")]
  SpirvShaderPassthrough,
  #[webidl(rename = "multiview")]
  Multiview,
  #[webidl(rename = "vertex-attribute-64-bit")]
  VertexAttribute64Bit,
  #[webidl(rename = "shader-f64")]
  ShaderF64,
  #[webidl(rename = "shader-i16")]
  ShaderI16,
  #[webidl(rename = "shader-primitive-index")]
  ShaderPrimitiveIndex,
  #[webidl(rename = "shader-early-depth-test")]
  ShaderEarlyDepthTest,
}

pub fn feature_names_to_features(
  names: Vec<GPUFeatureName>,
) -> wgpu_types::Features {
  use wgpu_types::Features;
  let mut features = Features::empty();

  for name in names {
    #[rustfmt::skip]
    let feature = match name {
      GPUFeatureName::DepthClipControl => Features::DEPTH_CLIP_CONTROL,
      GPUFeatureName::TimestampQuery => Features::TIMESTAMP_QUERY,
      GPUFeatureName::IndirectFirstInstance => Features::INDIRECT_FIRST_INSTANCE,
      GPUFeatureName::ShaderF16 => Features::SHADER_F16,
      GPUFeatureName::Depth32floatStencil8 => Features::DEPTH32FLOAT_STENCIL8,
      GPUFeatureName::TextureCompressionBc => Features::TEXTURE_COMPRESSION_BC,
      GPUFeatureName::TextureCompressionBcSliced3d => Features::TEXTURE_COMPRESSION_BC_SLICED_3D,
      GPUFeatureName::TextureCompressionEtc2 => Features::TEXTURE_COMPRESSION_ETC2,
      GPUFeatureName::TextureCompressionAstc => Features::TEXTURE_COMPRESSION_ASTC,
      GPUFeatureName::Rg11b10ufloatRenderable => Features::RG11B10UFLOAT_RENDERABLE,
      GPUFeatureName::Bgra8unormStorage => Features::BGRA8UNORM_STORAGE,
      GPUFeatureName::Float32Filterable => Features::FLOAT32_FILTERABLE,
      GPUFeatureName::DualSourceBlending => Features::DUAL_SOURCE_BLENDING,
      GPUFeatureName::Subgroups => Features::SUBGROUP,
      GPUFeatureName::TextureFormat16BitNorm => Features::TEXTURE_FORMAT_16BIT_NORM,
      GPUFeatureName::TextureCompressionAstcHdr => Features::TEXTURE_COMPRESSION_ASTC_HDR,
      GPUFeatureName::TextureAdapterSpecificFormatFeatures => Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
      GPUFeatureName::PipelineStatisticsQuery => Features::PIPELINE_STATISTICS_QUERY,
      GPUFeatureName::TimestampQueryInsidePasses => Features::TIMESTAMP_QUERY_INSIDE_PASSES,
      GPUFeatureName::MappablePrimaryBuffers => Features::MAPPABLE_PRIMARY_BUFFERS,
      GPUFeatureName::TextureBindingArray => Features::TEXTURE_BINDING_ARRAY,
      GPUFeatureName::BufferBindingArray => Features::BUFFER_BINDING_ARRAY,
      GPUFeatureName::StorageResourceBindingArray => Features::STORAGE_RESOURCE_BINDING_ARRAY,
      GPUFeatureName::SampledTextureAndStorageBufferArrayNonUniformIndexing => Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
      GPUFeatureName::UniformBufferAndStorageTextureArrayNonUniformIndexing => Features::UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING,
      GPUFeatureName::PartiallyBoundBindingArray => Features::PARTIALLY_BOUND_BINDING_ARRAY,
      GPUFeatureName::MultiDrawIndirect => Features::MULTI_DRAW_INDIRECT,
      GPUFeatureName::MultiDrawIndirectCount => Features::MULTI_DRAW_INDIRECT_COUNT,
      GPUFeatureName::PushConstants => Features::PUSH_CONSTANTS,
      GPUFeatureName::AddressModeClampToZero => Features::ADDRESS_MODE_CLAMP_TO_ZERO,
      GPUFeatureName::AddressModeClampToBorder => Features::ADDRESS_MODE_CLAMP_TO_BORDER,
      GPUFeatureName::PolygonModeLine => Features::POLYGON_MODE_LINE,
      GPUFeatureName::PolygonModePoint => Features::POLYGON_MODE_POINT,
      GPUFeatureName::ConservativeRasterization => Features::CONSERVATIVE_RASTERIZATION,
      GPUFeatureName::VertexWritableStorage => Features::VERTEX_WRITABLE_STORAGE,
      GPUFeatureName::ClearTexture => Features::CLEAR_TEXTURE,
      GPUFeatureName::SpirvShaderPassthrough => Features::SPIRV_SHADER_PASSTHROUGH,
      GPUFeatureName::Multiview => Features::MULTIVIEW,
      GPUFeatureName::VertexAttribute64Bit => Features::VERTEX_ATTRIBUTE_64BIT,
      GPUFeatureName::ShaderF64 => Features::SHADER_F64,
      GPUFeatureName::ShaderI16 => Features::SHADER_F16,
      GPUFeatureName::ShaderPrimitiveIndex => Features::SHADER_PRIMITIVE_INDEX,
      GPUFeatureName::ShaderEarlyDepthTest => Features::SHADER_EARLY_DEPTH_TEST,
    };
    features.set(feature, true);
  }

  features
}

pub fn features_to_feature_names(
  features: wgpu_types::Features,
) -> HashSet<GPUFeatureName> {
  use GPUFeatureName::*;
  let mut return_features = HashSet::new();

  // api
  if features.contains(wgpu_types::Features::DEPTH_CLIP_CONTROL) {
    return_features.insert(DepthClipControl);
  }
  if features.contains(wgpu_types::Features::TIMESTAMP_QUERY) {
    return_features.insert(TimestampQuery);
  }
  if features.contains(wgpu_types::Features::INDIRECT_FIRST_INSTANCE) {
    return_features.insert(IndirectFirstInstance);
  }
  // shader
  if features.contains(wgpu_types::Features::SHADER_F16) {
    return_features.insert(ShaderF16);
  }
  // texture formats
  if features.contains(wgpu_types::Features::DEPTH32FLOAT_STENCIL8) {
    return_features.insert(Depth32floatStencil8);
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_BC) {
    return_features.insert(TextureCompressionBc);
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_BC_SLICED_3D) {
    return_features.insert(TextureCompressionBcSliced3d);
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_ETC2) {
    return_features.insert(TextureCompressionEtc2);
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_ASTC) {
    return_features.insert(TextureCompressionAstc);
  }
  if features.contains(wgpu_types::Features::RG11B10UFLOAT_RENDERABLE) {
    return_features.insert(Rg11b10ufloatRenderable);
  }
  if features.contains(wgpu_types::Features::BGRA8UNORM_STORAGE) {
    return_features.insert(Bgra8unormStorage);
  }
  if features.contains(wgpu_types::Features::FLOAT32_FILTERABLE) {
    return_features.insert(Float32Filterable);
  }
  if features.contains(wgpu_types::Features::DUAL_SOURCE_BLENDING) {
    return_features.insert(DualSourceBlending);
  }
  if features.contains(wgpu_types::Features::SUBGROUP) {
    return_features.insert(Subgroups);
  }

  // extended from spec

  // texture formats
  if features.contains(wgpu_types::Features::TEXTURE_FORMAT_16BIT_NORM) {
    return_features.insert(TextureFormat16BitNorm);
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_ASTC_HDR) {
    return_features.insert(TextureCompressionAstcHdr);
  }
  if features
    .contains(wgpu_types::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
  {
    return_features.insert(TextureAdapterSpecificFormatFeatures);
  }
  // api
  if features.contains(wgpu_types::Features::PIPELINE_STATISTICS_QUERY) {
    return_features.insert(PipelineStatisticsQuery);
  }
  if features.contains(wgpu_types::Features::TIMESTAMP_QUERY_INSIDE_PASSES) {
    return_features.insert(TimestampQueryInsidePasses);
  }
  if features.contains(wgpu_types::Features::MAPPABLE_PRIMARY_BUFFERS) {
    return_features.insert(MappablePrimaryBuffers);
  }
  if features.contains(wgpu_types::Features::TEXTURE_BINDING_ARRAY) {
    return_features.insert(TextureBindingArray);
  }
  if features.contains(wgpu_types::Features::BUFFER_BINDING_ARRAY) {
    return_features.insert(BufferBindingArray);
  }
  if features.contains(wgpu_types::Features::STORAGE_RESOURCE_BINDING_ARRAY) {
    return_features.insert(StorageResourceBindingArray);
  }
  if features.contains(
    wgpu_types::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
  ) {
    return_features.insert(SampledTextureAndStorageBufferArrayNonUniformIndexing);
  }
  if features.contains(
    wgpu_types::Features::UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING,
  ) {
    return_features.insert(UniformBufferAndStorageTextureArrayNonUniformIndexing);
  }
  if features.contains(wgpu_types::Features::PARTIALLY_BOUND_BINDING_ARRAY) {
    return_features.insert(PartiallyBoundBindingArray);
  }
  if features.contains(wgpu_types::Features::MULTI_DRAW_INDIRECT) {
    return_features.insert(MultiDrawIndirect);
  }
  if features.contains(wgpu_types::Features::MULTI_DRAW_INDIRECT_COUNT) {
    return_features.insert(MultiDrawIndirectCount);
  }
  if features.contains(wgpu_types::Features::PUSH_CONSTANTS) {
    return_features.insert(PushConstants);
  }
  if features.contains(wgpu_types::Features::ADDRESS_MODE_CLAMP_TO_ZERO) {
    return_features.insert(AddressModeClampToZero);
  }
  if features.contains(wgpu_types::Features::ADDRESS_MODE_CLAMP_TO_BORDER) {
    return_features.insert(AddressModeClampToBorder);
  }
  if features.contains(wgpu_types::Features::POLYGON_MODE_LINE) {
    return_features.insert(PolygonModeLine);
  }
  if features.contains(wgpu_types::Features::POLYGON_MODE_POINT) {
    return_features.insert(PolygonModePoint);
  }
  if features.contains(wgpu_types::Features::CONSERVATIVE_RASTERIZATION) {
    return_features.insert(ConservativeRasterization);
  }
  if features.contains(wgpu_types::Features::VERTEX_WRITABLE_STORAGE) {
    return_features.insert(VertexWritableStorage);
  }
  if features.contains(wgpu_types::Features::CLEAR_TEXTURE) {
    return_features.insert(ClearTexture);
  }
  if features.contains(wgpu_types::Features::SPIRV_SHADER_PASSTHROUGH) {
    return_features.insert(SpirvShaderPassthrough);
  }
  if features.contains(wgpu_types::Features::MULTIVIEW) {
    return_features.insert(Multiview);
  }
  if features.contains(wgpu_types::Features::VERTEX_ATTRIBUTE_64BIT) {
    return_features.insert(VertexAttribute64Bit);
  }
  // shader
  if features.contains(wgpu_types::Features::SHADER_F64) {
    return_features.insert(ShaderF64);
  }
  if features.contains(wgpu_types::Features::SHADER_I16) {
    return_features.insert(ShaderI16);
  }
  if features.contains(wgpu_types::Features::SHADER_PRIMITIVE_INDEX) {
    return_features.insert(ShaderPrimitiveIndex);
  }
  if features.contains(wgpu_types::Features::SHADER_EARLY_DEPTH_TEST) {
    return_features.insert(ShaderEarlyDepthTest);
  }

  return_features
}
