// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::num::NonZeroU64;
use std::rc::Rc;

use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_error::JsErrorBox;
use wgpu_core::binding_model::BindingResource;
use wgpu_core::pipeline::ProgrammableStageDescriptor;
use wgpu_types::BindingType;

use super::bind_group::GPUBindGroup;
use super::bind_group::GPUBindingResource;
use super::bind_group_layout::GPUBindGroupLayout;
use super::buffer::GPUBuffer;
use super::compute_pipeline::GPUComputePipeline;
use super::pipeline_layout::GPUPipelineLayout;
use super::queue::GPUQueue;
use super::sampler::GPUSampler;
use super::shader::GPUShaderModule;
use super::texture::GPUTexture;
use crate::adapter::GPUAdapterInfo;
use crate::adapter::GPUSupportedFeatures;
use crate::adapter::GPUSupportedLimits;
use crate::command_encoder::GPUCommandEncoder;
use crate::query_set::GPUQuerySet;
use crate::render_bundle::GPURenderBundleEncoder;
use crate::render_pipeline::GPURenderPipeline;
use crate::webidl::features_to_feature_names;
use crate::Instance;

pub struct GPUDevice {
  pub instance: Instance,
  pub id: wgpu_core::id::DeviceId,
  pub adapter: wgpu_core::id::AdapterId,
  pub queue: wgpu_core::id::QueueId,

  pub label: String,

  pub features: SameObject<GPUSupportedFeatures>,
  pub limits: SameObject<GPUSupportedLimits>,
  pub adapter_info: Rc<SameObject<GPUAdapterInfo>>,

  pub queue_obj: SameObject<GPUQueue>,

  pub error_handler: super::error::ErrorHandler,
  pub lost_receiver:
    tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

impl Drop for GPUDevice {
  fn drop(&mut self) {
    self.instance.device_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUDevice {
  const NAME: &'static str = "GPUDevice";
}

impl GarbageCollected for GPUDevice {}

// EventTarget is extended in JS
#[op2]
impl GPUDevice {
  #[getter]
  #[string]
  fn label(&self) -> String {
    self.label.clone()
  }
  #[setter]
  #[string]
  fn label(&self, #[webidl] _label: String) {
    // TODO(@crowlKats): no-op, needs wpgu to implement changing the label
  }

  #[getter]
  #[global]
  fn features(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.features.get(scope, |scope| {
      let features = self.instance.device_features(self.id);
      let features = features_to_feature_names(features);
      GPUSupportedFeatures::new(scope, features)
    })
  }

  #[getter]
  #[global]
  fn limits(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.limits.get(scope, |_| {
      let limits = self.instance.device_limits(self.id);
      GPUSupportedLimits(limits)
    })
  }

  #[getter]
  #[global]
  fn adapter_info(
    &self,
    scope: &mut v8::HandleScope,
  ) -> v8::Global<v8::Object> {
    self.adapter_info.get(scope, |_| {
      let info = self.instance.adapter_get_info(self.adapter);
      let limits = self.instance.adapter_limits(self.adapter);

      GPUAdapterInfo {
        info,
        subgroup_min_size: limits.min_subgroup_size,
        subgroup_max_size: limits.max_subgroup_size,
      }
    })
  }

  #[getter]
  #[global]
  fn queue(&self, scope: &mut v8::HandleScope) -> v8::Global<v8::Object> {
    self.queue_obj.get(scope, |_| GPUQueue {
      id: self.queue,
      error_handler: self.error_handler.clone(),
      instance: self.instance.clone(),
      label: self.label.clone(),
    })
  }

  #[fast]
  fn destroy(&self) {
    self.instance.device_destroy(self.id);
  }

  #[required(1)]
  #[cppgc]
  fn create_buffer(
    &self,
    #[webidl] descriptor: super::buffer::GPUBufferDescriptor,
  ) -> Result<GPUBuffer, JsErrorBox> {
    let wgpu_descriptor = wgpu_core::resource::BufferDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      size: descriptor.size,
      usage: wgpu_types::BufferUsages::from_bits(descriptor.usage)
        .ok_or_else(|| JsErrorBox::type_error("usage is not valid"))?,
      mapped_at_creation: descriptor.mapped_at_creation,
    };

    let (id, err) =
      self
        .instance
        .device_create_buffer(self.id, &wgpu_descriptor, None);

    self.error_handler.push_error(err);

    Ok(GPUBuffer {
      instance: self.instance.clone(),
      error_handler: self.error_handler.clone(),
      id,
      device: self.id,
      label: descriptor.label,
      size: descriptor.size,
      usage: descriptor.usage,
      map_state: RefCell::new(if descriptor.mapped_at_creation {
        "mapped"
      } else {
        "unmapped"
      }),
      map_mode: RefCell::new(if descriptor.mapped_at_creation {
        Some(wgpu_core::device::HostMap::Write)
      } else {
        None
      }),
      mapped_js_buffers: RefCell::new(vec![]),
    })
  }

  #[required(1)]
  #[cppgc]
  fn create_texture(
    &self,
    #[webidl] descriptor: super::texture::GPUTextureDescriptor,
  ) -> Result<GPUTexture, JsErrorBox> {
    let wgpu_descriptor = wgpu_core::resource::TextureDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      size: descriptor.size.into(),
      mip_level_count: descriptor.mip_level_count,
      sample_count: descriptor.sample_count,
      dimension: descriptor.dimension.clone().into(),
      format: descriptor.format.clone().into(),
      usage: wgpu_types::TextureUsages::from_bits(descriptor.usage)
        .ok_or_else(|| JsErrorBox::type_error("usage is not valid"))?,
      view_formats: descriptor
        .view_formats
        .into_iter()
        .map(Into::into)
        .collect(),
    };

    let (id, err) =
      self
        .instance
        .device_create_texture(self.id, &wgpu_descriptor, None);

    self.error_handler.push_error(err);

    Ok(GPUTexture {
      instance: self.instance.clone(),
      error_handler: self.error_handler.clone(),
      id,
      device_id: self.id,
      queue_id: self.queue,
      label: descriptor.label,
      size: wgpu_descriptor.size,
      mip_level_count: wgpu_descriptor.mip_level_count,
      sample_count: wgpu_descriptor.sample_count,
      dimension: descriptor.dimension,
      format: descriptor.format,
      usage: descriptor.usage,
    })
  }

