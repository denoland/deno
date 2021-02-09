// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{serde_json, RcRef, ZeroCopyBuf};
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

use super::sampler::serialize_compare_function;
use super::texture::serialize_texture_format;
use crate::shader::WebGPUShaderModule;

pub(crate) struct WebGPUPipelineLayout(pub(crate) wgc::id::PipelineLayoutId);
impl Resource for WebGPUPipelineLayout {
  fn name(&self) -> Cow<str> {
    "webGPUPipelineLayout".into()
  }
}

pub(crate) struct WebGPUComputePipeline(pub(crate) wgc::id::ComputePipelineId);
impl Resource for WebGPUComputePipeline {
  fn name(&self) -> Cow<str> {
    "webGPUComputePipeline".into()
  }
}

pub(crate) struct WebGPURenderPipeline(pub(crate) wgc::id::RenderPipelineId);
impl Resource for WebGPURenderPipeline {
  fn name(&self) -> Cow<str> {
    "webGPURenderPipeline".into()
  }
}

pub fn serialize_index_format(format: String) -> wgt::IndexFormat {
  match format.as_str() {
    "uint16" => wgt::IndexFormat::Uint16,
    "uint32" => wgt::IndexFormat::Uint32,
    _ => unreachable!(),
  }
}

fn serialize_stencil_operation(operation: &str) -> wgt::StencilOperation {
  match operation {
    "keep" => wgt::StencilOperation::Keep,
    "zero" => wgt::StencilOperation::Zero,
    "replace" => wgt::StencilOperation::Replace,
    "invert" => wgt::StencilOperation::Invert,
    "increment-clamp" => wgt::StencilOperation::IncrementClamp,
    "decrement-clamp" => wgt::StencilOperation::DecrementClamp,
    "increment-wrap" => wgt::StencilOperation::IncrementWrap,
    "decrement-wrap" => wgt::StencilOperation::DecrementWrap,
    _ => unreachable!(),
  }
}

fn serialize_stencil_face_state(state: GPUStencilFaceState) -> wgt::StencilFaceState {
  wgt::StencilFaceState {
    compare: state
      .compare
      .as_ref()
      .map_or(wgt::CompareFunction::Always, |op| {
        serialize_compare_function(op)
      }),
    fail_op: state
      .fail_op
      .as_ref()
      .map_or(wgt::StencilOperation::Keep, |op| {
        serialize_stencil_operation(op)
      }),
    depth_fail_op: state
      .depth_fail_op
      .as_ref()
      .map_or(wgt::StencilOperation::Keep, |op| {
        serialize_stencil_operation(op)
      }),
    pass_op: state
      .pass_op
      .as_ref()
      .map_or(wgt::StencilOperation::Keep, |op| {
        serialize_stencil_operation(op)
      }),
  }
}

fn serialize_blend_factor(blend_factor: &str) -> wgt::BlendFactor {
  match blend_factor {
    "zero" => wgt::BlendFactor::Zero,
    "one" => wgt::BlendFactor::One,
    "src-color" => wgt::BlendFactor::SrcColor,
    "one-minus-src-color" => wgt::BlendFactor::OneMinusSrcColor,
    "src-alpha" => wgt::BlendFactor::SrcAlpha,
    "one-minus-src-alpha" => wgt::BlendFactor::OneMinusSrcAlpha,
    "dst-color" => wgt::BlendFactor::DstColor,
    "one-minus-dst-color" => wgt::BlendFactor::OneMinusDstColor,
    "dst-alpha" => wgt::BlendFactor::DstAlpha,
    "one-minus-dst-alpha" => wgt::BlendFactor::OneMinusDstAlpha,
    "src-alpha-saturated" => wgt::BlendFactor::SrcAlphaSaturated,
    "blend-color" => wgt::BlendFactor::BlendColor,
    "one-minus-blend-color" => wgt::BlendFactor::OneMinusBlendColor,
    _ => unreachable!(),
  }
}

