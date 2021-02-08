// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::{serde_json, ZeroCopyBuf};
use deno_core::{OpState, Resource};
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use serde::Deserialize;
use std::borrow::Cow;

use super::texture::{serialize_dimension, serialize_texture_format};

pub(crate) struct WebGPUBindGroupLayout(pub(crate) wgc::id::BindGroupLayoutId);
impl Resource for WebGPUBindGroupLayout {
  fn name(&self) -> Cow<str> {
    "webGPUBindGroupLayout".into()
  }
}

pub(crate) struct WebGPUBindGroup(pub(crate) wgc::id::BindGroupId);
impl Resource for WebGPUBindGroup {
  fn name(&self) -> Cow<str> {
    "webGPUBindGroup".into()
  }
}

fn serialize_texture_component_type(
  component_type: String,
) -> Result<wgt::TextureComponentType, AnyError> {
  Ok(match component_type.as_str() {
    "float" => wgt::TextureComponentType::Float,
    "sint" => wgt::TextureComponentType::Sint,
    "uint" => wgt::TextureComponentType::Uint,
    "depth-comparison" => return Err(not_supported()),
    _ => unreachable!(),
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUBindGroupLayoutEntry {
  binding: u32,
  visibility: u32,
  #[serde(rename = "type")]
  kind: String,
  has_dynamic_offset: Option<bool>,
  min_buffer_binding_size: Option<u64>,
  view_dimension: Option<String>,
  texture_component_type: Option<String>,
  storage_texture_format: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBindGroupLayoutArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  entries: Vec<GPUBindGroupLayoutEntry>,
}

pub fn op_webgpu_create_bind_group_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateBindGroupLayoutArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let mut entries = vec![];

  for entry in &args.entries {
    let e = wgt::BindGroupLayoutEntry {
      binding: entry.binding,
      visibility: wgt::ShaderStage::from_bits(entry.visibility).unwrap(),
      ty: match entry.kind.as_str() {
        "uniform-buffer" => wgt::BindingType::UniformBuffer {
          dynamic: entry.has_dynamic_offset.unwrap_or(false),
          min_binding_size: std::num::NonZeroU64::new(
            entry.min_buffer_binding_size.unwrap_or(0),
          ),
        },
        "storage-buffer" => wgt::BindingType::StorageBuffer {
          dynamic: entry.has_dynamic_offset.unwrap_or(false),
          min_binding_size: std::num::NonZeroU64::new(
            entry.min_buffer_binding_size.unwrap_or(0),
          ),
          readonly: false,
        },
        "readonly-storage-buffer" => wgt::BindingType::StorageBuffer {
          dynamic: entry.has_dynamic_offset.unwrap_or(false),
          min_binding_size: std::num::NonZeroU64::new(
            entry.min_buffer_binding_size.unwrap_or(0),
          ),
          readonly: true,
        },
        "sampler" => wgt::BindingType::Sampler { comparison: false },
        "comparison-sampler" => wgt::BindingType::Sampler { comparison: true },
        "sampled-texture" => wgt::BindingType::SampledTexture {
          dimension: serialize_dimension(entry.view_dimension.clone().unwrap()),
          component_type: serialize_texture_component_type(
            entry.texture_component_type.clone().unwrap(),
          )?,
          multisampled: false,
        },
        "multisampled-texture" => wgt::BindingType::SampledTexture {
          dimension: serialize_dimension(entry.view_dimension.clone().unwrap()),
          component_type: serialize_texture_component_type(
            entry.texture_component_type.clone().unwrap(),
          )?,
          multisampled: true,
        },
        "readonly-storage-texture" => wgt::BindingType::StorageTexture {
          dimension: serialize_dimension(entry.view_dimension.clone().unwrap()),
          format: serialize_texture_format(
            entry.storage_texture_format.clone().unwrap(),
          )?,
          readonly: true,
        },
        "writeonly-storage-texture" => wgt::BindingType::StorageTexture {
          dimension: serialize_dimension(entry.view_dimension.clone().unwrap()),
          format: serialize_texture_format(
            entry.storage_texture_format.clone().unwrap(),
          )?,
          readonly: false,
        },
        _ => unreachable!(),
      },
      count: None,
    };
    entries.push(e);
  }

  let descriptor = wgc::binding_model::BindGroupLayoutDescriptor {
    label: args.label.map(Cow::Owned),
    entries: Cow::Owned(entries),
  };
  let bind_group_layout = wgc::gfx_select!(device => instance.device_create_bind_group_layout(
    device,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state
    .resource_table
    .add(WebGPUBindGroupLayout(bind_group_layout));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePipelineLayoutArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  bind_group_layouts: Vec<u32>,
}

pub fn op_webgpu_create_pipeline_layout(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreatePipelineLayoutArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let mut bind_group_layouts = vec![];

  for rid in &args.bind_group_layouts {
    let bind_group_layout = state
      .resource_table
      .get::<WebGPUBindGroupLayout>(*rid)
      .ok_or_else(bad_resource_id)?;
    bind_group_layouts.push(bind_group_layout.0);
  }

  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let descriptor = wgc::binding_model::PipelineLayoutDescriptor {
    label: args.label.map(Cow::Owned),
    bind_group_layouts: Cow::Owned(bind_group_layouts),
    push_constant_ranges: Default::default(),
  };
  let pipeline_layout = wgc::gfx_select!(device => instance.device_create_pipeline_layout(
    device,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state
    .resource_table
    .add(super::pipeline::WebGPUPipelineLayout(pipeline_layout));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPUBindGroupEntry {
  binding: u32,
  kind: String,
  resource: u32,
  offset: Option<u64>,
  size: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBindGroupArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  layout: u32,
  entries: Vec<GPUBindGroupEntry>,
}

pub fn op_webgpu_create_bind_group(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateBindGroupArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<super::WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let mut entries = vec![];

  for entry in &args.entries {
    let e = wgc::binding_model::BindGroupEntry {
      binding: entry.binding,
      resource: match entry.kind.as_str() {
        "GPUSampler" => {
          let sampler_resource = state
            .resource_table
            .get::<super::sampler::WebGPUSampler>(entry.resource)
            .ok_or_else(bad_resource_id)?;
          wgc::binding_model::BindingResource::Sampler(sampler_resource.0)
        }
        "GPUTextureView" => {
          let texture_view_resource = state
            .resource_table
            .get::<super::texture::WebGPUTextureView>(entry.resource)
            .ok_or_else(bad_resource_id)?;
          wgc::binding_model::BindingResource::TextureView(
            texture_view_resource.0,
          )
        }
        "GPUBufferBinding" => {
          let buffer_resource = state
            .resource_table
            .get::<super::buffer::WebGPUBuffer>(entry.resource)
            .ok_or_else(bad_resource_id)?;
          wgc::binding_model::BindingResource::Buffer(
            wgc::binding_model::BufferBinding {
              buffer_id: buffer_resource.0,
              offset: entry.offset.unwrap_or(0),
              size: std::num::NonZeroU64::new(entry.size.unwrap_or(0)),
            },
          )
        }
        _ => unreachable!(),
      },
    };
    entries.push(e);
  }

  let bind_group_layout = state
    .resource_table
    .get::<WebGPUBindGroupLayout>(args.layout)
    .ok_or_else(bad_resource_id)?;

  let descriptor = wgc::binding_model::BindGroupDescriptor {
    label: args.label.map(Cow::Owned),
    layout: bind_group_layout.0,
    entries: Cow::Owned(entries),
  };

  let instance_resource = state
    .resource_table
    .get::<super::WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let bind_group = wgc::gfx_select!(device => instance.device_create_bind_group(
    device,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state.resource_table.add(WebGPUBindGroup(bind_group));

  Ok(json!({
    "rid": rid,
  }))
}
