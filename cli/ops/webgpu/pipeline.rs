// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use super::sampler::serialize_compare_function;
use super::texture::serialize_texture_format;
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

fn serialize_programmable_stage_descriptor(
  state: &mut OpState,
  programmable_stage_descriptor: GPUProgrammableStageDescriptor,
) -> Result<wgpu::ProgrammableStageDescriptor, AnyError> {
  Ok(wgpu::ProgrammableStageDescriptor {
    module: state
      .resource_table
      .get_mut::<wgpu::ShaderModule>(programmable_stage_descriptor.module)
      .ok_or_else(bad_resource_id)?,
    entry_point: &programmable_stage_descriptor.entry_point,
  })
}

fn serialize_stencil_operation(operation: String) -> wgpu::StencilOperation {
  match operation {
    &"keep" => wgpu::StencilOperation::Keep,
    &"zero" => wgpu::StencilOperation::Zero,
    &"replace" => wgpu::StencilOperation::Replace,
    &"invert" => wgpu::StencilOperation::Invert,
    &"increment-clamp" => wgpu::StencilOperation::IncrementClamp,
    &"decrement-clamp" => wgpu::StencilOperation::DecrementClamp,
    &"increment-wrap" => wgpu::StencilOperation::IncrementWrap,
    &"decrement-wrap" => wgpu::StencilOperation::DecrementWrap,
    _ => unreachable!(),
  }
}

fn serialize_stencil_state_face_descriptor(
  state: GPUStencilStateFaceDescriptor,
) -> wgpu::StencilStateFaceDescriptor {
  wgpu::StencilStateFaceDescriptor {
    compare: state
      .compare
      .map_or(wgpu::CompareFunction::Always, |compare| {
        serialize_compare_function(compare)
      }),
    fail_op: state.fail_op.map_or(wgpu::StencilOperation::Keep, |op| {
      serialize_stencil_operation(op)
    }),
    depth_fail_op: state
      .depth_fail_op
      .map_or(wgpu::StencilOperation::Keep, |op| {
        serialize_stencil_operation(op)
      }),
    pass_op: state.pass_op.map_or(wgpu::StencilOperation::Keep, |op| {
      serialize_stencil_operation(op)
    }),
  }
}

fn serialize_blend_factor(blend_factor: String) -> wgpu::BlendFactor {
  match blend_factor {
    &"zero" => wgpu::BlendFactor::Zero,
    &"one" => wgpu::BlendFactor::One,
    &"src-color" => wgpu::BlendFactor::SrcColor,
    &"one-minus-src-color" => wgpu::BlendFactor::OneMinusSrcColor,
    &"src-alpha" => wgpu::BlendFactor::SrcAlpha,
    &"one-minus-src-alpha" => wgpu::BlendFactor::OneMinusSrcAlpha,
    &"dst-color" => wgpu::BlendFactor::DstColor,
    &"one-minus-dst-color" => wgpu::BlendFactor::OneMinusDstColor,
    &"dst-alpha" => wgpu::BlendFactor::DstAlpha,
    &"one-minus-dst-alpha" => wgpu::BlendFactor::OneMinusDstAlpha,
    &"src-alpha-saturated" => wgpu::BlendFactor::SrcAlphaSaturated,
    &"blend-color" => wgpu::BlendFactor::BlendColor,
    &"one-minus-blend-color" => wgpu::BlendFactor::OneMinusBlendColor,
    _ => unreachable!(),
  }
}

