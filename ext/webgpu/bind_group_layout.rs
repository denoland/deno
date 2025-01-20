// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

use crate::texture::GPUTextureViewDimension;
use crate::Instance;

pub struct GPUBindGroupLayout {
  pub instance: Instance,
  pub id: wgpu_core::id::BindGroupLayoutId,
  pub label: String,
}

impl Drop for GPUBindGroupLayout {
  fn drop(&mut self) {
    self.instance.bind_group_layout_drop(self.id);
  }
}

impl deno_core::webidl::WebIdlInterfaceConverter for GPUBindGroupLayout {
  const NAME: &'static str = "GPUBindGroupLayout";
}

impl GarbageCollected for GPUBindGroupLayout {}

#[op2]
impl GPUBindGroupLayout {
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
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUBindGroupLayoutDescriptor {
  #[webidl(default = String::new())]
  pub label: String,
  pub entries: Vec<GPUBindGroupLayoutEntry>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUBindGroupLayoutEntry {
  #[options(enforce_range = true)]
  pub binding: u32,
  #[options(enforce_range = true)]
  pub visibility: u32,
  pub buffer: Option<GPUBufferBindingLayout>,
  pub sampler: Option<GPUSamplerBindingLayout>,
  pub texture: Option<GPUTextureBindingLayout>,
  pub storage_texture: Option<GPUStorageTextureBindingLayout>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUBufferBindingLayout {
  #[webidl(default = GPUBufferBindingType::Uniform)]
  pub r#type: GPUBufferBindingType,
  #[webidl(default = false)]
  pub has_dynamic_offset: bool,
  #[webidl(default = 0)]
  pub min_binding_size: u64,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUBufferBindingType {
  Uniform,
  Storage,
  ReadOnlyStorage,
}

impl From<GPUBufferBindingType> for wgpu_types::BufferBindingType {
  fn from(value: GPUBufferBindingType) -> Self {
    match value {
      GPUBufferBindingType::Uniform => Self::Uniform,
      GPUBufferBindingType::Storage => Self::Storage { read_only: false },
      GPUBufferBindingType::ReadOnlyStorage => {
        Self::Storage { read_only: true }
      }
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUSamplerBindingLayout {
  #[webidl(default = GPUSamplerBindingType::Filtering)]
  pub r#type: GPUSamplerBindingType,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUSamplerBindingType {
  Filtering,
  NonFiltering,
  Comparison,
}

impl From<GPUSamplerBindingType> for wgpu_types::SamplerBindingType {
  fn from(value: GPUSamplerBindingType) -> Self {
    match value {
      GPUSamplerBindingType::Filtering => Self::Filtering,
      GPUSamplerBindingType::NonFiltering => Self::NonFiltering,
      GPUSamplerBindingType::Comparison => Self::Comparison,
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUTextureBindingLayout {
  #[webidl(default = GPUTextureSampleType::Float)]
  pub sample_type: GPUTextureSampleType,
  #[webidl(default = GPUTextureViewDimension::D2)]
  pub view_dimension: GPUTextureViewDimension,
  #[webidl(default = false)]
  pub multisampled: bool,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUTextureSampleType {
  Float,
  UnfilterableFloat,
  Depth,
  Sint,
  Uint,
}

impl From<GPUTextureSampleType> for wgpu_types::TextureSampleType {
  fn from(value: GPUTextureSampleType) -> Self {
    match value {
      GPUTextureSampleType::Float => Self::Float { filterable: true },
      GPUTextureSampleType::UnfilterableFloat => {
        Self::Float { filterable: false }
      }
      GPUTextureSampleType::Depth => Self::Depth,
      GPUTextureSampleType::Sint => Self::Sint,
      GPUTextureSampleType::Uint => Self::Uint,
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUStorageTextureBindingLayout {
  #[webidl(default = GPUStorageTextureAccess::WriteOnly)]
  pub access: GPUStorageTextureAccess,
  pub format: super::texture::GPUTextureFormat,
  #[webidl(default = GPUTextureViewDimension::D2)]
  pub view_dimension: GPUTextureViewDimension,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUStorageTextureAccess {
  WriteOnly,
  ReadOnly,
  ReadWrite,
}

impl From<GPUStorageTextureAccess> for wgpu_types::StorageTextureAccess {
  fn from(value: GPUStorageTextureAccess) -> Self {
    match value {
      GPUStorageTextureAccess::WriteOnly => Self::WriteOnly,
      GPUStorageTextureAccess::ReadOnly => Self::ReadOnly,
      GPUStorageTextureAccess::ReadWrite => Self::ReadWrite,
    }
  }
}
