// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::GarbageCollected;
use deno_core::op2;
use deno_core::v8::cppgc::Visitor;
use deno_error::JsErrorBox;

/// Metrics for a piece of text as defined by the Canvas 2D specification.
pub struct TextMetrics {
  pub width: f64,
  pub actual_bounding_box_left: f64,
  pub actual_bounding_box_right: f64,
  pub font_bounding_box_ascent: f64,
  pub font_bounding_box_descent: f64,
  pub actual_bounding_box_ascent: f64,
  pub actual_bounding_box_descent: f64,
  pub em_height_ascent: f64,
  pub em_height_descent: f64,
  pub hanging_baseline: f64,
  pub alphabetic_baseline: f64,
  pub ideographic_baseline: f64,
}

// SAFETY: TextMetrics holds only plain f64 values; safe to GC.
unsafe impl GarbageCollected for TextMetrics {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TextMetrics"
  }
}

#[op2]
impl TextMetrics {
  #[constructor]
  #[cppgc]
  fn new() -> Result<TextMetrics, JsErrorBox> {
    Err(JsErrorBox::type_error("Illegal constructor"))
  }

  #[getter]
  fn width(&self) -> f64 {
    self.width
  }

  #[getter]
  fn actual_bounding_box_left(&self) -> f64 {
    self.actual_bounding_box_left
  }

  #[getter]
  fn actual_bounding_box_right(&self) -> f64 {
    self.actual_bounding_box_right
  }

  #[getter]
  fn font_bounding_box_ascent(&self) -> f64 {
    self.font_bounding_box_ascent
  }

  #[getter]
  fn font_bounding_box_descent(&self) -> f64 {
    self.font_bounding_box_descent
  }

  #[getter]
  fn actual_bounding_box_ascent(&self) -> f64 {
    self.actual_bounding_box_ascent
  }

  #[getter]
  fn actual_bounding_box_descent(&self) -> f64 {
    self.actual_bounding_box_descent
  }

  #[getter]
  fn em_height_ascent(&self) -> f64 {
    self.em_height_ascent
  }

  #[getter]
  fn em_height_descent(&self) -> f64 {
    self.em_height_descent
  }

  #[getter]
  fn hanging_baseline(&self) -> f64 {
    self.hanging_baseline
  }

  #[getter]
  fn alphabetic_baseline(&self) -> f64 {
    self.alphabetic_baseline
  }

  #[getter]
  fn ideographic_baseline(&self) -> f64 {
    self.ideographic_baseline
  }
}
