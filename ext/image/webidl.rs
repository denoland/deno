// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::WebIDL;

#[derive(WebIDL, Clone, Copy, PartialEq, Eq)]
#[webidl(enum)]
pub enum PredefinedColorSpace {
  #[webidl(rename = "srgb")]
  Srgb,
  #[webidl(rename = "srgb-linear")]
  SrgbLinear,
  #[webidl(rename = "display-p3")]
  DisplayP3,
  #[webidl(rename = "display-p3-linear")]
  DisplayP3Linear,
}
