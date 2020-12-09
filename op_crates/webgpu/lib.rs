// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

pub mod binding;
pub mod buffer;
pub mod bundle;
pub mod command_encoder;
pub mod compute_pass;
pub mod pipeline;
pub mod queue;
pub mod render_pass;
pub mod sampler;
pub mod shader;
pub mod texture;

use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use serde::Deserialize;
use std::cell::RefCell;
use std::rc::Rc;

/// Execute this crates' JS source files.
pub fn init(isolate: &mut deno_core::JsRuntime) {
  let files = vec![(
    "deno:op_crates/webgpu/14_webgpu.js",
    include_str!("14_webgpu.js"),
  )];
  for (url, source_code) in files {
    isolate.execute(url, source_code).unwrap();
  }
}

fn deserialize_features(features: &wgt::Features) -> Vec<&str> {
  let mut return_features: Vec<&str> = vec![];

  if features.contains(wgt::Features::DEPTH_CLAMPING) {
    return_features.push("depth-clamping");
  }
  if features.contains(wgt::Features::TEXTURE_COMPRESSION_BC) {
    return_features.push("texture-compression-bc");
  }

  return_features
}

type WgcInstance = wgc::hub::Global<wgc::hub::IdentityManagerFactory>;

pub fn op_webgpu_create_instance(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = wgc::hub::Global::new(
    "webgpu",
    wgc::hub::IdentityManagerFactory,
    wgt::BackendBit::PRIMARY,
  );

  let rid = state
    .resource_table
    .add("webGPUInstance", Box::new(instance));

  Ok(json!({
    "rid": rid,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestAdapterArgs {
  instance_rid: u32,
  power_preference: Option<String>,
}

pub async fn op_webgpu_request_adapter(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: RequestAdapterArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  let instance = state
    .resource_table
    .get_mut::<WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  let descriptor = wgc::instance::RequestAdapterOptions {
    power_preference: match args.power_preference {
      Some(power_preference) => match power_preference.as_str() {
        "low-power" => wgt::PowerPreference::LowPower,
        "high-performance" => wgt::PowerPreference::HighPerformance,
        _ => unreachable!(),
      },
      None => wgt::PowerPreference::Default,
    },
    compatible_surface: None, // windowless
  };
  let adapter = instance.request_adapter(
    &descriptor,
    wgc::instance::AdapterInputs::Mask(wgt::BackendBit::PRIMARY, |_| {
      std::marker::PhantomData
    }),
  )?;

  let name =
    wgc::gfx_select!(adapter => instance.adapter_get_info(adapter))?.name;
  let adapter_features =
    wgc::gfx_select!(adapter => instance.adapter_features(adapter))?;
  let features = deserialize_features(&adapter_features);

  let rid = state.resource_table.add("webGPUAdapter", Box::new(adapter));

  Ok(json!({
    "rid": rid,
    "name": name,
    "features": features,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPULimits {
  max_bind_groups: Option<u32>,
  max_dynamic_uniform_buffers_per_pipeline_layout: Option<u32>,
  max_dynamic_storage_buffers_per_pipeline_layout: Option<u32>,
  max_sampled_textures_per_shader_stage: Option<u32>,
  max_samplers_per_shader_stage: Option<u32>,
  max_storage_buffers_per_shader_stage: Option<u32>,
  max_storage_textures_per_shader_stage: Option<u32>,
  max_uniform_buffers_per_shader_stage: Option<u32>,
  max_uniform_buffer_binding_size: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestDeviceArgs {
  instance_rid: u32,
  adapter_rid: u32,
  _label: Option<String>, // wgpu#976
  features: Option<Vec<String>>,
  limits: Option<GPULimits>,
}

pub async fn op_webgpu_request_device(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: RequestDeviceArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  let adapter = *state
    .resource_table
    .get_mut::<wgc::id::AdapterId>(args.adapter_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = state
    .resource_table
    .get_mut::<WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;

  let mut features = wgt::Features::default();

  if let Some(passed_features) = args.features {
    if passed_features.contains(&"depth-clamping".to_string()) {
      features.set(wgt::Features::DEPTH_CLAMPING, true);
    }
    if passed_features.contains(&"texture-compression-bc".to_string()) {
      features.set(wgt::Features::TEXTURE_COMPRESSION_BC, true);
    }
  }

  let descriptor = wgt::DeviceDescriptor {
    features,
    limits: args
      .limits
      .map_or(Default::default(), |limits| wgt::Limits {
        max_bind_groups: limits.max_bind_groups.unwrap_or(4),
        max_dynamic_uniform_buffers_per_pipeline_layout: limits
          .max_dynamic_uniform_buffers_per_pipeline_layout
          .unwrap_or(8),
        max_dynamic_storage_buffers_per_pipeline_layout: limits
          .max_dynamic_storage_buffers_per_pipeline_layout
          .unwrap_or(4),
        max_sampled_textures_per_shader_stage: limits
          .max_sampled_textures_per_shader_stage
          .unwrap_or(16),
        max_samplers_per_shader_stage: limits
          .max_samplers_per_shader_stage
          .unwrap_or(16),
        max_storage_buffers_per_shader_stage: limits
          .max_storage_buffers_per_shader_stage
          .unwrap_or(4),
        max_storage_textures_per_shader_stage: limits
          .max_storage_textures_per_shader_stage
          .unwrap_or(4),
        max_uniform_buffers_per_shader_stage: limits
          .max_uniform_buffers_per_shader_stage
          .unwrap_or(12),
        max_uniform_buffer_binding_size: limits
          .max_uniform_buffer_binding_size
          .unwrap_or(16384),
        max_push_constant_size: 0,
      }),
    shader_validation: false,
  };
  let device = wgc::gfx_select!(adapter => instance.adapter_request_device(
    adapter,
    &descriptor,
    None,
    std::marker::PhantomData
  ))?;

  let device_features =
    wgc::gfx_select!(device => instance.device_features(device))?;
  let features = deserialize_features(&device_features);
  let limits = wgc::gfx_select!(device => instance.device_limits(device))?;
  let json_limits = json!({
     "max_bind_groups": limits.max_bind_groups,
     "max_dynamic_uniform_buffers_per_pipeline_layout": limits.max_dynamic_uniform_buffers_per_pipeline_layout,
     "max_dynamic_storage_buffers_per_pipeline_layout": limits.max_dynamic_storage_buffers_per_pipeline_layout,
     "max_sampled_textures_per_shader_stage": limits.max_sampled_textures_per_shader_stage,
     "max_samplers_per_shader_stage": limits.max_samplers_per_shader_stage,
     "max_storage_buffers_per_shader_stage": limits.max_storage_buffers_per_shader_stage,
     "max_storage_textures_per_shader_stage": limits.max_storage_textures_per_shader_stage,
     "max_uniform_buffers_per_shader_stage": limits.max_uniform_buffers_per_shader_stage,
     "max_uniform_buffer_binding_size": limits.max_uniform_buffer_binding_size,
  });

  let rid = state.resource_table.add("webGPUDevice", Box::new(device));

  Ok(json!({
    "rid": rid,
    "features": features,
    "limits": json_limits,
  }))
}
