// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use deno_core::{BufVec, Resource};
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
pub use wgpu_core;
pub use wgpu_types;

use error::DOMExceptionOperationError;
use error::WebGPUError;

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
}

pub mod binding;
pub mod buffer;
pub mod bundle;
pub mod command_encoder;
pub mod compute_pass;
pub mod error;
pub mod pipeline;
pub mod queue;
pub mod render_pass;
pub mod sampler;
pub mod shader;
pub mod texture;

pub struct Unstable(pub bool);

fn check_unstable(state: &OpState, api_name: &str) {
  let unstable = state.borrow::<Unstable>();

  if !unstable.0 {
    eprintln!(
      "Unstable API '{}'. The --unstable flag must be provided.",
      api_name
    );
    std::process::exit(70);
  }
}

type Instance = wgpu_core::hub::Global<wgpu_core::hub::IdentityManagerFactory>;

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
  let files = vec![
    (
      "deno:op_crates/webgpu/01_webgpu.js",
      include_str!("01_webgpu.js"),
    ),
    (
      "deno:op_crates/webgpu/02_idl_types.js",
      include_str!("02_idl_types.js"),
    ),
  ];
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

  // extended from spec
  if features.contains(wgpu_types::Features::MAPPABLE_PRIMARY_BUFFERS) {
    return_features.push("mappable-primary-buffers");
  }
  if features.contains(wgpu_types::Features::SAMPLED_TEXTURE_BINDING_ARRAY) {
    return_features.push("sampled-texture-binding-array");
  }
  if features
    .contains(wgpu_types::Features::SAMPLED_TEXTURE_ARRAY_DYNAMIC_INDEXING)
  {
    return_features.push("sampled-texture-array-dynamic-indexing");
  }
  if features
    .contains(wgpu_types::Features::SAMPLED_TEXTURE_ARRAY_NON_UNIFORM_INDEXING)
  {
    return_features.push("sampled-texture-array-non-uniform-indexing");
  }
  if features.contains(wgpu_types::Features::UNSIZED_BINDING_ARRAY) {
    return_features.push("unsized-binding-array");
  }
  if features.contains(wgpu_types::Features::MULTI_DRAW_INDIRECT) {
    return_features.push("multi-draw-indirect");
  }
  if features.contains(wgpu_types::Features::MULTI_DRAW_INDIRECT_COUNT) {
    return_features.push("multi-draw-indirect-count");
  }
  if features.contains(wgpu_types::Features::PUSH_CONSTANTS) {
    return_features.push("push-constants");
  }
  if features.contains(wgpu_types::Features::ADDRESS_MODE_CLAMP_TO_BORDER) {
    return_features.push("address-mode-clamp-to-border");
  }
  if features.contains(wgpu_types::Features::NON_FILL_POLYGON_MODE) {
    return_features.push("non-fill-polygon-mode");
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_ETC2) {
    return_features.push("texture-compression-etc2");
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_ASTC_LDR) {
    return_features.push("texture-compression-astc-ldr");
  }
  if features
    .contains(wgpu_types::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
  {
    return_features.push("texture-adapter-specific-format-features");
  }
  if features.contains(wgpu_types::Features::SHADER_FLOAT64) {
    return_features.push("shader-float64");
  }
  if features.contains(wgpu_types::Features::VERTEX_ATTRIBUTE_64BIT) {
    return_features.push("vertex-attribute-64bit");
  }

  return_features
}

