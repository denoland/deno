// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::rc::Rc;

use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;

use super::error::WebGpuResult;

pub(crate) struct WebGpuBindGroupLayout(
  pub(crate) crate::Instance,
  pub(crate) wgpu_core::id::BindGroupLayoutId,
);
impl Resource for WebGpuBindGroupLayout {
  fn name(&self) -> Cow<str> {
    "webGPUBindGroupLayout".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.bind_group_layout_drop(self.1));
  }
}

pub(crate) struct WebGpuBindGroup(
  pub(crate) crate::Instance,
  pub(crate) wgpu_core::id::BindGroupId,
);
impl Resource for WebGpuBindGroup {
  fn name(&self) -> Cow<str> {
    "webGPUBindGroup".into()
  }

  fn close(self: Rc<Self>) {
    gfx_select!(self.1 => self.0.bind_group_drop(self.1));
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuBufferBindingLayout {
  r#type: GpuBufferBindingType,
  has_dynamic_offset: bool,
  min_binding_size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum GpuBufferBindingType {
  Uniform,
  Storage,
  ReadOnlyStorage,
}

impl From<GpuBufferBindingType> for wgpu_types::BufferBindingType {
  fn from(binding_type: GpuBufferBindingType) -> Self {
    match binding_type {
      GpuBufferBindingType::Uniform => wgpu_types::BufferBindingType::Uniform,
      GpuBufferBindingType::Storage => {
        wgpu_types::BufferBindingType::Storage { read_only: false }
      }
      GpuBufferBindingType::ReadOnlyStorage => {
        wgpu_types::BufferBindingType::Storage { read_only: true }
      }
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuSamplerBindingLayout {
  r#type: wgpu_types::SamplerBindingType,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuTextureBindingLayout {
  sample_type: GpuTextureSampleType,
  view_dimension: wgpu_types::TextureViewDimension,
  multisampled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum GpuTextureSampleType {
  Float,
  UnfilterableFloat,
  Depth,
  Sint,
  Uint,
}

impl From<GpuTextureSampleType> for wgpu_types::TextureSampleType {
  fn from(sample_type: GpuTextureSampleType) -> Self {
    match sample_type {
      GpuTextureSampleType::Float => {
        wgpu_types::TextureSampleType::Float { filterable: true }
      }
      GpuTextureSampleType::UnfilterableFloat => {
        wgpu_types::TextureSampleType::Float { filterable: false }
      }
      GpuTextureSampleType::Depth => wgpu_types::TextureSampleType::Depth,
      GpuTextureSampleType::Sint => wgpu_types::TextureSampleType::Sint,
      GpuTextureSampleType::Uint => wgpu_types::TextureSampleType::Uint,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuStorageTextureBindingLayout {
  access: wgpu_types::StorageTextureAccess,
  format: wgpu_types::TextureFormat,
  view_dimension: wgpu_types::TextureViewDimension,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuBindGroupLayoutEntry {
  binding: u32,
  visibility: u32,
  #[serde(flatten)]
  binding_type: GpuBindingType,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum GpuBindingType {
  Buffer(GpuBufferBindingLayout),
  Sampler(GpuSamplerBindingLayout),
  Texture(GpuTextureBindingLayout),
  StorageTexture(GpuStorageTextureBindingLayout),
}

impl From<GpuBindingType> for wgpu_types::BindingType {
  fn from(binding_type: GpuBindingType) -> wgpu_types::BindingType {
    match binding_type {
      GpuBindingType::Buffer(buffer) => wgpu_types::BindingType::Buffer {
        ty: buffer.r#type.into(),
        has_dynamic_offset: buffer.has_dynamic_offset,
        min_binding_size: std::num::NonZeroU64::new(buffer.min_binding_size),
      },
      GpuBindingType::Sampler(sampler) => {
        wgpu_types::BindingType::Sampler(sampler.r#type)
      }
      GpuBindingType::Texture(texture) => wgpu_types::BindingType::Texture {
        sample_type: texture.sample_type.into(),
        view_dimension: texture.view_dimension,
        multisampled: texture.multisampled,
      },
      GpuBindingType::StorageTexture(storage_texture) => {
        wgpu_types::BindingType::StorageTexture {
          access: storage_texture.access,
          format: storage_texture.format,
          view_dimension: storage_texture.view_dimension,
        }
      }
    }
  }
}

#[op2]
#[serde]
pub fn op_webgpu_create_bind_group_layout(
  state: &mut OpState,
  #[smi] device_rid: ResourceId,
  #[string] label: Cow<str>,
  #[serde] entries: Vec<GpuBindGroupLayoutEntry>,
) -> Result<WebGpuResult, ResourceError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.1;

  let entries = entries
    .into_iter()
    .map(|entry| {
      wgpu_types::BindGroupLayoutEntry {
        binding: entry.binding,
        visibility: wgpu_types::ShaderStages::from_bits(entry.visibility)
          .unwrap(),
        ty: entry.binding_type.into(),
        count: None, // native-only
      }
    })
    .collect::<Vec<_>>();

  let descriptor = wgpu_core::binding_model::BindGroupLayoutDescriptor {
    label: Some(label),
    entries: Cow::from(entries),
  };

  gfx_put!(device => instance.device_create_bind_group_layout(
    device,
    &descriptor,
    None
  ) => state, WebGpuBindGroupLayout)
}

#[op2]
#[serde]
pub fn op_webgpu_create_pipeline_layout(
  state: &mut OpState,
  #[smi] device_rid: ResourceId,
  #[string] label: Cow<str>,
  #[serde] bind_group_layouts: Vec<u32>,
) -> Result<WebGpuResult, ResourceError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.1;

  let bind_group_layouts = bind_group_layouts
    .into_iter()
    .map(|rid| {
      let bind_group_layout =
        state.resource_table.get::<WebGpuBindGroupLayout>(rid)?;
      Ok(bind_group_layout.1)
    })
    .collect::<Result<Vec<_>, ResourceError>>()?;

  let descriptor = wgpu_core::binding_model::PipelineLayoutDescriptor {
    label: Some(label),
    bind_group_layouts: Cow::from(bind_group_layouts),
    push_constant_ranges: Default::default(),
  };

  gfx_put!(device => instance.device_create_pipeline_layout(
    device,
    &descriptor,
    None
  ) => state, super::pipeline::WebGpuPipelineLayout)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuBindGroupEntry {
  binding: u32,
  kind: String,
  resource: ResourceId,
  offset: Option<u64>,
  size: Option<u64>,
}

#[op2]
#[serde]
pub fn op_webgpu_create_bind_group(
  state: &mut OpState,
  #[smi] device_rid: ResourceId,
  #[string] label: Cow<str>,
  #[smi] layout: ResourceId,
  #[serde] entries: Vec<GpuBindGroupEntry>,
) -> Result<WebGpuResult, ResourceError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(device_rid)?;
  let device = device_resource.1;

  let entries = entries
    .into_iter()
    .map(|entry| {
      Ok(wgpu_core::binding_model::BindGroupEntry {
        binding: entry.binding,
        resource: match entry.kind.as_str() {
          "GPUSampler" => {
            let sampler_resource =
              state
                .resource_table
                .get::<super::sampler::WebGpuSampler>(entry.resource)?;
            wgpu_core::binding_model::BindingResource::Sampler(
              sampler_resource.1,
            )
          }
          "GPUTextureView" => {
            let texture_view_resource =
              state
                .resource_table
                .get::<super::texture::WebGpuTextureView>(entry.resource)?;
            wgpu_core::binding_model::BindingResource::TextureView(
              texture_view_resource.1,
            )
          }
          "GPUBufferBinding" => {
            let buffer_resource =
              state
                .resource_table
                .get::<super::buffer::WebGpuBuffer>(entry.resource)?;
            wgpu_core::binding_model::BindingResource::Buffer(
              wgpu_core::binding_model::BufferBinding {
                buffer_id: buffer_resource.1,
                offset: entry.offset.unwrap_or(0),
                size: std::num::NonZeroU64::new(entry.size.unwrap_or(0)),
              },
            )
          }
          _ => unreachable!(),
        },
      })
    })
    .collect::<Result<Vec<_>, ResourceError>>()?;

  let bind_group_layout =
    state.resource_table.get::<WebGpuBindGroupLayout>(layout)?;

  let descriptor = wgpu_core::binding_model::BindGroupDescriptor {
    label: Some(label),
    layout: bind_group_layout.1,
    entries: Cow::from(entries),
  };

  gfx_put!(device => instance.device_create_bind_group(
    device,
    &descriptor,
    None
  ) => state, WebGpuBindGroup)
}