  #[cppgc]
  fn create_sampler(
    &self,
    #[webidl] descriptor: super::sampler::GPUSamplerDescriptor,
  ) -> Result<GPUSampler, JsErrorBox> {
    let wgpu_descriptor = wgpu_core::resource::SamplerDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      address_modes: [
        descriptor.address_mode_u.into(),
        descriptor.address_mode_v.into(),
        descriptor.address_mode_w.into(),
      ],
      mag_filter: descriptor.mag_filter.into(),
      min_filter: descriptor.min_filter.into(),
      mipmap_filter: descriptor.mipmap_filter.into(),
      lod_min_clamp: descriptor.lod_min_clamp,
      lod_max_clamp: descriptor.lod_max_clamp,
      compare: descriptor.compare.map(Into::into),
      anisotropy_clamp: descriptor.max_anisotropy,
      border_color: None,
    };

    let (id, err) =
      self
        .instance
        .device_create_sampler(self.id, &wgpu_descriptor, None);

    self.error_handler.push_error(err);

    Ok(GPUSampler {
      instance: self.instance.clone(),
      id,
      label: descriptor.label,
    })
  }

  #[required(1)]
  #[cppgc]
  fn create_bind_group_layout(
    &self,
    #[webidl]
    descriptor: super::bind_group_layout::GPUBindGroupLayoutDescriptor,
  ) -> Result<GPUBindGroupLayout, JsErrorBox> {
    let mut entries = Vec::with_capacity(descriptor.entries.len());

    for entry in descriptor.entries {
      let n_entries = [
        entry.buffer.is_some(),
        entry.sampler.is_some(),
        entry.texture.is_some(),
        entry.storage_texture.is_some(),
      ]
      .into_iter()
      .filter(|t| *t)
      .count();

      if n_entries != 1 {
        return Err(JsErrorBox::type_error("Only one of 'buffer', 'sampler', 'texture' and 'storageTexture' may be specified"));
      }

      let ty = if let Some(buffer) = entry.buffer {
        BindingType::Buffer {
          ty: buffer.r#type.into(),
          has_dynamic_offset: buffer.has_dynamic_offset,
          min_binding_size: NonZeroU64::new(buffer.min_binding_size),
        }
      } else if let Some(sampler) = entry.sampler {
        BindingType::Sampler(sampler.r#type.into())
      } else if let Some(texture) = entry.texture {
        BindingType::Texture {
          sample_type: texture.sample_type.into(),
          view_dimension: texture.view_dimension.into(),
          multisampled: texture.multisampled,
        }
      } else if let Some(storage_texture) = entry.storage_texture {
        BindingType::StorageTexture {
          access: storage_texture.access.into(),
          format: storage_texture.format.into(),
          view_dimension: storage_texture.view_dimension.into(),
        }
      } else {
        unreachable!()
      };

      entries.push(wgpu_types::BindGroupLayoutEntry {
        binding: entry.binding,
        visibility: wgpu_types::ShaderStages::from_bits(entry.visibility)
          .ok_or_else(|| JsErrorBox::type_error("usage is not valid"))?,
        ty,
        count: None, // native-only
      });
    }

    let wgpu_descriptor = wgpu_core::binding_model::BindGroupLayoutDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      entries: Cow::Owned(entries),
    };

    let (id, err) = self.instance.device_create_bind_group_layout(
      self.id,
      &wgpu_descriptor,
      None,
    );

    self.error_handler.push_error(err);

    Ok(GPUBindGroupLayout {
      instance: self.instance.clone(),
      id,
      label: descriptor.label,
    })
  }

