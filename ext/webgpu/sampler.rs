// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

use crate::Instance;

pub struct GPUSampler {
  pub instance: Instance,
  pub id: wgpu_core::id::SamplerId,
  pub label: String,
}

impl Drop for GPUSampler {
  fn drop(&mut self) {
    self.instance.sampler_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUSampler {
  const NAME: &'static str = "GPUSampler";
}

impl GarbageCollected for GPUSampler {}

#[op2]
impl GPUSampler {
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
pub(super) struct GPUSamplerDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  #[webidl(default = GPUAddressMode::ClampToEdge)]
  pub address_mode_u: GPUAddressMode,
  #[webidl(default = GPUAddressMode::ClampToEdge)]
  pub address_mode_v: GPUAddressMode,
  #[webidl(default = GPUAddressMode::ClampToEdge)]
  pub address_mode_w: GPUAddressMode,
  #[webidl(default = GPUFilterMode::Nearest)]
  pub mag_filter: GPUFilterMode,
  #[webidl(default = GPUFilterMode::Nearest)]
  pub min_filter: GPUFilterMode,
  #[webidl(default = GPUFilterMode::Nearest)]
  pub mipmap_filter: GPUFilterMode,

  #[webidl(default = 0.0)]
  pub lod_min_clamp: f32,
  #[webidl(default = 32.0)]
  pub lod_max_clamp: f32,

  pub compare: Option<GPUCompareFunction>,

  #[webidl(default = 1)]
  #[options(clamp = true)]
  pub max_anisotropy: u16,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUAddressMode {
  ClampToEdge,
  Repeat,
  MirrorRepeat,
}

impl From<GPUAddressMode> for wgpu_types::AddressMode {
  fn from(value: GPUAddressMode) -> Self {
    match value {
      GPUAddressMode::ClampToEdge => Self::ClampToEdge,
      GPUAddressMode::Repeat => Self::Repeat,
      GPUAddressMode::MirrorRepeat => Self::MirrorRepeat,
    }
  }
}

// Same as GPUMipmapFilterMode
#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUFilterMode {
  Nearest,
  Linear,
}

impl From<GPUFilterMode> for wgpu_types::FilterMode {
  fn from(value: GPUFilterMode) -> Self {
    match value {
      GPUFilterMode::Nearest => Self::Nearest,
      GPUFilterMode::Linear => Self::Linear,
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUCompareFunction {
  Never,
  Less,
  Equal,
  LessEqual,
  Greater,
  NotEqual,
  GreaterEqual,
  Always,
}

impl From<GPUCompareFunction> for wgpu_types::CompareFunction {
  fn from(value: GPUCompareFunction) -> Self {
    match value {
      GPUCompareFunction::Never => Self::Never,
      GPUCompareFunction::Less => Self::Less,
      GPUCompareFunction::Equal => Self::Equal,
      GPUCompareFunction::LessEqual => Self::LessEqual,
      GPUCompareFunction::Greater => Self::Greater,
      GPUCompareFunction::NotEqual => Self::NotEqual,
      GPUCompareFunction::GreaterEqual => Self::GreaterEqual,
      GPUCompareFunction::Always => Self::Always,
    }
  }
}
