// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::ContextFn;
use deno_core::webidl::IntOptions;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlErrorKind;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ImageDataError {
  #[class(inherit)]
  #[error(transparent)]
  WebIDL(#[from] WebIdlError),
  #[class("DOMExceptionInvalidStateError")]
  #[error("Failed to construct 'ImageData': the input data has zero elements")]
  ZeroElements,
  #[class("DOMExceptionInvalidStateError")]
  #[error(
    "Failed to construct 'ImageData': the input data length is not a multiple of 4, received {0}"
  )]
  NotMultipleOfFour(usize),
  #[class("DOMExceptionIndexSizeError")]
  #[error(
    "Failed to construct 'ImageData': the source width is zero or not a number"
  )]
  ZeroWidth,
  #[class("DOMExceptionIndexSizeError")]
  #[error(
    "Failed to construct 'ImageData': the source height is zero or not a number"
  )]
  ZeroHeight,
  #[class("DOMExceptionIndexSizeError")]
  #[error(
    "Failed to construct 'ImageData': the input data length is not a multiple of (4 * width)"
  )]
  NotMultipleOfRow,
  #[class("DOMExceptionIndexSizeError")]
  #[error(
    "Failed to construct 'ImageData': the input data length is not equal to (4 * width * height)"
  )]
  WrongLength,
  #[class("DOMExceptionInvalidStateError")]
  #[error(
    "Failed to construct 'ImageData': Uint8ClampedArray must use rgba-unorm8 pixelFormat."
  )]
  Uint8ClampedNeedsUnorm,
  #[class("DOMExceptionInvalidStateError")]
  #[error(
    "Failed to construct 'ImageData': Float16Array must use rgba-float16 pixelFormat."
  )]
  Float16NeedsFloat16,
  #[class(generic)]
  #[error("Failed to allocate ImageData backing store")]
  AllocationFailed,
  #[class(generic)]
  #[error("pixel data length mismatch")]
  PixelDataMismatch,
}

#[derive(WebIDL, Debug, Clone, Copy, PartialEq, Eq)]
#[webidl(enum)]
pub enum PredefinedColorSpace {
  #[webidl(rename = "srgb")]
  Srgb,
  #[webidl(rename = "display-p3")]
  DisplayP3,
}

impl PredefinedColorSpace {
  fn name(self) -> &'static str {
    match self {
      Self::Srgb => "srgb",
      Self::DisplayP3 => "display-p3",
    }
  }
}

#[derive(WebIDL, Debug, Clone, Copy, PartialEq, Eq)]
#[webidl(enum)]
pub enum ImageDataPixelFormat {
  #[webidl(rename = "rgba-unorm8")]
  RgbaUnorm8,
  #[webidl(rename = "rgba-float16")]
  RgbaFloat16,
}

impl ImageDataPixelFormat {
  fn name(self) -> &'static str {
    match self {
      Self::RgbaUnorm8 => "rgba-unorm8",
      Self::RgbaFloat16 => "rgba-float16",
    }
  }
}

#[derive(WebIDL, Debug)]
#[webidl(dictionary)]
pub struct ImageDataSettings {
  #[webidl(default = None)]
  color_space: Option<PredefinedColorSpace>,
  #[webidl(default = ImageDataPixelFormat::RgbaUnorm8)]
  pixel_format: ImageDataPixelFormat,
}

pub struct ImageData {
  width: u32,
  height: u32,
  pixel_format: ImageDataPixelFormat,
  color_space: PredefinedColorSpace,
  data: v8::TracedReference<v8::Object>,
}

impl ImageData {
  pub fn get_width(&self) -> u32 {
    self.width
  }

  pub fn get_height(&self) -> u32 {
    self.height
  }

