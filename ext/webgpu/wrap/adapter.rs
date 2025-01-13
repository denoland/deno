// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::sync::Arc;

use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

use super::device::GPUDevice;
use crate::Instance;

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
  required_features: Vec<super::webidl::GPUFeatureName>,
  #[webidl(default = Default::default())]
  #[options(enforce_range = true)]
  required_limits: indexmap::IndexMap<String, u64>,
}

pub struct GPUAdapter {
  pub instance: Instance,
  pub id: wgpu_core::id::AdapterId,

  pub features: SameObject<GPUAdapter>,
  pub limits: SameObject<GPUSupportedLimits>,
  pub info: Arc<SameObject<GPUAdapterInfo>>,
}

impl GarbageCollected for GPUAdapter {}

#[op2]
impl GPUAdapter {
  #[getter]
  #[global]
  fn info(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.info.get(scope, || {
      let info = self.instance.adapter_get_info(self.id);
      GPUAdapterInfo(info)
    })
  }

  #[getter]
  #[global]
  fn features(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.features.get(scope, || {
      let features = self.instance.adapter_features(self.id);

      /*
      function createGPUSupportedFeatures(features) {
        /** @type {GPUSupportedFeatures} */
        const supportedFeatures = webidl.createBranded(GPUSupportedFeatures);
        supportedFeatures[webidl.setlikeInner] = new SafeSet(features);
        webidl.setlike(
          supportedFeatures,
          GPUSupportedFeaturesPrototype,
          true,
        );
        return supportedFeatures;
      }
       */

      todo!()
    })
  }
  #[getter]
  #[global]
  fn limits(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.limits.get(scope, || {
      let adapter_limits = self.instance.adapter_limits(self.id);
      GPUSupportedLimits(adapter_limits)
    })
  }
  #[getter]
  fn is_fallback_adapter(&self) -> bool {
    // TODO(lucacasonato): report correctly from wgpu
    false
  }

  #[async_method]
  #[cppgc]
  async fn request_device(
    &self,
    #[webidl] descriptor: GPUDeviceDescriptor,
  ) -> Result<GPUDevice, CreateDeviceError> {
    let features = self.instance.adapter_features(self.id);
    let supported_features =
      crate::wrap::webidl::features_to_feature_names(&features);
    let required_features = descriptor
      .required_features
      .iter()
      .cloned()
      .collect::<HashSet<_>>();

    if !required_features.is_subset(&supported_features) {
      return Err(CreateDeviceError::RequiredFeaturesNotASubset);
    }

    let required_limits = serde_json::from_value(serde_json::to_value(
      descriptor.required_limits,
    )?)?;

    let wgpu_descriptor = wgpu_types::DeviceDescriptor {
      label: Some(std::borrow::Cow::Owned(descriptor.label.clone())),
      required_features: super::webidl::feature_names_to_features(
        descriptor.required_features,
      ),
      required_limits,
      memory_hints: Default::default(),
    };

    let (device, queue) = self.instance.adapter_request_device(
      self.id,
      &wgpu_descriptor,
      std::env::var("DENO_WEBGPU_TRACE")
        .ok()
        .as_ref()
        .map(std::path::Path::new),
      None,
      None,
    )?;

    let (sender, receiver) = tokio::sync::oneshot::channel();

    Ok(GPUDevice {
      instance: self.instance.clone(),
      id: device,
      queue,
      label: descriptor.label,
      queue_obj: SameObject::new(),
      info: self.info.clone(),
      error_handler: Arc::new(super::error::DeviceErrorHandler::new(sender)),
      adapter: self.id,
      lost_receiver: receiver,
    })
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
  #[class(type)]
  #[error(transparent)]
  Device(#[from] wgpu_core::instance::RequestDeviceError),
}

pub struct GPUSupportedLimits(wgpu_types::Limits);

impl GarbageCollected for GPUSupportedLimits {}

#[op2]
impl GPUSupportedLimits {
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

pub struct GPUAdapterInfo(pub wgpu_types::AdapterInfo);

impl GarbageCollected for GPUAdapterInfo {}

#[op2]
impl GPUAdapterInfo {
  #[getter]
  #[string]
  fn vendor(&self) -> String {
    self.0.vendor.to_string()
  }

  #[getter]
  #[string]
  fn architecture(&self) -> &'static str {
    "" // TODO: wgpu#2170
  }

  #[getter]
  #[string]
  fn device(&self) -> String {
    self.0.device.to_string()
  }

  #[getter]
  #[string]
  fn description(&self) -> String {
    self.0.name.clone()
  }
}
