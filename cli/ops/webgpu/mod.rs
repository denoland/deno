// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod binding;
mod buffer;
mod bundle;
mod command_encoder;
mod compute_pass;
mod pipeline;
mod queue;
mod render_pass;
mod sampler;
mod shader;
mod texture;

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

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(
    rt,
    "op_webgpu_create_instance",
    op_webgpu_create_instance,
  );
  super::reg_json_async(
    rt,
    "op_webgpu_request_adapter",
    op_webgpu_request_adapter,
  );
  super::reg_json_async(
    rt,
    "op_webgpu_request_device",
    op_webgpu_request_device,
  );

  buffer::init(rt);
  texture::init(rt);
  sampler::init(rt);
  binding::init(rt);
  pipeline::init(rt);
  command_encoder::init(rt);
  render_pass::init(rt);
  compute_pass::init(rt);
  bundle::init(rt);
  queue::init(rt);
  shader::init(rt);
}

fn serialize_features(features: &wgt::Features) -> Vec<&str> {
  let mut extensions: Vec<&str> = vec![];

  if features.contains(wgt::Features::DEPTH_CLAMPING) {
    extensions.push("depth-clamping");
  }
  if features.contains(wgt::Features) {
    // TODO
    extensions.push("depth24unorm-stencil8");
  }
  if features.contains(wgt::Features) {
    // TODO
    extensions.push("depth32float-stencil8");
  }
  if features.contains(wgt::Features) {
    // TODO
    extensions.push("pipeline-statistics-query");
  }
  if features.contains(wgt::Features::TEXTURE_COMPRESSION_BC) {
    extensions.push("texture-compression-bc");
  }
  if features.contains(wgt::Features) {
    // TODO
    extensions.push("timestamp-query");
  }

  extensions
}

pub type WgcInstance = wgc::hub::Global<wgc::hub::IdentityManagerFactory>;

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

  let adapter = instance.request_adapter(
    &wgc::instance::RequestAdapterOptions {
      power_preference: match args.power_preference {
        Some(&"low-power") => wgt::PowerPreference::LowPower,
        Some(&"high-performance") => wgt::PowerPreference::HighPerformance,
        Some(_) => unreachable!(),
        None => wgt::PowerPreference::Default,
      },
      compatible_surface: None, // windowless
    },
    wgc::instance::AdapterInputs::Mask(wgt::BackendBit::PRIMARY, ()), // TODO
  )?;

  let name = instance.adapter_get_info(adapter)?.name;
  let features = serialize_features(&instance.adapter_features(adapter)?);

  let rid = state
    .resource_table
    .add("webGPUInstance", Box::new(instance));
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
  // TODO: each should be Option
  max_bind_groups: u32,
  max_dynamic_uniform_buffers_per_pipeline_layout: u32,
  max_dynamic_storage_buffers_per_pipeline_layout: u32,
  max_sampled_textures_per_shader_stage: u32,
  max_samplers_per_shader_stage: u32,
  max_storage_buffers_per_shader_stage: u32,
  max_storage_textures_per_shader_stage: u32,
  max_uniform_buffers_per_shader_stage: u32,
  max_uniform_buffer_binding_size: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestDeviceArgs {
  instance_rid: u32,
  adapter_rid: u32,
  label: Option<String>, // wgpu#976
  features: Option<[String]>,
  limits: Option<GPULimits>,
}

pub async fn op_webgpu_request_device(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: RequestDeviceArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  let instance = state
    .resource_table
    .get_mut::<WgcInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let adapter = state
    .resource_table
    .get_mut::<wgc::id::AdapterId>(args.adapter_rid)
    .ok_or_else(bad_resource_id)?;

  let device = instance.adapter_request_device(
    *adapter,
    &wgt::DeviceDescriptor {
      features: Default::default(), // TODO
      limits: args.limits.map_or(Default::default(), |limits| {
        wgt::Limits {
          max_bind_groups: limits.max_bind_groups,
          max_dynamic_uniform_buffers_per_pipeline_layout: limits
            .max_dynamic_uniform_buffers_per_pipeline_layout,
          max_dynamic_storage_buffers_per_pipeline_layout: limits
            .max_dynamic_storage_buffers_per_pipeline_layout,
          max_sampled_textures_per_shader_stage: limits
            .max_sampled_textures_per_shader_stage,
          max_samplers_per_shader_stage: limits.max_samplers_per_shader_stage,
          max_storage_buffers_per_shader_stage: limits
            .max_storage_buffers_per_shader_stage,
          max_storage_textures_per_shader_stage: limits
            .max_storage_textures_per_shader_stage,
          max_uniform_buffers_per_shader_stage: limits
            .max_uniform_buffers_per_shader_stage,
          max_uniform_buffer_binding_size: limits
            .max_uniform_buffer_binding_size,
          max_push_constant_size: 0, // TODO
        }
      }),
      shader_validation: false, // TODO
    },
    None,
    std::marker::PhantomData,
  )?;

  let features = serialize_features(&instance.device_features(device)?);
  let limits = instance.device_limits(device)?;
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
