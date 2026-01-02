// Copyright 2018-2025 the Deno authors. MIT license.

#[allow(clippy::disallowed_types)]
use std::collections::HashSet;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::V8TaskSpawner;
use deno_core::WebIDL;
use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::v8;

use super::device::GPUDevice;
use crate::Instance;
use crate::error::GPUGenericError;
use crate::webidl::GPUFeatureName;
use crate::webidl::features_to_feature_names;

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURequestAdapterOptions {
  pub power_preference: Option<GPUPowerPreference>,
  #[webidl(default = false)]
  pub force_fallback_adapter: bool,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUPowerPreference {
  LowPower,
  HighPerformance,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct GPUDeviceDescriptor {
  #[webidl(default = String::new())]
  label: String,

  #[webidl(default = vec![])]
  required_features: Vec<GPUFeatureName>,
  #[webidl(default = Default::default())]
  #[options(enforce_range = true)]
  required_limits: indexmap::IndexMap<String, Option<u64>>,
}

pub struct GPUAdapter {
  pub instance: Instance,
  pub id: wgpu_core::id::AdapterId,

  pub features: SameObject<GPUSupportedFeatures>,
  pub limits: SameObject<GPUSupportedLimits>,
  pub info: Rc<SameObject<GPUAdapterInfo>>,
}

impl Drop for GPUAdapter {
  fn drop(&mut self) {
    self.instance.adapter_drop(self.id);
  }
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUAdapter {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUAdapter"
  }
}

#[op2]
impl GPUAdapter {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUAdapter, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[getter]
  #[global]
  fn info(&self, scope: &mut v8::PinScope<'_, '_>) -> v8::Global<v8::Object> {
    self.info.get(scope, |_| {
      let info = self.instance.adapter_get_info(self.id);

      GPUAdapterInfo { info }
    })
  }

  #[getter]
  #[global]
  fn features(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
  ) -> v8::Global<v8::Object> {
    self.features.get(scope, |scope| {
      let features = self.instance.adapter_features(self.id);
      // Only expose WebGPU features, not wgpu native-only features
      let features = features & wgpu_types::Features::all_webgpu_mask();
      let features = features_to_feature_names(features);
      GPUSupportedFeatures::new(scope, features)
    })
  }

  #[getter]
  #[global]
  fn limits(&self, scope: &mut v8::PinScope<'_, '_>) -> v8::Global<v8::Object> {
    self.limits.get(scope, |_| {
      let adapter_limits = self.instance.adapter_limits(self.id);
      GPUSupportedLimits(adapter_limits)
    })
  }

  #[async_method(fake)]
  #[global]
  fn request_device(
    &self,
    state: &mut OpState,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] descriptor: GPUDeviceDescriptor,
  ) -> Result<v8::Global<v8::Value>, CreateDeviceError> {
    let features = self.instance.adapter_features(self.id);
    let supported_features = features_to_feature_names(features);
    #[allow(clippy::disallowed_types)]
    let required_features = descriptor
      .required_features
      .iter()
      .cloned()
      .collect::<HashSet<_>>();

    if !required_features.is_subset(&supported_features) {
      return Err(CreateDeviceError::RequiredFeaturesNotASubset);
    }

    // When support for compatibility mode is added, this will need to look
    // at whether the adapter is "compatibility-defaulting" or
    // "core-defaulting", and choose the appropriate set of defaults.
    //
    // Support for compatibility mode is tracked in
    // https://github.com/gfx-rs/wgpu/issues/8124.
    let required_limits = serde_json::from_value::<wgpu_types::Limits>(
      serde_json::to_value(descriptor.required_limits)?,
    )?
    .or_better_values_from(&wgpu_types::Limits::default());

    let trace = std::env::var_os("DENO_WEBGPU_TRACE")
      .map(|path| wgpu_types::Trace::Directory(std::path::PathBuf::from(path)))
      .unwrap_or_default();

    let wgpu_descriptor = wgpu_types::DeviceDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      required_features: super::webidl::feature_names_to_features(
        descriptor.required_features,
      ),
      required_limits,
      experimental_features: wgpu_types::ExperimentalFeatures::disabled(),
      memory_hints: Default::default(),
      trace,
    };

    let (device, queue) = self.instance.adapter_request_device(
      self.id,
      &wgpu_descriptor,
      None,
      None,
    )?;

    let spawner = state.borrow::<V8TaskSpawner>().clone();
    let lost_resolver = v8::PromiseResolver::new(scope).unwrap();
    let lost_promise = lost_resolver.get_promise(scope);
    let device = GPUDevice {
      instance: self.instance.clone(),
      id: device,
      queue,
      label: descriptor.label,
      queue_obj: SameObject::new(),
      adapter_info: self.info.clone(),
      error_handler: Rc::new(super::error::DeviceErrorHandler::new(
        v8::Global::new(scope, lost_resolver),
        spawner,
      )),
      adapter: self.id,
      lost_promise: v8::Global::new(scope, lost_promise),
      limits: SameObject::new(),
      features: SameObject::new(),
      has_active_capture: std::cell::RefCell::new(false),
    };
    let device = deno_core::cppgc::make_cppgc_object(scope, device);
    let weak_device = v8::Weak::new(scope, device);
    let event_target_setup = state.borrow::<crate::EventTargetSetup>();
    let webidl_brand = v8::Local::new(scope, event_target_setup.brand.clone());
    device.set(scope, webidl_brand, webidl_brand);
    let set_event_target_data =
      v8::Local::new(scope, event_target_setup.set_event_target_data.clone())
        .cast::<v8::Function>();
    let null = v8::null(scope);
    set_event_target_data.call(scope, null.into(), &[device.into()]);

    // Now that the device is fully constructed, give the error handler a
    // weak reference to it.
    let device = device.cast::<v8::Value>();
    deno_core::cppgc::try_unwrap_cppgc_object::<GPUDevice>(scope, device)
      .unwrap()
      .error_handler
      .set_device(weak_device);

    Ok(v8::Global::new(scope, device))
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CreateDeviceError {
  #[class(type)]
  #[error("requiredFeatures must be a subset of the adapter features")]
  RequiredFeaturesNotASubset,
  #[class(inherit)]
  #[error(transparent)]
  Serde(#[from] serde_json::Error),
  #[class("DOMExceptionOperationError")]
  #[error(transparent)]
  Device(#[from] wgpu_core::instance::RequestDeviceError),
}

pub struct GPUSupportedLimits(pub wgpu_types::Limits);

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUSupportedLimits {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUSupportedLimits"
  }
}

#[op2]
impl GPUSupportedLimits {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUSupportedLimits, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[getter]
  fn maxTextureDimension1D(&self) -> u32 {
    self.0.max_texture_dimension_1d
  }

  #[getter]
  fn maxTextureDimension2D(&self) -> u32 {
    self.0.max_texture_dimension_2d
  }

  #[getter]
  fn maxTextureDimension3D(&self) -> u32 {
    self.0.max_texture_dimension_3d
  }

  #[getter]
  fn maxTextureArrayLayers(&self) -> u32 {
    self.0.max_texture_array_layers
  }

  #[getter]
  fn maxBindGroups(&self) -> u32 {
    self.0.max_bind_groups
  }

  // TODO(@crowlKats): support max_bind_groups_plus_vertex_buffers

  #[getter]
  fn maxBindingsPerBindGroup(&self) -> u32 {
    self.0.max_bindings_per_bind_group
  }

  #[getter]
  fn maxDynamicUniformBuffersPerPipelineLayout(&self) -> u32 {
    self.0.max_dynamic_uniform_buffers_per_pipeline_layout
  }

  #[getter]
  fn maxDynamicStorageBuffersPerPipelineLayout(&self) -> u32 {
    self.0.max_dynamic_storage_buffers_per_pipeline_layout
  }

  #[getter]
  fn maxSampledTexturesPerShaderStage(&self) -> u32 {
    self.0.max_sampled_textures_per_shader_stage
  }

  #[getter]
  fn maxSamplersPerShaderStage(&self) -> u32 {
    self.0.max_samplers_per_shader_stage
  }

  #[getter]
  fn maxStorageBuffersPerShaderStage(&self) -> u32 {
    self.0.max_storage_buffers_per_shader_stage
  }

  #[getter]
  fn maxStorageTexturesPerShaderStage(&self) -> u32 {
    self.0.max_storage_textures_per_shader_stage
  }

  #[getter]
  fn maxUniformBuffersPerShaderStage(&self) -> u32 {
    self.0.max_uniform_buffers_per_shader_stage
  }

  #[getter]
  fn maxUniformBufferBindingSize(&self) -> u32 {
    self.0.max_uniform_buffer_binding_size
  }

  #[getter]
  fn maxStorageBufferBindingSize(&self) -> u32 {
    self.0.max_storage_buffer_binding_size
  }

  #[getter]
  fn minUniformBufferOffsetAlignment(&self) -> u32 {
    self.0.min_uniform_buffer_offset_alignment
  }

  #[getter]
  fn minStorageBufferOffsetAlignment(&self) -> u32 {
    self.0.min_storage_buffer_offset_alignment
  }

  #[getter]
  fn maxVertexBuffers(&self) -> u32 {
    self.0.max_vertex_buffers
  }

  #[getter]
  #[number]
  fn maxBufferSize(&self) -> u64 {
    self.0.max_buffer_size
  }

  #[getter]
  fn maxVertexAttributes(&self) -> u32 {
    self.0.max_vertex_attributes
  }

  #[getter]
  fn maxVertexBufferArrayStride(&self) -> u32 {
    self.0.max_vertex_buffer_array_stride
  }

  // TODO(@crowlKats): support max_inter_stage_shader_variables

  #[getter]
  fn maxColorAttachments(&self) -> u32 {
    self.0.max_color_attachments
  }

  #[getter]
  fn maxColorAttachmentBytesPerSample(&self) -> u32 {
    self.0.max_color_attachment_bytes_per_sample
  }

  #[getter]
  fn maxComputeWorkgroupStorageSize(&self) -> u32 {
    self.0.max_compute_workgroup_storage_size
  }

  #[getter]
  fn maxComputeInvocationsPerWorkgroup(&self) -> u32 {
    self.0.max_compute_invocations_per_workgroup
  }

  #[getter]
  fn maxComputeWorkgroupSizeX(&self) -> u32 {
    self.0.max_compute_workgroup_size_x
  }

  #[getter]
  fn maxComputeWorkgroupSizeY(&self) -> u32 {
    self.0.max_compute_workgroup_size_y
  }

  #[getter]
  fn maxComputeWorkgroupSizeZ(&self) -> u32 {
    self.0.max_compute_workgroup_size_z
  }

  #[getter]
  fn maxComputeWorkgroupsPerDimension(&self) -> u32 {
    self.0.max_compute_workgroups_per_dimension
  }
}

pub struct GPUSupportedFeatures(v8::Global<v8::Value>);

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUSupportedFeatures {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUSupportedFeatures"
  }
}

impl GPUSupportedFeatures {
  #[allow(clippy::disallowed_types)]
  pub fn new(
    scope: &mut v8::PinScope<'_, '_>,
    features: HashSet<GPUFeatureName>,
  ) -> Self {
    let set = v8::Set::new(scope);

    for feature in features {
      let key = v8::String::new(scope, feature.as_str()).unwrap();
      set.add(scope, key.into());
    }

    Self(v8::Global::new(scope, <v8::Local<v8::Value>>::from(set)))
  }
}

#[op2]
impl GPUSupportedFeatures {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUSupportedFeatures, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[global]
  #[symbol("setlike_set")]
  fn set(&self) -> v8::Global<v8::Value> {
    self.0.clone()
  }
}

pub struct GPUAdapterInfo {
  pub info: wgpu_types::AdapterInfo,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUAdapterInfo {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUAdapterInfo"
  }
}

#[op2]
impl GPUAdapterInfo {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUAdapterInfo, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[getter]
  #[string]
  fn vendor(&self) -> String {
    self.info.vendor.to_string()
  }

  #[getter]
  #[string]
  fn architecture(&self) -> &'static str {
    "" // TODO: wgpu#2170
  }

  #[getter]
  #[string]
  fn device(&self) -> String {
    self.info.device.to_string()
  }

  #[getter]
  #[string]
  fn description(&self) -> String {
    self.info.name.clone()
  }

  #[getter]
  fn subgroup_min_size(&self) -> u32 {
    self.info.subgroup_min_size
  }

  #[getter]
  fn subgroup_max_size(&self) -> u32 {
    self.info.subgroup_max_size
  }

  #[getter]
  fn is_fallback_adapter(&self) -> bool {
    // TODO(lucacasonato): report correctly from wgpu
    false
  }
}
