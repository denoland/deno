// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::cppgc::Ref;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::IntOptions;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_error::JsErrorBox;
use wgpu_core::command::PassChannel;
use wgpu_types::BufferAddress;
use wgpu_types::TexelCopyBufferInfo;

use crate::Instance;
use crate::buffer::GPUBuffer;
use crate::command_buffer::GPUCommandBuffer;
use crate::compute_pass::GPUComputePassEncoder;
use crate::error::GPUGenericError;
use crate::queue::GPUTexelCopyTextureInfo;
use crate::render_pass::GPULoadOp;
use crate::render_pass::GPURenderPassEncoder;
use crate::webidl::GPUExtent3D;

pub struct GPUCommandEncoder {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub id: wgpu_core::id::CommandEncoderId,
  pub label: String,
}

impl Drop for GPUCommandEncoder {
  fn drop(&mut self) {
    self.instance.command_encoder_drop(self.id);
  }
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUCommandEncoder {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUCommandEncoder"
  }
}

#[op2]
impl GPUCommandEncoder {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUCommandEncoder, GPUGenericError> {
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

  #[required(1)]
  #[cppgc]
  fn begin_render_pass(
    &self,
    #[webidl] descriptor: crate::render_pass::GPURenderPassDescriptor,
  ) -> Result<GPURenderPassEncoder, JsErrorBox> {
    let color_attachments = Cow::Owned(
      descriptor
        .color_attachments
        .into_iter()
        .map(|attachment| {
          attachment.into_option().map(|attachment| {
            wgpu_core::command::RenderPassColorAttachment {
              view: attachment.view.to_view_id(),
              depth_slice: attachment.depth_slice,
              resolve_target: attachment
                .resolve_target
                .map(|target| target.to_view_id()),
              load_op: attachment
                .load_op
                .with_default_value(attachment.clear_value.map(Into::into)),
              store_op: attachment.store_op.into(),
            }
          })
        })
        .collect::<Vec<_>>(),
    );

    let depth_stencil_attachment = descriptor
            .depth_stencil_attachment
            .map(|attachment| {
                if attachment
                    .depth_load_op
                    .as_ref()
                    .is_some_and(|op| matches!(op, GPULoadOp::Clear))
                    && attachment.depth_clear_value.is_none()
                {
                    return Err(JsErrorBox::type_error(
                        r#"'depthClearValue' must be specified when 'depthLoadOp' is "clear""#,
                    ));
                }

                Ok(wgpu_core::command::RenderPassDepthStencilAttachment {
                    view: attachment.view.to_view_id(),
                    depth: PassChannel {
                        load_op: attachment
                            .depth_load_op
                            .map(|load_op| load_op.with_value(attachment.depth_clear_value)),
                        store_op: attachment.depth_store_op.map(Into::into),
                        read_only: attachment.depth_read_only,
                    },
                    stencil: PassChannel {
                        load_op: attachment.stencil_load_op.map(|load_op| {
                            load_op.with_value(Some(attachment.stencil_clear_value))
                        }),
                        store_op: attachment.stencil_store_op.map(Into::into),
                        read_only: attachment.stencil_read_only,
                    },
                })
            })
            .transpose()?;

    let timestamp_writes =
      descriptor.timestamp_writes.map(|timestamp_writes| {
        wgpu_core::command::PassTimestampWrites {
          query_set: timestamp_writes.query_set.id,
          beginning_of_pass_write_index: timestamp_writes
            .beginning_of_pass_write_index,
          end_of_pass_write_index: timestamp_writes.end_of_pass_write_index,
        }
      });

    let wgpu_descriptor = wgpu_core::command::RenderPassDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      color_attachments,
      depth_stencil_attachment: depth_stencil_attachment.as_ref(),
      timestamp_writes: timestamp_writes.as_ref(),
      occlusion_query_set: descriptor
        .occlusion_query_set
        .map(|query_set| query_set.id),
    };

    let (render_pass, err) = self
      .instance
      .command_encoder_begin_render_pass(self.id, &wgpu_descriptor);

    self.error_handler.push_error(err);

    Ok(GPURenderPassEncoder {
      instance: self.instance.clone(),
      error_handler: self.error_handler.clone(),
      render_pass: RefCell::new(render_pass),
      label: descriptor.label,
    })
  }

