// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;

pub mod error;
pub mod idl;
mod image_decoder;
mod image_ops;
mod op_create_image_bitmap;
use op_create_image_bitmap::op_create_image_bitmap;

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_create_image_bitmap],
  lazy_loaded_esm = ["01_image.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_canvas.d.ts")
}
