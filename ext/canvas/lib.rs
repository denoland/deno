// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::op2;
use deno_core::ToJsBuffer;
use image::imageops::FilterType;
use image::ColorType;
use image::ImageDecoder;
use image::Pixel;
use image::RgbaImage;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CanvasError {
  #[error("Color type '{0:?}' not supported")]
  UnsupportedColorType(ColorType),
  #[error(transparent)]
  Image(#[from] image::ImageError),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImageResizeQuality {
  Pixelated,
  Low,
  Medium,
  High,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageProcessArgs {
  width: u32,
  height: u32,
  surface_width: u32,
  surface_height: u32,
  input_x: i64,
  input_y: i64,
  output_width: u32,
  output_height: u32,
  resize_quality: ImageResizeQuality,
  flip_y: bool,
  premultiply: Option<bool>,
}

#[op2]
#[serde]
fn op_image_process(
  #[buffer] buf: &[u8],
  #[serde] args: ImageProcessArgs,
) -> ToJsBuffer {
  let view =
    RgbaImage::from_vec(args.width, args.height, buf.to_vec()).unwrap();

  let surface = if !(args.width == args.surface_width
    && args.height == args.surface_height
    && args.input_x == 0
    && args.input_y == 0)
  {
    let mut surface = RgbaImage::new(args.surface_width, args.surface_height);

    image::imageops::overlay(&mut surface, &view, args.input_x, args.input_y);

    surface
  } else {
    view
  };

  let filter_type = match args.resize_quality {
    ImageResizeQuality::Pixelated => FilterType::Nearest,
    ImageResizeQuality::Low => FilterType::Triangle,
    ImageResizeQuality::Medium => FilterType::CatmullRom,
    ImageResizeQuality::High => FilterType::Lanczos3,
  };

  let mut image_out = image::imageops::resize(
    &surface,
    args.output_width,
    args.output_height,
    filter_type,
  );

  if args.flip_y {
    image::imageops::flip_vertical_in_place(&mut image_out);
  }

  // ignore 9.

  if let Some(premultiply) = args.premultiply {
    let is_not_premultiplied = image_out.pixels().any(|pixel| {
      (pixel.0[0].max(pixel.0[1]).max(pixel.0[2])) > (255 * pixel.0[3])
    });

    if premultiply {
      if is_not_premultiplied {
        for pixel in image_out.pixels_mut() {
          let alpha = pixel.0[3];
          pixel.apply_without_alpha(|channel| {
            (channel as f32 * (alpha as f32 / 255.0)) as u8
          })
        }
      }
    } else if !is_not_premultiplied {
      for pixel in image_out.pixels_mut() {
        let alpha = pixel.0[3];
        pixel.apply_without_alpha(|channel| {
          (channel as f32 / (alpha as f32 / 255.0)) as u8
        })
      }
    }
  }

  image_out.to_vec().into()
}

#[derive(Debug, Serialize)]
struct DecodedPng {
  data: ToJsBuffer,
  width: u32,
  height: u32,
}

#[op2]
#[serde]
fn op_image_decode_png(
  #[buffer] buf: &[u8],
) -> Result<DecodedPng, CanvasError> {
  let png = image::codecs::png::PngDecoder::new(buf)?;

  let (width, height) = png.dimensions();

  // TODO(@crowlKats): maybe use DynamicImage https://docs.rs/image/0.24.7/image/enum.DynamicImage.html ?
  if png.color_type() != ColorType::Rgba8 {
    return Err(CanvasError::UnsupportedColorType(png.color_type()));
  }

  // read_image will assert that the buffer is the correct size, so we need to fill it with zeros
  let mut png_data = vec![0_u8; png.total_bytes() as usize];

  png.read_image(&mut png_data)?;

  Ok(DecodedPng {
    data: png_data.into(),
    width,
    height,
  })
}

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_image_process, op_image_decode_png],
  lazy_loaded_esm = ["01_image.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_canvas.d.ts")
}