  #[cppgc]
  fn begin_compute_pass(
    &self,
    #[webidl] descriptor: crate::compute_pass::GPUComputePassDescriptor,
  ) -> GPUComputePassEncoder {
    let timestamp_writes =
      descriptor.timestamp_writes.map(|timestamp_writes| {
        wgpu_core::command::PassTimestampWrites {
          query_set: timestamp_writes.query_set.id,
          beginning_of_pass_write_index: timestamp_writes
            .beginning_of_pass_write_index,
          end_of_pass_write_index: timestamp_writes.end_of_pass_write_index,
        }
      });

    let wgpu_descriptor = wgpu_core::command::ComputePassDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
      timestamp_writes,
    };

    let (compute_pass, err) = self
      .instance
      .command_encoder_begin_compute_pass(self.id, &wgpu_descriptor);

    self.error_handler.push_error(err);

    GPUComputePassEncoder {
      instance: self.instance.clone(),
      error_handler: self.error_handler.clone(),
      compute_pass: RefCell::new(compute_pass),
      label: descriptor.label,
    }
  }

  #[required(2)]
  #[undefined]
  fn copy_buffer_to_buffer<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] source: Ref<GPUBuffer>,
    arg2: v8::Local<'a, v8::Value>,
    arg3: v8::Local<'a, v8::Value>,
    arg4: v8::Local<'a, v8::Value>,
    arg5: v8::Local<'a, v8::Value>,
  ) -> Result<(), WebIdlError> {
    let prefix = "Failed to execute 'GPUCommandEncoder.copyBufferToBuffer'";
    let int_options = IntOptions {
      clamp: false,
      enforce_range: true,
    };

    let source_offset: BufferAddress;
    let destination: Ref<GPUBuffer>;
    let destination_offset: BufferAddress;
    let size: Option<BufferAddress>;
    // Note that the last argument to either overload of `copy_buffer_to_buffer`
    // is optional, so `arg5.is_undefined()` would not work here.
    if arg4.is_undefined() {
      // 3-argument overload
      source_offset = 0;
      destination = Ref::<GPUBuffer>::convert(
        scope,
        arg2,
        Cow::Borrowed(prefix),
        (|| Cow::Borrowed("destination")).into(),
        &(),
      )?;
      destination_offset = 0;
      size = <Option<u64>>::convert(
        scope,
        arg3,
        Cow::Borrowed(prefix),
        (|| Cow::Borrowed("size")).into(),
        &int_options,
      )?;
    } else {
      // 5-argument overload
      source_offset = u64::convert(
        scope,
        arg2,
        Cow::Borrowed(prefix),
        (|| Cow::Borrowed("sourceOffset")).into(),
        &int_options,
      )?;
      destination = Ref::<GPUBuffer>::convert(
        scope,
        arg3,
        Cow::Borrowed(prefix),
        (|| Cow::Borrowed("destination")).into(),
        &(),
      )?;
      destination_offset = u64::convert(
        scope,
        arg4,
        Cow::Borrowed(prefix),
        (|| Cow::Borrowed("destinationOffset")).into(),
        &int_options,
      )?;
      size = <Option<u64>>::convert(
        scope,
        arg5,
        Cow::Borrowed(prefix),
        (|| Cow::Borrowed("size")).into(),
        &int_options,
      )?;
    }

    let err = self
      .instance
      .command_encoder_copy_buffer_to_buffer(
        self.id,
        source.id,
        source_offset,
        destination.id,
        destination_offset,
        size,
      )
      .err();

    self.error_handler.push_error(err);

    Ok(())
  }

  #[required(3)]
  #[undefined]
  fn copy_buffer_to_texture(
    &self,
    #[webidl] source: GPUTexelCopyBufferInfo,
    #[webidl] destination: GPUTexelCopyTextureInfo,
    #[webidl] copy_size: GPUExtent3D,
  ) {
    let source = TexelCopyBufferInfo {
      buffer: source.buffer.id,
      layout: wgpu_types::TexelCopyBufferLayout {
        offset: source.offset,
        bytes_per_row: source.bytes_per_row,
        rows_per_image: source.rows_per_image,
      },
    };
    let destination = wgpu_types::TexelCopyTextureInfo {
      texture: destination.texture.id,
      mip_level: destination.mip_level,
      origin: destination.origin.into(),
      aspect: destination.aspect.into(),
    };

    let err = self
      .instance
      .command_encoder_copy_buffer_to_texture(
        self.id,
        &source,
        &destination,
        &copy_size.into(),
      )
      .err();

    self.error_handler.push_error(err);
  }

  #[required(3)]
  #[undefined]
  fn copy_texture_to_buffer(
    &self,
    #[webidl] source: GPUTexelCopyTextureInfo,
    #[webidl] destination: GPUTexelCopyBufferInfo,
    #[webidl] copy_size: GPUExtent3D,
  ) {
    let source = wgpu_types::TexelCopyTextureInfo {
      texture: source.texture.id,
      mip_level: source.mip_level,
      origin: source.origin.into(),
      aspect: source.aspect.into(),
    };
    let destination = TexelCopyBufferInfo {
      buffer: destination.buffer.id,
      layout: wgpu_types::TexelCopyBufferLayout {
        offset: destination.offset,
        bytes_per_row: destination.bytes_per_row,
        rows_per_image: destination.rows_per_image,
      },
    };

    let err = self
      .instance
      .command_encoder_copy_texture_to_buffer(
        self.id,
        &source,
        &destination,
        &copy_size.into(),
      )
      .err();

    self.error_handler.push_error(err);
  }

  #[required(3)]
  #[undefined]
  fn copy_texture_to_texture(
    &self,
    #[webidl] source: GPUTexelCopyTextureInfo,
    #[webidl] destination: GPUTexelCopyTextureInfo,
    #[webidl] copy_size: GPUExtent3D,
  ) {
    let source = wgpu_types::TexelCopyTextureInfo {
      texture: source.texture.id,
      mip_level: source.mip_level,
      origin: source.origin.into(),
      aspect: source.aspect.into(),
    };
    let destination = wgpu_types::TexelCopyTextureInfo {
      texture: destination.texture.id,
      mip_level: destination.mip_level,
      origin: destination.origin.into(),
      aspect: destination.aspect.into(),
    };

    let err = self
      .instance
      .command_encoder_copy_texture_to_texture(
        self.id,
        &source,
        &destination,
        &copy_size.into(),
      )
      .err();

    self.error_handler.push_error(err);
  }

  #[required(1)]
  #[undefined]
  fn clear_buffer(
    &self,
    #[webidl] buffer: Ref<GPUBuffer>,
    #[webidl(default = 0, options(enforce_range = true))] offset: u64,
    #[webidl(options(enforce_range = true))] size: Option<u64>,
  ) {
    let err = self
      .instance
      .command_encoder_clear_buffer(self.id, buffer.id, offset, size)
      .err();
    self.error_handler.push_error(err);
  }

  #[required(5)]
  #[undefined]
  fn resolve_query_set(
    &self,
    #[webidl] query_set: Ref<super::query_set::GPUQuerySet>,
    #[webidl(options(enforce_range = true))] first_query: u32,
    #[webidl(options(enforce_range = true))] query_count: u32,
    #[webidl] destination: Ref<GPUBuffer>,
    #[webidl(options(enforce_range = true))] destination_offset: u64,
  ) {
    let err = self
      .instance
      .command_encoder_resolve_query_set(
        self.id,
        query_set.id,
        first_query,
        query_count,
        destination.id,
        destination_offset,
      )
      .err();

    self.error_handler.push_error(err);
  }

  #[cppgc]
  fn finish(
    &self,
    #[webidl] descriptor: crate::command_buffer::GPUCommandBufferDescriptor,
  ) -> GPUCommandBuffer {
    let wgpu_descriptor = wgpu_types::CommandBufferDescriptor {
      label: crate::transform_label(descriptor.label.clone()),
    };

    let (id, err) =
      self
        .instance
        .command_encoder_finish(self.id, &wgpu_descriptor, None);

    self.error_handler.push_error(err);

    GPUCommandBuffer {
      instance: self.instance.clone(),
      id,
      label: descriptor.label,
    }
  }

  fn push_debug_group(&self, #[webidl] group_label: String) {
    let err = self
      .instance
      .command_encoder_push_debug_group(self.id, &group_label)
      .err();
    self.error_handler.push_error(err);
  }

  #[fast]
  fn pop_debug_group(&self) {
    let err = self.instance.command_encoder_pop_debug_group(self.id).err();
    self.error_handler.push_error(err);
  }

  fn insert_debug_marker(&self, #[webidl] marker_label: String) {
    let err = self
      .instance
      .command_encoder_insert_debug_marker(self.id, &marker_label)
      .err();
    self.error_handler.push_error(err);
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUCommandEncoderDescriptor {
  #[webidl(default = String::new())]
  pub label: String,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUTexelCopyBufferInfo {
  pub buffer: Ref<GPUBuffer>,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  offset: u64,
  #[options(enforce_range = true)]
  bytes_per_row: Option<u32>,
  #[options(enforce_range = true)]
  rows_per_image: Option<u32>,
}
