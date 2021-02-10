// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::{serde_json, ZeroCopyBuf};
use deno_core::{BufVec, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[macro_use]
mod macros {
  macro_rules! gfx_select {
    ($id:expr => $global:ident.$method:ident( $($param:expr),* )) => {
      match $id.backend() {
        #[cfg(all(not(target_arch = "wasm32"), not(any(target_os = "ios", target_os = "macos"))))]
        wgpu_types::Backend::Vulkan => $global.$method::<wgpu_core::backend::Vulkan>( $($param),* ),
        #[cfg(all(not(target_arch = "wasm32"), any(target_os = "ios", target_os = "macos")))]
        wgpu_types::Backend::Metal => $global.$method::<wgpu_core::backend::Metal>( $($param),* ),
        #[cfg(all(not(target_arch = "wasm32"), windows))]
        wgpu_types::Backend::Dx12 => $global.$method::<wgpu_core::backend::Dx12>( $($param),* ),
        #[cfg(all(not(target_arch = "wasm32"), windows))]
        wgpu_types::Backend::Dx11 => $global.$method::<wgpu_core::backend::Dx11>( $($param),* ),
        #[cfg(any(target_arch = "wasm32", all(unix, not(any(target_os = "ios", target_os = "macos")))))]
        wgpu_types::Backend::Gl => $global.$method::<wgpu_core::backend::Gl>( $($param),+ ),
        other => panic!("Unexpected backend {:?}", other),
      }
    };
  }

  macro_rules! gfx_select_err {
    ($id:expr => $global:ident.$method:ident( $($param:expr),* )) => {
      {
        let (data, err) = gfx_select!($id => $global.$method( $($param),* ));
        if let Some(err) = err {
          Err(err)
        } else {
          Ok(data)
        }
      }
    };
  }
}

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

struct WebGPUInstance(
  wgpu_core::hub::Global<wgpu_core::hub::IdentityManagerFactory>,
);
impl Resource for WebGPUInstance {
  fn name(&self) -> Cow<str> {
    "webGPUInstance".into()
  }
}

struct WebGPUAdapter(wgpu_core::id::AdapterId);
impl Resource for WebGPUAdapter {
  fn name(&self) -> Cow<str> {
    "webGPUAdapter".into()
  }
}

struct WebGPUDevice(wgpu_core::id::DeviceId);
impl Resource for WebGPUDevice {
  fn name(&self) -> Cow<str> {
    "webGPUDevice".into()
  }
}

struct WebGPUQuerySet(wgpu_core::id::QuerySetId);
impl Resource for WebGPUQuerySet {
  fn name(&self) -> Cow<str> {
    "webGPUQuerySet".into()
  }
}

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

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_webgpu.d.ts")
}

fn deserialize_features(features: &wgpu_types::Features) -> Vec<&str> {
  let mut return_features: Vec<&str> = vec![];

  if features.contains(wgpu_types::Features::DEPTH_CLAMPING) {
    return_features.push("depth-clamping");
  }
  if features.contains(wgpu_types::Features::PIPELINE_STATISTICS_QUERY) {
    return_features.push("pipeline-statistics-query");
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_BC) {
    return_features.push("texture-compression-bc");
  }
  if features.contains(wgpu_types::Features::TIMESTAMP_QUERY) {
    return_features.push("timestamp-query");
  }

  return_features
}

