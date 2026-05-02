// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::cppgc::Ref;
use deno_core::futures::channel::oneshot;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_image::ImageBitmap;
use deno_image::ImageError;
use deno_image::image;
use deno_image::image::EncodableLayout;
use deno_image::image::GenericImageView;
use deno_image::webidl::PredefinedColorSpace;

use crate::Instance;
use crate::buffer::GPUBuffer;
use crate::command_buffer::GPUCommandBuffer;
use crate::error::GPUGenericError;
use crate::texture::GPUTexture;
use crate::texture::GPUTextureAspect;
use crate::webidl::GPUExtent3D;
use crate::webidl::GPUOrigin2D;
use crate::webidl::GPUOrigin3D;

pub struct GPUQueue {
  pub instance: Instance,
  pub error_handler: super::error::ErrorHandler,

  pub label: String,

  pub id: wgpu_core::id::QueueId,
  pub device: wgpu_core::id::DeviceId,
}

impl Drop for GPUQueue {
  fn drop(&mut self) {
    self.instance.queue_drop(self.id);
  }
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUQueue {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUQueue"
  }
}

#[op2]
impl GPUQueue {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUQueue, GPUGenericError> {
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
  #[undefined]
  fn submit(
    &self,
    #[webidl] command_buffers: Vec<Ref<GPUCommandBuffer>>,
  ) -> Result<(), JsErrorBox> {
    let ids = command_buffers
      .into_iter()
      .map(|cb| cb.id)
      .collect::<Vec<_>>();

    let err = self.instance.queue_submit(self.id, &ids).err();

    if let Some((_, err)) = err {
      self.error_handler.push_error(Some(err));
    }

    Ok(())
  }

  // In the successful case, the promise should resolve to undefined, but
  // `#[undefined]` does not seem to work here.
  // https://github.com/denoland/deno/issues/29603
  async fn on_submitted_work_done(&self) -> Result<(), JsErrorBox> {
    let (sender, receiver) = oneshot::channel::<()>();

    let callback = Box::new(move || {
      sender.send(()).unwrap();
    });

    self
      .instance
      .queue_on_submitted_work_done(self.id, callback);

    let done = Rc::new(RefCell::new(false));
    let done_ = done.clone();
    let device_poll_fut = async move {
      while !*done.borrow() {
        {
          self
            .instance
            .device_poll(self.device, wgpu_types::PollType::wait_indefinitely())
            .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
      }
      Ok::<(), JsErrorBox>(())
    };

    let receiver_fut = async move {
      receiver
        .await
        .map_err(|e| JsErrorBox::generic(e.to_string()))?;
      let mut done = done_.borrow_mut();
      *done = true;
      Ok::<(), JsErrorBox>(())
    };

    tokio::try_join!(device_poll_fut, receiver_fut)?;

    Ok(())
  }

  #[required(3)]
  #[undefined]
  fn write_buffer<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl] buffer: Ref<GPUBuffer>,
    #[webidl(options(enforce_range = true))] buffer_offset: u64,
    data_arg: v8::Local<'a, v8::Value>,
    #[webidl(default = 0, options(enforce_range = true))] data_offset: u64,
    #[webidl(options(enforce_range = true))] size: Option<u64>,
  ) -> Result<(), JsErrorBox> {
    // Per the WebGPU spec, dataOffset and size are in elements (not bytes)
    // when data is a TypedArray, and in bytes otherwise.
    let (buf, bytes_per_element) = if let Ok(typed_array) =
      v8::Local::<v8::TypedArray>::try_from(data_arg)
    {
      let len = typed_array.length();
      let bpe = if len > 0 {
        typed_array.byte_length() / len
      } else {
        1
      };
      let byte_offset = typed_array.byte_offset();
      let byte_len = typed_array.byte_length();
      let ab = typed_array.buffer(scope).unwrap();
      // SAFETY: Pointer is non-null, and V8 guarantees that the
      // byte_offset is within the buffer backing store.
      let ptr = unsafe { ab.data().unwrap().as_ptr().add(byte_offset) };
      let buf =
          // SAFETY: the slice is within the bounds of the backing store
          unsafe { std::slice::from_raw_parts(ptr as *const u8, byte_len) };
      (buf, bpe)
    } else if let Ok(ab) = v8::Local::<v8::ArrayBuffer>::try_from(data_arg) {
      let byte_len = ab.byte_length();
      let ptr = ab.data().unwrap().as_ptr();
      let buf =
        // SAFETY: Pointer is non-null and byte_len is within the backing store.
        unsafe { std::slice::from_raw_parts(ptr as *const u8, byte_len) };
      (buf, 1)
    } else if let Ok(view) =
      v8::Local::<v8::ArrayBufferView>::try_from(data_arg)
    {
      let byte_offset = view.byte_offset();
      let byte_len = view.byte_length();
      let ab = view.buffer(scope).unwrap();
      // SAFETY: Pointer is non-null, and V8 guarantees that the
      // byte_offset is within the buffer backing store.
      let ptr = unsafe { ab.data().unwrap().as_ptr().add(byte_offset) };
      // SAFETY: the slice is within the bounds of the backing store
      let buf =
        unsafe { std::slice::from_raw_parts(ptr as *const u8, byte_len) };
      (buf, 1)
    } else {
      return Err(JsErrorBox::type_error(
        "data must be an ArrayBuffer or ArrayBufferView",
      ));
    };

    let data_offset_bytes = data_offset as usize * bytes_per_element;
    let data = match size {
      Some(size) => {
        let size_bytes = size as usize * bytes_per_element;
        &buf[data_offset_bytes..(data_offset_bytes + size_bytes)]
      }
      None => &buf[data_offset_bytes..],
    };

    let err = self
      .instance
      .queue_write_buffer(self.id, buffer.id, buffer_offset, data)
      .err();

    self.error_handler.push_error(err);

    Ok(())
  }

