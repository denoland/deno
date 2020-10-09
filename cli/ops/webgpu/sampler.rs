// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

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

fn serialize_address_mode(address_mode: Option<String>) -> wgpu::AddressMode {
  match address_mode {
    Some(&"clamp-to-edge") => wgpu::AddressMode::ClampToEdge,
    Some(&"repeat") => wgpu::AddressMode::Repeat,
    Some(&"mirror-repeat") => wgpu::AddressMode::MirrorRepeat,
    Some(_) => unreachable!(),
    None => wgpu::AddressMode::ClampToEdge,
  }
}

fn serialize_filter_mode(filter_mode: Option<String>) -> wgpu::FilterMode {
  match filter_mode {
    Some(&"nearest") => wgpu::FilterMode::Nearest,
    Some(&"linear") => wgpu::FilterMode::Linear,
    Some(_) => unreachable!(),
    None => wgpu::FilterMode::Nearest,
  }
}

pub fn serialize_compare_function(compare: String) -> wgpu::CompareFunction {
  match compare {
    &"never" => wgpu::CompareFunction::Never,
    &"less" => wgpu::CompareFunction::Less,
    &"equal" => wgpu::CompareFunction::Equal,
    &"less-equal" => wgpu::CompareFunction::LessEqual,
    &"greater" => wgpu::CompareFunction::Greater,
    &"not-equal" => wgpu::CompareFunction::NotEqual,
    &"greater-equal" => wgpu::CompareFunction::GreaterEqual,
    &"always" => wgpu::CompareFunction::Always,
    _ => unreachable!(),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSamplerArgs {
  rid: u32,
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

  let device = state
    .resource_table
    .get_mut::<wgpu::Device>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    label: args.label.map(|label| &label),
    address_mode_u: serialize_address_mode(args.address_mode_u),
    address_mode_v: serialize_address_mode(args.address_mode_v),
    address_mode_w: serialize_address_mode(args.address_mode_w),
    mag_filter: serialize_filter_mode(args.mag_filter),
    min_filter: serialize_filter_mode(args.min_filter),
    mipmap_filter: serialize_filter_mode(args.mipmap_filter),
    lod_min_clamp: args.lod_min_clamp.unwrap_or(0.0),
    lod_max_clamp: args.lod_max_clamp.unwrap_or(0xffffffff as f32), // TODO
    compare: args
      .compare
      .map(|compare| serialize_compare_function(compare)),
    anisotropy_clamp: args.max_anisotropy.unwrap_or(1), // TODO
  });

  let rid = state.resource_table.add("webGPUTexture", Box::new(sampler));

  Ok(json!({
    "rid": rid,
  }))
}