  pub fn read_pixels_rgba8(&self, scope: &mut v8::PinScope<'_, '_>) -> Vec<u8> {
    let data_obj = self.data.get(scope).unwrap();
    let data_val: v8::Local<v8::Value> = data_obj.into();
    let ta = v8::Local::<v8::TypedArray>::try_from(data_val).unwrap();
    let byte_len = ta.byte_length();
    let mut bytes = vec![0u8; byte_len];
    ta.copy_contents(&mut bytes);
    match self.pixel_format {
      ImageDataPixelFormat::RgbaUnorm8 => bytes,
      ImageDataPixelFormat::RgbaFloat16 => {
        let pixel_count = self.width as usize * self.height as usize;
        let mut out = vec![0u8; pixel_count * 4];
        for i in 0..pixel_count * 4 {
          let bits = u16::from_le_bytes([bytes[i * 2], bytes[i * 2 + 1]]);
          let f = half::f16::from_bits(bits).to_f32();
          out[i] = (f.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
        out
      }
    }
  }

  /// Create an ImageData from raw RGBA8 pixel data (for internal use by getImageData etc).
  pub fn new_rgba_unorm8(
    scope: &mut v8::PinScope<'_, '_>,
    width: u32,
    height: u32,
    pixels: &[u8],
  ) -> Result<Self, ImageDataError> {
    Self::new_rgba_unorm8_with_color_space(
      scope,
      width,
      height,
      pixels,
      PredefinedColorSpace::Srgb,
    )
  }

  pub fn new_rgba_unorm8_with_color_space(
    scope: &mut v8::PinScope<'_, '_>,
    width: u32,
    height: u32,
    pixels: &[u8],
    color_space: PredefinedColorSpace,
  ) -> Result<Self, ImageDataError> {
    if pixels.len() != (width as usize * height as usize * 4) {
      return Err(ImageDataError::PixelDataMismatch);
    }
    let byte_len = pixels.len();
    let ab = v8::ArrayBuffer::new(scope, byte_len);
    let Some(ta) = v8::Uint8ClampedArray::new(scope, ab, 0, byte_len) else {
      return Err(ImageDataError::AllocationFailed);
    };
    let buf_ptr = ab.data().unwrap().as_ptr() as *mut u8;
    // SAFETY: `buf_ptr` points to the ArrayBuffer's backing store with at
    // least `byte_len` bytes (allocated above), and `pixels` has exactly
    // `byte_len` bytes. The regions do not overlap.
    unsafe {
      std::ptr::copy_nonoverlapping(pixels.as_ptr(), buf_ptr, byte_len);
    }
    let data_obj: v8::Local<v8::Object> = ta.into();

    Ok(ImageData {
      width,
      height,
      pixel_format: ImageDataPixelFormat::RgbaUnorm8,
      color_space,
      data: v8::TracedReference::new(scope, data_obj),
    })
  }
}

// SAFETY: we're sure `ImageData` can be GCed.
unsafe impl GarbageCollected for ImageData {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.data);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ImageData"
  }
}

#[inline]
fn convert_unsigned_long<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
  context: &'static str,
) -> Result<u32, WebIdlError> {
  u32::convert(
    scope,
    value,
    Cow::Borrowed("Failed to construct 'ImageData'"),
    ContextFn::new_borrowed(&|| Cow::Borrowed(context)),
    &IntOptions::default(),
  )
}

#[inline]
fn convert_settings<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  value: v8::Local<'a, v8::Value>,
  context: &'static str,
) -> Result<ImageDataSettings, WebIdlError> {
  ImageDataSettings::convert(
    scope,
    value,
    Cow::Borrowed("Failed to construct 'ImageData'"),
    ContextFn::new_borrowed(&|| Cow::Borrowed(context)),
    &Default::default(),
  )
}

#[inline]
fn alloc_typed_array<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  pixel_format: ImageDataPixelFormat,
  width: u32,
  height: u32,
) -> Result<v8::Local<'a, v8::Object>, ImageDataError> {
  let pixel_count = (width as usize)
    .checked_mul(height as usize)
    .and_then(|v| v.checked_mul(4))
    .ok_or(ImageDataError::AllocationFailed)?;
  match pixel_format {
    ImageDataPixelFormat::RgbaUnorm8 => {
      let buffer = v8::ArrayBuffer::new(scope, pixel_count);
      v8::Uint8ClampedArray::new(scope, buffer, 0, pixel_count)
        .map(Into::into)
        .ok_or(ImageDataError::AllocationFailed)
    }
    ImageDataPixelFormat::RgbaFloat16 => {
      let byte_length = pixel_count
        .checked_mul(2)
        .ok_or(ImageDataError::AllocationFailed)?;
      let buffer = v8::ArrayBuffer::new(scope, byte_length);
      let arr = v8::Float16Array::new(scope, buffer, 0, pixel_count)
        .ok_or(ImageDataError::AllocationFailed)?;
      // SAFETY: a Float16Array is a v8::Object (transmute is the existing
      // workaround until rusty_v8 implements `Into`).
      Ok(unsafe {
        std::mem::transmute::<v8::Local<v8::Float16Array>, v8::Local<v8::Object>>(
          arr,
        )
      })
    }
  }
}

