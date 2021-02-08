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

fn serialize_programmable_stage_descriptor(
  state: &OpState,
  programmable_stage_descriptor: GPUProgrammableStageDescriptor,
) -> Result<wgc::pipeline::ProgrammableStageDescriptor, AnyError> {
  let shader_module_resource = state
    .resource_table
    .get::<super::shader::WebGPUShaderModule>(
      programmable_stage_descriptor.module,
    )
    .ok_or_else(bad_resource_id)?;
  Ok(wgc::pipeline::ProgrammableStageDescriptor {
    module: shader_module_resource.0,
    entry_point: Cow::Owned(programmable_stage_descriptor.entry_point),
  })
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

fn serialize_stencil_state_face_descriptor(
  state: &GPUStencilStateFaceDescriptor,
) -> wgt::StencilStateFaceDescriptor {
  wgt::StencilStateFaceDescriptor {
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

fn serialize_blend_descriptor(
  blend: &GPUBlendDescriptor,
) -> wgt::BlendDescriptor {
  wgt::BlendDescriptor {
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
struct GPUProgrammableStageDescriptor {
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
  compute_stage: GPUProgrammableStageDescriptor,
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

  let compute_stage =
    serialize_programmable_stage_descriptor(state, args.compute_stage)?;

  let descriptor = wgc::pipeline::ComputePipelineDescriptor {
    label: args.label.map(Cow::Owned),
    layout: pipeline_layout,
    compute_stage,
  };
  let implicit_pipelines = match args.layout {
    Some(_) => None,
    None => Some(wgc::device::ImplicitPipelineIds {
      root_id: std::marker::PhantomData,
      group_ids: &[std::marker::PhantomData; wgc::MAX_BIND_GROUPS],
    }),
  };

  let (compute_pipeline, _) = wgc::gfx_select!(device => instance.device_create_compute_pipeline(
    device,
    &descriptor,
    std::marker::PhantomData,
    implicit_pipelines
  ))?;

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

  let bind_group_layout = wgc::gfx_select!(compute_pipeline => instance
    .compute_pipeline_get_bind_group_layout(compute_pipeline, args.index))?;

  let rid = state
    .resource_table
    .add(super::binding::WebGPUBindGroupLayout(bind_group_layout));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPURasterizationStateDescriptor {
  front_face: Option<String>,
  cull_mode: Option<String>,
  clamp_depth: Option<bool>,
  depth_bias: Option<i32>,
  depth_bias_slope_scale: Option<f32>,
  depth_bias_clamp: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUBlendDescriptor {
  src_factor: Option<String>,
  dst_factor: Option<String>,
  operation: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUColorStateDescriptor {
  format: String,
  alpha_blend: Option<GPUBlendDescriptor>,
  color_blend: Option<GPUBlendDescriptor>,
  write_mask: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUStencilStateFaceDescriptor {
  compare: Option<String>,
  fail_op: Option<String>,
  depth_fail_op: Option<String>,
  pass_op: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUDepthStencilStateDescriptor {
  format: String,
  depth_write_enabled: Option<bool>,
  depth_compare: Option<String>,
  stencil_front: Option<GPUStencilStateFaceDescriptor>,
  stencil_back: Option<GPUStencilStateFaceDescriptor>,
  stencil_read_mask: Option<u32>,
  stencil_write_mask: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUVertexAttributeDescriptor {
  format: String,
  offset: u64,
  shader_location: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUVertexBufferLayoutDescriptor {
  array_stride: u64,
  step_mode: Option<String>,
  attributes: Vec<GPUVertexAttributeDescriptor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUVertexStateDescriptor {
  index_format: Option<String>,
  vertex_buffers: Option<Vec<Option<GPUVertexBufferLayoutDescriptor>>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRenderPipelineArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  layout: Option<u32>,
  vertex_stage: GPUProgrammableStageDescriptor,
  fragment_stage: Option<GPUProgrammableStageDescriptor>,
  primitive_topology: String,
  rasterization_state: Option<GPURasterizationStateDescriptor>,
  color_states: Vec<GPUColorStateDescriptor>,
  depth_stencil_state: Option<GPUDepthStencilStateDescriptor>,
  vertex_state: Option<GPUVertexStateDescriptor>,
  sample_count: Option<u32>,
  sample_mask: Option<u32>,
  alpha_to_coverage_enabled: Option<bool>,
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

  let mut color_states = vec![];

  for color_state in &args.color_states {
    let state = wgt::ColorStateDescriptor {
      format: serialize_texture_format(color_state.format.clone())?,
      alpha_blend: color_state
        .alpha_blend
        .as_ref()
        .map_or(Default::default(), serialize_blend_descriptor),
      color_blend: color_state
        .color_blend
        .as_ref()
        .map_or(Default::default(), serialize_blend_descriptor),
      write_mask: color_state.write_mask.map_or(Default::default(), |mask| {
        wgt::ColorWrite::from_bits(mask).unwrap()
      }),
    };
    color_states.push(state);
  }

  let mut depth_stencil_state = None;

  if let Some(state) = &args.depth_stencil_state {
    depth_stencil_state = Some(wgt::DepthStencilStateDescriptor {
      format: serialize_texture_format(state.format.clone())?,
      depth_write_enabled: state.depth_write_enabled.unwrap_or(false),
      depth_compare: state
        .depth_compare
        .as_ref()
        .map_or(wgt::CompareFunction::Always, |compare| {
          serialize_compare_function(compare)
        }),
      stencil: wgt::StencilStateDescriptor {
        front: state.stencil_front.as_ref().map_or(
          wgt::StencilStateFaceDescriptor::IGNORE,
          serialize_stencil_state_face_descriptor,
        ),
        back: state.stencil_back.as_ref().map_or(
          wgt::StencilStateFaceDescriptor::IGNORE,
          serialize_stencil_state_face_descriptor,
        ),
        read_mask: state.stencil_read_mask.unwrap_or(0xFFFFFFFF),
        write_mask: state.stencil_write_mask.unwrap_or(0xFFFFFFFF),
      },
    });
  }

  let layout = if let Some(rid) = args.layout {
    let id = state
      .resource_table
      .get::<WebGPUPipelineLayout>(rid)
      .ok_or_else(bad_resource_id)?;
    Some(id.0)
  } else {
    None
  };

  let fragment_stage =
    if let Some(programmable_stage_descriptor) = args.fragment_stage {
      Some(serialize_programmable_stage_descriptor(
        state,
        programmable_stage_descriptor,
      )?)
    } else {
      None
    };

  let vertex_stage =
    serialize_programmable_stage_descriptor(state, args.vertex_stage)?;

  let descriptor = wgc::pipeline::RenderPipelineDescriptor {
    label: args.label.map(Cow::Owned),
    layout,
    vertex_stage,
    fragment_stage,
    rasterization_state: args.rasterization_state.map(|rasterization_state| {
      wgt::RasterizationStateDescriptor {
        front_face: match rasterization_state.front_face {
          Some(front_face) => match front_face.as_str() {
            "ccw" => wgt::FrontFace::Ccw,
            "cw" => wgt::FrontFace::Cw,
            _ => unreachable!(),
          },
          None => wgt::FrontFace::Ccw,
        },
        cull_mode: match rasterization_state.cull_mode {
          Some(cull_mode) => match cull_mode.as_str() {
            "none" => wgt::CullMode::None,
            "front" => wgt::CullMode::Front,
            "back" => wgt::CullMode::Back,
            _ => unreachable!(),
          },
          None => wgt::CullMode::None,
        },
        clamp_depth: rasterization_state.clamp_depth.unwrap_or(false),
        depth_bias: rasterization_state.depth_bias.unwrap_or(0),
        depth_bias_slope_scale: rasterization_state
          .depth_bias_slope_scale
          .unwrap_or(0.0),
        depth_bias_clamp: rasterization_state.depth_bias_clamp.unwrap_or(0.0),
      }
    }),
    primitive_topology: match args.primitive_topology.as_str() {
      "point-list" => wgt::PrimitiveTopology::PointList,
      "line-list" => wgt::PrimitiveTopology::LineList,
      "line-strip" => wgt::PrimitiveTopology::LineStrip,
      "triangle-list" => wgt::PrimitiveTopology::TriangleList,
      "triangle-strip" => wgt::PrimitiveTopology::TriangleStrip,
      _ => unreachable!(),
    },
    color_states: Cow::Owned(color_states),
    depth_stencil_state,
    vertex_state: args.vertex_state.map_or(
      wgc::pipeline::VertexStateDescriptor {
        index_format: Default::default(),
        vertex_buffers: Default::default(),
      },
      |state| wgc::pipeline::VertexStateDescriptor {
        index_format: state.index_format.map_or(Default::default(), |format| {
          match format.as_str() {
            "uint16" => wgt::IndexFormat::Uint16,
            "uint32" => wgt::IndexFormat::Uint32,
            _ => unreachable!(),
          }
        }),
        vertex_buffers: state.vertex_buffers.map_or(
          Default::default(),
          |vertex_buffers| {
            Cow::Owned(
              vertex_buffers
                .iter()
                .map(|buffer| {
                  if let Some(buffer) = buffer {
                    wgc::pipeline::VertexBufferDescriptor {
                      stride: buffer.array_stride,
                      step_mode: match buffer.step_mode.clone() {
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
                          .map(|attribute| wgt::VertexAttributeDescriptor {
                            offset: attribute.offset,
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
                            shader_location: attribute.shader_location,
                          })
                          .collect::<Vec<wgt::VertexAttributeDescriptor>>(),
                      ),
                    }
                  } else {
                    wgc::pipeline::VertexBufferDescriptor {
                      stride: 0,
                      step_mode: wgt::InputStepMode::Vertex,
                      attributes: Default::default(),
                    }
                  }
                })
                .collect::<Vec<wgc::pipeline::VertexBufferDescriptor>>(),
            )
          },
        ),
      },
    ),
    sample_count: args.sample_count.unwrap_or(1),
    sample_mask: args.sample_mask.unwrap_or(0xFFFFFFFF),
    alpha_to_coverage_enabled: args.alpha_to_coverage_enabled.unwrap_or(false),
  };
  let implicit_pipelines = match args.layout {
    Some(_) => None,
    None => Some(wgc::device::ImplicitPipelineIds {
      root_id: std::marker::PhantomData,
      group_ids: &[std::marker::PhantomData; wgc::MAX_BIND_GROUPS],
    }),
  };

  let (render_pipeline, _) = wgc::gfx_select!(device => instance.device_create_render_pipeline(
    device,
    &descriptor,
    std::marker::PhantomData,
    implicit_pipelines
  ))?;

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

  let bind_group_layout = wgc::gfx_select!(render_pipeline => instance
    .render_pipeline_get_bind_group_layout(render_pipeline, args.index))?;

  let rid = state
    .resource_table
    .add(super::binding::WebGPUBindGroupLayout(bind_group_layout));

  Ok(json!({
    "rid": rid,
  }))
}
