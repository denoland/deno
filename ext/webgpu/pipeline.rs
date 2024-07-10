// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;

use super::error::WebGpuError;
use super::error::WebGpuResult;

const MAX_BIND_GROUPS: usize = 8;

pub(crate) struct WebGpuPipelineLayout(
  pub(crate) crate::Instance,
  pub(crate) wgpu_core::id::PipelineLayoutId,
);
impl Resource for WebGpuPipelineLayout {
  fn name(&self) -> Cow<str> {
    "webGPUPipelineLayout".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.pipeline_layout_drop(self.1));
  }
}

pub(crate) struct WebGpuComputePipeline(
  pub(crate) crate::Instance,
  pub(crate) wgpu_core::id::ComputePipelineId,
);
impl Resource for WebGpuComputePipeline {
  fn name(&self) -> Cow<str> {
    "webGPUComputePipeline".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.compute_pipeline_drop(self.1));
  }
}

pub(crate) struct WebGpuRenderPipeline(
  pub(crate) crate::Instance,
  pub(crate) wgpu_core::id::RenderPipelineId,
);
impl Resource for WebGpuRenderPipeline {
  fn name(&self) -> Cow<str> {
    "webGPURenderPipeline".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.render_pipeline_drop(self.1));
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GPUAutoLayoutMode {
  Auto,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum GPUPipelineLayoutOrGPUAutoLayoutMode {
  Layout(ResourceId),
  Auto(GPUAutoLayoutMode),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuProgrammableStage {
  module: ResourceId,
  entry_point: Option<String>,
  constants: Option<HashMap<String, f64>>,
}

#[op2]
#[serde]
pub fn op_webgpu_create_compute_pipeline(
  state: &mut OpState,
  #[smi] device_rid: ResourceId,
  #[string] label: Cow<str>,
  #[serde] layout: GPUPipelineLayoutOrGPUAutoLayoutMode,
  #[serde] compute: GpuProgrammableStage,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.1;

  let pipeline_layout = match layout {
    GPUPipelineLayoutOrGPUAutoLayoutMode::Layout(rid) => {
      let id = state.resource_table.get::<WebGpuPipelineLayout>(rid)?;
      Some(id.1)
    }
    GPUPipelineLayoutOrGPUAutoLayoutMode::Auto(GPUAutoLayoutMode::Auto) => None,
  };

  let compute_shader_module_resource =
    state
      .resource_table
      .get::<super::shader::WebGpuShaderModule>(compute.module)?;

  let descriptor = wgpu_core::pipeline::ComputePipelineDescriptor {
    label: Some(label),
    layout: pipeline_layout,
    stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
      module: compute_shader_module_resource.1,
      entry_point: compute.entry_point.map(Cow::from),
      constants: Cow::Owned(compute.constants.unwrap_or_default()),
      zero_initialize_workgroup_memory: true,
    },
  };
  let implicit_pipelines = match layout {
    GPUPipelineLayoutOrGPUAutoLayoutMode::Layout(_) => None,
    GPUPipelineLayoutOrGPUAutoLayoutMode::Auto(GPUAutoLayoutMode::Auto) => {
      Some(wgpu_core::device::ImplicitPipelineIds {
        root_id: None,
        group_ids: &[None; MAX_BIND_GROUPS],
      })
    }
  };

  let (compute_pipeline, maybe_err) = gfx_select!(device => instance.device_create_compute_pipeline(
    device,
    &descriptor,
    None,
    implicit_pipelines
  ));

  let rid = state
    .resource_table
    .add(WebGpuComputePipeline(instance.clone(), compute_pipeline));

  Ok(WebGpuResult::rid_err(rid, maybe_err))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineLayout {
  rid: ResourceId,
  label: String,
  err: Option<WebGpuError>,
}

#[op2]
#[serde]
pub fn op_webgpu_compute_pipeline_get_bind_group_layout(
  state: &mut OpState,
  #[smi] compute_pipeline_rid: ResourceId,
  index: u32,
) -> Result<PipelineLayout, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let compute_pipeline_resource = state
    .resource_table
    .get::<WebGpuComputePipeline>(compute_pipeline_rid)?;
  let compute_pipeline = compute_pipeline_resource.1;

  let (bind_group_layout, maybe_err) = gfx_select!(compute_pipeline => instance.compute_pipeline_get_bind_group_layout(compute_pipeline, index, None));

  let label = gfx_select!(bind_group_layout => instance.bind_group_layout_label(bind_group_layout));

  let rid = state
    .resource_table
    .add(super::binding::WebGpuBindGroupLayout(
      instance.clone(),
      bind_group_layout,
    ));

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

impl From<GpuDepthStencilState> for wgpu_types::DepthStencilState {
  fn from(state: GpuDepthStencilState) -> wgpu_types::DepthStencilState {
    wgpu_types::DepthStencilState {
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
    }
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
  entry_point: Option<String>,
  constants: Option<HashMap<String, f64>>,
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
  targets: Vec<Option<wgpu_types::ColorTargetState>>,
  module: u32,
  entry_point: Option<String>,
  constants: Option<HashMap<String, f64>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRenderPipelineArgs {
  device_rid: ResourceId,
  label: String,
  layout: GPUPipelineLayoutOrGPUAutoLayoutMode,
  vertex: GpuVertexState,
  primitive: GpuPrimitiveState,
  depth_stencil: Option<GpuDepthStencilState>,
  multisample: wgpu_types::MultisampleState,
  fragment: Option<GpuFragmentState>,
}

#[op2]
#[serde]
pub fn op_webgpu_create_render_pipeline(
  state: &mut OpState,
  #[serde] args: CreateRenderPipelineArgs,
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.1;

  let layout = match args.layout {
    GPUPipelineLayoutOrGPUAutoLayoutMode::Layout(rid) => {
      let pipeline_layout_resource =
        state.resource_table.get::<WebGpuPipelineLayout>(rid)?;
      Some(pipeline_layout_resource.1)
    }
    GPUPipelineLayoutOrGPUAutoLayoutMode::Auto(GPUAutoLayoutMode::Auto) => None,
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

    Some(wgpu_core::pipeline::FragmentState {
      stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
        module: fragment_shader_module_resource.1,
        entry_point: fragment.entry_point.map(Cow::from),
        constants: Cow::Owned(fragment.constants.unwrap_or_default()),
        // Required to be true for WebGPU
        zero_initialize_workgroup_memory: true,
      },
      targets: Cow::Owned(fragment.targets),
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
    label: Some(Cow::Owned(args.label)),
    layout,
    vertex: wgpu_core::pipeline::VertexState {
      stage: wgpu_core::pipeline::ProgrammableStageDescriptor {
        module: vertex_shader_module_resource.1,
        entry_point: args.vertex.entry_point.map(Cow::Owned),
        constants: Cow::Owned(args.vertex.constants.unwrap_or_default()),
        // Required to be true for WebGPU
        zero_initialize_workgroup_memory: true,
      },
      buffers: Cow::Owned(vertex_buffers),
    },
    primitive: args.primitive.into(),
    depth_stencil: args.depth_stencil.map(Into::into),
    multisample: args.multisample,
    fragment,
    multiview: None,
  };

  let implicit_pipelines = match args.layout {
    GPUPipelineLayoutOrGPUAutoLayoutMode::Layout(_) => None,
    GPUPipelineLayoutOrGPUAutoLayoutMode::Auto(GPUAutoLayoutMode::Auto) => {
      Some(wgpu_core::device::ImplicitPipelineIds {
        root_id: None,
        group_ids: &[None; MAX_BIND_GROUPS],
      })
    }
  };

  let (render_pipeline, maybe_err) = gfx_select!(device => instance.device_create_render_pipeline(
    device,
    &descriptor,
    None,
    implicit_pipelines
  ));

  let rid = state
    .resource_table
    .add(WebGpuRenderPipeline(instance.clone(), render_pipeline));

  Ok(WebGpuResult::rid_err(rid, maybe_err))
}

#[op2]
#[serde]
pub fn op_webgpu_render_pipeline_get_bind_group_layout(
  state: &mut OpState,
  #[smi] render_pipeline_rid: ResourceId,
  index: u32,
) -> Result<PipelineLayout, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let render_pipeline_resource = state
    .resource_table
    .get::<WebGpuRenderPipeline>(render_pipeline_rid)?;
  let render_pipeline = render_pipeline_resource.1;

  let (bind_group_layout, maybe_err) = gfx_select!(render_pipeline => instance.render_pipeline_get_bind_group_layout(render_pipeline, index, None));

  let label = gfx_select!(bind_group_layout => instance.bind_group_layout_label(bind_group_layout));

  let rid = state
    .resource_table
    .add(super::binding::WebGpuBindGroupLayout(
      instance.clone(),
      bind_group_layout,
    ));

  Ok(PipelineLayout {
    rid,
    label,
    err: maybe_err.map(WebGpuError::from),
  })
}
