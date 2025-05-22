// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::WebIDL;

#[derive(WebIDL)]
#[webidl(enum)]
pub enum PredefinedColorSpace {
  #[webidl(rename = "srgb")]
  Srgb,
  #[webidl(rename = "display-p3")]
  DisplayP3,
}

// TODO: add CanvasImageSource