pub fn op_webgpu_create_instance(
  state: &mut OpState,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  check_unstable(&state, "navigator.gpu");

  state.put(wgpu_core::hub::Global::new(
    "webgpu",
    wgpu_core::hub::IdentityManagerFactory,
    wgpu_types::BackendBit::PRIMARY,
  ));

  Ok(json!({}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestAdapterArgs {
  power_preference: Option<String>,
}

pub async fn op_webgpu_request_adapter(
  state: Rc<RefCell<OpState>>,
  args: RequestAdapterArgs,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let mut state = state.borrow_mut();
  let instance = state.borrow::<Instance>();

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
  let res = instance.request_adapter(
    &descriptor,
    wgpu_core::instance::AdapterInputs::Mask(
      wgpu_types::BackendBit::PRIMARY,
      |_| std::marker::PhantomData,
    ),
  );

  let adapter = match res {
    Ok(adapter) => adapter,
    Err(err) => {
      return Ok(json!({
        "err": err.to_string()
      }))
    }
  };
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
pub struct RequestDeviceArgs {
  adapter_rid: u32,
  label: Option<String>,
  non_guaranteed_features: Option<Vec<String>>,
  non_guaranteed_limits: Option<GPULimits>,
}

pub async fn op_webgpu_request_device(
  state: Rc<RefCell<OpState>>,
  args: RequestDeviceArgs,
  _bufs: BufVec,
) -> Result<Value, AnyError> {
  let mut state = state.borrow_mut();
  let adapter_resource = state
    .resource_table
    .get::<WebGPUAdapter>(args.adapter_rid)
    .ok_or_else(bad_resource_id)?;
  let adapter = adapter_resource.0;
  let instance = state.borrow::<Instance>();

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

    // extended from spec
    if passed_features.contains(&"mappable-primary-buffers".to_string()) {
      features.set(wgpu_types::Features::MAPPABLE_PRIMARY_BUFFERS, true);
    }
    if passed_features.contains(&"sampled-texture-binding-array".to_string()) {
      features.set(wgpu_types::Features::SAMPLED_TEXTURE_BINDING_ARRAY, true);
    }
    if passed_features
      .contains(&"sampled-texture-array-dynamic-indexing".to_string())
    {
      features.set(
        wgpu_types::Features::SAMPLED_TEXTURE_ARRAY_DYNAMIC_INDEXING,
        true,
      );
    }
    if passed_features
      .contains(&"sampled-texture-array-non-uniform-indexing".to_string())
    {
      features.set(
        wgpu_types::Features::SAMPLED_TEXTURE_ARRAY_NON_UNIFORM_INDEXING,
        true,
      );
    }
    if passed_features.contains(&"unsized-binding-array".to_string()) {
      features.set(wgpu_types::Features::UNSIZED_BINDING_ARRAY, true);
    }
    if passed_features.contains(&"multi-draw-indirect".to_string()) {
      features.set(wgpu_types::Features::MULTI_DRAW_INDIRECT, true);
    }
    if passed_features.contains(&"multi-draw-indirect-count".to_string()) {
      features.set(wgpu_types::Features::MULTI_DRAW_INDIRECT_COUNT, true);
    }
    if passed_features.contains(&"push-constants".to_string()) {
      features.set(wgpu_types::Features::PUSH_CONSTANTS, true);
    }
    if passed_features.contains(&"address-mode-clamp-to-border".to_string()) {
      features.set(wgpu_types::Features::ADDRESS_MODE_CLAMP_TO_BORDER, true);
    }
    if passed_features.contains(&"non-fill-polygon-mode".to_string()) {
      features.set(wgpu_types::Features::NON_FILL_POLYGON_MODE, true);
    }
    if passed_features.contains(&"texture-compression-etc2".to_string()) {
      features.set(wgpu_types::Features::TEXTURE_COMPRESSION_ETC2, true);
    }
    if passed_features.contains(&"texture-compression-astc-ldr".to_string()) {
      features.set(wgpu_types::Features::TEXTURE_COMPRESSION_ASTC_LDR, true);
    }
    if passed_features
      .contains(&"texture-adapter-specific-format-features".to_string())
    {
      features.set(
        wgpu_types::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
        true,
      );
    }
    if passed_features.contains(&"shader-float64".to_string()) {
      features.set(wgpu_types::Features::SHADER_FLOAT64, true);
    }
    if passed_features.contains(&"vertex-attribute-64bit".to_string()) {
      features.set(wgpu_types::Features::VERTEX_ATTRIBUTE_64BIT, true);
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

  let (device, maybe_err) = gfx_select!(adapter => instance.adapter_request_device(
    adapter,
    &descriptor,
    std::env::var("DENO_WEBGPU_TRACE").ok().as_ref().map(std::path::Path::new),
    std::marker::PhantomData
  ));
  if let Some(err) = maybe_err {
    return Err(DOMExceptionOperationError::new(&err.to_string()).into());
  }

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
pub struct CreateQuerySetArgs {
  device_rid: u32,
  _label: Option<String>, // not yet implemented
  #[serde(rename = "type")]
  kind: String,
  count: u32,
  pipeline_statistics: Option<Vec<String>>,
}

pub fn op_webgpu_create_query_set(
  state: &mut OpState,
  args: CreateQuerySetArgs,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let device_resource = state
    .resource_table
    .get::<WebGPUDevice>(args.device_rid)
    .ok_or_else(bad_resource_id)?;
  let device = device_resource.0;
  let instance = &state.borrow::<Instance>();

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

  let (query_set, maybe_err) = gfx_select!(device => instance.device_create_query_set(
    device,
    &descriptor,
    std::marker::PhantomData
  ));

  let rid = state.resource_table.add(WebGPUQuerySet(query_set));

  Ok(json!({
    "rid": rid,
    "err": maybe_err.map(WebGPUError::from),
  }))
}
