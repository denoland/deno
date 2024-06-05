// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::ToJsBuffer;
use image::imageops::FilterType;
use image::ExtendedColorType;
use image::GenericImageView;
use image::ImageEncoder;
use image::Pixel;
use image::RgbImage;
use image::RgbaImage;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;

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
) -> Result<ToJsBuffer, AnyError> {
  let view = RgbImage::from_vec(args.width, args.height, buf.to_vec()).unwrap();

  let surface = if !(args.width == args.surface_width
    && args.height == args.surface_height
    && args.input_x == 0
    && args.input_y == 0)
  {
    let mut surface = RgbImage::new(args.surface_width, args.surface_height);

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

  Ok(image_out.into_raw().into())
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
  let reader = std::io::BufReader::new(std::io::Cursor::new(buf));
  let decoder = image::codecs::png::PngDecoder::new(reader)?;
  let image = image::DynamicImage::from_decoder(decoder)?;
  let (width, height) = image.dimensions();

  Ok(DecodedPng {
    data: image.into_rgba8().into_raw().into(),
    width,
    height,
  })
}

#[op2]
#[serde]
fn op_image_encode_png(
  #[buffer] buf: &[u8],
  width: u32,
  height: u32,
) -> Result<Option<ToJsBuffer>, AnyError> {
  let mut out = vec![];
  let png = image::codecs::png::PngEncoder::new(&mut out);

  if png
    .write_image(buf, width, height, ExtendedColorType::Rgba8)
    .is_err()
  {
    return Ok(None);
  };
  Ok(Some(out.into()))
}

deno_core::extension!(
  deno_canvas,
  deps = [deno_webidl, deno_web, deno_webgpu],
  ops = [op_image_process, op_image_decode_png, op_image_encode_png],
  lazy_loaded_esm = ["01_image.js", "02_canvas.js"],
);

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_canvas.d.ts")
}
