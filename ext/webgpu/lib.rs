// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::not_supported;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpFn;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
pub use wgpu_core;
pub use wgpu_types;

use error::DomExceptionOperationError;
use error::WebGpuResult;

#[macro_use]
mod macros {
  macro_rules! gfx_select {
    ($id:expr => $global:ident.$method:ident( $($param:expr),* )) => {
      match $id.backend() {
        #[cfg(not(target_os = "macos"))]
        wgpu_types::Backend::Vulkan => $global.$method::<wgpu_core::backend::Vulkan>( $($param),* ),
        #[cfg(target_os = "macos")]
        wgpu_types::Backend::Metal => $global.$method::<wgpu_core::backend::Metal>( $($param),* ),
        #[cfg(windows)]
        wgpu_types::Backend::Dx12 => $global.$method::<wgpu_core::backend::Dx12>( $($param),* ),
        #[cfg(windows)]
        wgpu_types::Backend::Dx11 => $global.$method::<wgpu_core::backend::Dx11>( $($param),* ),
        #[cfg(all(unix, not(target_os = "macos")))]
        wgpu_types::Backend::Gl => $global.$method::<wgpu_core::backend::Gl>( $($param),+ ),
        other => panic!("Unexpected backend {:?}", other),
      }
    };
  }

  macro_rules! gfx_put {
    ($id:expr => $global:ident.$method:ident( $($param:expr),* ) => $state:expr, $rc:expr) => {{
      let (val, maybe_err) = gfx_select!($id => $global.$method($($param),*));
      let rid = $state.resource_table.add($rc(val));
      Ok(WebGpuResult::rid_err(rid, maybe_err))
    }};
  }

  macro_rules! gfx_ok {
    ($id:expr => $global:ident.$method:ident( $($param:expr),* )) => {{
      let maybe_err = gfx_select!($id => $global.$method($($param),*)).err();
      Ok(WebGpuResult::maybe_err(maybe_err))
    }};
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

struct WebGpuAdapter(wgpu_core::id::AdapterId);
impl Resource for WebGpuAdapter {
  fn name(&self) -> Cow<str> {
    "webGPUAdapter".into()
  }
}

struct WebGpuDevice(wgpu_core::id::DeviceId);
impl Resource for WebGpuDevice {
  fn name(&self) -> Cow<str> {
    "webGPUDevice".into()
  }
}

struct WebGpuQuerySet(wgpu_core::id::QuerySetId);
impl Resource for WebGpuQuerySet {
  fn name(&self) -> Cow<str> {
    "webGPUQuerySet".into()
  }
}

pub fn init(unstable: bool) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/webgpu",
      "01_webgpu.js",
      "02_idl_types.js",
    ))
    .ops(declare_webgpu_ops())
    .state(move |state| {
      // TODO: check & possibly streamline this
      // Unstable might be able to be OpMiddleware
      // let unstable_checker = state.borrow::<super::UnstableChecker>();
      // let unstable = unstable_checker.unstable;
      state.put(Unstable(unstable));
      Ok(())
    })
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_webgpu.d.ts")
}

fn deserialize_features(features: &wgpu_types::Features) -> Vec<&'static str> {
  let mut return_features: Vec<&'static str> = vec![];

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

#[derive(Serialize)]
#[serde(untagged)]
pub enum GpuAdapterDeviceOrErr {
  Error { err: String },
  Features(GpuAdapterDevice),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAdapterDevice {
  rid: ResourceId,
  name: Option<String>,
  limits: wgpu_types::Limits,
  features: Vec<&'static str>,
  is_software: bool,
}

pub async fn op_webgpu_request_adapter(
  state: Rc<RefCell<OpState>>,
  args: RequestAdapterArgs,
  _: (),
) -> Result<GpuAdapterDeviceOrErr, AnyError> {
  let mut state = state.borrow_mut();
  check_unstable(&state, "navigator.gpu.requestAdapter");
  let instance = if let Some(instance) = state.try_borrow::<Instance>() {
    instance
  } else {
    state.put(wgpu_core::hub::Global::new(
      "webgpu",
      wgpu_core::hub::IdentityManagerFactory,
      wgpu_types::BackendBit::PRIMARY,
    ));
    state.borrow::<Instance>()
  };

  let descriptor = wgpu_core::instance::RequestAdapterOptions {
    power_preference: match args.power_preference {
      Some(power_preference) => match power_preference.as_str() {
        "low-power" => wgpu_types::PowerPreference::LowPower,
        "high-performance" => wgpu_types::PowerPreference::HighPerformance,
        _ => unreachable!(),
      },
      None => Default::default(),
    },
    // TODO(lucacasonato): respect forceSoftware
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
      return Ok(GpuAdapterDeviceOrErr::Error {
        err: err.to_string(),
      })
    }
  };
  let name = gfx_select!(adapter => instance.adapter_get_info(adapter))?.name;
  let adapter_features =
    gfx_select!(adapter => instance.adapter_features(adapter))?;
  let features = deserialize_features(&adapter_features);
  let adapter_limits =
    gfx_select!(adapter => instance.adapter_limits(adapter))?;

