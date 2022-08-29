// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashSet;
use std::convert::TryFrom;
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
        wgpu_types::Backend::Vulkan => $global.$method::<wgpu_core::api::Vulkan>( $($param),* ),
        #[cfg(target_os = "macos")]
        wgpu_types::Backend::Metal => $global.$method::<wgpu_core::api::Metal>( $($param),* ),
        #[cfg(windows)]
        wgpu_types::Backend::Dx12 => $global.$method::<wgpu_core::api::Dx12>( $($param),* ),
        #[cfg(all(unix, not(target_os = "macos")))]
        wgpu_types::Backend::Gl => $global.$method::<wgpu_core::api::Gles>( $($param),+ ),
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

fn deserialize_features(features: &wgpu_types::Features) -> Vec<&'static str> {
  let mut return_features: Vec<&'static str> = vec![];

  if features.contains(wgpu_types::Features::DEPTH_CLIP_CONTROL) {
    return_features.push("depth-clip-control");
  }
  if features.contains(wgpu_types::Features::DEPTH24UNORM_STENCIL8) {
    return_features.push("depth24unorm-stencil8");
  }
  if features.contains(wgpu_types::Features::DEPTH32FLOAT_STENCIL8) {
    return_features.push("depth32float-stencil8");
  }
  if features.contains(wgpu_types::Features::PIPELINE_STATISTICS_QUERY) {
    return_features.push("pipeline-statistics-query");
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_BC) {
    return_features.push("texture-compression-bc");
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_ETC2) {
    return_features.push("texture-compression-etc2");
  }
  if features.contains(wgpu_types::Features::TEXTURE_COMPRESSION_ASTC_LDR) {
    return_features.push("texture-compression-astc");
  }
  if features.contains(wgpu_types::Features::TIMESTAMP_QUERY) {
    return_features.push("timestamp-query");
  }
  if features.contains(wgpu_types::Features::INDIRECT_FIRST_INSTANCE) {
    return_features.push("indirect-first-instance");
  }
  if features.contains(wgpu_types::Features::SHADER_FLOAT16) {
    return_features.push("shader-f16")
  }

  // extended from spec
  if features.contains(wgpu_types::Features::MAPPABLE_PRIMARY_BUFFERS) {
    return_features.push("mappable-primary-buffers");
  }
  if features.contains(wgpu_types::Features::TEXTURE_BINDING_ARRAY) {
    return_features.push("texture-binding-array");
  }
  if features.contains(wgpu_types::Features::BUFFER_BINDING_ARRAY) {
    return_features.push("buffer-binding-array");
  }
  if features.contains(wgpu_types::Features::STORAGE_RESOURCE_BINDING_ARRAY) {
    return_features.push("storage-resource-binding-array");
  }
  if features.contains(
        wgpu_types::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
    ) {
        return_features.push("sampled-texture-and-storage-buffer-array-non-uniform-indexing");
    }
  if features.contains(
        wgpu_types::Features::UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING,
    ) {
        return_features.push("uniform-buffer-and-storage-buffer-texture-non-uniform-indexing");
    }
  if features.contains(wgpu_types::Features::ADDRESS_MODE_CLAMP_TO_BORDER) {
    return_features.push("address-mode-clamp-to-border");
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
  if features.contains(wgpu_types::Features::VERTEX_WRITABLE_STORAGE) {
    return_features.push("vertex-writable-storage");
  }
  if features.contains(wgpu_types::Features::CLEAR_TEXTURE) {
    return_features.push("clear-texture");
  }
  if features.contains(wgpu_types::Features::SHADER_PRIMITIVE_INDEX) {
    return_features.push("shader-primitive-index");
  }
  if features.contains(wgpu_types::Features::PARTIALLY_BOUND_BINDING_ARRAY) {
    return_features.push("shader-primitive-index");
  }

  return_features
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
  limits: wgpu_types::Limits,
  features: Vec<&'static str>,
  is_software: bool,
}

