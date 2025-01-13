// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::num::NonZeroU64;

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::IntOptions;
use deno_core::webidl::Nullable;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

use crate::wrap::buffer::GPUBuffer;
use crate::wrap::texture::GPUTextureFormat;
use crate::Instance;

pub struct GPURenderBundleEncoder {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub encoder: RefCell<wgpu_core::command::RenderBundleEncoder>,
  pub label: String,
}

impl GarbageCollected for GPURenderBundleEncoder {}

#[op2]
impl GPURenderBundleEncoder {
  crate::with_label!();

  #[cppgc]
  fn finish(
    &self,
    #[webidl] descriptor: GPURenderBundleDescriptor,
  ) -> GPURenderBundle {
    let wgpu_descriptor = wgpu_core::command::RenderBundleDescriptor {
      label: Some(Cow::Owned(descriptor.label.clone())),
    };

    let (id, err) = self.instance.render_bundle_encoder_finish(
      self.encoder.into_inner(), // TODO
      &wgpu_descriptor,
      None,
    );

    self.error_handler.push_error(err);

    GPURenderBundle {
      id,
      label: descriptor.label.clone(),
    }
  }

  fn push_debug_group(&self, #[webidl] group_label: String) {
    let label = std::ffi::CString::new(group_label).unwrap();
    unsafe {
      wgpu_core::command::bundle_ffi::wgpu_render_bundle_push_debug_group(
        &mut self.encoder.borrow_mut(),
        label.as_ptr(),
      );
    }
  }

  #[fast]
  fn pop_debug_group(&self) {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_pop_debug_group(
      &mut self.encoder.borrow_mut(),
    );
  }

  fn insert_debug_marker(&self, #[webidl] marker_label: String) {
    let label = std::ffi::CString::new(marker_label).unwrap();

    unsafe {
      wgpu_core::command::bundle_ffi::wgpu_render_bundle_insert_debug_marker(
        &mut self.encoder.borrow_mut(),
        label.as_ptr(),
      );
    }
  }

