// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ResourceId;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::convert::{TryFrom, TryInto};

use super::error::WebGpuError;
use super::error::WebGpuResult;

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

#[op]
pub fn op_webgpu_create_compute_pipeline(
  state: &mut OpState,
  args: CreateComputePipelineArgs,
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

#[op]
pub fn op_webgpu_compute_pipeline_get_bind_group_layout(
  state: &mut OpState,
  args: ComputePipelineGetBindGroupLayoutArgs,
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
struct GpuPrimitiveState {
  topology: wgpu_types::PrimitiveTopology,
  strip_index_format: Option<wgpu_types::IndexFormat>,
  front_face: wgpu_types::FrontFace,
  cull_mode: GpuCullMode,
  unclipped_depth: bool,
}

impl From<GpuPrimitiveState> for wgpu_types::PrimitiveState {
  fn from(value: GpuPrimitiveState) -> wgpu_types::PrimitiveState {
    wgpu_types::PrimitiveState {
      topology: value.topology,
      strip_index_format: value.strip_index_format,
      front_face: value.front_face,
      cull_mode: value.cull_mode.into(),
      unclipped_depth: value.unclipped_depth,
      polygon_mode: Default::default(), // native-only
      conservative: false,              // native-only
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuDepthStencilState {
  format: wgpu_types::TextureFormat,
  depth_write_enabled: bool,
  depth_compare: wgpu_types::CompareFunction,
  stencil_front: wgpu_types::StencilFaceState,
  stencil_back: wgpu_types::StencilFaceState,
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
      format: state.format,
      depth_write_enabled: state.depth_write_enabled,
      depth_compare: state.depth_compare,
      stencil: wgpu_types::StencilState {
        front: state.stencil_front,
        back: state.stencil_back,
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
struct GpuVertexBufferLayout {
  array_stride: u64,
  step_mode: wgpu_types::VertexStepMode,
  attributes: Vec<wgpu_types::VertexAttribute>,
}

impl<'a> From<GpuVertexBufferLayout>
  for wgpu_core::pipeline::VertexBufferLayout<'a>
{
  fn from(
    layout: GpuVertexBufferLayout,
  ) -> wgpu_core::pipeline::VertexBufferLayout<'a> {
    wgpu_core::pipeline::VertexBufferLayout {
      array_stride: layout.array_stride,
      step_mode: layout.step_mode,
      attributes: Cow::Owned(layout.attributes),
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
  targets: Vec<wgpu_types::ColorTargetState>,
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
  multisample: wgpu_types::MultisampleState,
  fragment: Option<GpuFragmentState>,
}

#[op]
pub fn op_webgpu_create_render_pipeline(
  state: &mut OpState,
  args: CreateRenderPipelineArgs,
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
      targets.push(target);
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
    multisample: args.multisample,
    fragment,
    multiview: None,
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

#[op]
pub fn op_webgpu_render_pipeline_get_bind_group_layout(
  state: &mut OpState,
  args: RenderPipelineGetBindGroupLayoutArgs,
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
