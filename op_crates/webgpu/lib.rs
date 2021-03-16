// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

#![deny(warnings)]

use deno_core::declare_ops;
use deno_core::declare_ops_group;
use deno_core::error::AnyError;
use deno_core::error::{bad_resource_id, not_supported};
use deno_core::include_js_files;
use deno_core::json_op_async;
use deno_core::json_op_sync;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::OpFn;
use deno_core::OpState;
use deno_core::BasicModule;
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

pub fn init(unstable: bool) -> BasicModule {
  BasicModule::with_ops(
    include_js_files!(
      root "deno:op_crates/webgpu",
      "01_webgpu.js",
      "02_idl_types.js",
    ),
    declare_webgpu_ops(),
    Some(Box::new(move |state| {
      state.put(wgpu_core::hub::Global::new(
        "webgpu",
        wgpu_core::hub::IdentityManagerFactory,
        wgpu_types::BackendBit::PRIMARY,
      ));
      // TODO: check & possibly streamline this
      // Unstable might be able to be OpMiddleware
      // let unstable_checker = state.borrow::<super::UnstableChecker>();
      // let unstable = unstable_checker.unstable;
      state.put(Unstable(unstable));
      Ok(())
    })),
  )
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
  check_unstable(&state, "navigator.gpu.requestAdapter");
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

fn declare_webgpu_ops() -> Vec<(&'static str, Box<OpFn>)> {
  declare_ops_group(vec![
    // Request device/adapter
    declare_ops!(
      with(json_op_async),
      op_webgpu_request_adapter,
      op_webgpu_request_device,
    ),
    // Query Set
    declare_ops!(with(json_op_sync), op_webgpu_create_query_set,),
    // buffer
    declare_ops!(
      with(json_op_sync),
      buffer::op_webgpu_create_buffer,
      buffer::op_webgpu_buffer_get_mapped_range,
      buffer::op_webgpu_buffer_unmap,
    ),
    // buffer async
    declare_ops!(with(json_op_async), buffer::op_webgpu_buffer_get_map_async,),
    // remaining sync ops
    declare_ops!(
      with(json_op_sync),
      // texture
      texture::op_webgpu_create_texture,
      texture::op_webgpu_create_texture_view,
      // sampler
      sampler::op_webgpu_create_sampler,
      // binding
      binding::op_webgpu_create_bind_group_layout,
      binding::op_webgpu_create_pipeline_layout,
      binding::op_webgpu_create_bind_group,
      // pipeline
      pipeline::op_webgpu_create_compute_pipeline,
      pipeline::op_webgpu_compute_pipeline_get_bind_group_layout,
      pipeline::op_webgpu_create_render_pipeline,
      pipeline::op_webgpu_render_pipeline_get_bind_group_layout,
      // command_encoder
      command_encoder::op_webgpu_create_command_encoder,
      command_encoder::op_webgpu_command_encoder_begin_render_pass,
      command_encoder::op_webgpu_command_encoder_begin_compute_pass,
      command_encoder::op_webgpu_command_encoder_copy_buffer_to_buffer,
      command_encoder::op_webgpu_command_encoder_copy_buffer_to_texture,
      command_encoder::op_webgpu_command_encoder_copy_texture_to_buffer,
      command_encoder::op_webgpu_command_encoder_copy_texture_to_texture,
      command_encoder::op_webgpu_command_encoder_push_debug_group,
      command_encoder::op_webgpu_command_encoder_pop_debug_group,
      command_encoder::op_webgpu_command_encoder_insert_debug_marker,
      command_encoder::op_webgpu_command_encoder_write_timestamp,
      command_encoder::op_webgpu_command_encoder_resolve_query_set,
      command_encoder::op_webgpu_command_encoder_finish,
      // render_pass
      render_pass::op_webgpu_render_pass_set_viewport,
      render_pass::op_webgpu_render_pass_set_scissor_rect,
      render_pass::op_webgpu_render_pass_set_blend_color,
      render_pass::op_webgpu_render_pass_set_stencil_reference,
      render_pass::op_webgpu_render_pass_begin_pipeline_statistics_query,
      render_pass::op_webgpu_render_pass_end_pipeline_statistics_query,
      render_pass::op_webgpu_render_pass_write_timestamp,
      render_pass::op_webgpu_render_pass_execute_bundles,
      render_pass::op_webgpu_render_pass_end_pass,
      render_pass::op_webgpu_render_pass_set_bind_group,
      render_pass::op_webgpu_render_pass_push_debug_group,
      render_pass::op_webgpu_render_pass_pop_debug_group,
      render_pass::op_webgpu_render_pass_insert_debug_marker,
      render_pass::op_webgpu_render_pass_set_pipeline,
      render_pass::op_webgpu_render_pass_set_index_buffer,
      render_pass::op_webgpu_render_pass_set_vertex_buffer,
      render_pass::op_webgpu_render_pass_draw,
      render_pass::op_webgpu_render_pass_draw_indexed,
      render_pass::op_webgpu_render_pass_draw_indirect,
      render_pass::op_webgpu_render_pass_draw_indexed_indirect,
      // compute_pass
      compute_pass::op_webgpu_compute_pass_set_pipeline,
      compute_pass::op_webgpu_compute_pass_dispatch,
      compute_pass::op_webgpu_compute_pass_dispatch_indirect,
      compute_pass::op_webgpu_compute_pass_end_pass,
      compute_pass::op_webgpu_compute_pass_set_bind_group,
      compute_pass::op_webgpu_compute_pass_push_debug_group,
      compute_pass::op_webgpu_compute_pass_pop_debug_group,
      compute_pass::op_webgpu_compute_pass_insert_debug_marker,
      // bundle
      bundle::op_webgpu_create_render_bundle_encoder,
      bundle::op_webgpu_render_bundle_encoder_finish,
      bundle::op_webgpu_render_bundle_encoder_set_bind_group,
      bundle::op_webgpu_render_bundle_encoder_push_debug_group,
      bundle::op_webgpu_render_bundle_encoder_pop_debug_group,
      bundle::op_webgpu_render_bundle_encoder_insert_debug_marker,
      bundle::op_webgpu_render_bundle_encoder_set_pipeline,
      bundle::op_webgpu_render_bundle_encoder_set_index_buffer,
      bundle::op_webgpu_render_bundle_encoder_set_vertex_buffer,
      bundle::op_webgpu_render_bundle_encoder_draw,
      bundle::op_webgpu_render_bundle_encoder_draw_indexed,
      bundle::op_webgpu_render_bundle_encoder_draw_indirect,
      // queue
      queue::op_webgpu_queue_submit,
      queue::op_webgpu_write_buffer,
      queue::op_webgpu_write_texture,
      // shader
      shader::op_webgpu_create_shader_module,
    ),
  ])
}
