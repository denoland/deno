// Copyright 2018-2025 the Deno authors. MIT license.

pub mod bitmap;
mod image_ops;
pub use image;
use image::ColorType;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ImageError {
  /// Image formats that is 32-bit depth are not supported currently due to the following reasons:
  /// - e.g. OpenEXR, it's not covered by the spec.
  /// - JPEG XL supported by WebKit, but it cannot be called a standard today.
  ///   https://github.com/whatwg/mimesniff/issues/143
  ///
  #[class(type)]
  #[error("Unsupported color type and bit depth: '{0:?}'")]
  UnsupportedColorType(ColorType),
  #[class("DOMExceptionInvalidStateError")]
  #[error("Cannot decode image '{0}'")]
  InvalidImage(image::ImageError),
  #[class("DOMExceptionInvalidStateError")]
  #[error(
    "The chunk data is not big enough with the specified width: {0} and height: {1}"
  )]
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

impl ImageError {
  /// Convert an [`image::ImageError`] to an [`ImageError::InvalidImage`].
  fn image_error_to_invalid_image(error: image::ImageError) -> Self {
    ImageError::InvalidImage(error)
  }
}

deno_core::extension!(
  deno_image,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [bitmap::op_create_image_bitmap],
  objects = [bitmap::ImageBitmap],
  lazy_loaded_esm = ["01_image.js"],
);
