// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;

mod bitmaprenderer;
pub mod byow;
mod canvas;

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_init_canvas, canvas::op_canvas_is_offscreen_canvas],
  objects = [
    bitmaprenderer::ImageBitmapRenderingContext,
    canvas::OffscreenCanvas,
    byow::UnsafeWindowSurface,
  ],
  lazy_loaded_esm = ["01_canvas.js"],
  lazy_loaded_js = ["02_surface.js"],
);

#[op2(fast)]
pub fn op_init_canvas(
  state: &mut OpState,
  scope: &mut v8::PinScope<'_, '_>,
  blob: v8::Local<v8::Value>,
) {
  state.put(canvas::BlobHandle(v8::Global::new(scope, blob.cast())));
  deno_web::canvas2d::set_offscreen_canvas_pixel_sync(
    state,
    canvas::sync_offscreen_canvas_pixels_for_pattern,
  );
}