  #[required(1)]
  #[cppgc]
  fn create_pipeline_layout(
    &self,
    #[webidl] descriptor: super::pipeline_layout::GPUPipelineLayoutDescriptor,
  ) -> GPUPipelineLayout {
    let bind_group_layouts = descriptor
      .bind_group_layouts
      .into_iter()
      .map(|bind_group_layout| bind_group_layout.id)
      .collect();

    let wgpu_descriptor = wgpu_core::binding_model::PipelineLayoutDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      bind_group_layouts: Cow::Owned(bind_group_layouts),
      push_constant_ranges: Default::default(),
    };

    let (id, err) = self.instance.device_create_pipeline_layout(
      self.id,
      &wgpu_descriptor,
      None,
    );

    self.error_handler.push_error(err);

    GPUPipelineLayout {
      instance: self.instance.clone(),
      id,
      label: descriptor.label,
    }
  }

  #[required(1)]
  #[cppgc]
  fn create_bind_group(
    &self,
    #[webidl] descriptor: super::bind_group::GPUBindGroupDescriptor,
  ) -> GPUBindGroup {
    let entries = descriptor
      .entries
      .into_iter()
      .map(|entry| wgpu_core::binding_model::BindGroupEntry {
        binding: entry.binding,
        resource: match entry.resource {
          GPUBindingResource::Sampler(sampler) => {
            BindingResource::Sampler(sampler.id)
          }
          GPUBindingResource::TextureView(texture_view) => {
            BindingResource::TextureView(texture_view.id)
          }
          GPUBindingResource::BufferBinding(buffer_binding) => {
            BindingResource::Buffer(wgpu_core::binding_model::BufferBinding {
              buffer_id: buffer_binding.buffer.id,
              offset: buffer_binding.offset,
              size: buffer_binding.size.and_then(NonZeroU64::new),
            })
          }
        },
      })
      .collect::<Vec<_>>();

    let wgpu_descriptor = wgpu_core::binding_model::BindGroupDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      layout: descriptor.layout.id,
      entries: Cow::Owned(entries),
    };

    let (id, err) =
      self
        .instance
        .device_create_bind_group(self.id, &wgpu_descriptor, None);

    self.error_handler.push_error(err);

    GPUBindGroup {
      instance: self.instance.clone(),
      id,
      label: descriptor.label,
    }
  }

  #[required(1)]
  #[cppgc]
  fn create_shader_module(
    &self,
    #[webidl] descriptor: super::shader::GPUShaderModuleDescriptor,
  ) -> GPUShaderModule {
    let wgpu_descriptor = wgpu_core::pipeline::ShaderModuleDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      runtime_checks: wgpu_types::ShaderRuntimeChecks::default(),
    };

    let (id, err) = self.instance.device_create_shader_module(
      self.id,
      &wgpu_descriptor,
      wgpu_core::pipeline::ShaderModuleSource::Wgsl(Cow::Owned(
        descriptor.code,
      )),
      None,
    );

    self.error_handler.push_error(err);

    GPUShaderModule {
      instance: self.instance.clone(),
      id,
      label: descriptor.label,
    }
  }

  #[required(1)]
  #[cppgc]
  fn create_compute_pipeline(
    &self,
    #[webidl] descriptor: super::compute_pipeline::GPUComputePipelineDescriptor,
  ) -> GPUComputePipeline {
    self.new_compute_pipeline(descriptor)
  }

  #[required(1)]
  #[cppgc]
  fn create_render_pipeline(
    &self,
    #[webidl] descriptor: super::render_pipeline::GPURenderPipelineDescriptor,
  ) -> Result<GPURenderPipeline, JsErrorBox> {
    self.new_render_pipeline(descriptor)
  }

  #[async_method]
  #[required(1)]
  #[cppgc]
  async fn create_compute_pipeline_async(
    &self,
    #[webidl] descriptor: super::compute_pipeline::GPUComputePipelineDescriptor,
  ) -> GPUComputePipeline {
    self.new_compute_pipeline(descriptor)
  }

  #[async_method]
  #[required(1)]
  #[cppgc]
  async fn create_render_pipeline_async(
    &self,
    #[webidl] descriptor: super::render_pipeline::GPURenderPipelineDescriptor,
  ) -> Result<GPURenderPipeline, JsErrorBox> {
    self.new_render_pipeline(descriptor)
  }

  #[cppgc]
  fn create_command_encoder(
    &self,
    #[webidl] descriptor: Option<
      super::command_encoder::GPUCommandEncoderDescriptor,
    >,
  ) -> GPUCommandEncoder {
    let label = descriptor.map(|d| d.label).unwrap_or_default();
    let wgpu_descriptor = wgpu_types::CommandEncoderDescriptor {
      label: Some(Cow::Owned(label.clone())),
    };

    let (id, err) = self.instance.device_create_command_encoder(
      self.id,
      &wgpu_descriptor,
      None,
    );

    self.error_handler.push_error(err);

    GPUCommandEncoder {
      instance: self.instance.clone(),
      error_handler: self.error_handler.clone(),
      id,
      label,
    }
  }

  #[required(1)]
  #[cppgc]
  fn create_render_bundle_encoder(
    &self,
    #[webidl]
    descriptor: super::render_bundle::GPURenderBundleEncoderDescriptor,
  ) -> GPURenderBundleEncoder {
    let wgpu_descriptor = wgpu_core::command::RenderBundleEncoderDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      color_formats: Cow::Owned(
        descriptor
          .color_formats
          .into_iter()
          .map(|format| format.into_option().map(Into::into))
          .collect::<Vec<_>>(),
      ),
      depth_stencil: descriptor.depth_stencil_format.map(|format| {
        wgpu_types::RenderBundleDepthStencil {
          format: format.into(),
          depth_read_only: descriptor.depth_read_only,
          stencil_read_only: descriptor.stencil_read_only,
        }
      }),
      sample_count: descriptor.sample_count,
      multiview: None,
    };

    let res = wgpu_core::command::RenderBundleEncoder::new(
      &wgpu_descriptor,
      self.id,
      None,
    );
    let (encoder, err) = match res {
      Ok(encoder) => (encoder, None),
      Err(e) => (
        wgpu_core::command::RenderBundleEncoder::dummy(self.id),
        Some(e),
      ),
    };

    self.error_handler.push_error(err);

    GPURenderBundleEncoder {
      instance: self.instance.clone(),
      error_handler: self.error_handler.clone(),
      encoder: RefCell::new(Some(encoder)),
      label: descriptor.label,
    }
  }

  #[required(1)]
  #[cppgc]
  fn create_query_set(
    &self,
    #[webidl] descriptor: crate::query_set::GPUQuerySetDescriptor,
  ) -> GPUQuerySet {
    let wgpu_descriptor = wgpu_core::resource::QuerySetDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      ty: descriptor.r#type.clone().into(),
      count: descriptor.count,
    };

    let (id, err) =
      self
        .instance
        .device_create_query_set(self.id, &wgpu_descriptor, None);

    self.error_handler.push_error(err);

    GPUQuerySet {
      instance: self.instance.clone(),
      id,
      r#type: descriptor.r#type,
      count: descriptor.count,
      label: descriptor.label,
    }
  }

  // TODO(@crowlKats): support returning same promise
  #[async_method]
  #[getter]
  #[cppgc]
  async fn lost(&self) -> GPUDeviceLostInfo {
    if let Some(lost_receiver) = self.lost_receiver.lock().await.take() {
      let _ = lost_receiver.await;
    }

    GPUDeviceLostInfo
  }

  #[required(1)]
  fn push_error_scope(&self, #[webidl] filter: super::error::GPUErrorFilter) {
    self
      .error_handler
      .scopes
      .lock()
      .unwrap()
      .push((filter, vec![]));
  }

  #[async_method(fake)]
  #[global]
  fn pop_error_scope(
    &self,
    scope: &mut v8::HandleScope,
  ) -> Result<v8::Global<v8::Value>, JsErrorBox> {
    if self.error_handler.is_lost.get().is_some() {
      let val = v8::null(scope).cast::<v8::Value>();
      return Ok(v8::Global::new(scope, val));
    }

    let Some((_, errors)) = self.error_handler.scopes.lock().unwrap().pop()
    else {
      return Err(JsErrorBox::new(
        "DOMExceptionOperationError",
        "There are no error scopes on the error scope stack",
      ));
    };

    let val = if let Some(err) = errors.into_iter().next() {
      deno_core::error::to_v8_error(scope, &err)
    } else {
      v8::null(scope).into()
    };

    Ok(v8::Global::new(scope, val))
  }

  #[fast]
  fn start_capture(&self) {
    self.instance.device_start_capture(self.id);
  }
  #[fast]
  fn stop_capture(&self) {
    self
      .instance
      .device_poll(self.id, wgpu_types::Maintain::wait())
      .unwrap();
    self.instance.device_stop_capture(self.id);
  }
}