fn serialize_blend_descriptor(
  blend: GPUBlendDescriptor,
) -> wgpu::BlendDescriptor {
  wgpu::BlendDescriptor {
    src_factor: blend
      .src_factor
      .map_or(wgpu::BlendFactor::One, serialize_blend_factor),
    dst_factor: blend
      .dst_factor
      .map_or(wgpu::BlendFactor::Zero, serialize_blend_factor),
    operation: match blend.operation {
      Some(&"add") => wgpu::BlendOperation::Add,
      Some(&"subtract") => wgpu::BlendOperation::Subtract,
      Some(&"reverse-subtract") => wgpu::BlendOperation::ReverseSubtract,
      Some(&"min") => wgpu::BlendOperation::Min,
      Some(&"max") => wgpu::BlendOperation::Max,
      Some(_) => unreachable!(),
      None => wgpu::BlendOperation::Add,
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
  rid: u32,
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

  let device = state
    .resource_table
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let compute_pipeline =
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
      label: args.label.map(|label| &label),
      layout: args.layout.map(|rid| {
        state
          .resource_table
          .get_mut::<wgpu::PipelineLayout>(rid)
          .ok_or_else(bad_resource_id)?
      }),
      compute_stage: serialize_programmable_stage_descriptor(
        state,
        args.compute_stage,
      )?,
    });

  let rid = state
    .resource_table
    .add("webGPUComputePipeline", Box::new(compute_pipeline));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputePipelineGetBindGroupLayoutArgs {
  rid: u32,
  index: u32,
}

pub fn op_webgpu_compute_pipeline_get_bind_group_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: ComputePipelineGetBindGroupLayoutArgs =
    serde_json::from_value(args)?;

  let compute_pipeline = state
    .resource_table
    .get_mut::<wgpu::ComputePipeline>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let bind_group_layout = compute_pipeline.get_bind_group_layout(args.index);

  let rid = state
    .resource_table
    .add("webGPUBindGroupLayout", Box::new(bind_group_layout));

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
  attributes: [GPUVertexAttributeDescriptor],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUVertexStateDescriptor {
  index_format: Option<String>,
  vertex_buffers: Option<[GPUVertexBufferLayoutDescriptor]>, // TODO: nullable
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateRenderPipelineArgs {
  rid: u32,
  label: Option<String>,
  layout: Option<u32>,
  vertex_stage: GPUProgrammableStageDescriptor,
  fragment_stage: Option<GPUProgrammableStageDescriptor>,
  primitive_topology: String,
  rasterization_state: Option<GPURasterizationStateDescriptor>,
  color_states: [GPUColorStateDescriptor],
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

  let device = state
    .resource_table
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let render_pipeline =
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: args.label.map(|label| &label),
      layout: args.layout.map(|rid| {
        state
          .resource_table
          .get_mut::<wgpu::PipelineLayout>(rid)
          .ok_or_else(bad_resource_id)?
      }),
      vertex_stage: serialize_programmable_stage_descriptor(
        state,
        args.vertex_stage,
      )?,
      fragment_stage: args.fragment_stage.map(
        |programmable_stage_descriptor| {
          serialize_programmable_stage_descriptor(
            state,
            programmable_stage_descriptor,
          )?
        },
      ),
      rasterization_state: args.rasterization_state.map(
        |rasterization_state| wgpu::RasterizationStateDescriptor {
          front_face: match rasterization_state.front_face {
            Some(&"ccw") => wgpu::FrontFace::Ccw,
            Some(&"cw") => wgpu::FrontFace::Cw,
            Some(_) => unreachable!(),
            None => wgpu::FrontFace::Ccw,
          },
          cull_mode: match rasterization_state.cull_mode {
            Some(&"none") => wgpu::CullMode::None,
            Some(&"front") => wgpu::CullMode::Front,
            Some(&"back") => wgpu::CullMode::Back,
            Some(_) => unreachable!(),
            None => wgpu::CullMode::None,
          },
          clamp_depth: rasterization_state.clamp_depth.unwrap_or(false),
          depth_bias: rasterization_state.depth_bias.unwrap_or(0),
          depth_bias_slope_scale: rasterization_state
            .depth_bias_slope_scale
            .unwrap_or(0.0),
          depth_bias_clamp: rasterization_state.depth_bias_clamp.unwrap_or(0.0),
        },
      ),
      primitive_topology: match args.primitive_topology {
        &"point-list" => wgpu::PrimitiveTopology::PointList,
        &"line-list" => wgpu::PrimitiveTopology::LineList,
        &"line-strip" => wgpu::PrimitiveTopology::LineStrip,
        &"triangle-list" => wgpu::PrimitiveTopology::TriangleList,
        &"triangle-strip" => wgpu::PrimitiveTopology::TriangleStrip,
        _ => unreachable!(),
      },
      color_states: &args
        .color_states
        .iter()
        .map(|color_state| {
          wgpu::ColorStateDescriptor {
            format: serialize_texture_format(color_state.format.clone())?,
            alpha_blend: color_state
              .alpha_blend
              .map_or("", serialize_blend_descriptor), // TODO
            color_blend: color_state
              .color_blend
              .map_or("", serialize_blend_descriptor), // TODO
            write_mask: color_state.write_mask, // TODO
          }
        })
        .collect::<[wgpu::ColorStateDescriptor]>(),
      depth_stencil_state: args.depth_stencil_state.map(|state| {
        wgpu::DepthStencilStateDescriptor {
          format: serialize_texture_format(state.format)?,
          depth_write_enabled: state.depth_write_enabled.unwrap_or(false),
          depth_compare: state
            .depth_compare
            .map_or(wgpu::CompareFunction::Always, serialize_compare_function),
          stencil: wgpu::StencilStateDescriptor {
            front: state.stencil_front.map_or(
              wgpu::StencilStateFaceDescriptor::IGNORE,
              serialize_stencil_state_face_descriptor,
            ),
            back: state.stencil_front.map_or(
              wgpu::StencilStateFaceDescriptor::IGNORE,
              serialize_stencil_state_face_descriptor,
            ),
            read_mask: state.stencil_read_mask.unwrap_or(0xFFFFFFFF),
            write_mask: state.stencil_write_mask.unwrap_or(0xFFFFFFFF),
          },
        }
      }),
      vertex_state: wgpu::VertexStateDescriptor {
        index_format: args.vertex_state.unwrap().index_format.map_or(
          "",
          |format| match format {
            // TODO
            &"uint16" => wgpu::IndexFormat::Uint16,
            &"uint32" => wgpu::IndexFormat::Uint32,
            _ => unreachable!(),
          },
        ),
        vertex_buffers: &args
          .vertex_state
          .unwrap()
          .vertex_buffers
          .unwrap() // TODO
          .iter()
          .map(|buffer| wgpu::VertexBufferDescriptor {
            stride: buffer.array_stride,
            step_mode: match buffer.step_mode {
              Some(&"vertex") => wgpu::InputStepMode::Vertex,
              Some(&"instance") => wgpu::InputStepMode::Instance,
              Some(_) => unreachable!(),
              None => wgpu::InputStepMode::Vertex,
            },
            attributes: &buffer
              .attributes
              .iter()
              .map(|attribute| wgpu::VertexAttributeDescriptor {
                offset: attribute.offset,
                format: match attribute.format {
                  &"uchar2" => wgpu::VertexFormat::Uchar2,
                  &"uchar4" => wgpu::VertexFormat::Uchar4,
                  &"char2" => wgpu::VertexFormat::Char2,
                  &"char4" => wgpu::VertexFormat::Char4,
                  &"uchar2norm" => wgpu::VertexFormat::Uchar2Norm,
                  &"uchar4norm" => wgpu::VertexFormat::Uchar4,
                  &"char2norm" => wgpu::VertexFormat::Char2Norm,
                  &"char4norm" => wgpu::VertexFormat::Char4Norm,
                  &"ushort2" => wgpu::VertexFormat::Ushort2,
                  &"ushort4" => wgpu::VertexFormat::Ushort4,
                  &"short2" => wgpu::VertexFormat::Short2,
                  &"short4" => wgpu::VertexFormat::Short4,
                  &"ushort2norm" => wgpu::VertexFormat::Ushort2Norm,
                  &"ushort4norm" => wgpu::VertexFormat::Ushort4Norm,
                  &"short2norm" => wgpu::VertexFormat::Short2Norm,
                  &"short4norm" => wgpu::VertexFormat::Short4Norm,
                  &"half2" => wgpu::VertexFormat::Half2,
                  &"half4" => wgpu::VertexFormat::Half4,
                  &"float" => wgpu::VertexFormat::Float,
                  &"float2" => wgpu::VertexFormat::Float2,
                  &"float3" => wgpu::VertexFormat::Float3,
                  &"float4" => wgpu::VertexFormat::Float4,
                  &"uint" => wgpu::VertexFormat::Uint,
                  &"uint2" => wgpu::VertexFormat::Uint2,
                  &"uint3" => wgpu::VertexFormat::Uint3,
                  &"uint4" => wgpu::VertexFormat::Uint4,
                  &"int" => wgpu::VertexFormat::Int,
                  &"int2" => wgpu::VertexFormat::Int2,
                  &"int3" => wgpu::VertexFormat::Int3,
                  &"int4" => wgpu::VertexFormat::Int4,
                  _ => unreachable!(),
                },
                shader_location: attribute.shader_location,
              })
              .collect::<[wgpu::VertexAttributeDescriptor]>(),
          })
          .collect::<[wgpu::VertexBufferDescriptor]>(),
      },
      sample_count: args.sample_count.unwrap_or(1),
      sample_mask: args.sample_mask.unwrap_or(0xFFFFFFFF),
      alpha_to_coverage_enabled: args
        .alpha_to_coverage_enabled
        .unwrap_or(false),
    });

  let rid = state
    .resource_table
    .add("webGPURenderPipeline", Box::new(render_pipeline));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderPipelineGetBindGroupLayoutArgs {
  rid: u32,
  index: u32,
}

pub fn op_webgpu_render_pipeline_get_bind_group_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: RenderPipelineGetBindGroupLayoutArgs =
    serde_json::from_value(args)?;

  let render_pipeline = state
    .resource_table
    .get_mut::<wgpu::RenderPipeline>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let bind_group_layout = render_pipeline.get_bind_group_layout(args.index);

  let rid = state
    .resource_table
    .add("webGPUBindGroupLayout", Box::new(bind_group_layout));

  Ok(json!({
    "rid": rid,
  }))
}
