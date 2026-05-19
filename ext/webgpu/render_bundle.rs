// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::num::NonZeroU64;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::cppgc::Ref;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::IntOptions;
use deno_core::webidl::Nullable;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_error::JsErrorBox;

use crate::Instance;
use crate::buffer::GPUBuffer;
use crate::error::GPUGenericError;
use crate::texture::GPUTextureFormat;

fn c_string_truncated_at_first_nul<T: Into<Vec<u8>>>(
  src: T,
) -> std::ffi::CString {
  std::ffi::CString::new(src).unwrap_or_else(|err| {
    let nul_pos = err.nul_position();
    std::ffi::CString::new(err.into_vec().split_at(nul_pos).0).unwrap()
  })
}

pub struct GPURenderBundleEncoder {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub encoder: RefCell<Option<wgpu_core::command::RenderBundleEncoder>>,
  pub label: String,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPURenderBundleEncoder {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPURenderBundleEncoder"
  }
}

#[op2]
impl GPURenderBundleEncoder {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPURenderBundleEncoder, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

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

  #[cppgc]
  fn finish(
    &self,
    #[webidl] descriptor: GPURenderBundleDescriptor,
  ) -> GPURenderBundle {
    let wgpu_descriptor = wgpu_core::command::RenderBundleDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
    };

    let (id, err) = self.instance.render_bundle_encoder_finish(
      self.encoder.borrow_mut().take().unwrap(),
      &wgpu_descriptor,
      None,
    );

    self.error_handler.push_error(err);

