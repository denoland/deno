// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod binding;
mod buffer;
mod bundle;
mod command_encoding;
mod pipeline;
mod sampler;
mod shader;
mod texture;

use deno_core::error::bad_resource_id;
use deno_core::error::type_error;
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

  super::reg_json_sync(
    rt,
    "op_webgpu_create_buffer",
    buffer::op_webgpu_create_buffer,
  );
  super::reg_json_async(
    rt,
    "op_webgpu_buffer_get_map_async",
    buffer::op_webgpu_buffer_get_map_async,
  );
  super::reg_json_async(
    rt,
    "op_webgpu_buffer_get_mapped_range",
    buffer::op_webgpu_buffer_get_mapped_range,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_buffer_unmap",
    buffer::op_webgpu_buffer_unmap,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_texture",
    texture::op_webgpu_create_texture,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_create_texture_view",
    texture::op_webgpu_create_texture_view,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_sampler",
    sampler::op_webgpu_create_sampler,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_bind_group_layout",
    binding::op_webgpu_create_bind_group_layout,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_create_pipeline_layout",
    binding::op_webgpu_create_pipeline_layout,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_create_bind_group",
    binding::op_webgpu_create_bind_group,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_compute_pipeline",
    pipeline::op_webgpu_create_compute_pipeline,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_compute_pipeline_get_bind_group_layout",
    pipeline::op_webgpu_compute_pipeline_get_bind_group_layout,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_create_render_pipeline",
    pipeline::op_webgpu_create_render_pipeline,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_pipeline_get_bind_group_layout",
    pipeline::op_webgpu_render_pipeline_get_bind_group_layout,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_command_encoder",
    command_encoding::op_webgpu_create_command_encoder,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_begin_render_pass",
    command_encoding::op_webgpu_command_encoder_begin_render_pass,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_begin_compute_pass",
    command_encoding::op_webgpu_command_encoder_begin_compute_pass,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_command_encoder_copy_texture_to_texture",
    command_encoding::op_webgpu_command_encoder_copy_texture_to_texture,
  );

  super::reg_json_sync(
    rt,
    "op_webgpu_create_render_bundle_encoder",
    bundle::op_webgpu_create_render_bundle_encoder,
  );
  super::reg_json_sync(
    rt,
    "op_webgpu_render_bundle_encoder_finish",
    bundle::op_webgpu_render_bundle_encoder_finish,
  );
}

fn serialize_features(features: &wgt::Features) -> Vec<&str> {
  let mut extensions: Vec<&str> = vec![];

  if features.contains(wgt::Features::DEPTH_CLAMPING) {
    extensions.push("depth-clamping");
  }
  if features.contains(wgt::Features) { // TODO
    extensions.push("depth24unorm-stencil8");
  }
  if features.contains(wgt::Features) { // TODO
    extensions.push("depth32float-stencil8");
  }
  if features.contains(wgt::Features) { // TODO
    extensions.push("pipeline-statistics-query");
  }
  if features.contains(wgt::Features::TEXTURE_COMPRESSION_BC) {
    extensions.push("texture-compression-bc");
  }
  if features.contains(wgt::Features) { // TODO
    extensions.push("timestamp-query");
  }

  extensions
}

pub type WgcInstance = wgc::hub::Global<wgc::hub::IdentityManagerFactory>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestAdapterArgs {
  power_preference: Option<String>,
}

pub async fn op_webgpu_request_adapter(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: RequestAdapterArgs = serde_json::from_value(args)?;

  let instance = wgc::hub::Global::new(
    "webgpu",
    wgc::hub::IdentityManagerFactory,
    wgt::BackendBit::PRIMARY,
  ); // TODO: own op
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
  let extensions = serialize_features(&instance.adapter_features(adapter)?);

  let mut state = state.borrow_mut();
  let rid = state
    .resource_table
    .add("webGPUInstance", Box::new(instance));
  let rid = state.resource_table.add("webGPUAdapter", Box::new(adapter));

  Ok(json!({
    "rid": rid,
    "name": name,
    "extensions": extensions,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestDeviceArgs {
  instance_rid: u32,
  adapter_rid: u32,
  extensions: Option<[String]>,
  limits: Option<String>, // TODO
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
      // TODO: should accept label
      features: Default::default(), // TODO
      limits: Default::default(),   // TODO
      shader_validation: false,     // TODO
    },
    None,
    (), // TODO
  )?;

  let extensions = serialize_features(&instance.device_features(device)?);
  let limits = instance.device_limits(device)?; // TODO

  let device_rid = state.resource_table.add("webGPUDevice", Box::new(device));

  Ok(json!({
    "deviceRid": device_rid,
    "queueRid": queue_rid,
    "extensions": extensions,
    "limits", // TODO
  }))
}
