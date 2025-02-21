// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_error::JsErrorBox;

use crate::buffer::GPUBuffer;
use crate::command_buffer::GPUCommandBuffer;
use crate::texture::GPUTexture;
use crate::texture::GPUTextureAspect;
use crate::webidl::GPUExtent3D;
use crate::webidl::GPUOrigin2D;
use crate::webidl::GPUOrigin3D;
use crate::Instance;

pub struct GPUQueue {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub label: String,

  pub id: wgpu_core::id::QueueId,
}

impl Drop for GPUQueue {
  fn drop(&mut self) {
    self.instance.queue_drop(self.id);
  }
}

impl GarbageCollected for GPUQueue {}

#[op2]
impl GPUQueue {
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
  fn submit(
    &self,
    #[webidl] command_buffers: Vec<Ptr<GPUCommandBuffer>>,
  ) -> Result<(), JsErrorBox> {
    let ids = command_buffers
      .into_iter()
      .enumerate()
      .map(|(i, cb)| {
        if cb.consumed.set(()).is_err() {
          Err(JsErrorBox::type_error(format!(
            "The command buffer at position {i} has already been submitted."
          )))
        } else {
          Ok(cb.id)
        }
      })
      .collect::<Result<Vec<_>, _>>()?;

    let err = self.instance.queue_submit(self.id, &ids).err();

    if let Some((_, err)) = err {
      self.error_handler.push_error(Some(err));
    }

    Ok(())
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
    #[webidl(options(enforce_range = true))] buffer_offset: u64,
    #[anybuffer] buf: &[u8],
    #[webidl(default = 0, options(enforce_range = true))] data_offset: u64,
    #[webidl(options(enforce_range = true))] size: Option<u64>,
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
    #[anybuffer] buf: &[u8],
    #[webidl] data_layout: GPUTexelCopyBufferLayout,
    #[webidl] size: GPUExtent3D,
  ) {
    let destination = wgpu_core::command::TexelCopyTextureInfo {
      texture: destination.texture.id,
      mip_level: destination.mip_level,
      origin: destination.origin.into(),
      aspect: destination.aspect.into(),
    };

    let data_layout = wgpu_types::TexelCopyBufferLayout {
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

  #[required(3)]
  fn copy_external_image_to_texture(
    &self,
    #[webidl] source: GPUCopyExternalImageSourceInfo,
    #[webidl] destination: GPUCopyExternalImageDestInfo,
    #[webidl] copy_size: GPUExtent3D,
  ) -> Result<(), deno_canvas::CanvasError> {
    if source.source.detached.get().is_some() {
      // TODO: error
    }
    let mut data = source.source.data.borrow().clone();
    if source.flip_y {
      data.apply_orientation(
        deno_canvas::image::metadata::Orientation::FlipVertical,
      );
    }

    // TODO: source.origin
    // TODO: destination.color_space

    if destination.premultiplied_alpha {
      data = deno_canvas::premultiply_alpha(data)?;
    }

    let destination = wgpu_core::command::TexelCopyTextureInfo {
      texture: destination.texture.id,
      mip_level: destination.mip_level,
      origin: destination.origin.into(),
      aspect: destination.aspect.into(),
    };

    let data_layout = wgpu_types::TexelCopyBufferLayout {
      offset: 0,
      bytes_per_row: Some(4 * data.width()), // TODO: Shouldn't be hardcoded 4
      rows_per_image: Some(data.height()),
    };

    let err = self
      .instance
      .queue_write_texture(
        self.id,
        &destination,
        data.as_bytes(),
        &data_layout,
        &copy_size.into(),
      )
      .err();

    self.error_handler.push_error(err);

    Ok(())
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUTexelCopyTextureInfo {
  pub texture: Ptr<GPUTexture>,
  #[webidl(default = 0)]
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

#[derive(WebIDL)]
#[webidl(dictionary)]
struct GPUCopyExternalImageSourceInfo {
  source: Ptr<deno_canvas::ImageBitmap>, // TODO: union with ImageData
  origin: Option<GPUOrigin2D>,
  #[webidl(default = false)]
  flip_y: bool,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct GPUCopyExternalImageDestInfo {
  pub texture: Ptr<GPUTexture>,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  pub mip_level: u32,
  #[webidl(default = Default::default())]
  pub origin: GPUOrigin3D,
  #[webidl(default = GPUTextureAspect::All)]
  pub aspect: GPUTextureAspect,
  #[webidl(default = PredefinedColorSpace::Srgb)]
  pub color_space: PredefinedColorSpace,
  #[webidl(default = false)]
  pub premultiplied_alpha: bool,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
enum PredefinedColorSpace {
  #[webidl(rename = "srgb")]
  Srgb,
  #[webidl(rename = "display-p3")]
  DisplayP3,
}