#[op]
pub async fn op_webgpu_request_adapter(
  state: Rc<RefCell<OpState>>,
  power_preference: Option<wgpu_types::PowerPreference>,
  force_fallback_adapter: bool,
) -> Result<GpuAdapterDeviceOrErr, AnyError> {
  let mut state = state.borrow_mut();
  check_unstable(&state, "navigator.gpu.requestAdapter");
  let backends = std::env::var("DENO_WEBGPU_BACKEND").map_or_else(
    |_| wgpu_types::Backends::all(),
    |s| wgpu_core::instance::parse_backends_from_comma_list(&s),
  );
  let instance = if let Some(instance) = state.try_borrow::<Instance>() {
    instance
  } else {
    state.put(wgpu_core::hub::Global::new(
      "webgpu",
      wgpu_core::hub::IdentityManagerFactory,
      backends,
    ));
    state.borrow::<Instance>()
  };

  let descriptor = wgpu_core::instance::RequestAdapterOptions {
    power_preference: power_preference.unwrap_or_default(),
    force_fallback_adapter,
    compatible_surface: None, // windowless
  };
  let res = instance.request_adapter(
    &descriptor,
    wgpu_core::instance::AdapterInputs::Mask(backends, |_| {
      std::marker::PhantomData
    }),
  );

  let adapter = match res {
    Ok(adapter) => adapter,
    Err(err) => {
      return Ok(GpuAdapterDeviceOrErr::Error {
        err: err.to_string(),
      })
    }
  };
  let adapter_features =
    gfx_select!(adapter => instance.adapter_features(adapter))?;
  let features = deserialize_features(&adapter_features);
  let adapter_limits =
    gfx_select!(adapter => instance.adapter_limits(adapter))?;

  let rid = state.resource_table.add(WebGpuAdapter(adapter));

  Ok(GpuAdapterDeviceOrErr::Features(GpuAdapterDevice {
    rid,
    features,
    limits: adapter_limits,
    is_software: false,
  }))
}

#[derive(Deserialize)]
pub struct GpuRequiredFeatures(HashSet<String>);

impl From<GpuRequiredFeatures> for wgpu_types::Features {
  fn from(required_features: GpuRequiredFeatures) -> wgpu_types::Features {
    let mut features: wgpu_types::Features = wgpu_types::Features::empty();
    features.set(
      wgpu_types::Features::DEPTH_CLIP_CONTROL,
      required_features.0.contains("depth-clip-control"),
    );
    features.set(
      wgpu_types::Features::DEPTH24UNORM_STENCIL8,
      required_features.0.contains("depth24unorm-stencil8"),
    );
    features.set(
      wgpu_types::Features::DEPTH32FLOAT_STENCIL8,
      required_features.0.contains("depth32float-stencil8"),
    );
    features.set(
      wgpu_types::Features::PIPELINE_STATISTICS_QUERY,
      required_features.0.contains("pipeline-statistics-query"),
    );
    features.set(
      wgpu_types::Features::TEXTURE_COMPRESSION_BC,
      required_features.0.contains("texture-compression-bc"),
    );
    features.set(
      wgpu_types::Features::TEXTURE_COMPRESSION_ETC2,
      required_features.0.contains("texture-compression-etc2"),
    );
    features.set(
      wgpu_types::Features::TEXTURE_COMPRESSION_ASTC_LDR,
      required_features.0.contains("texture-compression-astc"),
    );
    features.set(
      wgpu_types::Features::TIMESTAMP_QUERY,
      required_features.0.contains("timestamp-query"),
    );
    features.set(
      wgpu_types::Features::INDIRECT_FIRST_INSTANCE,
      required_features.0.contains("indirect-first-instance"),
    );
    features.set(
      wgpu_types::Features::SHADER_FLOAT16,
      required_features.0.contains("shader-f16"),
    );

    // extended from spec
    features.set(
      wgpu_types::Features::MAPPABLE_PRIMARY_BUFFERS,
      required_features.0.contains("mappable-primary-buffers"),
    );
    features.set(
      wgpu_types::Features::TEXTURE_BINDING_ARRAY,
      required_features.0.contains("texture-binding-array"),
    );
    features.set(
      wgpu_types::Features::BUFFER_BINDING_ARRAY,
      required_features.0.contains("buffer-binding-array"),
    );
    features.set(
      wgpu_types::Features::STORAGE_RESOURCE_BINDING_ARRAY,
      required_features
        .0
        .contains("storage-resource-binding-array"),
    );
    features.set(
            wgpu_types::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
            required_features
                .0
                .contains("sampled-texture-and-storage-buffer-array-non-uniform-indexing"),
        );
    features.set(
            wgpu_types::Features::UNIFORM_BUFFER_AND_STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING,
            required_features
                .0
                .contains("uniform-buffer-and-storage-buffer-texture-non-uniform-indexing"),
        );
    features.set(
      wgpu_types::Features::ADDRESS_MODE_CLAMP_TO_BORDER,
      required_features.0.contains("address-mode-clamp-to-border"),
    );
    features.set(
      wgpu_types::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES,
      required_features
        .0
        .contains("texture-adapter-specific-format-features"),
    );
    features.set(
      wgpu_types::Features::SHADER_FLOAT64,
      required_features.0.contains("shader-float64"),
    );
    features.set(
      wgpu_types::Features::VERTEX_ATTRIBUTE_64BIT,
      required_features.0.contains("vertex-attribute-64bit"),
    );
    features.set(
      wgpu_types::Features::VERTEX_WRITABLE_STORAGE,
      required_features.0.contains("vertex-writable-storage"),
    );
    features.set(
      wgpu_types::Features::CLEAR_TEXTURE,
      required_features.0.contains("clear-commands"),
    );
    features.set(
      wgpu_types::Features::SHADER_PRIMITIVE_INDEX,
      required_features.0.contains("shader-primitive-index"),
    );
    features.set(
      wgpu_types::Features::PARTIALLY_BOUND_BINDING_ARRAY,
      required_features
        .0
        .contains("partially-bound-binding-array"),
    );

    features
  }
}

