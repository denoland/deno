// Copyright 2018-2025 the Deno authors. MIT license.
#![cfg(not(target_arch = "wasm32"))]
#![warn(unsafe_op_in_unsafe_fn)]

pub use wgpu_core;
pub use wgpu_types;

pub const UNSTABLE_FEATURE_NAME: &str = "webgpu";

//pub mod byow;
//pub mod surface;
mod wrap;

pub type Instance = std::sync::Arc<wgpu_core::global::Global>;

deno_core::extension!(
  deno_webgpu,
  deps = [deno_webidl, deno_web],
  /*ops = [
    // surface
    surface::op_webgpu_surface_configure,
    surface::op_webgpu_surface_get_current_texture,
    surface::op_webgpu_surface_present,
    // byow
    byow::op_webgpu_surface_create,
  ],*/
  esm = ["00_init.js" /* "02_surface.js"*/,],
  lazy_loaded_esm = ["01_webgpu.js"],
);