pub fn op_webgpu_create_instance(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let instance = wgpu_core::hub::Global::new(
    "webgpu",
    wgpu_core::hub::IdentityManagerFactory,
    wgpu_types::BackendBit::PRIMARY,
  );

  let rid = state.resource_table.add(WebGPUInstance(instance));

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
  let instance_resource = state
    .resource_table
    .get::<WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let descriptor = wgpu_core::instance::RequestAdapterOptions {
    power_preference: match args.power_preference {
      Some(power_preference) => match power_preference.as_str() {
        "low-power" => wgpu_types::PowerPreference::LowPower,
        "high-performance" => wgpu_types::PowerPreference::HighPerformance,
        _ => unreachable!(),
      },
      None => Default::default(),
    },
    compatible_surface: None, // windowless
  };
  let adapter = instance.request_adapter(
    &descriptor,
    wgpu_core::instance::AdapterInputs::Mask(
      wgpu_types::BackendBit::PRIMARY,
      |_| std::marker::PhantomData,
    ),
  )?;

  let name = gfx_select!(adapter => instance.adapter_get_info(adapter))?.name;
  let adapter_features =
    gfx_select!(adapter => instance.adapter_features(adapter))?;
  let features = deserialize_features(&adapter_features);
  let adapter_limits =
    gfx_select!(adapter => instance.adapter_limits(adapter))?;

  let limits = json!({
    "maxBindGroups": adapter_limits.max_bind_groups,
    "maxDynamicUniformBuffersPerPipelineLayout": adapter_limits.max_dynamic_uniform_buffers_per_pipeline_layout,
    "maxDynamicStorageBuffersPerPipelineLayout": adapter_limits.max_dynamic_storage_buffers_per_pipeline_layout,
    "maxSampledTexturesPerShaderStage": adapter_limits.max_sampled_textures_per_shader_stage,
    "maxSamplersPerShaderStage": adapter_limits.max_samplers_per_shader_stage,
    "maxStorageBuffersPerShaderStage": adapter_limits.max_storage_buffers_per_shader_stage,
    "maxStorageTexturesPerShaderStage": adapter_limits.max_storage_textures_per_shader_stage,
    "maxUniformBuffersPerShaderStage": adapter_limits.max_uniform_buffers_per_shader_stage,
    "maxUniformBufferBindingSize": adapter_limits.max_uniform_buffer_binding_size
  });

  let rid = state.resource_table.add(WebGPUAdapter(adapter));

  Ok(json!({
    "rid": rid,
    "name": name,
    "features": features,
    "limits": limits
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GPULimits {
  _max_texture_dimension1d: Option<u32>,
  _max_texture_dimension2d: Option<u32>,
  _max_texture_dimension3d: Option<u32>,
  _max_texture_array_layers: Option<u32>,
  max_bind_groups: Option<u32>,
  max_dynamic_uniform_buffers_per_pipeline_layout: Option<u32>,
  max_dynamic_storage_buffers_per_pipeline_layout: Option<u32>,
  max_sampled_textures_per_shader_stage: Option<u32>,
  max_samplers_per_shader_stage: Option<u32>,
  max_storage_buffers_per_shader_stage: Option<u32>,
  max_storage_textures_per_shader_stage: Option<u32>,
  max_uniform_buffers_per_shader_stage: Option<u32>,
  max_uniform_buffer_binding_size: Option<u32>,
  _max_storage_buffer_binding_size: Option<u32>,
  _max_vertex_buffers: Option<u32>,
  _max_vertex_attributes: Option<u32>,
  _max_vertex_buffer_array_stride: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestDeviceArgs {
  instance_rid: u32,
  adapter_rid: u32,
  label: Option<String>,
  non_guaranteed_features: Option<Vec<String>>,
  non_guaranteed_limits: Option<GPULimits>,
}

pub async fn op_webgpu_request_device(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let args: RequestDeviceArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  let adapter_resource = state
    .resource_table
    .get::<WebGPUAdapter>(args.adapter_rid)
    .ok_or_else(bad_resource_id)?;
  let adapter = adapter_resource.0;
  let instance_resource = state
    .resource_table
    .get::<WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let mut features: wgpu_types::Features = wgpu_types::Features::empty();

  if let Some(passed_features) = args.non_guaranteed_features {
    if passed_features.contains(&"depth-clamping".to_string()) {
      features.set(wgpu_types::Features::DEPTH_CLAMPING, true);
    }
    if passed_features.contains(&"pipeline-statistics-query".to_string()) {
      features.set(wgpu_types::Features::PIPELINE_STATISTICS_QUERY, true);
    }
    if passed_features.contains(&"texture-compression-bc".to_string()) {
      features.set(wgpu_types::Features::TEXTURE_COMPRESSION_BC, true);
    }
    if passed_features.contains(&"timestamp-query".to_string()) {
      features.set(wgpu_types::Features::TIMESTAMP_QUERY, true);
    }
  }

  let descriptor = wgpu_types::DeviceDescriptor {
    label: args.label.map(Cow::from),
    features,
    limits: args
      .non_guaranteed_limits
      .map_or(Default::default(), |limits| wgpu_types::Limits {
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
  };

  let device = gfx_select_err!(adapter => instance.adapter_request_device(
    adapter,
    &descriptor,
    Some(std::path::Path::new("./trace")), // TODO: remove
    std::marker::PhantomData
  ))?;

  let device_features =
    gfx_select!(device => instance.device_features(device))?;
  let features = deserialize_features(&device_features);
  let limits = gfx_select!(device => instance.device_limits(device))?;
  let json_limits = json!({
     "maxBindGroups": limits.max_bind_groups,
     "maxDynamicUniformBuffersPerPipelineLayout": limits.max_dynamic_uniform_buffers_per_pipeline_layout,
     "maxDynamicStorageBuffersPerPipelineLayout": limits.max_dynamic_storage_buffers_per_pipeline_layout,
     "maxSampledTexturesPerShaderStage": limits.max_sampled_textures_per_shader_stage,
     "maxSamplersPerShaderStage": limits.max_samplers_per_shader_stage,
     "maxStorageBuffersPerShaderStage": limits.max_storage_buffers_per_shader_stage,
     "maxStorageTexturesPerShaderStage": limits.max_storage_textures_per_shader_stage,
     "maxUniformBuffersPerShaderStage": limits.max_uniform_buffers_per_shader_stage,
     "maxUniformBufferBindingSize": limits.max_uniform_buffer_binding_size,
  });

  let rid = state.resource_table.add(WebGPUDevice(device));

  Ok(json!({
    "rid": rid,
    "features": features,
    "limits": json_limits,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateQuerySetArgs {
  instance_rid: u32,
  device_rid: u32,
  _label: Option<String>, // not yet implemented
  #[serde(rename = "type")]
  kind: String,
  count: u32,
  pipeline_statistics: Option<Vec<String>>,
}

pub fn op_webgpu_create_query_set(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: CreateQuerySetArgs = serde_json::from_value(args)?;

  let device_resource = state
    .resource_table
    .get::<WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance_resource = state
    .resource_table
    .get::<WebGPUInstance>(args.instance_rid)
    .ok_or_else(bad_resource_id)?;
  let instance = &instance_resource.0;

  let descriptor = wgpu_types::QuerySetDescriptor {
    ty: match args.kind.as_str() {
      "pipeline-statistics" => {
        let mut pipeline_statistics_names =
          wgpu_types::PipelineStatisticsTypes::empty();

        if let Some(pipeline_statistics) = args.pipeline_statistics {
          if pipeline_statistics
            .contains(&"vertex-shader-invocations".to_string())
          {
            pipeline_statistics_names.set(
              wgpu_types::PipelineStatisticsTypes::VERTEX_SHADER_INVOCATIONS,
              true,
            );
          }
          if pipeline_statistics.contains(&"clipper-invocations".to_string()) {
            pipeline_statistics_names.set(
              wgpu_types::PipelineStatisticsTypes::CLIPPER_INVOCATIONS,
              true,
            );
          }
          if pipeline_statistics.contains(&"clipper-primitives-out".to_string())
          {
            pipeline_statistics_names.set(
              wgpu_types::PipelineStatisticsTypes::CLIPPER_PRIMITIVES_OUT,
              true,
            );
          }
          if pipeline_statistics
            .contains(&"fragment-shader-invocations".to_string())
          {
            pipeline_statistics_names.set(
              wgpu_types::PipelineStatisticsTypes::FRAGMENT_SHADER_INVOCATIONS,
              true,
            );
          }
          if pipeline_statistics
            .contains(&"compute-shader-invocations".to_string())
          {
            pipeline_statistics_names.set(
              wgpu_types::PipelineStatisticsTypes::COMPUTE_SHADER_INVOCATIONS,
              true,
            );
          }
        };

        wgpu_types::QueryType::PipelineStatistics(pipeline_statistics_names)
      }
      "occlusion" => return Err(not_supported()),
      "timestamp" => wgpu_types::QueryType::Timestamp,
      _ => unreachable!(),
    },
    count: args.count,
  };

  let query_set = gfx_select_err!(device => instance.device_create_query_set(
    device,
    &descriptor,
    std::marker::PhantomData
  ))?;

  let rid = state.resource_table.add(WebGPUQuerySet(query_set));

  Ok(json!({
    "rid": rid,
  }))
}
