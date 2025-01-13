// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_error::JsErrorBox;

use crate::wrap::buffer::GPUBuffer;
use crate::wrap::command_buffer::GPUCommandBuffer;
use crate::wrap::texture::GPUTexture;
use crate::wrap::texture::GPUTextureAspect;
use crate::wrap::webidl::GPUExtent3D;
use crate::wrap::webidl::GPUOrigin3D;
use crate::Instance;

pub struct GPUQueue {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub label: String,

  pub id: wgpu_core::id::QueueId,
}
impl GarbageCollected for GPUQueue {}

#[op2]
impl GPUQueue {
  crate::with_label!();

  #[required(1)]
  fn submit(&self, #[webidl] command_buffers: Vec<Ptr<GPUCommandBuffer>>) {
    let ids = command_buffers
      .into_iter()
      .map(|cb| cb.id)
      .collect::<Vec<_>>();

    let err = self.instance.queue_submit(self.id, &ids).err();

    if let Some((_, err)) = err {
      self.error_handler.push_error(Some(err));
    }
  }

  #[async_method]
  async fn on_submitted_work_done(&self) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic(
      "This operation is currently not supported",
    ))
  }

  #[required(3)]
  fn write_buffer(
    &self,
    #[webidl] buffer: Ptr<GPUBuffer>,
    #[webidl/*(options(enforce_range = true))*/] buffer_offset: u64,
    #[anybuffer] buf: &[u8], // TODO: AllowSharedBufferSource
    #[webidl/*(default = 0, options(enforce_range = true))*/] data_offset: u64,
    #[webidl/*(options(enforce_range = true))*/] size: Option<u64>,
  ) {
    let data = match size {
      Some(size) => {
        &buf[(data_offset as usize)..((data_offset + size) as usize)]
      }
      None => &buf[(data_offset as usize)..],
    };

    let err = self
      .instance
      .queue_write_buffer(self.id, buffer.id, buffer_offset, data)
      .err();

    self.error_handler.push_error(err);
  }

  #[required(4)]
  fn write_texture(
    &self,
    #[webidl] destination: GPUTexelCopyTextureInfo,
    #[anybuffer] buf: &[u8], // TODO: AllowSharedBufferSource
    #[webidl] data_layout: GPUTexelCopyBufferLayout,
    #[webidl] size: GPUExtent3D,
  ) {
    let destination = wgpu_core::command::ImageCopyTexture {
      texture: destination.texture.id,
      mip_level: destination.mip_level,
      origin: destination.origin.into(),
      aspect: destination.aspect.into(),
    };

    let data_layout = wgpu_types::ImageDataLayout {
      offset: data_layout.offset,
      bytes_per_row: data_layout.bytes_per_row,
      rows_per_image: data_layout.rows_per_image,
    };

    let err = self
      .instance
      .queue_write_texture(
        self.id,
        &destination,
        buf,
        &data_layout,
        &size.into(),
      )
      .err();

    self.error_handler.push_error(err);
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUTexelCopyTextureInfo {
  pub texture: Ptr<GPUTexture>,
  #[options(enforce_range = true)]
  pub mip_level: u32,
  #[webidl(default = Default::default())]
  pub origin: GPUOrigin3D,
  #[webidl(default = GPUTextureAspect::All)]
  pub aspect: GPUTextureAspect,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct GPUTexelCopyBufferLayout {
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  offset: u64,
  #[options(enforce_range = true)]
  bytes_per_row: Option<u32>,
  #[options(enforce_range = true)]
  rows_per_image: Option<u32>,
}
