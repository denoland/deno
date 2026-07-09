// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::WebIdlConverter;
use vello::kurbo::Affine;
use vello::kurbo::Vec2;
use vello::peniko;

use crate::canvas2d::error::Canvas2DError;

/// Parsed createPattern repetition mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PatternRepetition {
  pub x_extend: peniko::Extend,
  pub y_extend: peniko::Extend,
}

/// Parses the createPattern repetition argument.
pub fn parse_repetition(s: &str) -> Result<PatternRepetition, Canvas2DError> {
  match s {
    "" | "repeat" => Ok(PatternRepetition {
      x_extend: peniko::Extend::Repeat,
      y_extend: peniko::Extend::Repeat,
    }),
    "repeat-x" => Ok(PatternRepetition {
      x_extend: peniko::Extend::Repeat,
      y_extend: peniko::Extend::Pad,
    }),
    "repeat-y" => Ok(PatternRepetition {
      x_extend: peniko::Extend::Pad,
      y_extend: peniko::Extend::Repeat,
    }),
    "no-repeat" => Ok(PatternRepetition {
      x_extend: peniko::Extend::Pad,
      y_extend: peniko::Extend::Pad,
    }),
    "null" | "undefined" => Err(Canvas2DError::PatternSyntax),
    _ => Err(Canvas2DError::PatternSyntax),
  }
}

pub struct CanvasPattern {
  pub(super) image: peniko::ImageData,
  pub(super) x_extend: peniko::Extend,
  pub(super) y_extend: peniko::Extend,
  pub(super) transform: RefCell<Affine>,
  /// Offset of the source content within the padded image.
  pub(super) content_offset: Vec2,
}

/// Pads non-repeating axes so `Extend::Pad` behaves like decal.
pub fn pad_pattern_image(
  pixels: &[u8],
  width: u32,
  height: u32,
  pad_x: bool,
  pad_y: bool,
) -> (Vec<u8>, u32, u32, Vec2) {
  if !pad_x && !pad_y {
    return (pixels.to_vec(), width, height, Vec2::ZERO);
  }
  let ox = if pad_x { 1 } else { 0 };
  let oy = if pad_y { 1 } else { 0 };
  let new_width = width + 2 * ox;
  let new_height = height + 2 * oy;
  let mut buf = vec![0u8; (new_width * new_height * 4) as usize];
  for y in 0..height {
    let src_start = (y * width * 4) as usize;
    let src_row = &pixels[src_start..src_start + (width * 4) as usize];
    let dst_start = (((y + oy) * new_width + ox) * 4) as usize;
    buf[dst_start..dst_start + (width * 4) as usize].copy_from_slice(src_row);
  }
  (buf, new_width, new_height, Vec2::new(ox as f64, oy as f64))
}

// SAFETY: CanvasPattern is only accessed from the JS thread.
unsafe impl GarbageCollected for CanvasPattern {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CanvasPattern"
  }
}

#[op2]
impl CanvasPattern {
  #[constructor]
  #[cppgc]
  fn new() -> Result<CanvasPattern, Canvas2DError> {
    Err(Canvas2DError::IllegalConstructor)
  }

  // WebIdlConverter for DOMMatrix2DInit reads the a/b/c/d/e/f properties off
  // the argument, which for a DOMMatrix object are themselves ops -- so
  // this op can end up calling back into another op before it returns.
  #[fast]
  #[reentrant]
  #[required(0)]
  #[undefined]
  fn set_transform<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    transform: Option<v8::Local<'s, v8::Value>>,
  ) -> Result<(), Canvas2DError> {
    let v = transform.unwrap_or_else(|| v8::undefined(scope).into());
    let init = crate::geometry::DOMMatrix2DInit::convert(
      scope,
      v,
      "Failed to execute 'setTransform' on 'CanvasPattern'".into(),
      (|| "Argument 1".into()).into(),
      &Default::default(),
    )?;
    *self.transform.borrow_mut() = Affine::new(init.to_affine()?);
    Ok(())
  }
}
