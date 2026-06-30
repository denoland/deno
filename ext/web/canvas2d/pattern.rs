// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::WebIdlConverter;
use vello::kurbo;
use vello::peniko;

use crate::canvas2d::error::Canvas2DError;

/// Parsed repetition modes for Canvas 2D createPattern().
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PatternRepetition {
  pub x_extend: peniko::Extend,
  pub y_extend: peniko::Extend,
}

/// Parses the repetition argument for createPattern().
///
/// `null` should be normalized to `""` by the caller before invoking this function.
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
  pub(super) transform: RefCell<kurbo::Affine>,
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

  #[fast]
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
      Default::default(),
      (|| "".into()).into(),
      &Default::default(),
    )?;
    let (a, b, c, d, e, f) = init.to_affine()?;
    *self.transform.borrow_mut() = kurbo::Affine::new([a, b, c, d, e, f]);
    Ok(())
  }
}
