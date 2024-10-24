// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

pub mod error;
mod image_decoder;
mod image_ops;
mod op_create_image_bitmap;
use image::ColorType;
use op_create_image_bitmap::op_create_image_bitmap;

#[derive(Debug, thiserror::Error)]
pub enum CanvasError {
  #[error("Color type '{0:?}' not supported")]
  UnsupportedColorType(ColorType),
  #[error(transparent)]
  Image(#[from] image::ImageError),
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
