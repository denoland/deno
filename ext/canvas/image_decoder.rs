// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::BufRead;
use std::io::BufReader;
use std::io::Cursor;
use std::io::Seek;

use deno_core::error::AnyError;
use image::codecs::bmp::BmpDecoder;
use image::codecs::gif::GifDecoder;
use image::codecs::ico::IcoDecoder;
use image::codecs::jpeg::JpegDecoder;
use image::codecs::png::PngDecoder;
use image::codecs::webp::WebPDecoder;
use image::DynamicImage;
use image::ImageDecoder;
use image::ImageError;

//
// About the animated image
// > Blob .4
// > ... If this is an animated image, imageBitmap's bitmap data must only be taken from
// > the default image of the animation (the one that the format defines is to be used when animation is
// > not supported or is disabled), or, if there is no such image, the first frame of the animation.
// https://html.spec.whatwg.org/multipage/imagebitmap-and-animations.html
//
// see also browser implementations: (The implementation of Gecko and WebKit is hard to read.)
// https://source.chromium.org/chromium/chromium/src/+/bdbc054a6cabbef991904b5df9066259505cc686:third_party/blink/renderer/platform/image-decoders/image_decoder.h;l=175-189
//

pub(crate) trait ImageDecoderFromReader<'a, R: BufRead + Seek> {
  fn to_decoder(
    reader: R,
    error_fn: fn(ImageError) -> AnyError,
  ) -> Result<Self, AnyError>
  where
    Self: Sized;
  fn to_intermediate_image(
    self,
    error_fn: fn(ImageError) -> AnyError,
  ) -> Result<DynamicImage, AnyError>;
  fn get_icc_profile(&mut self) -> Option<Vec<u8>>;
}

pub(crate) type ImageDecoderFromReaderType<'a> = BufReader<Cursor<&'a [u8]>>;

macro_rules! impl_image_decoder_from_reader {
  ($decoder:ty, $reader:ty) => {
    impl<'a, R: BufRead + Seek> ImageDecoderFromReader<'a, R> for $decoder {
      fn to_decoder(
        reader: R,
        error_fn: fn(ImageError) -> AnyError,
      ) -> Result<Self, AnyError>
      where
        Self: Sized,
      {
        match <$decoder>::new(reader) {
          Ok(decoder) => Ok(decoder),
          Err(err) => return Err(error_fn(err)),
        }
      }
      fn to_intermediate_image(
        self,
        error_fn: fn(ImageError) -> AnyError,
      ) -> Result<DynamicImage, AnyError> {
        match DynamicImage::from_decoder(self) {
          Ok(image) => Ok(image),
          Err(err) => Err(error_fn(err)),
        }
      }
      fn get_icc_profile(&mut self) -> Option<Vec<u8>> {
        match self.icc_profile() {
          Ok(profile) => profile,
          Err(_) => None,
        }
      }
    }
  };
}

// If PngDecoder decodes an animated image, it returns the default image if one is set, or the first frame if not.
impl_image_decoder_from_reader!(PngDecoder<R>, ImageDecoderFromReaderType);
impl_image_decoder_from_reader!(JpegDecoder<R>, ImageDecoderFromReaderType);
// The GifDecoder decodes the first frame.
impl_image_decoder_from_reader!(GifDecoder<R>, ImageDecoderFromReaderType);
impl_image_decoder_from_reader!(BmpDecoder<R>, ImageDecoderFromReaderType);
impl_image_decoder_from_reader!(IcoDecoder<R>, ImageDecoderFromReaderType);
// The WebPDecoder decodes the first frame.
impl_image_decoder_from_reader!(WebPDecoder<R>, ImageDecoderFromReaderType);
