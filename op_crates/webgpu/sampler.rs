// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::{serde_json, RcRef, ZeroCopyBuf};
use deno_core::{OpState, Resource};
use serde::Deserialize;
use std::borrow::Cow;

pub(crate) struct WebGPUSampler(pub(crate) wgc::id::SamplerId);
impl Resource for WebGPUSampler {
  fn name(&self) -> Cow<str> {
    "webGPUSampler".into()
  }
}

fn serialize_address_mode(address_mode: Option<String>) -> wgt::AddressMode {
  match address_mode {
    Some(address_mode) => match address_mode.as_str() {
      "clamp-to-edge" => wgt::AddressMode::ClampToEdge,
      "repeat" => wgt::AddressMode::Repeat,
      "mirror-repeat" => wgt::AddressMode::MirrorRepeat,
      _ => unreachable!(),
    },
    None => wgt::AddressMode::ClampToEdge,
  }
}

fn serialize_filter_mode(filter_mode: Option<String>) -> wgt::FilterMode {
  match filter_mode {
    Some(filter_mode) => match filter_mode.as_str() {
      "nearest" => wgt::FilterMode::Nearest,
      "linear" => wgt::FilterMode::Linear,
      _ => unreachable!(),
    },
    None => wgt::FilterMode::Nearest,
  }
}

pub fn serialize_compare_function(compare: &str) -> wgt::CompareFunction {
  match compare {
    "never" => wgt::CompareFunction::Never,
    "less" => wgt::CompareFunction::Less,
    "equal" => wgt::CompareFunction::Equal,
    "less-equal" => wgt::CompareFunction::LessEqual,
    "greater" => wgt::CompareFunction::Greater,
    "not-equal" => wgt::CompareFunction::NotEqual,
    "greater-equal" => wgt::CompareFunction::GreaterEqual,
    "always" => wgt::CompareFunction::Always,
    _ => unreachable!(),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSamplerArgs {
  instance_rid: u32,
  device_rid: u32,
  label: Option<String>,
  address_mode_u: Option<String>,
  address_mode_v: Option<String>,
  address_mode_w: Option<String>,
  mag_filter: Option<String>,
  min_filter: Option<String>,
  mipmap_filter: Option<String>,
  lod_min_clamp: Option<f32>,
  lod_max_clamp: Option<f32>,
  compare: Option<String>,
  max_anisotropy: Option<u8>,
}

pub fn op_webgpu_create_sampler(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateSamplerArgs = serde_json::from_value(args)?;

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

  let descriptor = wgc::resource::SamplerDescriptor {
    label: args.label.map(Cow::Owned),
    address_modes: [
      serialize_address_mode(args.address_mode_u),
      serialize_address_mode(args.address_mode_v),
      serialize_address_mode(args.address_mode_w),
    ],
    mag_filter: serialize_filter_mode(args.mag_filter),
    min_filter: serialize_filter_mode(args.min_filter),
    mipmap_filter: serialize_filter_mode(args.mipmap_filter),
    lod_min_clamp: args.lod_min_clamp.unwrap_or(0.0),
    lod_max_clamp: args
      .lod_max_clamp
      .unwrap_or(wgc::resource::SamplerDescriptor::default().lod_max_clamp),
    compare: args
      .compare
      .as_ref()
      .map(|compare| serialize_compare_function(compare)),
    anisotropy_clamp: std::num::NonZeroU8::new(
      args.max_anisotropy.unwrap_or(0),
    ),
  };
  let sampler = wgc::gfx_select!(device => instance.device_create_sampler(
    device,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state.resource_table.add(WebGPUSampler(sampler));

  Ok(json!({
    "rid": rid,
  }))
}