impl GPUDevice {
  fn new_compute_pipeline(
    &self,
    descriptor: super::compute_pipeline::GPUComputePipelineDescriptor,
  ) -> GPUComputePipeline {
    let wgpu_descriptor = wgpu_core::pipeline::ComputePipelineDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      layout: descriptor.layout.into(),
      stage: ProgrammableStageDescriptor {
        module: descriptor.compute.module.id,
        entry_point: descriptor.compute.entry_point.map(Into::into),
        constants: Cow::Owned(
          descriptor.compute.constants.into_iter().collect(),
        ),
        zero_initialize_workgroup_memory: true,
      },
      cache: None,
    };

    let (id, err) = self.instance.device_create_compute_pipeline(
      self.id,
      &wgpu_descriptor,
      None,
      None,
    );

    self.error_handler.push_error(err);

    GPUComputePipeline {
      instance: self.instance.clone(),
      error_handler: self.error_handler.clone(),
      id,
      label: descriptor.label.clone(),
    }
  }

  fn new_render_pipeline(
    &self,
    descriptor: super::render_pipeline::GPURenderPipelineDescriptor,
  ) -> Result<GPURenderPipeline, JsErrorBox> {
    let vertex = wgpu_core::pipeline::VertexState {
      stage: ProgrammableStageDescriptor {
        module: descriptor.vertex.module.id,
        entry_point: descriptor.vertex.entry_point.map(Into::into),
        constants: Cow::Owned(
          descriptor.vertex.constants.into_iter().collect(),
        ),
        zero_initialize_workgroup_memory: true,
      },
      buffers: Cow::Owned(
        descriptor
          .vertex
          .buffers
          .into_iter()
          .map(|b| {
            let layout = b.into_option().ok_or_else(|| {
              JsErrorBox::type_error(
                "Nullable GPUVertexBufferLayouts are currently not supported",
              )
            })?;

            Ok(wgpu_core::pipeline::VertexBufferLayout {
              array_stride: layout.array_stride,
              step_mode: layout.step_mode.into(),
              attributes: Cow::Owned(
                layout
                  .attributes
                  .into_iter()
                  .map(|attr| wgpu_types::VertexAttribute {
                    format: attr.format.into(),
                    offset: attr.offset,
                    shader_location: attr.shader_location,
                  })
                  .collect(),
              ),
            })
          })
          .collect::<Result<_, JsErrorBox>>()?,
      ),
    };

    let primitive = wgpu_types::PrimitiveState {
      topology: descriptor.primitive.topology.into(),
      strip_index_format: descriptor
        .primitive
        .strip_index_format
        .map(Into::into),
      front_face: descriptor.primitive.front_face.into(),
      cull_mode: descriptor.primitive.cull_mode.into(),
      unclipped_depth: descriptor.primitive.unclipped_depth,
      polygon_mode: Default::default(),
      conservative: false,
    };

    let depth_stencil = descriptor.depth_stencil.map(|depth_stencil| {
      let front = wgpu_types::StencilFaceState {
        compare: depth_stencil.stencil_front.compare.into(),
        fail_op: depth_stencil.stencil_front.fail_op.into(),
        depth_fail_op: depth_stencil.stencil_front.depth_fail_op.into(),
        pass_op: depth_stencil.stencil_front.pass_op.into(),
      };
      let back = wgpu_types::StencilFaceState {
        compare: depth_stencil.stencil_back.compare.into(),
        fail_op: depth_stencil.stencil_back.fail_op.into(),
        depth_fail_op: depth_stencil.stencil_back.depth_fail_op.into(),
        pass_op: depth_stencil.stencil_back.pass_op.into(),
      };

      wgpu_types::DepthStencilState {
        format: depth_stencil.format.into(),
        depth_write_enabled: depth_stencil
          .depth_write_enabled
          .unwrap_or_default(),
        depth_compare: depth_stencil
          .depth_compare
          .map(Into::into)
          .unwrap_or(wgpu_types::CompareFunction::Never), // TODO(wgpu): should be optional here
        stencil: wgpu_types::StencilState {
          front,
          back,
          read_mask: depth_stencil.stencil_read_mask,
          write_mask: depth_stencil.stencil_write_mask,
        },
        bias: wgpu_types::DepthBiasState {
          constant: depth_stencil.depth_bias,
          slope_scale: depth_stencil.depth_bias_slope_scale,
          clamp: depth_stencil.depth_bias_clamp,
        },
      }
    });

    let multisample = wgpu_types::MultisampleState {
      count: descriptor.multisample.count,
      mask: descriptor.multisample.mask as u64,
      alpha_to_coverage_enabled: descriptor
        .multisample
        .alpha_to_coverage_enabled,
    };

    let fragment = descriptor
      .fragment
      .map(|fragment| {
        Ok::<_, JsErrorBox>(wgpu_core::pipeline::FragmentState {
          stage: ProgrammableStageDescriptor {
            module: fragment.module.id,
            entry_point: fragment.entry_point.map(Into::into),
            constants: Cow::Owned(fragment.constants.into_iter().collect()),
            zero_initialize_workgroup_memory: true,
          },
          targets: Cow::Owned(
            fragment
              .targets
              .into_iter()
              .map(|target| {
                target
                  .into_option()
                  .map(|target| {
                    Ok(wgpu_types::ColorTargetState {
                      format: target.format.into(),
                      blend: target.blend.map(|blend| wgpu_types::BlendState {
                        color: wgpu_types::BlendComponent {
                          src_factor: blend.color.src_factor.into(),
                          dst_factor: blend.color.dst_factor.into(),
                          operation: blend.color.operation.into(),
                        },
                        alpha: wgpu_types::BlendComponent {
                          src_factor: blend.alpha.src_factor.into(),
                          dst_factor: blend.alpha.dst_factor.into(),
                          operation: blend.alpha.operation.into(),
                        },
                      }),
                      write_mask: wgpu_types::ColorWrites::from_bits(
                        target.write_mask,
                      )
                      .ok_or_else(|| {
                        JsErrorBox::type_error("usage is not valid")
                      })?,
                    })
                  })
                  .transpose()
              })
              .collect::<Result<_, JsErrorBox>>()?,
          ),
        })
      })
      .transpose()?;

    let wgpu_descriptor = wgpu_core::pipeline::RenderPipelineDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      layout: descriptor.layout.into(),
      vertex,
      primitive,
      depth_stencil,
      multisample,
      fragment,
      cache: None,
      multiview: None,
    };

    let (id, err) = self.instance.device_create_render_pipeline(
      self.id,
      &wgpu_descriptor,
      None,
      None,
    );

    self.error_handler.push_error(err);

    Ok(GPURenderPipeline {
      instance: self.instance.clone(),
      error_handler: self.error_handler.clone(),
      id,
      label: descriptor.label,
    })
  }
}

pub struct GPUDeviceLostInfo;

impl GarbageCollected for GPUDeviceLostInfo {}

#[op2]
impl GPUDeviceLostInfo {
  #[getter]
  #[string]
  fn reason(&self) -> &'static str {
    "unknown"
  }

  #[getter]
  #[string]
  fn message(&self) -> &'static str {
    "device was lost"
  }
}

#[op2(fast)]
pub fn op_webgpu_device_start_capture(#[cppgc] device: &GPUDevice) {
  device.instance.device_start_capture(device.id);
}

#[op2(fast)]
pub fn op_webgpu_device_stop_capture(#[cppgc] device: &GPUDevice) {
  device.instance.device_stop_capture(device.id);
}
