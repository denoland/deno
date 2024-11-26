// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

mod image_ops;
mod op_create_image_bitmap;
use image::ColorType;
use op_create_image_bitmap::op_create_image_bitmap;

#[derive(Debug, thiserror::Error)]
pub enum CanvasError {
  /// Image formats that is 32-bit depth are not supported currently due to the following reasons:
  /// - e.g. OpenEXR, it's not covered by the spec.
  /// - JPEG XL supported by WebKit, but it cannot be called a standard today.  
  ///   https://github.com/whatwg/mimesniff/issues/143
  ///
  /// This error will be mapped to TypeError.
  #[error("Unsupported color type and bit depth: '{0:?}'")]
  UnsupportedColorType(ColorType),
  /// This error will be mapped to DOMExceptionInvalidStateError.
  #[error("Cannot decode image '{0}'")]
  InvalidImage(String),
  #[error(transparent)]
  Lcms(#[from] lcms2::Error),
  #[error(transparent)]
  /// This error will be mapped to TypeError.
  Image(#[from] image::ImageError),
}

impl CanvasError {
  /// Convert an [`image::ImageError`] to an [`CanvasError::InvalidImage`].
  fn image_error_to_invalid_image(error: image::ImageError) -> Self {
    Self::InvalidImage(error.to_string())
  }
}

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_create_image_bitmap],
  lazy_loaded_esm = ["01_image.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_canvas.d.ts")
}
