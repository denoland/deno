// Copyright 2018-2025 the Deno authors. MIT license.

use std::path::PathBuf;

mod image_bitmap;
mod image_ops;
pub mod webidl;
pub use image;
use image::ColorType;
use image_bitmap::op_create_image_bitmap;
pub use image_bitmap::ImageBitmap;
pub use image_ops::crop;
pub use image_ops::premultiply_alpha;
pub use image_ops::transform_rgb_color_space;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CanvasError {
  /// Image formats that is 32-bit depth are not supported currently due to the following reasons:
  /// - e.g. OpenEXR, it's not covered by the spec.
  /// - JPEG XL supported by WebKit, but it cannot be called a standard today.  
  ///   https://github.com/whatwg/mimesniff/issues/143
  ///
  #[class(type)]
  #[error("Unsupported color type and bit depth: '{0:?}'")]
  UnsupportedColorType(ColorType),
  #[class(type)]
  #[error("Pixel ({0}, {1}) is out of bounds with the specified width: {2} and height: {3}")]
  PixelIndexOutOfBounds(u32, u32, u32, u32),
  #[class("DOMExceptionInvalidStateError")]
  #[error("Cannot decode image '{0}'")]
  InvalidImage(image::ImageError),
  #[class("DOMExceptionInvalidStateError")]
  #[error("The image source is no longer usable")]
  ImageSourceAleadyDetached,
  #[class("DOMExceptionInvalidStateError")]
  #[error("The chunk data is not big enough with the specified width: {0} and height: {1}")]
  NotBigEnoughChunk(u32, u32),
  #[class("DOMExceptionInvalidStateError")]
  #[error("The width: {0} or height: {1} could not be zero")]
  InvalidSizeZero(u32, u32),
  #[class(generic)]
  #[error(transparent)]
  Lcms(#[from] lcms2::Error),
  #[class(generic)]
  #[error(transparent)]
  Image(#[from] image::ImageError),
}

impl CanvasError {
  /// Convert an [`image::ImageError`] to an [`CanvasError::InvalidImage`].
  fn image_error_to_invalid_image(error: image::ImageError) -> Self {
    CanvasError::InvalidImage(error)
  }
}

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_create_image_bitmap],
  objects = [ImageBitmap],
  lazy_loaded_esm = ["01_image.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_canvas.d.ts")
}