#[op2]
impl ImageData {
  #[constructor]
  #[reentrant]
  #[required(2)]
  #[cppgc]
  fn constructor<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) -> Result<ImageData, ImageDataError> {
    // `#[required(2)]` ensures `args` has at least 2 entries.
    let args = args.expect("constructor requires arguments");
    let arg_count = args.length();
    let arg0 = args.get(0);
    let arg1 = args.get(1);
    let arg2 = if arg_count >= 3 {
      Some(args.get(2))
    } else {
      None
    };
    let arg3 = if arg_count >= 4 {
      Some(args.get(3))
    } else {
      None
    };

    let arg0_typed_array = v8::Local::<v8::TypedArray>::try_from(arg0).ok();
    let arg0_is_uint8_clamped = if arg0.is_object() {
      arg0.cast::<v8::Object>().is_uint8_clamped_array()
    } else {
      false
    };
    let arg0_is_float16 = if arg0.is_object() {
      arg0.cast::<v8::Object>().is_float16_array()
    } else {
      false
    };

    if arg_count > 3 || arg0_is_uint8_clamped || arg0_is_float16 {
      // Overload: new ImageData(data, sw [, sh [, settings ] ])
      let data = arg0_typed_array.ok_or_else(|| {
        WebIdlError::new(
          Cow::Borrowed("Failed to construct 'ImageData'"),
          ContextFn::new_borrowed(&|| Cow::Borrowed("Argument 1")),
          WebIdlErrorKind::ConvertToConverterType("ArrayBufferView"),
        )
      })?;
      let source_width = convert_unsigned_long(scope, arg1, "Argument 2")?;
      let source_height = match arg2 {
        Some(v) if !v.is_undefined() => {
          Some(convert_unsigned_long(scope, v, "Argument 3")?)
        }
        _ => None,
      };
      let settings_value = arg3.unwrap_or_else(|| v8::undefined(scope).into());
      let settings = convert_settings(scope, settings_value, "Argument 4")?;

      // Match `TypedArrayPrototypeGetLength` semantics: element count, not
      // byte count.
      let data_length = data.length();

      if data_length == 0 {
        return Err(ImageDataError::ZeroElements);
      }
      if data_length % 4 != 0 {
        return Err(ImageDataError::NotMultipleOfFour(data_length));
      }
      if source_width == 0 {
        return Err(ImageDataError::ZeroWidth);
      }
      if let Some(h) = source_height
        && h == 0
      {
        return Err(ImageDataError::ZeroHeight);
      }
      let pixel_count = data_length / 4;
      if pixel_count % source_width as usize != 0 {
        return Err(ImageDataError::NotMultipleOfRow);
      }
      let derived_height = (pixel_count / source_width as usize) as u32;
      if let Some(h) = source_height
        && h != derived_height
      {
        return Err(ImageDataError::WrongLength);
      }

      if arg0_is_uint8_clamped
        && !matches!(settings.pixel_format, ImageDataPixelFormat::RgbaUnorm8)
      {
        return Err(ImageDataError::Uint8ClampedNeedsUnorm);
      }
      if arg0_is_float16
        && !matches!(settings.pixel_format, ImageDataPixelFormat::RgbaFloat16)
      {
        return Err(ImageDataError::Float16NeedsFloat16);
      }

      let color_space =
        settings.color_space.unwrap_or(PredefinedColorSpace::Srgb);
      let height = source_height.unwrap_or(derived_height);
      let data_obj: v8::Local<v8::Object> = data.into();

      Ok(ImageData {
        width: source_width,
        height,
        pixel_format: settings.pixel_format,
        color_space,
        data: v8::TracedReference::new(scope, data_obj),
      })
    } else {
      // Overload: new ImageData(sw, sh [, settings])
      let source_width = convert_unsigned_long(scope, arg0, "Argument 1")?;
      let source_height = convert_unsigned_long(scope, arg1, "Argument 2")?;
      let settings_value = arg2.unwrap_or_else(|| v8::undefined(scope).into());
      let settings = convert_settings(scope, settings_value, "Argument 3")?;

      if source_width == 0 {
        return Err(ImageDataError::ZeroWidth);
      }
      if source_height == 0 {
        return Err(ImageDataError::ZeroHeight);
      }

      let data_obj = alloc_typed_array(
        scope,
        settings.pixel_format,
        source_width,
        source_height,
      )?;
      let color_space =
        settings.color_space.unwrap_or(PredefinedColorSpace::Srgb);

      Ok(ImageData {
        width: source_width,
        height: source_height,
        pixel_format: settings.pixel_format,
        color_space,
        data: v8::TracedReference::new(scope, data_obj),
      })
    }
  }

  #[fast]
  #[getter]
  fn width(&self) -> u32 {
    self.width
  }

  #[fast]
  #[getter]
  fn height(&self) -> u32 {
    self.height
  }

  #[getter]
  fn data<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    self.data.get(scope).unwrap()
  }

  #[getter]
  #[string]
  fn pixel_format(&self) -> &'static str {
    self.pixel_format.name()
  }

  #[getter]
  #[string]
  fn color_space(&self) -> &'static str {
    self.color_space.name()
  }
}