  let rid = state.resource_table.add(WebGpuAdapter(adapter));

  Ok(GpuAdapterDeviceOrErr::Features(GpuAdapterDevice {
    rid,
    name: Some(name),
    features,
    limits: adapter_limits,
    is_software: false,
  }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GpuLimits {
  max_texture_dimension_1d: Option<u32>,
  max_texture_dimension_2d: Option<u32>,
  max_texture_dimension_3d: Option<u32>,
  max_texture_array_layers: Option<u32>,
  max_bind_groups: Option<u32>,
  max_dynamic_uniform_buffers_per_pipeline_layout: Option<u32>,
  max_dynamic_storage_buffers_per_pipeline_layout: Option<u32>,
  max_sampled_textures_per_shader_stage: Option<u32>,
  max_samplers_per_shader_stage: Option<u32>,
  max_storage_buffers_per_shader_stage: Option<u32>,
  max_storage_textures_per_shader_stage: Option<u32>,
  max_uniform_buffers_per_shader_stage: Option<u32>,
  max_uniform_buffer_binding_size: Option<u32>,
  max_storage_buffer_binding_size: Option<u32>,
  // min_uniform_buffer_offset_alignment: Option<u32>,
  // min_storage_buffer_offset_alignment: Option<u32>,
  max_vertex_buffers: Option<u32>,
  max_vertex_attributes: Option<u32>,
  max_vertex_buffer_array_stride: Option<u32>,
  // max_inter_stage_shader_components: Option<u32>,
  // max_compute_workgroup_storage_size: Option<u32>,
  // max_compute_workgroup_invocations: Option<u32>,
  // max_compute_per_dimension_dispatch_size: Option<u32>,
}

impl From<GpuLimits> for wgpu_types::Limits {
  fn from(limits: GpuLimits) -> wgpu_types::Limits {
    wgpu_types::Limits {
      max_texture_dimension_1d: limits.max_texture_dimension_1d.unwrap_or(8192),
      max_texture_dimension_2d: limits.max_texture_dimension_2d.unwrap_or(8192),
      max_texture_dimension_3d: limits.max_texture_dimension_3d.unwrap_or(2048),
      max_texture_array_layers: limits.max_texture_array_layers.unwrap_or(2048),
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
      max_storage_buffer_binding_size: limits
        .max_storage_buffer_binding_size
        .unwrap_or(134217728),
      // min_uniform_buffer_offset_alignment: limits
      //   .min_uniform_buffer_offset_alignment
      //   .unwrap_or(default),
      // min_storage_buffer_offset_alignment: limits
      //   .min_storage_buffer_offset_alignment
      //   .unwrap_or(default),
      max_vertex_buffers: limits.max_vertex_buffers.unwrap_or(8),
      max_vertex_attributes: limits.max_vertex_attributes.unwrap_or(16),
      max_vertex_buffer_array_stride: limits
        .max_vertex_buffer_array_stride
        .unwrap_or(2048),
      // max_inter_stage_shader_components: limits
      //   .max_inter_stage_shader_components
      //   .unwrap_or(default),
      // max_compute_workgroup_storage_size: limits
      //   .max_compute_workgroup_storage_size
      //   .unwrap_or(default),
      // max_compute_workgroup_invocations: limits
      //   .max_compute_workgroup_invocations
      //   .unwrap_or(default),
      // max_compute_per_dimension_dispatch_size: limits
      //   .max_compute_per_dimension_dispatch_size
      //   .unwrap_or(default),
      max_push_constant_size: 0,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestDeviceArgs {
  adapter_rid: ResourceId,
  label: Option<String>,
  required_features: Option<Vec<String>>,
  required_limits: Option<GpuLimits>,
}

pub async fn op_webgpu_request_device(
  state: Rc<RefCell<OpState>>,
  args: RequestDeviceArgs,
  _: (),
) -> Result<GpuAdapterDevice, AnyError> {
  let mut state = state.borrow_mut();
  let adapter_resource = state
    .resource_table
    .get::<WebGpuAdapter>(args.adapter_rid)?;
  let adapter = adapter_resource.0;
  let instance = state.borrow::<Instance>();

  let mut features: wgpu_types::Features = wgpu_types::Features::empty();

  if let Some(passed_features) = args.required_features {
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
      .required_limits
      .map_or(wgpu_types::Limits::default(), Into::into),
  };

  let (device, maybe_err) = gfx_select!(adapter => instance.adapter_request_device(
    adapter,
    &descriptor,
    std::env::var("DENO_WEBGPU_TRACE").ok().as_ref().map(std::path::Path::new),
    std::marker::PhantomData
  ));
  if let Some(err) = maybe_err {
    return Err(DomExceptionOperationError::new(&err.to_string()).into());
  }

  let device_features =
    gfx_select!(device => instance.device_features(device))?;
  let features = deserialize_features(&device_features);
  let limits = gfx_select!(device => instance.device_limits(device))?;

  let rid = state.resource_table.add(WebGpuDevice(device));

  Ok(GpuAdapterDevice {
    rid,
    name: None,
    features,
    limits,
    // TODO(lucacasonato): report correctly from wgpu
    is_software: false,
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateQuerySetArgs {
  device_rid: ResourceId,
  _label: Option<String>, // not yet implemented
  #[serde(rename = "type")]
  kind: String,
  count: u32,
  pipeline_statistics: Option<Vec<String>>,
}

pub fn op_webgpu_create_query_set(
  state: &mut OpState,
  args: CreateQuerySetArgs,
  _: (),
) -> Result<WebGpuResult, AnyError> {
  let device_resource =
    state.resource_table.get::<WebGpuDevice>(args.device_rid)?;
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

  gfx_put!(device => instance.device_create_query_set(
    device,
    &descriptor,
    std::marker::PhantomData
  ) => state, WebGpuQuerySet)
}

fn declare_webgpu_ops() -> Vec<(&'static str, Box<OpFn>)> {
  vec![
    // Request device/adapter
    (
      "op_webgpu_request_adapter",
      op_async(op_webgpu_request_adapter),
    ),
    (
      "op_webgpu_request_device",
      op_async(op_webgpu_request_device),
    ),
    // Query Set
    (
      "op_webgpu_create_query_set",
      op_sync(op_webgpu_create_query_set),
    ),
    // buffer
    (
      "op_webgpu_create_buffer",
      op_sync(buffer::op_webgpu_create_buffer),
    ),
    (
      "op_webgpu_buffer_get_mapped_range",
      op_sync(buffer::op_webgpu_buffer_get_mapped_range),
    ),
    (
      "op_webgpu_buffer_unmap",
      op_sync(buffer::op_webgpu_buffer_unmap),
    ),
    // buffer async
    (
      "op_webgpu_buffer_get_map_async",
      op_async(buffer::op_webgpu_buffer_get_map_async),
    ),
    // remaining sync ops

    // texture
    (
      "op_webgpu_create_texture",
      op_sync(texture::op_webgpu_create_texture),
    ),
    (
      "op_webgpu_create_texture_view",
      op_sync(texture::op_webgpu_create_texture_view),
    ),
    // sampler
    (
      "op_webgpu_create_sampler",
      op_sync(sampler::op_webgpu_create_sampler),
    ),
    // binding
    (
      "op_webgpu_create_bind_group_layout",
      op_sync(binding::op_webgpu_create_bind_group_layout),
    ),
    (
      "op_webgpu_create_pipeline_layout",
      op_sync(binding::op_webgpu_create_pipeline_layout),
    ),
    (
      "op_webgpu_create_bind_group",
      op_sync(binding::op_webgpu_create_bind_group),
    ),
    // pipeline
    (
      "op_webgpu_create_compute_pipeline",
      op_sync(pipeline::op_webgpu_create_compute_pipeline),
    ),
    (
      "op_webgpu_compute_pipeline_get_bind_group_layout",
      op_sync(pipeline::op_webgpu_compute_pipeline_get_bind_group_layout),
    ),
    (
      "op_webgpu_create_render_pipeline",
      op_sync(pipeline::op_webgpu_create_render_pipeline),
    ),
    (
      "op_webgpu_render_pipeline_get_bind_group_layout",
      op_sync(pipeline::op_webgpu_render_pipeline_get_bind_group_layout),
    ),
    // command_encoder
    (
      "op_webgpu_create_command_encoder",
      op_sync(command_encoder::op_webgpu_create_command_encoder),
    ),
    (
      "op_webgpu_command_encoder_begin_render_pass",
      op_sync(command_encoder::op_webgpu_command_encoder_begin_render_pass),
    ),
    (
      "op_webgpu_command_encoder_begin_compute_pass",
      op_sync(command_encoder::op_webgpu_command_encoder_begin_compute_pass),
    ),
    (
      "op_webgpu_command_encoder_copy_buffer_to_buffer",
      op_sync(command_encoder::op_webgpu_command_encoder_copy_buffer_to_buffer),
    ),
    (
      "op_webgpu_command_encoder_copy_buffer_to_texture",
      op_sync(
        command_encoder::op_webgpu_command_encoder_copy_buffer_to_texture,
      ),
    ),
    (
      "op_webgpu_command_encoder_copy_texture_to_buffer",
      op_sync(
        command_encoder::op_webgpu_command_encoder_copy_texture_to_buffer,
      ),
    ),
    (
      "op_webgpu_command_encoder_copy_texture_to_texture",
      op_sync(
        command_encoder::op_webgpu_command_encoder_copy_texture_to_texture,
      ),
    ),
    (
      "op_webgpu_command_encoder_push_debug_group",
      op_sync(command_encoder::op_webgpu_command_encoder_push_debug_group),
    ),
    (
      "op_webgpu_command_encoder_pop_debug_group",
      op_sync(command_encoder::op_webgpu_command_encoder_pop_debug_group),
    ),
    (
      "op_webgpu_command_encoder_insert_debug_marker",
      op_sync(command_encoder::op_webgpu_command_encoder_insert_debug_marker),
    ),
    (
      "op_webgpu_command_encoder_write_timestamp",
      op_sync(command_encoder::op_webgpu_command_encoder_write_timestamp),
    ),
    (
      "op_webgpu_command_encoder_resolve_query_set",
      op_sync(command_encoder::op_webgpu_command_encoder_resolve_query_set),
    ),
    (
      "op_webgpu_command_encoder_finish",
      op_sync(command_encoder::op_webgpu_command_encoder_finish),
    ),
    // render_pass
    (
      "op_webgpu_render_pass_set_viewport",
      op_sync(render_pass::op_webgpu_render_pass_set_viewport),
    ),
    (
      "op_webgpu_render_pass_set_scissor_rect",
      op_sync(render_pass::op_webgpu_render_pass_set_scissor_rect),
    ),
    (
      "op_webgpu_render_pass_set_blend_constant",
      op_sync(render_pass::op_webgpu_render_pass_set_blend_constant),
    ),
    (
      "op_webgpu_render_pass_set_stencil_reference",
      op_sync(render_pass::op_webgpu_render_pass_set_stencil_reference),
    ),
    (
      "op_webgpu_render_pass_begin_pipeline_statistics_query",
      op_sync(
        render_pass::op_webgpu_render_pass_begin_pipeline_statistics_query,
      ),
    ),
    (
      "op_webgpu_render_pass_end_pipeline_statistics_query",
      op_sync(render_pass::op_webgpu_render_pass_end_pipeline_statistics_query),
    ),
    (
      "op_webgpu_render_pass_write_timestamp",
      op_sync(render_pass::op_webgpu_render_pass_write_timestamp),
    ),
    (
      "op_webgpu_render_pass_execute_bundles",
      op_sync(render_pass::op_webgpu_render_pass_execute_bundles),
    ),
    (
      "op_webgpu_render_pass_end_pass",
      op_sync(render_pass::op_webgpu_render_pass_end_pass),
    ),
    (
      "op_webgpu_render_pass_set_bind_group",
      op_sync(render_pass::op_webgpu_render_pass_set_bind_group),
    ),
    (
      "op_webgpu_render_pass_push_debug_group",
      op_sync(render_pass::op_webgpu_render_pass_push_debug_group),
    ),
    (
      "op_webgpu_render_pass_pop_debug_group",
      op_sync(render_pass::op_webgpu_render_pass_pop_debug_group),
    ),
    (
      "op_webgpu_render_pass_insert_debug_marker",
      op_sync(render_pass::op_webgpu_render_pass_insert_debug_marker),
    ),
    (
      "op_webgpu_render_pass_set_pipeline",
      op_sync(render_pass::op_webgpu_render_pass_set_pipeline),
    ),
    (
      "op_webgpu_render_pass_set_index_buffer",
      op_sync(render_pass::op_webgpu_render_pass_set_index_buffer),
    ),
    (
      "op_webgpu_render_pass_set_vertex_buffer",
      op_sync(render_pass::op_webgpu_render_pass_set_vertex_buffer),
    ),
    (
      "op_webgpu_render_pass_draw",
      op_sync(render_pass::op_webgpu_render_pass_draw),
    ),
    (
      "op_webgpu_render_pass_draw_indexed",
      op_sync(render_pass::op_webgpu_render_pass_draw_indexed),
    ),
    (
      "op_webgpu_render_pass_draw_indirect",
      op_sync(render_pass::op_webgpu_render_pass_draw_indirect),
    ),
    (
      "op_webgpu_render_pass_draw_indexed_indirect",
      op_sync(render_pass::op_webgpu_render_pass_draw_indexed_indirect),
    ),
    // compute_pass
    (
      "op_webgpu_compute_pass_set_pipeline",
      op_sync(compute_pass::op_webgpu_compute_pass_set_pipeline),
    ),
    (
      "op_webgpu_compute_pass_dispatch",
      op_sync(compute_pass::op_webgpu_compute_pass_dispatch),
    ),
    (
      "op_webgpu_compute_pass_dispatch_indirect",
      op_sync(compute_pass::op_webgpu_compute_pass_dispatch_indirect),
    ),
    (
      "op_webgpu_compute_pass_end_pass",
      op_sync(compute_pass::op_webgpu_compute_pass_end_pass),
    ),
    (
      "op_webgpu_compute_pass_set_bind_group",
      op_sync(compute_pass::op_webgpu_compute_pass_set_bind_group),
    ),
    (
      "op_webgpu_compute_pass_push_debug_group",
      op_sync(compute_pass::op_webgpu_compute_pass_push_debug_group),
    ),
    (
      "op_webgpu_compute_pass_pop_debug_group",
      op_sync(compute_pass::op_webgpu_compute_pass_pop_debug_group),
    ),
    (
      "op_webgpu_compute_pass_insert_debug_marker",
      op_sync(compute_pass::op_webgpu_compute_pass_insert_debug_marker),
    ),
    // bundle
    (
      "op_webgpu_create_render_bundle_encoder",
      op_sync(bundle::op_webgpu_create_render_bundle_encoder),
    ),
    (
      "op_webgpu_render_bundle_encoder_finish",
      op_sync(bundle::op_webgpu_render_bundle_encoder_finish),
    ),
    (
      "op_webgpu_render_bundle_encoder_set_bind_group",
      op_sync(bundle::op_webgpu_render_bundle_encoder_set_bind_group),
    ),
    (
      "op_webgpu_render_bundle_encoder_push_debug_group",
      op_sync(bundle::op_webgpu_render_bundle_encoder_push_debug_group),
    ),
    (
      "op_webgpu_render_bundle_encoder_pop_debug_group",
      op_sync(bundle::op_webgpu_render_bundle_encoder_pop_debug_group),
    ),
    (
      "op_webgpu_render_bundle_encoder_insert_debug_marker",
      op_sync(bundle::op_webgpu_render_bundle_encoder_insert_debug_marker),
    ),
    (
      "op_webgpu_render_bundle_encoder_set_pipeline",
      op_sync(bundle::op_webgpu_render_bundle_encoder_set_pipeline),
    ),
    (
      "op_webgpu_render_bundle_encoder_set_index_buffer",
      op_sync(bundle::op_webgpu_render_bundle_encoder_set_index_buffer),
    ),
    (
      "op_webgpu_render_bundle_encoder_set_vertex_buffer",
      op_sync(bundle::op_webgpu_render_bundle_encoder_set_vertex_buffer),
    ),
    (
      "op_webgpu_render_bundle_encoder_draw",
      op_sync(bundle::op_webgpu_render_bundle_encoder_draw),
    ),
    (
      "op_webgpu_render_bundle_encoder_draw_indexed",
      op_sync(bundle::op_webgpu_render_bundle_encoder_draw_indexed),
    ),
    (
      "op_webgpu_render_bundle_encoder_draw_indirect",
      op_sync(bundle::op_webgpu_render_bundle_encoder_draw_indirect),
    ),
    // queue
    (
      "op_webgpu_queue_submit",
      op_sync(queue::op_webgpu_queue_submit),
    ),
    (
      "op_webgpu_write_buffer",
      op_sync(queue::op_webgpu_write_buffer),
    ),
    (
      "op_webgpu_write_texture",
      op_sync(queue::op_webgpu_write_texture),
    ),
    // shader
    (
      "op_webgpu_create_shader_module",
      op_sync(shader::op_webgpu_create_shader_module),
    ),
  ]
}
