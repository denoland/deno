// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::webidl::Nullable;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use indexmap::IndexMap;

use crate::bind_group_layout::GPUBindGroupLayout;
use crate::sampler::GPUCompareFunction;
use crate::shader::GPUShaderModule;
use crate::texture::GPUTextureFormat;
use crate::webidl::GPUPipelineLayoutOrGPUAutoLayoutMode;
use crate::Instance;

pub struct GPURenderPipeline {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub id: wgpu_core::id::RenderPipelineId,
  pub label: String,
}

impl Drop for GPURenderPipeline {
  fn drop(&mut self) {
    self.instance.render_pipeline_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPURenderPipeline {
  const NAME: &'static str = "GPURenderPipeline";
}

impl GarbageCollected for GPURenderPipeline {}

#[op2]
impl GPURenderPipeline {
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

  #[cppgc]
  fn get_bind_group_layout(&self, #[webidl] index: u32) -> GPUBindGroupLayout {
    let (id, err) = self
      .instance
      .render_pipeline_get_bind_group_layout(self.id, index, None);

    self.error_handler.push_error(err);

    // TODO(wgpu): needs to add a way to retrieve the label
    GPUBindGroupLayout {
      instance: self.instance.clone(),
      id,
      label: "".to_string(),
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURenderPipelineDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub layout: GPUPipelineLayoutOrGPUAutoLayoutMode,
  pub vertex: GPUVertexState,
  pub primitive: GPUPrimitiveState,
  pub depth_stencil: Option<GPUDepthStencilState>,
  pub multisample: GPUMultisampleState,
  pub fragment: Option<GPUFragmentState>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUMultisampleState {
  #[webidl(default = 1)]
  #[options(enforce_range = true)]
  pub count: u32,
  #[webidl(default = 0xFFFFFFFF)]
  #[options(enforce_range = true)]
  pub mask: u32,
  #[webidl(default = false)]
  pub alpha_to_coverage_enabled: bool,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUDepthStencilState {
  pub format: GPUTextureFormat,
  pub depth_write_enabled: Option<bool>,
  pub depth_compare: Option<GPUCompareFunction>,
  pub stencil_front: GPUStencilFaceState,
  pub stencil_back: GPUStencilFaceState,
  #[webidl(default = 0xFFFFFFFF)]
  #[options(enforce_range = true)]
  pub stencil_read_mask: u32,
  #[webidl(default = 0xFFFFFFFF)]
  #[options(enforce_range = true)]
  pub stencil_write_mask: u32,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  pub depth_bias: i32,
  #[webidl(default = 0.0)]
  pub depth_bias_slope_scale: f32,
  #[webidl(default = 0.0)]
  pub depth_bias_clamp: f32,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUStencilFaceState {
  #[webidl(default = GPUCompareFunction::Always)]
  pub compare: GPUCompareFunction,
  #[webidl(default = GPUStencilOperation::Keep)]
  pub fail_op: GPUStencilOperation,
  #[webidl(default = GPUStencilOperation::Keep)]
  pub depth_fail_op: GPUStencilOperation,
  #[webidl(default = GPUStencilOperation::Keep)]
  pub pass_op: GPUStencilOperation,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUStencilOperation {
  Keep,
  Zero,
  Replace,
  Invert,
  IncrementClamp,
  DecrementClamp,
  IncrementWrap,
  DecrementWrap,
}

impl From<GPUStencilOperation> for wgpu_types::StencilOperation {
  fn from(value: GPUStencilOperation) -> Self {
    match value {
      GPUStencilOperation::Keep => Self::Keep,
      GPUStencilOperation::Zero => Self::Zero,
      GPUStencilOperation::Replace => Self::Replace,
      GPUStencilOperation::Invert => Self::Invert,
      GPUStencilOperation::IncrementClamp => Self::IncrementClamp,
      GPUStencilOperation::DecrementClamp => Self::DecrementClamp,
      GPUStencilOperation::IncrementWrap => Self::IncrementWrap,
      GPUStencilOperation::DecrementWrap => Self::DecrementWrap,
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUVertexState {
  pub module: Ptr<GPUShaderModule>,
  pub entry_point: Option<String>,
  #[webidl(default = Default::default())]
  pub constants: IndexMap<String, f64>,
  #[webidl(default = vec![])]
  pub buffers: Vec<Nullable<GPUVertexBufferLayout>>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUFragmentState {
  pub module: Ptr<GPUShaderModule>,
  pub entry_point: Option<String>,
  #[webidl(default = Default::default())]
  pub constants: IndexMap<String, f64>,
  pub targets: Vec<Nullable<GPUColorTargetState>>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUColorTargetState {
  pub format: GPUTextureFormat,
  pub blend: Option<GPUBlendState>,
  #[webidl(default = 0xF)]
  #[options(enforce_range = true)]
  pub write_mask: u32,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUBlendState {
  pub color: GPUBlendComponent,
  pub alpha: GPUBlendComponent,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUBlendComponent {
  #[webidl(default = GPUBlendOperation::Add)]
  pub operation: GPUBlendOperation,
  #[webidl(default = GPUBlendFactor::One)]
  pub src_factor: GPUBlendFactor,
  #[webidl(default = GPUBlendFactor::Zero)]
  pub dst_factor: GPUBlendFactor,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUBlendOperation {
  Add,
  Subtract,
  ReverseSubtract,
  Min,
  Max,
}

impl From<GPUBlendOperation> for wgpu_types::BlendOperation {
  fn from(value: GPUBlendOperation) -> Self {
    match value {
      GPUBlendOperation::Add => Self::Add,
      GPUBlendOperation::Subtract => Self::Subtract,
      GPUBlendOperation::ReverseSubtract => Self::ReverseSubtract,
      GPUBlendOperation::Min => Self::Min,
      GPUBlendOperation::Max => Self::Max,
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUBlendFactor {
  #[webidl(rename = "zero")]
  Zero,
  #[webidl(rename = "one")]
  One,
  #[webidl(rename = "src")]
  Src,
  #[webidl(rename = "one-minus-src")]
  OneMinusSrc,
  #[webidl(rename = "src-alpha")]
  SrcAlpha,
  #[webidl(rename = "one-minus-src-alpha")]
  OneMinusSrcAlpha,
  #[webidl(rename = "dst")]
  Dst,
  #[webidl(rename = "one-minus-dst")]
  OneMinusDst,
  #[webidl(rename = "dst-alpha")]
  DstAlpha,
  #[webidl(rename = "one-minus-dst-alpha")]
  OneMinusDstAlpha,
  #[webidl(rename = "src-alpha-saturated")]
  SrcAlphaSaturated,
  #[webidl(rename = "constant")]
  Constant,
  #[webidl(rename = "one-minus-constant")]
  OneMinusConstant,
  #[webidl(rename = "src1")]
  Src1,
  #[webidl(rename = "one-minus-src1")]
  OneMinusSrc1,
  #[webidl(rename = "src1-alpha")]
  Src1Alpha,
  #[webidl(rename = "one-minus-src1-alpha")]
  OneMinusSrc1Alpha,
}

impl From<GPUBlendFactor> for wgpu_types::BlendFactor {
  fn from(value: GPUBlendFactor) -> Self {
    match value {
      GPUBlendFactor::Zero => Self::Zero,
      GPUBlendFactor::One => Self::One,
      GPUBlendFactor::Src => Self::Src,
      GPUBlendFactor::OneMinusSrc => Self::OneMinusSrc,
      GPUBlendFactor::SrcAlpha => Self::SrcAlpha,
      GPUBlendFactor::OneMinusSrcAlpha => Self::OneMinusSrcAlpha,
      GPUBlendFactor::Dst => Self::Dst,
      GPUBlendFactor::OneMinusDst => Self::OneMinusDst,
      GPUBlendFactor::DstAlpha => Self::DstAlpha,
      GPUBlendFactor::OneMinusDstAlpha => Self::OneMinusDstAlpha,
      GPUBlendFactor::SrcAlphaSaturated => Self::SrcAlphaSaturated,
      GPUBlendFactor::Constant => Self::Constant,
      GPUBlendFactor::OneMinusConstant => Self::OneMinusConstant,
      GPUBlendFactor::Src1 => Self::Src1,
      GPUBlendFactor::OneMinusSrc1 => Self::OneMinusSrc1,
      GPUBlendFactor::Src1Alpha => Self::Src1Alpha,
      GPUBlendFactor::OneMinusSrc1Alpha => Self::OneMinusSrc1Alpha,
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUPrimitiveState {
  #[webidl(default = GPUPrimitiveTopology::TriangleList)]
  pub topology: GPUPrimitiveTopology,
  pub strip_index_format: Option<GPUIndexFormat>,
  #[webidl(default = GPUFrontFace::Ccw)]
  pub front_face: GPUFrontFace,
  #[webidl(default = GPUCullMode::None)]
  pub cull_mode: GPUCullMode,
  #[webidl(default = false)]
  pub unclipped_depth: bool,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUPrimitiveTopology {
  PointList,
  LineList,
  LineStrip,
  TriangleList,
  TriangleStrip,
}

impl From<GPUPrimitiveTopology> for wgpu_types::PrimitiveTopology {
  fn from(value: GPUPrimitiveTopology) -> Self {
    match value {
      GPUPrimitiveTopology::PointList => Self::PointList,
      GPUPrimitiveTopology::LineList => Self::LineList,
      GPUPrimitiveTopology::LineStrip => Self::LineStrip,
      GPUPrimitiveTopology::TriangleList => Self::TriangleList,
      GPUPrimitiveTopology::TriangleStrip => Self::TriangleStrip,
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUIndexFormat {
  #[webidl(rename = "uint16")]
  Uint16,
  #[webidl(rename = "uint32")]
  Uint32,
}

impl From<GPUIndexFormat> for wgpu_types::IndexFormat {
  fn from(value: GPUIndexFormat) -> Self {
    match value {
      GPUIndexFormat::Uint16 => Self::Uint16,
      GPUIndexFormat::Uint32 => Self::Uint32,
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUFrontFace {
  Ccw,
  Cw,
}

impl From<GPUFrontFace> for wgpu_types::FrontFace {
  fn from(value: GPUFrontFace) -> Self {
    match value {
      GPUFrontFace::Ccw => Self::Ccw,
      GPUFrontFace::Cw => Self::Cw,
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUCullMode {
  None,
  Front,
  Back,
}

impl From<GPUCullMode> for Option<wgpu_types::Face> {
  fn from(value: GPUCullMode) -> Self {
    match value {
      GPUCullMode::None => None,
      GPUCullMode::Front => Some(wgpu_types::Face::Front),
      GPUCullMode::Back => Some(wgpu_types::Face::Back),
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUVertexBufferLayout {
  #[options(enforce_range = true)]
  pub array_stride: u64,
  #[webidl(default = GPUVertexStepMode::Vertex)]
  pub step_mode: GPUVertexStepMode,
  pub attributes: Vec<GPUVertexAttribute>,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUVertexStepMode {
  Vertex,
  Instance,
}

impl From<GPUVertexStepMode> for wgpu_types::VertexStepMode {
  fn from(value: GPUVertexStepMode) -> Self {
    match value {
      GPUVertexStepMode::Vertex => Self::Vertex,
      GPUVertexStepMode::Instance => Self::Instance,
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUVertexAttribute {
  pub format: GPUVertexFormat,
  #[options(enforce_range = true)]
  pub offset: u64,
  #[options(enforce_range = true)]
  pub shader_location: u32,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUVertexFormat {
  // #[webidl(rename = "uint8")]
  // Uint8,
  #[webidl(rename = "uint8x2")]
  Uint8x2,
  #[webidl(rename = "uint8x4")]
  Uint8x4,
  // #[webidl(rename = "sint8")]
  // Sint8,
  #[webidl(rename = "sint8x2")]
  Sint8x2,
  #[webidl(rename = "sint8x4")]
  Sint8x4,
  // #[webidl(rename = "unorm8")]
  // Unorm8,
  #[webidl(rename = "unorm8x2")]
  Unorm8x2,
  #[webidl(rename = "unorm8x4")]
  Unorm8x4,
  // #[webidl(rename = "snorm8")]
  // Snorm8,
  #[webidl(rename = "snorm8x2")]
  Snorm8x2,
  #[webidl(rename = "snorm8x4")]
  Snorm8x4,
  // #[webidl(rename = "uint16")]
  // Uint16,
  #[webidl(rename = "uint16x2")]
  Uint16x2,
  #[webidl(rename = "uint16x4")]
  Uint16x4,
  // #[webidl(rename = "sint16")]
  // Sint16,
  #[webidl(rename = "sint16x2")]
  Sint16x2,
  #[webidl(rename = "sint16x4")]
  Sint16x4,
  // #[webidl(rename = "unorm16")]
  // Unorm16,
  #[webidl(rename = "unorm16x2")]
  Unorm16x2,
  #[webidl(rename = "unorm16x4")]
  Unorm16x4,
  // #[webidl(rename = "snorm16")]
  // Snorm16,
  #[webidl(rename = "snorm16x2")]
  Snorm16x2,
  #[webidl(rename = "snorm16x4")]
  Snorm16x4,
  // #[webidl(rename = "float16")]
  // Float16,
  #[webidl(rename = "float16x2")]
  Float16x2,
  #[webidl(rename = "float16x4")]
  Float16x4,
  #[webidl(rename = "float32")]
  Float32,
  #[webidl(rename = "float32x2")]
  Float32x2,
  #[webidl(rename = "float32x3")]
  Float32x3,
  #[webidl(rename = "float32x4")]
  Float32x4,
  #[webidl(rename = "uint32")]
  Uint32,
  #[webidl(rename = "uint32x2")]
  Uint32x2,
  #[webidl(rename = "uint32x3")]
  Uint32x3,
  #[webidl(rename = "uint32x4")]
  Uint32x4,
  #[webidl(rename = "sint32")]
  Sint32,
  #[webidl(rename = "sint32x2")]
  Sint32x2,
  #[webidl(rename = "sint32x3")]
  Sint32x3,
  #[webidl(rename = "sint32x4")]
  Sint32x4,
  #[webidl(rename = "unorm10-10-10-2")]
  Unorm1010102,
  // #[webidl(rename = "unorm8x4-bgra")]
  // Unorm8x4Bgra,
}

impl From<GPUVertexFormat> for wgpu_types::VertexFormat {
  fn from(value: GPUVertexFormat) -> Self {
    match value {
      //GPUVertexFormat::Uint8 => Self::Uint8,
      GPUVertexFormat::Uint8x2 => Self::Uint8x2,
      GPUVertexFormat::Uint8x4 => Self::Uint8x4,
      //GPUVertexFormat::Sint8 => Self::Sint8,
      GPUVertexFormat::Sint8x2 => Self::Sint8x2,
      GPUVertexFormat::Sint8x4 => Self::Sint8x4,
      //GPUVertexFormat::Unorm8 => Self::Unorm8,
      GPUVertexFormat::Unorm8x2 => Self::Unorm8x2,
      GPUVertexFormat::Unorm8x4 => Self::Unorm8x4,
      //GPUVertexFormat::Snorm8 => Self::Snorm8,
      GPUVertexFormat::Snorm8x2 => Self::Snorm8x2,
      GPUVertexFormat::Snorm8x4 => Self::Snorm8x4,
      //GPUVertexFormat::Uint16 => Self::Uint16,
      GPUVertexFormat::Uint16x2 => Self::Uint16x2,
      GPUVertexFormat::Uint16x4 => Self::Uint16x4,
      //GPUVertexFormat::Sint16 => Self::Sint16,
      GPUVertexFormat::Sint16x2 => Self::Sint16x2,
      GPUVertexFormat::Sint16x4 => Self::Sint16x4,
      //GPUVertexFormat::Unorm16 => Self::Unorm16,
      GPUVertexFormat::Unorm16x2 => Self::Unorm16x2,
      GPUVertexFormat::Unorm16x4 => Self::Unorm16x4,
      //GPUVertexFormat::Snorm16 => Self::Snorm16,
      GPUVertexFormat::Snorm16x2 => Self::Snorm16x2,
      GPUVertexFormat::Snorm16x4 => Self::Snorm16x4,
      //GPUVertexFormat::Float16 => Self::Float16,
      GPUVertexFormat::Float16x2 => Self::Float16x2,
      GPUVertexFormat::Float16x4 => Self::Float16x4,
      GPUVertexFormat::Float32 => Self::Float32,
      GPUVertexFormat::Float32x2 => Self::Float32x2,
      GPUVertexFormat::Float32x3 => Self::Float32x3,
      GPUVertexFormat::Float32x4 => Self::Float32x4,
      GPUVertexFormat::Uint32 => Self::Uint32,
      GPUVertexFormat::Uint32x2 => Self::Uint32x2,
      GPUVertexFormat::Uint32x3 => Self::Uint32x3,
      GPUVertexFormat::Uint32x4 => Self::Uint32x4,
      GPUVertexFormat::Sint32 => Self::Sint32,
      GPUVertexFormat::Sint32x2 => Self::Sint32x2,
      GPUVertexFormat::Sint32x3 => Self::Sint32x3,
      GPUVertexFormat::Sint32x4 => Self::Sint32x4,
      GPUVertexFormat::Unorm1010102 => Self::Unorm10_10_10_2,
      //GPUVertexFormat::Unorm8x4Bgra => Self::Unorm8x4Bgra,
    }
  }
}
