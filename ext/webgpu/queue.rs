// Copyright 2018-2025 the Deno authors. MIT license.

use deno_canvas::image::GenericImageView;
use deno_canvas::webidl::PredefinedColorSpace;
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
  ) -> Result<(), JsErrorBox> {
    if source.source.detached.get().is_some() {
      // TODO: error
    }
    let mut data = source.source.data.borrow().clone();
    if source.flip_y {
      data.apply_orientation(
        deno_canvas::image::metadata::Orientation::FlipVertical,
      );
    }

    // Content timeline steps:
    // 6.
    if let Some(origin) = source.origin {
      let (origin_x, origin_y) = origin.dimensions();
      let (copy_size_width, copy_size_height, copy_size_height_depth) =
        copy_size.dimensions();
      let (source_image_width, source_image_height) = data.dimensions();
      if !(origin_x + copy_size_width <= source_image_width) {
        return Err(JsErrorBox::new(
          "DOMExceptionOperationError",
          "source.origin.x + copySize.width must be less than the width of source.source",
        ));
      }
      if !(origin_y + copy_size_height <= source_image_height) {
        return Err(JsErrorBox::new(
          "DOMExceptionOperationError",
          "source.origin.y + copySize.height must be less than the height of source.source",
        ));
      }
      if !(copy_size_height_depth <= 1) {
        return Err(JsErrorBox::new(
          "DOMExceptionOperationError",
          "copySize.depthOrArrayLayers must be less than 1",
        ));
      }
    }

    // Queue timeline steps:
    // 5.2.1
    // This step is depending on the source.source type, conversion may or may not be required.
    // https://gpuweb.github.io/gpuweb/#color-space-conversion-elision

    // NOTE: According to the spec, there is no way to check that if source.source is ImageBitmap that was aleady premultiplied or not.
    // We check whether the source.source is premultiplied or not by the is_premultiplied_alpha method inside,
    // however it's not any implementation coverd by the spec.
    let data = if destination.premultiplied_alpha {
      deno_canvas::premultiply_alpha(data).map_err(JsErrorBox::from_err)?
    } else {
      data
    };

    // It's same issue as the above, there is no way to check that
    // if the color space of source.source is ImageBitmap that was aleady transformed or not.
    let data = deno_canvas::transform_rgb_color_space(
      data,
      match destination.color_space {
        PredefinedColorSpace::Srgb => PredefinedColorSpace::DisplayP3,
        PredefinedColorSpace::DisplayP3 => PredefinedColorSpace::Srgb,
      },
      destination.color_space,
    )
    .map_err(JsErrorBox::from_err)?;

    let destination = wgpu_core::command::TexelCopyTextureInfo {
      texture: destination.texture.id,
      mip_level: destination.mip_level,
      origin: destination.origin.into(),
      aspect: destination.aspect.into(),
    };

    let data_layout = wgpu_types::TexelCopyBufferLayout {
      offset: 0,
      // The source.source is always a shape of 2D that is one of GPUCopyExternalImageSource,
      // it can simply be calculated by multiplying the width of the image by the number of bytes per pixel.
      bytes_per_row: Some(
        (data.color().bytes_per_pixel() as u32 * data.width()).into(),
      ),
      // nothing to set due to copySize.depthOrArrayLayers is always 1 for 2D images
      rows_per_image: None,
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