  #[required(4)]
  #[undefined]
  fn write_texture(
    &self,
    #[webidl] destination: GPUTexelCopyTextureInfo,
    #[anybuffer] buf: &[u8],
    #[webidl] data_layout: GPUTexelCopyBufferLayout,
    #[webidl] size: GPUExtent3D,
  ) {
    let destination = wgpu_types::TexelCopyTextureInfo {
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
    let data = source.source.data.borrow().clone();
    let (copy_size_width, copy_size_height, copy_size_depth_or_array_layers) =
      copy_size.dimensions();

    // Content timeline steps:
    // 6.
    let (src_origin_x, src_origin_y) = source.origin.dimensions();
    let (src_image_width, src_image_height) = data.dimensions();
    if src_origin_x + copy_size_width > src_image_width {
      return Err(JsErrorBox::new(
        "DOMExceptionOperationError",
        "source.origin.x + copySize.width must be less than the width of source.source",
      ));
    }
    if src_origin_y + copy_size_height > src_image_height {
      return Err(JsErrorBox::new(
        "DOMExceptionOperationError",
        "source.origin.y + copySize.height must be less than the height of source.source",
      ));
    }
    if copy_size_depth_or_array_layers > 1 {
      return Err(JsErrorBox::new(
        "DOMExceptionOperationError",
        "copySize.depthOrArrayLayers must be less than 1",
      ));
    }
    // 7.
    if source.source.detached.get().is_some() {
      return Err(JsErrorBox::from_err(ImageError::ImageSourceAleadyDetached));
    }

    // NOTE: The Device timeline steps are not implemented yet on wgpu side.

    // Queue timeline steps:
    // 1.
    let dst_texture_format: wgpu_types::TextureFormat =
      destination.texture.format.clone().into();
    let dst_texture_block_dimensions = dst_texture_format.block_dimensions();
    if dst_texture_block_dimensions.0 != 1
      || dst_texture_block_dimensions.1 != 1
      || copy_size_depth_or_array_layers != 1
    {
      return Err(JsErrorBox::new(
        "DOMExceptionOperationError",
        format!(
          "The destination texture format {:#?} is not supported",
          dst_texture_format
        ),
      ));
    }
    // 5.
    // 5.1
    // crop the rectangle first
    let mut data = deno_image::crop(
      data,
      src_origin_x,
      src_origin_y,
      copy_size_width,
      copy_size_height,
    );
    if source.flip_y {
      data.apply_orientation(image::metadata::Orientation::FlipVertical);
    }
    // 5.2.1
    // These steps are depending on the source.source type, conversion may or may not be required.
    // https://gpuweb.github.io/gpuweb/#color-space-conversion-elision

    // NOTE: According to the spec, there is no way to check when source.source is ImageBitmap that was aleady premultiplied or not.
    // We acutually check whether the source.source is premultiplied or not by the is_premultiplied_alpha method inside,
    // however it's not any implementation covered by the spec.
    // https://github.com/whatwg/html/issues/11029
    let data = if destination.premultiplied_alpha {
      deno_image::premultiply_alpha(data).map_err(JsErrorBox::from_err)?
    } else {
      data
    };

    // It's same issue as the above, there is no way to check
    // when the color space of source.source is ImageBitmap that was aleady transformed or not.
    // We need to not immediately convert ImageBitmap but on-the-fly.
    let data = deno_image::transform_rgb_color_space(
      data,
      match destination.color_space {
        PredefinedColorSpace::Srgb => PredefinedColorSpace::DisplayP3,
        PredefinedColorSpace::DisplayP3 => PredefinedColorSpace::Srgb,
      },
      destination.color_space,
    )
    // convert to GPUError::GPUValidationError could be better?
    .map_err(JsErrorBox::from_err)?;

    let destination = wgpu_core::command::TexelCopyTextureInfo {
      texture: destination.texture.id,
      mip_level: destination.mip_level,
      origin: destination.origin.into(),
      aspect: destination.aspect.into(),
    };

    let data_layout = wgpu_types::TexelCopyBufferLayout {
      offset: 0,
      bytes_per_row: Some(
        dst_texture_format.components() as u32 * copy_size_width,
      ),
      rows_per_image: Some(copy_size_height),
    };

    let data = match data.color() {
      // RGB does not support, so we convert it to RGBA
      // https://github.com/gpuweb/gpuweb/issues/66
      image::ColorType::Rgb8 => data.to_rgba8().to_vec(),
      image::ColorType::Rgb16 => data.to_rgba16().as_bytes().to_vec(),
      _ => data.into_bytes(),
    };

    let err = self
      .instance
      .queue_write_texture(
        self.id,
        &destination,
        &data,
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
  pub texture: Ref<GPUTexture>,
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
  source: Ref<ImageBitmap>, // TODO: union with ImageData
  origin: GPUOrigin2D,
  #[webidl(default = false)]
  flip_y: bool,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
struct GPUCopyExternalImageDestInfo {
  pub texture: Ref<GPUTexture>,
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