#[op]
pub async fn op_webgpu_request_device(
  state: Rc<RefCell<OpState>>,
  adapter_rid: ResourceId,
  label: Option<String>,
  required_features: GpuRequiredFeatures,
  required_limits: Option<wgpu_types::Limits>,
) -> Result<GpuAdapterDevice, AnyError> {
  let mut state = state.borrow_mut();
  let adapter_resource =
    state.resource_table.get::<WebGpuAdapter>(adapter_rid)?;
  let adapter = adapter_resource.0;
  let instance = state.borrow::<Instance>();

  let descriptor = wgpu_types::DeviceDescriptor {
    label: label.map(Cow::from),
    features: required_features.into(),
    limits: required_limits.unwrap_or_default(),
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
    features,
    limits,
    // TODO(lucacasonato): report correctly from wgpu
    is_software: false,
  })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GPUAdapterInfo {
  vendor: String,
  architecture: String,
  device: String,
  description: String,
}

#[op]
pub async fn op_webgpu_request_adapter_info(
  state: Rc<RefCell<OpState>>,
  adapter_rid: ResourceId,
) -> Result<GPUAdapterInfo, AnyError> {
  let state = state.borrow_mut();
  let adapter_resource =
    state.resource_table.get::<WebGpuAdapter>(adapter_rid)?;
  let adapter = adapter_resource.0;
  let instance = state.borrow::<Instance>();

  let info = gfx_select!(adapter => instance.adapter_get_info(adapter))?;

  Ok(GPUAdapterInfo {
    vendor: info.vendor.to_string(),
    architecture: String::new(), // TODO(#2170)
    device: info.device.to_string(),
    description: info.name,
  })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateQuerySetArgs {
  device_rid: ResourceId,
  label: Option<String>,
  #[serde(flatten)]
  r#type: GpuQueryType,
  count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
enum GpuQueryType {
  Occlusion,
  #[serde(rename_all = "camelCase")]
  PipelineStatistics {
    pipeline_statistics: HashSet<String>,
  },
  Timestamp,
}

impl From<GpuQueryType> for wgpu_types::QueryType {
  fn from(query_type: GpuQueryType) -> Self {
    match query_type {
      GpuQueryType::Occlusion => wgpu_types::QueryType::Occlusion,
      GpuQueryType::PipelineStatistics {
        pipeline_statistics,
      } => {
        use wgpu_types::PipelineStatisticsTypes;

        let mut types = PipelineStatisticsTypes::empty();

        if pipeline_statistics.contains("vertex-shader-invocations") {
          types.set(PipelineStatisticsTypes::VERTEX_SHADER_INVOCATIONS, true);
        }
        if pipeline_statistics.contains("clipper-invocations") {
          types.set(PipelineStatisticsTypes::CLIPPER_INVOCATIONS, true);
        }
        if pipeline_statistics.contains("clipper-primitives-out") {
          types.set(PipelineStatisticsTypes::CLIPPER_PRIMITIVES_OUT, true);
        }
        if pipeline_statistics.contains("fragment-shader-invocations") {
          types.set(PipelineStatisticsTypes::FRAGMENT_SHADER_INVOCATIONS, true);
        }
        if pipeline_statistics.contains("compute-shader-invocations") {
          types.set(PipelineStatisticsTypes::COMPUTE_SHADER_INVOCATIONS, true);
        }

        wgpu_types::QueryType::PipelineStatistics(types)
      }
      GpuQueryType::Timestamp => wgpu_types::QueryType::Timestamp,
    }
  }
}

#[op]
pub fn op_webgpu_create_query_set(
  state: &mut OpState,
  args: CreateQuerySetArgs,
) -> Result<WebGpuResult, AnyError> {
  let device_resource =
    state.resource_table.get::<WebGpuDevice>(args.device_rid)?;
  let device = device_resource.0;
  let instance = &state.borrow::<Instance>();

  let descriptor = wgpu_types::QuerySetDescriptor {
    label: args.label.map(Cow::from),
    ty: args.r#type.into(),
    count: args.count,
  };

  gfx_put!(device => instance.device_create_query_set(
    device,
    &descriptor,
    std::marker::PhantomData
  ) => state, WebGpuQuerySet)
}

fn declare_webgpu_ops() -> Vec<deno_core::OpDecl> {
  vec![
    // Request device/adapter
    op_webgpu_request_adapter::decl(),
    op_webgpu_request_device::decl(),
    op_webgpu_request_adapter_info::decl(),
    // Query Set
    op_webgpu_create_query_set::decl(),
    // buffer
    buffer::op_webgpu_create_buffer::decl(),
    buffer::op_webgpu_buffer_get_mapped_range::decl(),
    buffer::op_webgpu_buffer_unmap::decl(),
    // buffer async
    buffer::op_webgpu_buffer_get_map_async::decl(),
    // remaining sync ops

    // texture
    texture::op_webgpu_create_texture::decl(),
    texture::op_webgpu_create_texture_view::decl(),
    // sampler
    sampler::op_webgpu_create_sampler::decl(),
    // binding
    binding::op_webgpu_create_bind_group_layout::decl(),
    binding::op_webgpu_create_pipeline_layout::decl(),
    binding::op_webgpu_create_bind_group::decl(),
    // pipeline
    pipeline::op_webgpu_create_compute_pipeline::decl(),
    pipeline::op_webgpu_compute_pipeline_get_bind_group_layout::decl(),
    pipeline::op_webgpu_create_render_pipeline::decl(),
    pipeline::op_webgpu_render_pipeline_get_bind_group_layout::decl(),
    // command_encoder
    command_encoder::op_webgpu_create_command_encoder::decl(),
    command_encoder::op_webgpu_command_encoder_begin_render_pass::decl(),
    command_encoder::op_webgpu_command_encoder_begin_compute_pass::decl(),
    command_encoder::op_webgpu_command_encoder_copy_buffer_to_buffer::decl(),
    command_encoder::op_webgpu_command_encoder_copy_buffer_to_texture::decl(),
    command_encoder::op_webgpu_command_encoder_copy_texture_to_buffer::decl(),
    command_encoder::op_webgpu_command_encoder_copy_texture_to_texture::decl(),
    command_encoder::op_webgpu_command_encoder_clear_buffer::decl(),
    command_encoder::op_webgpu_command_encoder_push_debug_group::decl(),
    command_encoder::op_webgpu_command_encoder_pop_debug_group::decl(),
    command_encoder::op_webgpu_command_encoder_insert_debug_marker::decl(),
    command_encoder::op_webgpu_command_encoder_write_timestamp::decl(),
    command_encoder::op_webgpu_command_encoder_resolve_query_set::decl(),
    command_encoder::op_webgpu_command_encoder_finish::decl(),
    // render_pass
    render_pass::op_webgpu_render_pass_set_viewport::decl(),
    render_pass::op_webgpu_render_pass_set_scissor_rect::decl(),
    render_pass::op_webgpu_render_pass_set_blend_constant::decl(),
    render_pass::op_webgpu_render_pass_set_stencil_reference::decl(),
    render_pass::op_webgpu_render_pass_begin_pipeline_statistics_query::decl(),
    render_pass::op_webgpu_render_pass_end_pipeline_statistics_query::decl(),
    render_pass::op_webgpu_render_pass_write_timestamp::decl(),
    render_pass::op_webgpu_render_pass_execute_bundles::decl(),
    render_pass::op_webgpu_render_pass_end::decl(),
    render_pass::op_webgpu_render_pass_set_bind_group::decl(),
    render_pass::op_webgpu_render_pass_push_debug_group::decl(),
    render_pass::op_webgpu_render_pass_pop_debug_group::decl(),
    render_pass::op_webgpu_render_pass_insert_debug_marker::decl(),
    render_pass::op_webgpu_render_pass_set_pipeline::decl(),
    render_pass::op_webgpu_render_pass_set_index_buffer::decl(),
    render_pass::op_webgpu_render_pass_set_vertex_buffer::decl(),
    render_pass::op_webgpu_render_pass_draw::decl(),
    render_pass::op_webgpu_render_pass_draw_indexed::decl(),
    render_pass::op_webgpu_render_pass_draw_indirect::decl(),
    render_pass::op_webgpu_render_pass_draw_indexed_indirect::decl(),
    // compute_pass
    compute_pass::op_webgpu_compute_pass_set_pipeline::decl(),
    compute_pass::op_webgpu_compute_pass_dispatch_workgroups::decl(),
    compute_pass::op_webgpu_compute_pass_dispatch_workgroups_indirect::decl(),
    compute_pass::op_webgpu_compute_pass_begin_pipeline_statistics_query::decl(
    ),
    compute_pass::op_webgpu_compute_pass_end_pipeline_statistics_query::decl(),
    compute_pass::op_webgpu_compute_pass_write_timestamp::decl(),
    compute_pass::op_webgpu_compute_pass_end::decl(),
    compute_pass::op_webgpu_compute_pass_set_bind_group::decl(),
    compute_pass::op_webgpu_compute_pass_push_debug_group::decl(),
    compute_pass::op_webgpu_compute_pass_pop_debug_group::decl(),
    compute_pass::op_webgpu_compute_pass_insert_debug_marker::decl(),
    // bundle
    bundle::op_webgpu_create_render_bundle_encoder::decl(),
    bundle::op_webgpu_render_bundle_encoder_finish::decl(),
    bundle::op_webgpu_render_bundle_encoder_set_bind_group::decl(),
    bundle::op_webgpu_render_bundle_encoder_push_debug_group::decl(),
    bundle::op_webgpu_render_bundle_encoder_pop_debug_group::decl(),
    bundle::op_webgpu_render_bundle_encoder_insert_debug_marker::decl(),
    bundle::op_webgpu_render_bundle_encoder_set_pipeline::decl(),
    bundle::op_webgpu_render_bundle_encoder_set_index_buffer::decl(),
    bundle::op_webgpu_render_bundle_encoder_set_vertex_buffer::decl(),
    bundle::op_webgpu_render_bundle_encoder_draw::decl(),
    bundle::op_webgpu_render_bundle_encoder_draw_indexed::decl(),
    bundle::op_webgpu_render_bundle_encoder_draw_indirect::decl(),
    // queue
    queue::op_webgpu_queue_submit::decl(),
    queue::op_webgpu_write_buffer::decl(),
    queue::op_webgpu_write_texture::decl(),
    // shader
    shader::op_webgpu_create_shader_module::decl(),
  ]
}
