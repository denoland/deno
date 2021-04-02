// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

use super::error::WebGpuError;

pub(crate) struct WebGpuBindGroupLayout(
  pub(crate) wgpu_core::id::BindGroupLayoutId,
);
impl Resource for WebGpuBindGroupLayout {
  fn name(&self) -> Cow<str> {
    "webGPUBindGroupLayout".into()
  }
}

pub(crate) struct WebGpuBindGroup(pub(crate) wgpu_core::id::BindGroupId);
impl Resource for WebGpuBindGroup {
  fn name(&self) -> Cow<str> {
    "webGPUBindGroup".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuBufferBindingLayout {
  #[serde(rename = "type")]
  kind: Option<String>,
  has_dynamic_offset: Option<bool>,
  min_binding_size: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuSamplerBindingLayout {
  #[serde(rename = "type")]
  kind: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuTextureBindingLayout {
  sample_type: Option<String>,
  view_dimension: Option<String>,
  multisampled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuStorageTextureBindingLayout {
  access: String,
  format: String,
  view_dimension: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuBindGroupLayoutEntry {
  binding: u32,
  visibility: u32,
  buffer: Option<GpuBufferBindingLayout>,
  sampler: Option<GpuSamplerBindingLayout>,
  texture: Option<GpuTextureBindingLayout>,
  storage_texture: Option<GpuStorageTextureBindingLayout>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBindGroupLayoutArgs {
  device_rid: ResourceId,
  label: Option<String>,
  entries: Vec<GpuBindGroupLayoutEntry>,
}

pub fn op_webgpu_create_bind_group_layout(
  state: &mut OpState,
  args: CreateBindGroupLayoutArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let mut entries = vec![];

  for entry in &args.entries {
    entries.push(wgpu_types::BindGroupLayoutEntry {
      binding: entry.binding,
      visibility: wgpu_types::ShaderStage::from_bits(entry.visibility).unwrap(),
      ty: if let Some(buffer) = &entry.buffer {
        wgpu_types::BindingType::Buffer {
          ty: match &buffer.kind {
            Some(kind) => match kind.as_str() {
              "uniform" => wgpu_types::BufferBindingType::Uniform,
              "storage" => {
                wgpu_types::BufferBindingType::Storage { read_only: false }
              }
              "read-only-storage" => {
                wgpu_types::BufferBindingType::Storage { read_only: true }
              }
              _ => unreachable!(),
            },
            None => wgpu_types::BufferBindingType::Uniform,
          },
          has_dynamic_offset: buffer.has_dynamic_offset.unwrap_or(false),
          min_binding_size: if let Some(min_binding_size) =
            buffer.min_binding_size
          {
            std::num::NonZeroU64::new(min_binding_size)
          } else {
            None
          },
        }
      } else if let Some(sampler) = &entry.sampler {
        match &sampler.kind {
          Some(kind) => match kind.as_str() {
            "filtering" => wgpu_types::BindingType::Sampler {
              filtering: true,
              comparison: false,
            },
            "non-filtering" => wgpu_types::BindingType::Sampler {
              filtering: false,
              comparison: false,
            },
            "comparison" => wgpu_types::BindingType::Sampler {
              filtering: false,
              comparison: true,
            },
            _ => unreachable!(),
          },
          None => wgpu_types::BindingType::Sampler {
            filtering: true,
            comparison: false,
          },
        }
      } else if let Some(texture) = &entry.texture {
        wgpu_types::BindingType::Texture {
          sample_type: match &texture.sample_type {
            Some(sample_type) => match sample_type.as_str() {
              "float" => {
                wgpu_types::TextureSampleType::Float { filterable: true }
              }
              "unfilterable-float" => {
                wgpu_types::TextureSampleType::Float { filterable: false }
              }
              "depth" => wgpu_types::TextureSampleType::Depth,
              "sint" => wgpu_types::TextureSampleType::Sint,
              "uint" => wgpu_types::TextureSampleType::Uint,
              _ => unreachable!(),
            },
            None => wgpu_types::TextureSampleType::Float { filterable: true },
          },
          view_dimension: match &texture.view_dimension {
            Some(view_dimension) => {
              super::texture::serialize_dimension(view_dimension)
            }
            None => wgpu_types::TextureViewDimension::D2,
          },
          multisampled: texture.multisampled.unwrap_or(false),
        }
      } else if let Some(storage_texture) = &entry.storage_texture {
        wgpu_types::BindingType::StorageTexture {
          access: match storage_texture.access.as_str() {
            "read-only" => wgpu_types::StorageTextureAccess::ReadOnly,
            "write-only" => wgpu_types::StorageTextureAccess::WriteOnly,
            _ => unreachable!(),
          },
          format: super::texture::serialize_texture_format(
            &storage_texture.format,
          )?,
          view_dimension: match &storage_texture.view_dimension {
            Some(view_dimension) => {
              super::texture::serialize_dimension(view_dimension)
            }
            None => wgpu_types::TextureViewDimension::D2,
          },
        }
      } else {
        unreachable!()
      },
      count: None, // native-only
    });
  }

  let descriptor = wgpu_core::binding_model::BindGroupLayoutDescriptor {
    label: args.label.map(Cow::from),
    entries: Cow::from(entries),
  };

  let (bind_group_layout, maybe_err) = gfx_select!(device => instance.device_create_bind_group_layout(
    device,
    &descriptor,
    std::marker::PhantomData
  ));

  let rid = state
    .resource_table
    .add(WebGpuBindGroupLayout(bind_group_layout));

  Ok(json!({
    "rid": rid,
    "err": maybe_err.map(WebGpuError::from)
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePipelineLayoutArgs {
  device_rid: ResourceId,
  label: Option<String>,
  bind_group_layouts: Vec<u32>,
}

pub fn op_webgpu_create_pipeline_layout(
  state: &mut OpState,
  args: CreatePipelineLayoutArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let mut bind_group_layouts = vec![];

  for rid in &args.bind_group_layouts {
    let bind_group_layout = state
      .resource_table
      .get::<WebGpuBindGroupLayout>(*rid)
      .ok_or_else(bad_resource_id)?;
    bind_group_layouts.push(bind_group_layout.0);
  }

  let descriptor = wgpu_core::binding_model::PipelineLayoutDescriptor {
    label: args.label.map(Cow::from),
    bind_group_layouts: Cow::from(bind_group_layouts),
    push_constant_ranges: Default::default(),
  };

  let (pipeline_layout, maybe_err) = gfx_select!(device => instance.device_create_pipeline_layout(
    device,
    &descriptor,
    std::marker::PhantomData
  ));

  let rid = state
    .resource_table
    .add(super::pipeline::WebGpuPipelineLayout(pipeline_layout));

  Ok(json!({
    "rid": rid,
    "err": maybe_err.map(WebGpuError::from)
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuBindGroupEntry {
  binding: u32,
  kind: String,
  resource: u32,
  offset: Option<u64>,
  size: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBindGroupArgs {
  device_rid: ResourceId,
  label: Option<String>,
  layout: u32,
  entries: Vec<GpuBindGroupEntry>,
}

pub fn op_webgpu_create_bind_group(
  state: &mut OpState,
  args: CreateBindGroupArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;

  let mut entries = vec![];

  for entry in &args.entries {
    let e = wgpu_core::binding_model::BindGroupEntry {
      binding: entry.binding,
      resource: match entry.kind.as_str() {
        "GPUSampler" => {
          let sampler_resource = state
            .resource_table
            .get::<super::sampler::WebGpuSampler>(entry.resource)
            .ok_or_else(bad_resource_id)?;
          wgpu_core::binding_model::BindingResource::Sampler(sampler_resource.0)
        }
        "GPUTextureView" => {
          let texture_view_resource = state
            .resource_table
            .get::<super::texture::WebGpuTextureView>(entry.resource)
            .ok_or_else(bad_resource_id)?;
          wgpu_core::binding_model::BindingResource::TextureView(
            texture_view_resource.0,
          )
        }
        "GPUBufferBinding" => {
          let buffer_resource = state
            .resource_table
            .get::<super::buffer::WebGpuBuffer>(entry.resource)
            .ok_or_else(bad_resource_id)?;
          wgpu_core::binding_model::BindingResource::Buffer(
            wgpu_core::binding_model::BufferBinding {
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
    .get::<WebGpuBindGroupLayout>(args.layout)
    .ok_or_else(bad_resource_id)?;

  let descriptor = wgpu_core::binding_model::BindGroupDescriptor {
    label: args.label.map(Cow::from),
    layout: bind_group_layout.0,
    entries: Cow::from(entries),
  };

  let (bind_group, maybe_err) = gfx_select!(device => instance.device_create_bind_group(
    device,
    &descriptor,
    std::marker::PhantomData
  ));

  let rid = state.resource_table.add(WebGpuBindGroup(bind_group));

  Ok(json!({
    "rid": rid,
    "err": maybe_err.map(WebGpuError::from)
  }))
}