  fn set_bind_group<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl/*(options(enforce_range = true))*/] index: u32,
    #[webidl] bind_group: Nullable<Ptr<crate::wrap::bind_group::GPUBindGroup>>,
    dynamic_offsets: v8::Local<'a, v8::Value>,
    dynamic_offsets_data_start: v8::Local<'a, v8::Value>,
    dynamic_offsets_data_length: v8::Local<'a, v8::Value>,
  ) {
    const PREFIX: &str =
      "Failed to execute 'setBindGroup' on 'GPUComputePassEncoder'";
    let offsets =
      if let Ok(uint_32) = dynamic_offsets.try_cast::<v8::Uint32Array>() {
        let start = u64::convert(
          scope,
          dynamic_offsets,
          Cow::Borrowed(PREFIX),
          (|| Cow::Borrowed("Argument 4")).into(),
          &IntOptions {
            clamp: false,
            enforce_range: true,
          },
        )
        .unwrap(); // TODO: dont unwrap err
        let len = u32::convert(
          scope,
          dynamic_offsets,
          Cow::Borrowed(PREFIX),
          (|| Cow::Borrowed("Argument 5")).into(),
          &IntOptions {
            clamp: false,
            enforce_range: true,
          },
        )
        .unwrap(); // TODO: dont unwrap err

        // TODO

        vec![]
      } else {
        <Option<Vec<u32>>>::convert(
          scope,
          dynamic_offsets,
          Cow::Borrowed(PREFIX),
          (|| Cow::Borrowed("Argument 3")).into(),
          &IntOptions {
            clamp: false,
            enforce_range: true,
          },
        )
        .unwrap() // TODO: dont unwrap err
        .unwrap_or_default()
      };

    unsafe {
      wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
        &mut self.encoder.borrow_mut(),
        index,
        bind_group.into_option().map(|bind_group| bind_group.id),
        offsets.as_ptr(),
        offsets.len(),
      );
    }
  }

  fn set_pipeline(
    &self,
    #[webidl] pipeline: Ptr<crate::wrap::render_pipeline::GPURenderPipeline>,
  ) {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_pipeline(
      &mut self.encoder.borrow_mut(),
      pipeline.id,
    );
  }

  #[required(2)]
  fn set_index_buffer(
    &self,
    #[webidl] buffer: Ptr<GPUBuffer>,
    #[webidl] index_format: crate::wrap::render_pipeline::GPUIndexFormat,
    #[webidl/*(default = 0, options(enforce_range = true))*/] offset: u64,
    #[webidl/*(options(enforce_range = true))*/] size: Option<u64>,
  ) {
    self.encoder.borrow_mut().set_index_buffer(
      buffer.id,
      index_format.into(),
      offset,
      size.and_then(NonZeroU64::new),
    );
  }

  #[required(2)]
  fn set_vertex_buffer(
    &self,
    #[webidl/*(options(enforce_range = true))*/] slot: u32,
    #[webidl] buffer: Ptr<GPUBuffer>, // TODO: support nullable buffer
    #[webidl/*(default = 0, options(enforce_range = true))*/] offset: u64,
    #[webidl/*(options(enforce_range = true))*/] size: Option<u64>,
  ) {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_vertex_buffer(
      &mut self.encoder.borrow_mut(),
      slot,
      buffer.id,
      offset,
      size.and_then(NonZeroU64::new),
    );
  }

  #[required(1)]
  fn draw(
    &self,
    #[webidl/*(options(enforce_range = true))*/] vertex_count: u32,
    #[webidl/*(default = 1, options(enforce_range = true))*/]
    instance_count: u32,
    #[webidl/*(default = 0, options(enforce_range = true))*/] first_vertex: u32,
    #[webidl/*(default = 0, options(enforce_range = true))*/]
    first_instance: u32,
  ) {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw(
      &mut self.encoder.borrow_mut(),
      vertex_count,
      instance_count,
      first_vertex,
      first_instance,
    );
  }

  #[required(1)]
  fn draw_indexed(
    &self,
    #[webidl/*(options(enforce_range = true))*/] index_count: u32,
    #[webidl/*(default = 1, options(enforce_range = true))*/]
    instance_count: u32,
    #[webidl/*(default = 0, options(enforce_range = true))*/] first_index: u32,
    #[webidl/*(default = 0, options(enforce_range = true))*/] base_vertex: i32,
    #[webidl/*(default = 0, options(enforce_range = true))*/]
    first_instance: u32,
  ) {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indexed(
      &mut self.encoder.borrow_mut(),
      index_count,
      instance_count,
      first_index,
      base_vertex,
      first_instance,
    );
  }

  #[required(2)]
  fn draw_indirect(
    &self,
    #[webidl] indirect_buffer: Ptr<GPUBuffer>,
    #[webidl/*(options(enforce_range = true))*/] indirect_offset: u64,
  ) {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indirect(
      &mut self.encoder.borrow_mut(),
      indirect_buffer.id,
      indirect_offset,
    );
  }

  #[required(2)]
  fn draw_indexed_indirect(
    &self,
    #[webidl] indirect_buffer: Ptr<GPUBuffer>,
    #[webidl/*(options(enforce_range = true))*/] indirect_offset: u64,
  ) {
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indexed_indirect(
      &mut self.encoder.borrow_mut(),
      indirect_buffer.id,
      indirect_offset,
    );
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURenderBundleEncoderDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub color_formats: Vec<Nullable<GPUTextureFormat>>,
  pub depth_stencil_format: Option<GPUTextureFormat>,
  #[webidl(default = 1)]
  #[options(enforce_range = true)]
  pub sample_count: u32,

  #[webidl(default = false)]
  pub depth_read_only: bool,
  #[webidl(default = false)]
  pub stencil_read_only: bool,
}

pub struct GPURenderBundle {
  pub id: wgpu_core::id::RenderBundleId,
  pub label: String,
}

impl WebIdlInterfaceConverter for GPURenderBundle {
  const NAME: &'static str = "GPURenderBundle";
}

impl GarbageCollected for GPURenderBundle {}

#[op2]
impl GPURenderBundle {
  crate::with_label!();
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURenderBundleDescriptor {
  #[webidl(default = String::new())]
  pub label: String,
}
