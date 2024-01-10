// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::ToJsBuffer;
use image::imageops::FilterType;
use image::ImageDecoder;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImageResizeQuality {
  Pixelated,
  Low,
  Medium,
  High,
}

#[op2]
#[serde]
fn op_image_process(
  #[buffer] buf: &[u8],
  width: u32,
  height: u32,
  new_width: u32,
  new_height: u32,
  #[serde] quality: ImageResizeQuality,
  flip_y: bool,
) -> Result<ToJsBuffer, AnyError> {
  let filter_type = match quality {
    ImageResizeQuality::Pixelated => FilterType::Nearest,
    ImageResizeQuality::Low => FilterType::Triangle,
    ImageResizeQuality::Medium => FilterType::CatmullRom,
    ImageResizeQuality::High => FilterType::Lanczos3,
  };

  let view = image::RgbaImage::from_vec(width, height, buf.to_vec()).unwrap();
  let mut image_out =
    image::imageops::resize(&view, new_width, new_height, filter_type);

  if flip_y {
    image::imageops::flip_vertical_in_place(&mut image_out);
  }

  Ok(image_out.to_vec().into())
}

#[derive(Debug, Serialize)]
struct DecodedPng {
  data: ToJsBuffer,
  width: u32,
  height: u32,
}

#[op2]
#[serde]
fn op_image_decode_png(#[buffer] buf: &[u8]) -> Result<DecodedPng, AnyError> {
  let png = image::codecs::png::PngDecoder::new(buf)?;
  let mut png_data = vec![];
  png.read_image(&mut png_data)?;

  let (width, height) = png.dimensions();

  Ok(DecodedPng {
    data: png_data.into(),
    width,
    height,
  })
}

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_image_process, op_image_decode_png,],
  esm = ["01_image.js"],
);
