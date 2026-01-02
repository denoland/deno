// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;

mod bitmaprenderer;
mod byow;
mod canvas;

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_init_canvas],
  objects = [
    bitmaprenderer::ImageBitmapRenderingContext,
    canvas::OffscreenCanvas,
    byow::UnsafeWindowSurface,
  ],
  esm = ["02_surface.js"],
  lazy_loaded_esm = ["01_canvas.js"],
);

#[op2(fast)]
pub fn op_init_canvas(
  state: &mut OpState,
  scope: &mut v8::PinScope<'_, '_>,
  blob: v8::Local<v8::Value>,
) {
  state.put(canvas::BlobHandle(v8::Global::new(scope, blob.cast())));
}
