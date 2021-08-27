// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::ResourceId;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::convert::{TryFrom, TryInto};

use crate::sampler::GpuCompareFunction;
use crate::texture::GpuTextureFormat;

use super::error::{WebGpuError, WebGpuResult};

const MAX_BIND_GROUPS: usize = 8;

pub(crate) struct WebGpuPipelineLayout(
  pub(crate) wgpu_core::id::PipelineLayoutId,
);
impl Resource for WebGpuPipelineLayout {
  fn name(&self) -> Cow<str> {
    "webGPUPipelineLayout".into()
  }
}

pub(crate) struct WebGpuComputePipeline(
  pub(crate) wgpu_core::id::ComputePipelineId,
);
impl Resource for WebGpuComputePipeline {
  fn name(&self) -> Cow<str> {
    "webGPUComputePipeline".into()
  }
}

pub(crate) struct WebGpuRenderPipeline(
  pub(crate) wgpu_core::id::RenderPipelineId,
);
impl Resource for WebGpuRenderPipeline {
  fn name(&self) -> Cow<str> {
    "webGPURenderPipeline".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuIndexFormat {
  Uint16,
  Uint32,
}

impl From<GpuIndexFormat> for wgpu_types::IndexFormat {
  fn from(value: GpuIndexFormat) -> wgpu_types::IndexFormat {
    match value {
      GpuIndexFormat::Uint16 => wgpu_types::IndexFormat::Uint16,
      GpuIndexFormat::Uint32 => wgpu_types::IndexFormat::Uint32,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GPUStencilOperation {
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
  fn from(value: GPUStencilOperation) -> wgpu_types::StencilOperation {
    match value {
      GPUStencilOperation::Keep => wgpu_types::StencilOperation::Keep,
      GPUStencilOperation::Zero => wgpu_types::StencilOperation::Zero,
      GPUStencilOperation::Replace => wgpu_types::StencilOperation::Replace,
      GPUStencilOperation::Invert => wgpu_types::StencilOperation::Invert,
      GPUStencilOperation::IncrementClamp => {
        wgpu_types::StencilOperation::IncrementClamp
      }
      GPUStencilOperation::DecrementClamp => {
        wgpu_types::StencilOperation::DecrementClamp
      }
      GPUStencilOperation::IncrementWrap => {
        wgpu_types::StencilOperation::IncrementWrap
      }
      GPUStencilOperation::DecrementWrap => {
        wgpu_types::StencilOperation::DecrementWrap
      }
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuBlendFactor {
  Zero,
  One,
  Src,
  OneMinusSrc,
  SrcAlpha,
  OneMinusSrcAlpha,
  Dst,
  OneMinusDst,
  DstAlpha,
  OneMinusDstAlpha,
  SrcAlphaSaturated,
  Constant,
  OneMinusConstant,
}

impl From<GpuBlendFactor> for wgpu_types::BlendFactor {
  fn from(value: GpuBlendFactor) -> wgpu_types::BlendFactor {
    match value {
      GpuBlendFactor::Zero => wgpu_types::BlendFactor::Zero,
      GpuBlendFactor::One => wgpu_types::BlendFactor::One,
      GpuBlendFactor::Src => wgpu_types::BlendFactor::Src,
      GpuBlendFactor::OneMinusSrc => wgpu_types::BlendFactor::OneMinusSrc,
      GpuBlendFactor::SrcAlpha => wgpu_types::BlendFactor::SrcAlpha,
      GpuBlendFactor::OneMinusSrcAlpha => {
        wgpu_types::BlendFactor::OneMinusSrcAlpha
      }
      GpuBlendFactor::Dst => wgpu_types::BlendFactor::Dst,
      GpuBlendFactor::OneMinusDst => wgpu_types::BlendFactor::OneMinusDst,
      GpuBlendFactor::DstAlpha => wgpu_types::BlendFactor::DstAlpha,
      GpuBlendFactor::OneMinusDstAlpha => {
        wgpu_types::BlendFactor::OneMinusDstAlpha
      }
      GpuBlendFactor::SrcAlphaSaturated => {
        wgpu_types::BlendFactor::SrcAlphaSaturated
      }
      GpuBlendFactor::Constant => wgpu_types::BlendFactor::Constant,
      GpuBlendFactor::OneMinusConstant => {
        wgpu_types::BlendFactor::OneMinusConstant
      }
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuBlendOperation {
  Add,
  Subtract,
  ReverseSubtract,
  Min,
  Max,
}

impl From<GpuBlendOperation> for wgpu_types::BlendOperation {
  fn from(value: GpuBlendOperation) -> wgpu_types::BlendOperation {
    match value {
      GpuBlendOperation::Add => wgpu_types::BlendOperation::Add,
      GpuBlendOperation::Subtract => wgpu_types::BlendOperation::Subtract,
      GpuBlendOperation::ReverseSubtract => {
        wgpu_types::BlendOperation::ReverseSubtract
      }
      GpuBlendOperation::Min => wgpu_types::BlendOperation::Min,
      GpuBlendOperation::Max => wgpu_types::BlendOperation::Max,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuPrimitiveTopology {
  PointList,
  LineList,
  LineStrip,
  TriangleList,
  TriangleStrip,
}

impl From<GpuPrimitiveTopology> for wgpu_types::PrimitiveTopology {
  fn from(value: GpuPrimitiveTopology) -> wgpu_types::PrimitiveTopology {
    match value {
      GpuPrimitiveTopology::PointList => {
        wgpu_types::PrimitiveTopology::PointList
      }
      GpuPrimitiveTopology::LineList => wgpu_types::PrimitiveTopology::LineList,
      GpuPrimitiveTopology::LineStrip => {
        wgpu_types::PrimitiveTopology::LineStrip
      }
      GpuPrimitiveTopology::TriangleList => {
        wgpu_types::PrimitiveTopology::TriangleList
      }
      GpuPrimitiveTopology::TriangleStrip => {
        wgpu_types::PrimitiveTopology::TriangleStrip
      }
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuFrontFace {
  Ccw,
  Cw,
}

impl From<GpuFrontFace> for wgpu_types::FrontFace {
  fn from(value: GpuFrontFace) -> wgpu_types::FrontFace {
    match value {
      GpuFrontFace::Ccw => wgpu_types::FrontFace::Ccw,
      GpuFrontFace::Cw => wgpu_types::FrontFace::Cw,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuCullMode {
  None,
  Front,
  Back,
}

impl From<GpuCullMode> for Option<wgpu_types::Face> {
  fn from(value: GpuCullMode) -> Option<wgpu_types::Face> {
    match value {
      GpuCullMode::None => None,
      GpuCullMode::Front => Some(wgpu_types::Face::Front),
      GpuCullMode::Back => Some(wgpu_types::Face::Back),
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuProgrammableStage {
  module: ResourceId,
  entry_point: String,
  // constants: HashMap<String, GPUPipelineConstantValue>
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateComputePipelineArgs {
  device_rid: ResourceId,
  label: Option<String>,
  layout: Option<ResourceId>,
  compute: GpuProgrammableStage,
}

pub fn op_webgpu_create_compute_pipeline(
  state: &mut OpState,
  args: CreateComputePipelineArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let pipeline_layout = if let Some(rid) = args.layout {
    let id = state.resource_table.get::<WebGpuPipelineLayout>(rid)?;
    Some(id.0)
  } else {
    None
  };

  let compute_shader_module_resource =
    state
      .resource_table
      .get::<super::shader::WebGpuShaderModule>(args.compute.module)?;

  let descriptor = wgpu_core::pipeline::ComputePipelineDescriptor {
    label: args.label.map(Cow::from),
    layout: pipeline_layout,
    stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
      module: compute_shader_module_resource.0,
      entry_point: Cow::from(args.compute.entry_point),
      // TODO(lucacasonato): support args.compute.constants
    },
  };
  let implicit_pipelines = match args.layout {
    Some(_) => None,
    None => Some(wgpu_core::device::ImplicitPipelineIds {
      root_id: std::marker::PhantomData,
      group_ids: &[std::marker::PhantomData; MAX_BIND_GROUPS],
    }),
  };

  let (compute_pipeline, maybe_err) = gfx_select!(device => instance.device_create_compute_pipeline(
    device,
    &descriptor,
    std::marker::PhantomData,
    implicit_pipelines
  ));

  let rid = state
    .resource_table
    .add(WebGpuComputePipeline(compute_pipeline));

  Ok(WebGpuResult::rid_err(rid, maybe_err))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputePipelineGetBindGroupLayoutArgs {
  compute_pipeline_rid: ResourceId,
  index: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineLayout {
  rid: ResourceId,
  label: String,
  err: Option<WebGpuError>,
}

pub fn op_webgpu_compute_pipeline_get_bind_group_layout(
  state: &mut OpState,
  args: ComputePipelineGetBindGroupLayoutArgs,
  _: (),
) -> Result<PipelineLayout, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let compute_pipeline_resource = state
    .resource_table
    .get::<WebGpuComputePipeline>(args.compute_pipeline_rid)?;
  let compute_pipeline = compute_pipeline_resource.0;

  let (bind_group_layout, maybe_err) = gfx_select!(compute_pipeline => instance.compute_pipeline_get_bind_group_layout(compute_pipeline, args.index, std::marker::PhantomData));

  let label = gfx_select!(bind_group_layout => instance.bind_group_layout_label(bind_group_layout));

  let rid = state
    .resource_table
    .add(super::binding::WebGpuBindGroupLayout(bind_group_layout));

  Ok(PipelineLayout {
    rid,
    label,
    err: maybe_err.map(WebGpuError::from),
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuPrimitiveState {
  topology: GpuPrimitiveTopology,
  strip_index_format: Option<GpuIndexFormat>,
  front_face: GpuFrontFace,
  cull_mode: GpuCullMode,
  clamp_depth: bool,
}

impl From<GpuPrimitiveState> for wgpu_types::PrimitiveState {
  fn from(value: GpuPrimitiveState) -> wgpu_types::PrimitiveState {
    wgpu_types::PrimitiveState {
      topology: value.topology.into(),
      strip_index_format: value.strip_index_format.map(Into::into),
      front_face: value.front_face.into(),
      cull_mode: value.cull_mode.into(),
      clamp_depth: value.clamp_depth,
      polygon_mode: Default::default(), // native-only
      conservative: false,              // native-only
    }
  }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuBlendComponent {
  src_factor: GpuBlendFactor,
  dst_factor: GpuBlendFactor,
  operation: GpuBlendOperation,
}

impl From<GpuBlendComponent> for wgpu_types::BlendComponent {
  fn from(component: GpuBlendComponent) -> Self {
    wgpu_types::BlendComponent {
      src_factor: component.src_factor.into(),
      dst_factor: component.dst_factor.into(),
      operation: component.operation.into(),
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuBlendState {
  color: GpuBlendComponent,
  alpha: GpuBlendComponent,
}

impl From<GpuBlendState> for wgpu_types::BlendState {
  fn from(state: GpuBlendState) -> wgpu_types::BlendState {
    wgpu_types::BlendState {
      color: state.color.into(),
      alpha: state.alpha.into(),
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuColorTargetState {
  format: GpuTextureFormat,
  blend: Option<GpuBlendState>,
  write_mask: u32,
}

impl TryFrom<GpuColorTargetState> for wgpu_types::ColorTargetState {
  type Error = AnyError;
  fn try_from(
    state: GpuColorTargetState,
  ) -> Result<wgpu_types::ColorTargetState, AnyError> {
    Ok(wgpu_types::ColorTargetState {
      format: state.format.try_into()?,
      blend: state.blend.map(Into::into),
      write_mask: wgpu_types::ColorWrites::from_bits_truncate(state.write_mask),
    })
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuStencilFaceState {
  compare: GpuCompareFunction,
  fail_op: GPUStencilOperation,
  depth_fail_op: GPUStencilOperation,
  pass_op: GPUStencilOperation,
}

impl From<GpuStencilFaceState> for wgpu_types::StencilFaceState {
  fn from(state: GpuStencilFaceState) -> Self {
    wgpu_types::StencilFaceState {
      compare: state.compare.into(),
      fail_op: state.fail_op.into(),
      depth_fail_op: state.depth_fail_op.into(),
      pass_op: state.pass_op.into(),
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuDepthStencilState {
  format: GpuTextureFormat,
  depth_write_enabled: bool,
  depth_compare: GpuCompareFunction,
  stencil_front: GpuStencilFaceState,
  stencil_back: GpuStencilFaceState,
  stencil_read_mask: u32,
  stencil_write_mask: u32,
  depth_bias: i32,
  depth_bias_slope_scale: f32,
  depth_bias_clamp: f32,
}

impl TryFrom<GpuDepthStencilState> for wgpu_types::DepthStencilState {
  type Error = AnyError;
  fn try_from(
    state: GpuDepthStencilState,
  ) -> Result<wgpu_types::DepthStencilState, AnyError> {
    Ok(wgpu_types::DepthStencilState {
      format: state.format.try_into()?,
      depth_write_enabled: state.depth_write_enabled,
      depth_compare: state.depth_compare.into(),
      stencil: wgpu_types::StencilState {
        front: state.stencil_front.into(),
        back: state.stencil_back.into(),
        read_mask: state.stencil_read_mask,
        write_mask: state.stencil_write_mask,
      },
      bias: wgpu_types::DepthBiasState {
        constant: state.depth_bias,
        slope_scale: state.depth_bias_slope_scale,
        clamp: state.depth_bias_clamp,
      },
    })
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuVertexAttribute {
  format: GpuVertexFormat,
  offset: u64,
  shader_location: u32,
}

impl From<GpuVertexAttribute> for wgpu_types::VertexAttribute {
  fn from(attribute: GpuVertexAttribute) -> Self {
    wgpu_types::VertexAttribute {
      format: attribute.format.into(),
      offset: attribute.offset,
      shader_location: attribute.shader_location,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum GpuVertexFormat {
  Uint8x2,
  Uint8x4,
  Sint8x2,
  Sint8x4,
  Unorm8x2,
  Unorm8x4,
  Snorm8x2,
  Snorm8x4,
  Uint16x2,
  Uint16x4,
  Sint16x2,
  Sint16x4,
  Unorm16x2,
  Unorm16x4,
  Snorm16x2,
  Snorm16x4,
  Float16x2,
  Float16x4,
  Float32,
  Float32x2,
  Float32x3,
  Float32x4,
  Uint32,
  Uint32x2,
  Uint32x3,
  Uint32x4,
  Sint32,
  Sint32x2,
  Sint32x3,
  Sint32x4,
  Float64,
  Float64x2,
  Float64x3,
  Float64x4,
}

impl From<GpuVertexFormat> for wgpu_types::VertexFormat {
  fn from(vf: GpuVertexFormat) -> wgpu_types::VertexFormat {
    use wgpu_types::VertexFormat;
    match vf {
      GpuVertexFormat::Uint8x2 => VertexFormat::Uint8x2,
      GpuVertexFormat::Uint8x4 => VertexFormat::Uint8x4,
      GpuVertexFormat::Sint8x2 => VertexFormat::Sint8x2,
      GpuVertexFormat::Sint8x4 => VertexFormat::Sint8x4,
      GpuVertexFormat::Unorm8x2 => VertexFormat::Unorm8x2,
      GpuVertexFormat::Unorm8x4 => VertexFormat::Unorm8x4,
      GpuVertexFormat::Snorm8x2 => VertexFormat::Snorm8x2,
      GpuVertexFormat::Snorm8x4 => VertexFormat::Snorm8x4,
      GpuVertexFormat::Uint16x2 => VertexFormat::Uint16x2,
      GpuVertexFormat::Uint16x4 => VertexFormat::Uint16x4,
      GpuVertexFormat::Sint16x2 => VertexFormat::Sint16x2,
      GpuVertexFormat::Sint16x4 => VertexFormat::Sint16x4,
      GpuVertexFormat::Unorm16x2 => VertexFormat::Unorm16x2,
      GpuVertexFormat::Unorm16x4 => VertexFormat::Unorm16x4,
      GpuVertexFormat::Snorm16x2 => VertexFormat::Snorm16x2,
      GpuVertexFormat::Snorm16x4 => VertexFormat::Snorm16x4,
      GpuVertexFormat::Float16x2 => VertexFormat::Float16x2,
      GpuVertexFormat::Float16x4 => VertexFormat::Float16x4,
      GpuVertexFormat::Float32 => VertexFormat::Float32,
      GpuVertexFormat::Float32x2 => VertexFormat::Float32x2,
      GpuVertexFormat::Float32x3 => VertexFormat::Float32x3,
      GpuVertexFormat::Float32x4 => VertexFormat::Float32x4,
      GpuVertexFormat::Uint32 => VertexFormat::Uint32,
      GpuVertexFormat::Uint32x2 => VertexFormat::Uint32x2,
      GpuVertexFormat::Uint32x3 => VertexFormat::Uint32x3,
      GpuVertexFormat::Uint32x4 => VertexFormat::Uint32x4,
      GpuVertexFormat::Sint32 => VertexFormat::Sint32,
      GpuVertexFormat::Sint32x2 => VertexFormat::Sint32x2,
      GpuVertexFormat::Sint32x3 => VertexFormat::Sint32x3,
      GpuVertexFormat::Sint32x4 => VertexFormat::Sint32x4,
      GpuVertexFormat::Float64 => VertexFormat::Float64,
      GpuVertexFormat::Float64x2 => VertexFormat::Float64x2,
      GpuVertexFormat::Float64x3 => VertexFormat::Float64x3,
      GpuVertexFormat::Float64x4 => VertexFormat::Float64x4,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum GpuVertexStepMode {
  Vertex,
  Instance,
}

impl From<GpuVertexStepMode> for wgpu_types::VertexStepMode {
  fn from(vsm: GpuVertexStepMode) -> wgpu_types::VertexStepMode {
    use wgpu_types::VertexStepMode;
    match vsm {
      GpuVertexStepMode::Vertex => VertexStepMode::Vertex,
      GpuVertexStepMode::Instance => VertexStepMode::Instance,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuVertexBufferLayout {
  array_stride: u64,
  step_mode: GpuVertexStepMode,
  attributes: Vec<GpuVertexAttribute>,
}

impl<'a> From<GpuVertexBufferLayout>
  for wgpu_core::pipeline::VertexBufferLayout<'a>
{
  fn from(
    layout: GpuVertexBufferLayout,
  ) -> wgpu_core::pipeline::VertexBufferLayout<'a> {
    wgpu_core::pipeline::VertexBufferLayout {
      array_stride: layout.array_stride,
      step_mode: layout.step_mode.into(),
      attributes: Cow::Owned(
        layout.attributes.into_iter().map(Into::into).collect(),
      ),
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuVertexState {
  module: ResourceId,
  entry_point: String,
  buffers: Vec<Option<GpuVertexBufferLayout>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuMultisampleState {
  count: u32,
  mask: u64,
  alpha_to_coverage_enabled: bool,
}

impl From<GpuMultisampleState> for wgpu_types::MultisampleState {
  fn from(gms: GpuMultisampleState) -> wgpu_types::MultisampleState {
    wgpu_types::MultisampleState {
      count: gms.count,
      mask: gms.mask,
      alpha_to_coverage_enabled: gms.alpha_to_coverage_enabled,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuFragmentState {
  targets: Vec<GpuColorTargetState>,
  module: u32,
  entry_point: String,
  // TODO(lucacasonato): constants
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRenderPipelineArgs {
  device_rid: ResourceId,
  label: Option<String>,
  layout: Option<ResourceId>,
  vertex: GpuVertexState,
  primitive: GpuPrimitiveState,
  depth_stencil: Option<GpuDepthStencilState>,
  multisample: GpuMultisampleState,
  fragment: Option<GpuFragmentState>,
}

pub fn op_webgpu_create_render_pipeline(
  state: &mut OpState,
  args: CreateRenderPipelineArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let layout = if let Some(rid) = args.layout {
    let pipeline_layout_resource =
      state.resource_table.get::<WebGpuPipelineLayout>(rid)?;
    Some(pipeline_layout_resource.0)
  } else {
    None
  };

  let vertex_shader_module_resource =
    state
      .resource_table
      .get::<super::shader::WebGpuShaderModule>(args.vertex.module)?;

  let fragment = if let Some(fragment) = args.fragment {
    let fragment_shader_module_resource =
      state
        .resource_table
        .get::<super::shader::WebGpuShaderModule>(fragment.module)?;

    let mut targets = Vec::with_capacity(fragment.targets.len());

    for target in fragment.targets {
      targets.push(target.try_into()?);
    }

    Some(wgpu_core::pipeline::FragmentState {
      stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
        module: fragment_shader_module_resource.0,
        entry_point: Cow::from(fragment.entry_point),
      },
      targets: Cow::from(targets),
    })
  } else {
    None
  };

  let vertex_buffers = args
    .vertex
    .buffers
    .into_iter()
    .flatten()
    .map(Into::into)
    .collect();

  let descriptor = wgpu_core::pipeline::RenderPipelineDescriptor {
    label: args.label.map(Cow::Owned),
    layout,
    vertex: wgpu_core::pipeline::VertexState {
      stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
        module: vertex_shader_module_resource.0,
        entry_point: Cow::Owned(args.vertex.entry_point),
      },
      buffers: Cow::Owned(vertex_buffers),
    },
    primitive: args.primitive.into(),
    depth_stencil: args.depth_stencil.map(TryInto::try_into).transpose()?,
    multisample: args.multisample.into(),
    fragment,
  };

  let implicit_pipelines = match args.layout {
    Some(_) => None,
    None => Some(wgpu_core::device::ImplicitPipelineIds {
      root_id: std::marker::PhantomData,
      group_ids: &[std::marker::PhantomData; MAX_BIND_GROUPS],
    }),
  };

  let (render_pipeline, maybe_err) = gfx_select!(device => instance.device_create_render_pipeline(
    device,
    &descriptor,
    std::marker::PhantomData,
    implicit_pipelines
  ));

  let rid = state
    .resource_table
    .add(WebGpuRenderPipeline(render_pipeline));

  Ok(WebGpuResult::rid_err(rid, maybe_err))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPipelineGetBindGroupLayoutArgs {
  render_pipeline_rid: ResourceId,
  index: u32,
}

pub fn op_webgpu_render_pipeline_get_bind_group_layout(
  state: &mut OpState,
  args: RenderPipelineGetBindGroupLayoutArgs,
  _: (),
) -> Result<PipelineLayout, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let render_pipeline_resource = state
    .resource_table
    .get::<WebGpuRenderPipeline>(args.render_pipeline_rid)?;
  let render_pipeline = render_pipeline_resource.0;

  let (bind_group_layout, maybe_err) = gfx_select!(render_pipeline => instance.render_pipeline_get_bind_group_layout(render_pipeline, args.index, std::marker::PhantomData));

  let label = gfx_select!(bind_group_layout => instance.bind_group_layout_label(bind_group_layout));

  let rid = state
    .resource_table
    .add(super::binding::WebGpuBindGroupLayout(bind_group_layout));

  Ok(PipelineLayout {
    rid,
    label,
    err: maybe_err.map(WebGpuError::from),
  })
}
