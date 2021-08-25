// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::ResourceId;
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

use super::error::WebGpuResult;

pub(crate) struct WebGpuSampler(pub(crate) wgpu_core::id::SamplerId);
impl Resource for WebGpuSampler {
  fn name(&self) -> Cow<str> {
    "webGPUSampler".into()
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum GpuAddressMode {
  ClampToEdge,
  Repeat,
  MirrorRepeat,
}

impl From<GpuAddressMode> for wgpu_types::AddressMode {
  fn from(value: GpuAddressMode) -> wgpu_types::AddressMode {
    match value {
      GpuAddressMode::ClampToEdge => wgpu_types::AddressMode::ClampToEdge,
      GpuAddressMode::Repeat => wgpu_types::AddressMode::Repeat,
      GpuAddressMode::MirrorRepeat => wgpu_types::AddressMode::MirrorRepeat,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum GpuFilterMode {
  Nearest,
  Linear,
}

impl From<GpuFilterMode> for wgpu_types::FilterMode {
  fn from(value: GpuFilterMode) -> wgpu_types::FilterMode {
    match value {
      GpuFilterMode::Nearest => wgpu_types::FilterMode::Nearest,
      GpuFilterMode::Linear => wgpu_types::FilterMode::Linear,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuCompareFunction {
  Never,
  Less,
  Equal,
  LessEqual,
  Greater,
  NotEqual,
  GreaterEqual,
  Always,
}

impl From<GpuCompareFunction> for wgpu_types::CompareFunction {
  fn from(value: GpuCompareFunction) -> wgpu_types::CompareFunction {
    match value {
      GpuCompareFunction::Never => wgpu_types::CompareFunction::Never,
      GpuCompareFunction::Less => wgpu_types::CompareFunction::Less,
      GpuCompareFunction::Equal => wgpu_types::CompareFunction::Equal,
      GpuCompareFunction::LessEqual => wgpu_types::CompareFunction::LessEqual,
      GpuCompareFunction::Greater => wgpu_types::CompareFunction::Greater,
      GpuCompareFunction::NotEqual => wgpu_types::CompareFunction::NotEqual,
      GpuCompareFunction::GreaterEqual => {
        wgpu_types::CompareFunction::GreaterEqual
      }
      GpuCompareFunction::Always => wgpu_types::CompareFunction::Always,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSamplerArgs {
  device_rid: ResourceId,
  label: Option<String>,
  address_mode_u: GpuAddressMode,
  address_mode_v: GpuAddressMode,
  address_mode_w: GpuAddressMode,
  mag_filter: GpuFilterMode,
  min_filter: GpuFilterMode,
  mipmap_filter: GpuFilterMode,
  lod_min_clamp: f32,
  lod_max_clamp: f32,
  compare: Option<GpuCompareFunction>,
  max_anisotropy: u8,
}

pub fn op_webgpu_create_sampler(
  state: &mut OpState,
  args: CreateSamplerArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let instance = state.borrow::<super::Instance>();
  let device_resource = state
    .resource_table
    .get::<super::WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;

  let descriptor = wgpu_core::resource::SamplerDescriptor {
    label: args.label.map(Cow::from),
    address_modes: [
      args.address_mode_u.into(),
      args.address_mode_v.into(),
      args.address_mode_w.into(),
    ],
    mag_filter: args.mag_filter.into(),
    min_filter: args.min_filter.into(),
    mipmap_filter: args.mipmap_filter.into(),
    lod_min_clamp: args.lod_min_clamp,
    lod_max_clamp: args.lod_max_clamp,
    compare: args.compare.map(Into::into),
    anisotropy_clamp: std::num::NonZeroU8::new(args.max_anisotropy),
    border_color: None, // native-only
  };

  gfx_put!(device => instance.device_create_sampler(
    device,
    &descriptor,
    std::marker::PhantomData
  ) => state, WebGpuSampler)
}