    GPURenderBundle {
      instance: self.instance.clone(),
      id,
      label: descriptor.label.clone(),
    }
  }

  #[undefined]
  fn push_debug_group(
    &self,
    #[webidl] group_label: String,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    let label = c_string_truncated_at_first_nul(group_label);
    // SAFETY: the string the raw pointer points to lives longer than the below
    // function invocation.
    unsafe {
      wgpu_core::command::bundle_ffi::wgpu_render_bundle_push_debug_group(
        encoder,
        label.as_ptr(),
      );
    }

    Ok(())
  }

  #[fast]
  #[undefined]
  fn pop_debug_group(&self) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;
    wgpu_core::command::bundle_ffi::wgpu_render_bundle_pop_debug_group(encoder);
    Ok(())
  }

  #[undefined]
  fn insert_debug_marker(
    &self,
    #[webidl] marker_label: String,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    let label = c_string_truncated_at_first_nul(marker_label);
    // SAFETY: the string the raw pointer points to lives longer than the below
    // function invocation.
    unsafe {
      wgpu_core::command::bundle_ffi::wgpu_render_bundle_insert_debug_marker(
        encoder,
        label.as_ptr(),
      );
    }
    Ok(())
  }

  #[undefined]
  fn set_bind_group<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl(options(enforce_range = true))] index: u32,
    #[webidl] bind_group: Nullable<Ref<crate::bind_group::GPUBindGroup>>,
    dynamic_offsets: v8::Local<'a, v8::Value>,
    dynamic_offsets_data_start: v8::Local<'a, v8::Value>,
    dynamic_offsets_data_length: v8::Local<'a, v8::Value>,
  ) -> Result<(), SetBindGroupError> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    const PREFIX: &str =
      "Failed to execute 'setBindGroup' on 'GPUComputePassEncoder'";
    if let Ok(uint_32) = dynamic_offsets.try_cast::<v8::Uint32Array>() {
      let start = u64::convert(
        scope,
        dynamic_offsets_data_start,
        Cow::Borrowed(PREFIX),
        (|| Cow::Borrowed("Argument 4")).into(),
        &IntOptions {
          clamp: false,
          enforce_range: true,
        },
      )? as usize;
      let len = u32::convert(
        scope,
        dynamic_offsets_data_length,
        Cow::Borrowed(PREFIX),
        (|| Cow::Borrowed("Argument 5")).into(),
        &IntOptions {
          clamp: false,
          enforce_range: true,
        },
      )? as usize;

      let view_byte_offset = uint_32.byte_offset();
      let view_len = uint_32.length();

      // Validate `start..start+len` against the **view** length, not the
      // backing buffer's length. Without this check, `&data[start..end]`
      // below would panic on out-of-range input; the panic crosses the
      // op's `extern "C"` boundary and aborts the process. See #33956.
      let Some(end) = start.checked_add(len).filter(|end| *end <= view_len)
      else {
        return Err(JsErrorBox::generic(format!(
          "{PREFIX}: dynamicOffsetsDataStart + dynamicOffsetsDataLength ({start} + {len}) is outside the bounds of dynamicOffsetsData (length {view_len})",
        )).into());
      };

      let ab = uint_32.buffer(scope).unwrap();
      let ptr = ab.data().unwrap();

      // SAFETY: `ptr` is the start of the backing ArrayBuffer's data; the
      // Uint32Array constructor guarantees `byte_offset + view_len * 4`
      // fits within `byte_length`, so the resulting slice covers exactly
      // the view's window. `data` is dropped at the end of this call;
      // `ab` keeps the backing buffer alive for that duration.
      let data = unsafe {
        std::slice::from_raw_parts(
          (ptr.as_ptr() as *const u8).add(view_byte_offset) as *const u32,
          view_len,
        )
      };

      let offsets = &data[start..end];

      // SAFETY: wgpu FFI call
      unsafe {
        wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
          encoder,
          index,
          bind_group.into_option().map(|bind_group| bind_group.id),
          offsets.as_ptr(),
          offsets.len(),
        );
      }
    } else {
      let offsets = <Option<Vec<u32>>>::convert(
        scope,
        dynamic_offsets,
        Cow::Borrowed(PREFIX),
        (|| Cow::Borrowed("Argument 3")).into(),
        &IntOptions {
          clamp: false,
          enforce_range: true,
        },
      )?
      .unwrap_or_default();

      // SAFETY: wgpu FFI call
      unsafe {
        wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_bind_group(
          encoder,
          index,
          bind_group.into_option().map(|bind_group| bind_group.id),
          offsets.as_ptr(),
          offsets.len(),
        );
      }
    }

    Ok(())
  }

  #[undefined]
  fn set_pipeline(
    &self,
    #[webidl] pipeline: Ref<crate::render_pipeline::GPURenderPipeline>,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_pipeline(
      encoder,
      pipeline.id,
    );
    Ok(())
  }

  #[required(2)]
  #[undefined]
  fn set_index_buffer(
    &self,
    #[webidl] buffer: Ref<GPUBuffer>,
    #[webidl] index_format: crate::render_pipeline::GPUIndexFormat,
    #[webidl(default = 0, options(enforce_range = true))] offset: u64,
    #[webidl(options(enforce_range = true))] size: Option<u64>,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    encoder.set_index_buffer(
      buffer.id,
      index_format.into(),
      offset,
      size.and_then(NonZeroU64::new),
    );
    Ok(())
  }

  #[required(2)]
  #[undefined]
  fn set_vertex_buffer(
    &self,
    #[webidl(options(enforce_range = true))] slot: u32,
    #[webidl] buffer: Ref<GPUBuffer>, // TODO(wgpu): support nullable buffer
    #[webidl(default = 0, options(enforce_range = true))] offset: u64,
    #[webidl(options(enforce_range = true))] size: Option<u64>,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    wgpu_core::command::bundle_ffi::wgpu_render_bundle_set_vertex_buffer(
      encoder,
      slot,
      buffer.id,
      offset,
      size.and_then(NonZeroU64::new),
    );
    Ok(())
  }

  #[required(1)]
  #[undefined]
  fn draw(
    &self,
    #[webidl(options(enforce_range = true))] vertex_count: u32,
    #[webidl(default = 1, options(enforce_range = true))] instance_count: u32,
    #[webidl(default = 0, options(enforce_range = true))] first_vertex: u32,
    #[webidl(default = 0, options(enforce_range = true))] first_instance: u32,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw(
      encoder,
      vertex_count,
      instance_count,
      first_vertex,
      first_instance,
    );
    Ok(())
  }

  #[required(1)]
  #[undefined]
  fn draw_indexed(
    &self,
    #[webidl(options(enforce_range = true))] index_count: u32,
    #[webidl(default = 1, options(enforce_range = true))] instance_count: u32,
    #[webidl(default = 0, options(enforce_range = true))] first_index: u32,
    #[webidl(default = 0, options(enforce_range = true))] base_vertex: i32,
    #[webidl(default = 0, options(enforce_range = true))] first_instance: u32,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indexed(
      encoder,
      index_count,
      instance_count,
      first_index,
      base_vertex,
      first_instance,
    );
    Ok(())
  }

  #[required(2)]
  #[undefined]
  fn draw_indirect(
    &self,
    #[webidl] indirect_buffer: Ref<GPUBuffer>,
    #[webidl(options(enforce_range = true))] indirect_offset: u64,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indirect(
      encoder,
      indirect_buffer.id,
      indirect_offset,
    );
    Ok(())
  }

  #[required(2)]
  #[undefined]
  fn draw_indexed_indirect(
    &self,
    #[webidl] indirect_buffer: Ref<GPUBuffer>,
    #[webidl(options(enforce_range = true))] indirect_offset: u64,
  ) -> Result<(), JsErrorBox> {
    let mut encoder = self.encoder.borrow_mut();
    let encoder = encoder.as_mut().ok_or_else(|| {
      JsErrorBox::generic("Encoder has already been finished")
    })?;

    wgpu_core::command::bundle_ffi::wgpu_render_bundle_draw_indexed_indirect(
      encoder,
      indirect_buffer.id,
      indirect_offset,
    );
    Ok(())
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

#[derive(Debug, thiserror::Error, deno_error::JsError)]
enum SetBindGroupError {
  #[class(inherit)]
  #[error(transparent)]
  WebIDL(#[from] WebIdlError),
  #[class(inherit)]
  #[error(transparent)]
  Other(#[from] JsErrorBox),
}

pub struct GPURenderBundle {
  pub instance: Instance,
  pub id: wgpu_core::id::RenderBundleId,
  pub label: String,
}

impl Drop for GPURenderBundle {
  fn drop(&mut self) {
    self.instance.render_bundle_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPURenderBundle {
  const NAME: &'static str = "GPURenderBundle";
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPURenderBundle {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPURenderBundle"
  }
}

#[op2]
impl GPURenderBundle {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPURenderBundle, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

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
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURenderBundleDescriptor {
  #[webidl(default = String::new())]
  pub label: String,
}