fn serialize_blend_component(blend: GPUBlendComponent) -> wgt::BlendState {
  wgt::BlendState {
    src_factor: blend
      .src_factor
      .as_ref()
      .map_or(wgt::BlendFactor::One, |factor| {
        serialize_blend_factor(factor)
      }),
    dst_factor: blend
      .dst_factor
      .as_ref()
      .map_or(wgt::BlendFactor::Zero, |factor| {
        serialize_blend_factor(factor)
      }),
    operation: match &blend.operation {
      Some(operation) => match operation.as_str() {
        "add" => wgt::BlendOperation::Add,
        "subtract" => wgt::BlendOperation::Subtract,
        "reverse-subtract" => wgt::BlendOperation::ReverseSubtract,
        "min" => wgt::BlendOperation::Min,
        "max" => wgt::BlendOperation::Max,
        _ => unreachable!(),
      },
      None => wgt::BlendOperation::Add,
    },
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUProgrammableStage {
  module: u32,
  entry_point: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateComputePipelineArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  layout: Option<u32>,
  compute: GPUProgrammableStage,
}

pub fn op_webgpu_create_compute_pipeline(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateComputePipelineArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  let pipeline_layout = if let Some(rid) = args.layout {
    let id = state
      .resource_table
      .get::<WebGPUPipelineLayout>(rid)
      .ok_or_else(bad_resource_id)?;
    Some(id.0)
  } else {
    None
  };

  let compute_shader_module_resource = state
    .resource_table
    .get::<WebGPUShaderModule>(args.compute.module)
    .ok_or_else(bad_resource_id)?;

  let descriptor = wgc::pipeline::ComputePipelineDescriptor {
    label: args.label.map(Cow::Owned),
    layout: pipeline_layout,
    stage: wgc::pipeline::ProgrammableStageDescriptor {
      module: compute_shader_module_resource.0,
      entry_point: Cow::Owned(args.compute.entry_point)
    },
  };
  let implicit_pipelines = match args.layout {
    Some(_) => None,
    None => Some(wgc::device::ImplicitPipelineIds {
      root_id: std::marker::PhantomData,
      group_ids: &[std::marker::PhantomData; wgc::MAX_BIND_GROUPS],
    }),
  };

  // TODO
  let (compute_pipeline, _, _) = wgc::gfx_select!(device => instance.device_create_compute_pipeline(
    device,
    &descriptor,
    std::marker::PhantomData,
    implicit_pipelines
  ));

  let rid = state
    .resource_table
    .add(WebGPUComputePipeline(compute_pipeline));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePipelineGetBindGroupLayoutArgs {
  instance_rid: u32,
  compute_pipeline_rid: u32,
  index: u32,
}

pub fn op_webgpu_compute_pipeline_get_bind_group_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePipelineGetBindGroupLayoutArgs =
    serde_json::from_value(args)?;

  let compute_pipeline_resource = state
    .resource_table
    .get::<WebGPUComputePipeline>(args.compute_pipeline_rid)
    .ok_or_else(bad_resource_id)?;
  let compute_pipeline = compute_pipeline_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  // TODO
  let (bind_group_layout, _) = wgc::gfx_select!(compute_pipeline => instance.compute_pipeline_get_bind_group_layout(compute_pipeline, args.index, std::marker::PhantomData));

  let rid = state
    .resource_table
    .add(super::binding::WebGPUBindGroupLayout(bind_group_layout));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUPrimitiveState {
  topology: Option<String>,
  strip_index_format: Option<String>,
  front_face: Option<String>,
  cull_mode: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GPUBlendComponent {
  src_factor: Option<String>,
  dst_factor: Option<String>,
  operation: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GPUBlendState {
  color: GPUBlendComponent,
  alpha: GPUBlendComponent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUColorTargetState {
  format: String,
  blend: Option<GPUBlendState>,
  write_mask: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUStencilFaceState {
  compare: Option<String>,
  fail_op: Option<String>,
  depth_fail_op: Option<String>,
  pass_op: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUDepthStencilState {
  format: String,
  depth_write_enabled: Option<bool>,
  depth_compare: Option<String>,
  stencil_front: Option<GPUStencilFaceState>,
  stencil_back: Option<GPUStencilFaceState>,
  stencil_read_mask: Option<u32>,
  stencil_write_mask: Option<u32>,
  depth_bias: Option<i32>,
  depth_bias_slope_scale: Option<f32>,
  depth_bias_clamp: Option<f32>,
  clamp_depth: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUVertexAttribute {
  format: String,
  offset: u64,
  shader_location: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUVertexBufferLayout {
  array_stride: u64,
  step_mode: Option<String>,
  attributes: Vec<GPUVertexAttribute>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUVertexState {
  module: u32,
  entry_point: String,
  vertex_buffers: Option<Vec<Option<GPUVertexBufferLayout>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUMultisampleState {
  count: Option<u32>,
  mask: Option<u64>, // against spec, but future proof
  alpha_to_coverage_enabled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUFragmentState {
  targets: Vec<GPUColorTargetState>,
  module: u32,
  entry_point: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRenderPipelineArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  layout: Option<u32>,
  vertex: GPUVertexState,
  primitive: Option<GPUPrimitiveState>,
  depth_stencil: Option<GPUDepthStencilState>,
  multisample: Option<GPUMultisampleState>,
  fragment: Option<GPUFragmentState>,
}

pub fn op_webgpu_create_render_pipeline(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateRenderPipelineArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  let layout = if let Some(rid) = args.layout {
    let pipeline_layout_resource = state
      .resource_table
      .get::<WebGPUPipelineLayout>(rid)
      .ok_or_else(bad_resource_id)?;
    Some(pipeline_layout_resource.0)
  } else {
    None
  };

  let vertex_shader_module_resource = state
    .resource_table
    .get::<super::shader::WebGPUShaderModule>(args.vertex.module)
    .ok_or_else(bad_resource_id)?;

  let descriptor = wgc::pipeline::RenderPipelineDescriptor {
    label: args.label.map(Cow::Owned),
    layout,
    vertex: wgc::pipeline::VertexState {
      stage: wgc::pipeline::ProgrammableStageDescriptor {
        module: vertex_shader_module_resource.0,
        entry_point: Cow::Owned(args.vertex.entry_point),
      },
      buffers: Cow::Owned(if let Some(buffers) = args.vertex.vertex_buffers {
        let mut return_buffers = vec![];
        for buffer in buffers {
          if let Some(buffer) = buffer {
            return_buffers.push(wgc::pipeline::VertexBufferLayout {
              array_stride: buffer.array_stride,
              step_mode: match buffer.step_mode {
                Some(step_mode) => match step_mode.as_str() {
                  "vertex" => wgt::InputStepMode::Vertex,
                  "instance" => wgt::InputStepMode::Instance,
                  _ => unreachable!(),
                },
                None => wgt::InputStepMode::Vertex,
              },
              attributes: Cow::Owned(
                buffer
                  .attributes
                  .iter()
                  .map(|attribute| wgt::VertexAttribute {
                    format: match attribute.format.as_str() {
                      "uchar2" => wgt::VertexFormat::Uchar2,
                      "uchar4" => wgt::VertexFormat::Uchar4,
                      "char2" => wgt::VertexFormat::Char2,
                      "char4" => wgt::VertexFormat::Char4,
                      "uchar2norm" => wgt::VertexFormat::Uchar2Norm,
                      "uchar4norm" => wgt::VertexFormat::Uchar4,
                      "char2norm" => wgt::VertexFormat::Char2Norm,
                      "char4norm" => wgt::VertexFormat::Char4Norm,
                      "ushort2" => wgt::VertexFormat::Ushort2,
                      "ushort4" => wgt::VertexFormat::Ushort4,
                      "short2" => wgt::VertexFormat::Short2,
                      "short4" => wgt::VertexFormat::Short4,
                      "ushort2norm" => wgt::VertexFormat::Ushort2Norm,
                      "ushort4norm" => wgt::VertexFormat::Ushort4Norm,
                      "short2norm" => wgt::VertexFormat::Short2Norm,
                      "short4norm" => wgt::VertexFormat::Short4Norm,
                      "half2" => wgt::VertexFormat::Half2,
                      "half4" => wgt::VertexFormat::Half4,
                      "float" => wgt::VertexFormat::Float,
                      "float2" => wgt::VertexFormat::Float2,
                      "float3" => wgt::VertexFormat::Float3,
                      "float4" => wgt::VertexFormat::Float4,
                      "uint" => wgt::VertexFormat::Uint,
                      "uint2" => wgt::VertexFormat::Uint2,
                      "uint3" => wgt::VertexFormat::Uint3,
                      "uint4" => wgt::VertexFormat::Uint4,
                      "int" => wgt::VertexFormat::Int,
                      "int2" => wgt::VertexFormat::Int2,
                      "int3" => wgt::VertexFormat::Int3,
                      "int4" => wgt::VertexFormat::Int4,
                      _ => unreachable!(),
                    },
                    offset: attribute.offset,
                    shader_location: attribute.shader_location,
                  })
                  .collect(),
              ),
            })
          }
        }
        return_buffers
      } else {
        vec![]
      }),
    },
    primitive: args.primitive.map_or(Default::default(), |primitive| {
      // TODO
      wgt::PrimitiveState {
        topology: match primitive.topology {
          Some(topology) => match topology.as_str() {
            "point-list" => wgt::PrimitiveTopology::PointList,
            "line-list" => wgt::PrimitiveTopology::LineList,
            "line-strip" => wgt::PrimitiveTopology::LineStrip,
            "triangle-list" => wgt::PrimitiveTopology::TriangleList,
            "triangle-strip" => wgt::PrimitiveTopology::TriangleStrip,
            _ => unreachable!(),
          },
          None => wgt::PrimitiveTopology::TriangleList,
        },
        strip_index_format: primitive.strip_index_format.map(serialize_index_format),
        front_face: match primitive.front_face {
          Some(front_face) => match front_face.as_str() {
            "ccw" => wgt::FrontFace::Ccw,
            "cw" => wgt::FrontFace::Cw,
            _ => unreachable!(),
          },
          None => wgt::FrontFace::Ccw,
        },
        cull_mode: match primitive.cull_mode {
          Some(cull_mode) => match cull_mode.as_str() {
            "none" => wgt::CullMode::None,
            "front" => wgt::CullMode::Front,
            "back" => wgt::CullMode::Back,
            _ => unreachable!(),
          },
          None => wgt::CullMode::None,
        },
        polygon_mode: Default::default(), // native-only
      }
    }),
    depth_stencil: args.depth_stencil.map(|depth_stencil| {
      wgt::DepthStencilState {
        format: super::texture::serialize_texture_format(&depth_stencil.format)
          .unwrap(),
        depth_write_enabled: depth_stencil.depth_write_enabled.unwrap_or(false),
        depth_compare: match depth_stencil.depth_compare {
          Some(depth_compare) => {
            super::sampler::serialize_compare_function(&depth_compare)
          }
          None => wgt::CompareFunction::Always,
        },
        stencil: wgt::StencilState {
          front: depth_stencil
            .stencil_front
            .map_or(Default::default(), serialize_stencil_face_state),
          back: depth_stencil
            .stencil_back
            .map_or(Default::default(), serialize_stencil_face_state),
          read_mask: depth_stencil.stencil_read_mask.unwrap_or(0xFFFFFFFF),
          write_mask: depth_stencil.stencil_write_mask.unwrap_or(0xFFFFFFFF),
        },
        bias: wgt::DepthBiasState {
          constant: depth_stencil.depth_bias.unwrap_or(0),
          slope_scale: depth_stencil.depth_bias_slope_scale.unwrap_or(0.0),
          clamp: depth_stencil.depth_bias_clamp.unwrap_or(0.0),
        },
        clamp_depth: depth_stencil.clamp_depth.unwrap_or(false),
      }
    }),
    multisample: args.multisample.map_or(Default::default(), |multisample| {
      wgt::MultisampleState {
        // TODO
        count: multisample.count.unwrap_or(1),
        mask: multisample.mask.unwrap_or(0xFFFFFFFF),
        alpha_to_coverage_enabled: multisample
          .alpha_to_coverage_enabled
          .unwrap_or(false),
      }
    }),
    fragment: args.fragment.map(|fragment| {
      let fragment_shader_module_resource = state
        .resource_table
        .get::<super::shader::WebGPUShaderModule>(fragment.module)
        .ok_or_else(bad_resource_id)
        .unwrap(); // TODO

      wgc::pipeline::FragmentState {
        stage: wgc::pipeline::ProgrammableStageDescriptor {
          module: fragment_shader_module_resource.0,
          entry_point: Cow::Owned(fragment.entry_point),
        },
        targets: Cow::Owned(
          fragment
            .targets
            .iter()
            .map(|target| {
              let blends = target.blend.clone().map(|blend| {
                (
                  serialize_blend_component(blend.alpha),
                  serialize_blend_component(blend.color),
                )
              });

              wgt::ColorTargetState {
                format: serialize_texture_format(&target.format).unwrap(),
                alpha_blend: blends.clone()
                  .map_or(Default::default(), |states| states.0),
                color_blend: blends
                  .map_or(Default::default(), |states| states.1),
                write_mask: target
                  .write_mask
                  .map_or(Default::default(), |mask| {
                    wgt::ColorWrite::from_bits(mask).unwrap()
                  }),
              }
            })
            .collect(),
        ),
      }
    }),
  };

  let implicit_pipelines = match args.layout {
    Some(_) => None,
    None => Some(wgc::device::ImplicitPipelineIds {
      root_id: std::marker::PhantomData,
      group_ids: &[std::marker::PhantomData; wgc::MAX_BIND_GROUPS],
    }),
  };

  // TODO
  let (render_pipeline, _, _) = wgc::gfx_select!(device => instance.device_create_render_pipeline(
    device,
    &descriptor,
    std::marker::PhantomData,
    implicit_pipelines
  ));

  let rid = state
    .resource_table
    .add(WebGPURenderPipeline(render_pipeline));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPipelineGetBindGroupLayoutArgs {
  instance_rid: u32,
  render_pipeline_rid: u32,
  index: u32,
}

pub fn op_webgpu_render_pipeline_get_bind_group_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPipelineGetBindGroupLayoutArgs =
    serde_json::from_value(args)?;

  let render_pipeline_resource = state
    .resource_table
    .get::<WebGPURenderPipeline>(args.render_pipeline_rid)
    .ok_or_else(bad_resource_id)?;
  let render_pipeline = render_pipeline_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = RcRef::map(&instance_resource, |r| &r.0)
    .try_borrow()
    .unwrap();

  // TODO
  let (bind_group_layout, _) = wgc::gfx_select!(render_pipeline => instance.render_pipeline_get_bind_group_layout(render_pipeline, args.index, std::marker::PhantomData));

  let rid = state
    .resource_table
    .add(super::binding::WebGPUBindGroupLayout(bind_group_layout));

  Ok(json!({
    "rid": rid,
  }))
}
