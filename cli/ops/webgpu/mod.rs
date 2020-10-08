// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

mod buffer;
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
}

fn serialize_features(features: &wgpu::Features) -> Vec<&str> {
  let mut extensions: Vec<&str> = vec![];

  if features.contains(wgpu::Features::DEPTH_CLAMPING) {
    extensions.push("depth-clamping");
  }
  if features.contains(wgpu::Features) {
    // TODO
    extensions.push("depth24unorm-stencil8");
  }
  if features.contains(wgpu::Features) {
    // TODO
    extensions.push("depth32float-stencil8");
  }
  if features.contains(wgpu::Features) {
    // TODO
    extensions.push("pipeline-statistics-query");
  }
  if features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
    extensions.push("texture-compression-bc");
  }
  if features.contains(wgpu::Features) {
    // TODO
    extensions.push("timestamp-query");
  }

  extensions
}

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

  let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY); // TODO: own op
  let adapter = instance
    .request_adapter(&wgpu::RequestAdapterOptions {
      power_preference: match args.power_preference {
        Some(&"low-power") => wgpu::PowerPreference::LowPower,
        Some(&"high-performance") => wgpu::PowerPreference::HighPerformance,
        Some(_) => unreachable!(),
        None => wgpu::PowerPreference::Default,
      },
      compatible_surface: None, // windowless
    })
    .await
    .unwrap(); // TODO: dont unwrap

  let name = adapter.get_info().name;
  let extensions = serialize_features(&adapter.features());

  let mut state = state.borrow_mut();
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
  rid: u32,
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
  let adapter = state
    .resource_table
    .get_mut::<wgpu::Adapter>(args.rid)
    .ok_or_else(bad_resource_id)?;

  let (device, queue) = adapter
    .request_device(
      &wgpu::DeviceDescriptor {
        features: Default::default(), // TODO
        limits: Default::default(),   // TODO
        shader_validation: false,     // TODO
      },
      None, // debug
    )
    .await
    .unwrap(); // TODO: dont unwrap

  let extensions = serialize_features(&device.features());
  let limits = device.limits();

  let device_rid = state.resource_table.add("webGPUDevice", Box::new(device));

  let queue_rid = state.resource_table.add("webGPUQueue", Box::new(queue));

  Ok(json!({
    "deviceRid": device_rid,
    "queueRid": queue_rid,
    "extensions": extensions,
    "limits", // TODO
  }))
}
