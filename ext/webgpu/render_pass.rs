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
use deno_core::webidl::WebIdlError;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

use crate::buffer::GPUBuffer;
use crate::render_bundle::GPURenderBundle;
use crate::texture::GPUTextureView;
use crate::webidl::GPUColor;
use crate::Instance;

pub struct GPURenderPassEncoder {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub render_pass: RefCell<wgpu_core::command::RenderPass>,
  pub label: String,
}

impl GarbageCollected for GPURenderPassEncoder {}

#[op2]
impl GPURenderPassEncoder {
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

  #[required(6)]
  fn set_viewport(
    &self,
    #[webidl] x: f32,
    #[webidl] y: f32,
    #[webidl] width: f32,
    #[webidl] height: f32,
    #[webidl] min_depth: f32,
    #[webidl] max_depth: f32,
  ) {
    let err = self
      .instance
      .render_pass_set_viewport(
        &mut self.render_pass.borrow_mut(),
        x,
        y,
        width,
        height,
        min_depth,
        max_depth,
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(4)]
  fn set_scissor_rect(
    &self,
    #[webidl(options(enforce_range = true))] x: u32,
    #[webidl(options(enforce_range = true))] y: u32,
    #[webidl(options(enforce_range = true))] width: u32,
    #[webidl(options(enforce_range = true))] height: u32,
  ) {
    let err = self
      .instance
      .render_pass_set_scissor_rect(
        &mut self.render_pass.borrow_mut(),
        x,
        y,
        width,
        height,
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(1)]
  fn set_blend_constant(&self, #[webidl] color: GPUColor) {
    let err = self
      .instance
      .render_pass_set_blend_constant(
        &mut self.render_pass.borrow_mut(),
        color.into(),
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(1)]
  fn set_stencil_reference(
    &self,
    #[webidl(options(enforce_range = true))] reference: u32,
  ) {
    let err = self
      .instance
      .render_pass_set_stencil_reference(
        &mut self.render_pass.borrow_mut(),
        reference,
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(1)]
  fn begin_occlusion_query(
    &self,
    #[webidl(options(enforce_range = true))] query_index: u32,
  ) {
    let err = self
      .instance
      .render_pass_begin_occlusion_query(
        &mut self.render_pass.borrow_mut(),
        query_index,
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[fast]
  fn end_occlusion_query(&self) {
    let err = self
      .instance
      .render_pass_end_occlusion_query(&mut self.render_pass.borrow_mut())
      .err();
    self.error_handler.push_error(err);
  }

  #[required(1)]
  fn execute_bundles(&self, #[webidl] bundles: Vec<Ptr<GPURenderBundle>>) {
    let err = self
      .instance
      .render_pass_execute_bundles(
        &mut self.render_pass.borrow_mut(),
        &bundles
          .into_iter()
          .map(|bundle| bundle.id)
          .collect::<Vec<_>>(),
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[fast]
  fn end(&self) {
    let err = self
      .instance
      .render_pass_end(&mut self.render_pass.borrow_mut())
      .err();
    self.error_handler.push_error(err);
  }

  fn push_debug_group(&self, #[webidl] group_label: String) {
    let err = self
      .instance
      .render_pass_push_debug_group(
        &mut self.render_pass.borrow_mut(),
        &group_label,
        0, // wgpu#975
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[fast]
  fn pop_debug_group(&self) {
    let err = self
      .instance
      .render_pass_pop_debug_group(&mut self.render_pass.borrow_mut())
      .err();
    self.error_handler.push_error(err);
  }

  fn insert_debug_marker(&self, #[webidl] marker_label: String) {
    let err = self
      .instance
      .render_pass_insert_debug_marker(
        &mut self.render_pass.borrow_mut(),
        &marker_label,
        0, // wgpu#975
      )
      .err();
    self.error_handler.push_error(err);
  }

  fn set_bind_group<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
    #[webidl(options(enforce_range = true))] index: u32,
    #[webidl] bind_group: Nullable<Ptr<crate::bind_group::GPUBindGroup>>,
    dynamic_offsets: v8::Local<'a, v8::Value>,
    dynamic_offsets_data_start: v8::Local<'a, v8::Value>,
    dynamic_offsets_data_length: v8::Local<'a, v8::Value>,
  ) -> Result<(), WebIdlError> {
    const PREFIX: &str =
      "Failed to execute 'setBindGroup' on 'GPUComputePassEncoder'";

    let err = if let Ok(uint_32) = dynamic_offsets.try_cast::<v8::Uint32Array>()
    {
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

      let ab = uint_32.buffer(scope).unwrap();
      let ptr = ab.data().unwrap();
      let ab_len = ab.byte_length() / 4;

      // SAFETY: created from an array buffer, slice is dropped at end of function call
      let data =
        unsafe { std::slice::from_raw_parts(ptr.as_ptr() as _, ab_len) };

      let offsets = &data[start..(start + len)];

      self
        .instance
        .render_pass_set_bind_group(
          &mut self.render_pass.borrow_mut(),
          index,
          bind_group.into_option().map(|bind_group| bind_group.id),
          offsets,
        )
        .err()
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

      self
        .instance
        .render_pass_set_bind_group(
          &mut self.render_pass.borrow_mut(),
          index,
          bind_group.into_option().map(|bind_group| bind_group.id),
          &offsets,
        )
        .err()
    };

    self.error_handler.push_error(err);

    Ok(())
  }

  fn set_pipeline(
    &self,
    #[webidl] pipeline: Ptr<crate::render_pipeline::GPURenderPipeline>,
  ) {
    let err = self
      .instance
      .render_pass_set_pipeline(&mut self.render_pass.borrow_mut(), pipeline.id)
      .err();
    self.error_handler.push_error(err);
  }

  #[required(2)]
  fn set_index_buffer(
    &self,
    #[webidl] buffer: Ptr<GPUBuffer>,
    #[webidl] index_format: crate::render_pipeline::GPUIndexFormat,
    #[webidl(default = 0, options(enforce_range = true))] offset: u64,
    #[webidl(options(enforce_range = true))] size: Option<u64>,
  ) {
    let err = self
      .instance
      .render_pass_set_index_buffer(
        &mut self.render_pass.borrow_mut(),
        buffer.id,
        index_format.into(),
        offset,
        size.and_then(NonZeroU64::new),
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(2)]
  fn set_vertex_buffer(
    &self,
    #[webidl(options(enforce_range = true))] slot: u32,
    #[webidl] buffer: Ptr<GPUBuffer>, // TODO(wgpu): support nullable buffer
    #[webidl(default = 0, options(enforce_range = true))] offset: u64,
    #[webidl(options(enforce_range = true))] size: Option<u64>,
  ) {
    let err = self
      .instance
      .render_pass_set_vertex_buffer(
        &mut self.render_pass.borrow_mut(),
        slot,
        buffer.id,
        offset,
        size.and_then(NonZeroU64::new),
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(1)]
  fn draw(
    &self,
    #[webidl(options(enforce_range = true))] vertex_count: u32,
    #[webidl(default = 1, options(enforce_range = true))] instance_count: u32,
    #[webidl(default = 0, options(enforce_range = true))] first_vertex: u32,
    #[webidl(default = 0, options(enforce_range = true))] first_instance: u32,
  ) {
    let err = self
      .instance
      .render_pass_draw(
        &mut self.render_pass.borrow_mut(),
        vertex_count,
        instance_count,
        first_vertex,
        first_instance,
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(1)]
  fn draw_indexed(
    &self,
    #[webidl(options(enforce_range = true))] index_count: u32,
    #[webidl(default = 1, options(enforce_range = true))] instance_count: u32,
    #[webidl(default = 0, options(enforce_range = true))] first_index: u32,
    #[webidl(default = 0, options(enforce_range = true))] base_vertex: i32,
    #[webidl(default = 0, options(enforce_range = true))] first_instance: u32,
  ) {
    let err = self
      .instance
      .render_pass_draw_indexed(
        &mut self.render_pass.borrow_mut(),
        index_count,
        instance_count,
        first_index,
        base_vertex,
        first_instance,
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(2)]
  fn draw_indirect(
    &self,
    #[webidl] indirect_buffer: Ptr<GPUBuffer>,
    #[webidl(options(enforce_range = true))] indirect_offset: u64,
  ) {
    let err = self
      .instance
      .render_pass_draw_indirect(
        &mut self.render_pass.borrow_mut(),
        indirect_buffer.id,
        indirect_offset,
      )
      .err();
    self.error_handler.push_error(err);
  }

  #[required(2)]
  fn draw_indexed_indirect(
    &self,
    #[webidl] indirect_buffer: Ptr<GPUBuffer>,
    #[webidl(options(enforce_range = true))] indirect_offset: u64,
  ) {
    let err = self
      .instance
      .render_pass_draw_indexed_indirect(
        &mut self.render_pass.borrow_mut(),
        indirect_buffer.id,
        indirect_offset,
      )
      .err();
    self.error_handler.push_error(err);
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURenderPassDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub color_attachments: Vec<Nullable<GPURenderPassColorAttachment>>,
  pub depth_stencil_attachment: Option<GPURenderPassDepthStencilAttachment>,
  pub occlusion_query_set: Option<Ptr<crate::query_set::GPUQuerySet>>,
  pub timestamp_writes: Option<GPURenderPassTimestampWrites>,
  /*#[webidl(default = 50000000)]
  #[options(enforce_range = true)]
  pub max_draw_count: u64,*/
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURenderPassColorAttachment {
  pub view: Ptr<GPUTextureView>,
  /*#[options(enforce_range = true)]
  pub depth_slice: Option<u32>,*/
  pub resolve_target: Option<Ptr<GPUTextureView>>,
  pub clear_value: Option<GPUColor>,
  pub load_op: GPULoadOp,
  pub store_op: GPUStoreOp,
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPULoadOp {
  Load,
  Clear,
}
impl GPULoadOp {
  pub fn with_default_value<V: Default>(
    self,
    val: Option<V>,
  ) -> wgpu_core::command::LoadOp<V> {
    match self {
      GPULoadOp::Load => wgpu_core::command::LoadOp::Load,
      GPULoadOp::Clear => {
        wgpu_core::command::LoadOp::Clear(val.unwrap_or_default())
      }
    }
  }

  pub fn with_value<V>(self, val: V) -> wgpu_core::command::LoadOp<V> {
    match self {
      GPULoadOp::Load => wgpu_core::command::LoadOp::Load,
      GPULoadOp::Clear => wgpu_core::command::LoadOp::Clear(val),
    }
  }
}

#[derive(WebIDL)]
#[webidl(enum)]
pub(crate) enum GPUStoreOp {
  Store,
  Discard,
}
impl From<GPUStoreOp> for wgpu_core::command::StoreOp {
  fn from(value: GPUStoreOp) -> Self {
    match value {
      GPUStoreOp::Store => Self::Store,
      GPUStoreOp::Discard => Self::Discard,
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURenderPassDepthStencilAttachment {
  pub view: Ptr<GPUTextureView>,
  pub depth_clear_value: Option<f32>,
  pub depth_load_op: Option<GPULoadOp>,
  pub depth_store_op: Option<GPUStoreOp>,
  #[webidl(default = false)]
  pub depth_read_only: bool,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  pub stencil_clear_value: u32,
  pub stencil_load_op: Option<GPULoadOp>,
  pub stencil_store_op: Option<GPUStoreOp>,
  #[webidl(default = false)]
  pub stencil_read_only: bool,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPURenderPassTimestampWrites {
  pub query_set: Ptr<crate::query_set::GPUQuerySet>,
  #[options(enforce_range = true)]
  pub beginning_of_pass_write_index: Option<u32>,
  #[options(enforce_range = true)]
  pub end_of_pass_write_index: Option<u32>,
}
