// Copyright 2018-2026 the Deno authors. MIT license.

use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::cppgc;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_error::JsErrorBox;

use super::error::Canvas2DError;
use super::text::MetricsRect;
use super::text::TextMeasurement;
use super::text_cluster::TextCluster;
use super::text_cluster::TextClusterOptions;
use crate::geometry::DOMRectReadOnly;

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
  /// The text and the drawing styles the range and cluster APIs answer
  /// against; `TextMetrics` is an opaque object for their sake.
  pub(super) measurement: Rc<TextMeasurement>,
}

// SAFETY: TextMetrics holds plain f64 values and the retained measurement,
// neither of which reference the V8 heap; safe to GC.
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

  #[required(2)]
  fn get_selection_rects<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    #[webidl(options(enforce_range = true))] start: u32,
    #[webidl(options(enforce_range = true))] end: u32,
  ) -> Result<v8::Local<'a, v8::Value>, Canvas2DError> {
    let rects = self.measurement.selection_rects(start, end)?;
    let elements = rects
      .into_iter()
      .map(|rect| make_rect(scope, rect).into())
      .collect::<Vec<v8::Local<v8::Value>>>();
    Ok(v8::Array::new_with_elements(scope, &elements).into())
  }

  #[required(2)]
  #[cppgc]
  fn get_actual_bounding_box(
    &self,
    #[webidl(options(enforce_range = true))] start: u32,
    #[webidl(options(enforce_range = true))] end: u32,
  ) -> Result<DOMRectReadOnly, Canvas2DError> {
    let rect = self.measurement.actual_bounding_box(start, end)?;
    Ok(DOMRectReadOnly::from_values(
      rect.x,
      rect.y,
      rect.width,
      rect.height,
    ))
  }

  #[required(1)]
  fn get_index_from_offset(&self, #[webidl] offset: f64) -> u32 {
    self.measurement.index_from_offset(offset)
  }

  /// Both overloads. WebIDL picks between them by argument count, since only
  /// the range form can take two or more.
  ///
  /// https://html.spec.whatwg.org/multipage/canvas.html#dom-textmetrics-gettextclusters
  #[required(0)]
  fn get_text_clusters<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
    start_or_options: Option<v8::Local<'a, v8::Value>>,
    end: Option<v8::Local<'a, v8::Value>>,
    options: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<v8::Local<'a, v8::Value>, Canvas2DError> {
    const PREFIX: &str = "Failed to execute 'getTextClusters' on 'TextMetrics'";

    let (start, end, options) = match end {
      // getTextClusters(start, end [, options])
      Some(end) => {
        let start =
          convert_index(scope, PREFIX, "parameter 1", start_or_options)?;
        let end = convert_index(scope, PREFIX, "parameter 2", Some(end))?;
        (start, end, options)
      }
      // getTextClusters([ options ])
      None => (0, self.measurement.utf16_len(), start_or_options),
    };
    let options = TextClusterOptions::from_value(scope, PREFIX, options)?;

    let measurement = &self.measurement;
    let align = options.align.unwrap_or_else(|| measurement.text_align());
    let baseline = options
      .baseline
      .unwrap_or_else(|| measurement.text_baseline());
    let clusters = measurement.text_clusters(start, end, align, baseline)?;

    let elements = clusters
      .into_iter()
      .map(|geometry| {
        let cluster = TextCluster {
          measurement: Rc::clone(measurement),
          start: geometry.start,
          end: geometry.end,
          x: geometry.x,
          y: geometry.y,
          align,
          baseline,
        };
        cppgc::make_cppgc_object(scope, cluster).into()
      })
      .collect::<Vec<v8::Local<v8::Value>>>();
    Ok(v8::Array::new_with_elements(scope, &elements).into())
  }
}

fn make_rect<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  rect: MetricsRect,
) -> v8::Local<'a, v8::Object> {
  cppgc::make_cppgc_object(
    scope,
    DOMRectReadOnly::from_values(rect.x, rect.y, rect.width, rect.height),
  )
}

/// `unsigned long` with `[EnforceRange]`, so that an out-of-range index is a
/// `TypeError` from the binding rather than an `IndexSizeError`.
fn convert_index<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  prefix: &'static str,
  context: &'static str,
  value: Option<v8::Local<'a, v8::Value>>,
) -> Result<u32, Canvas2DError> {
  let value = value.unwrap_or_else(|| v8::undefined(scope).into());
  let options = deno_core::webidl::IntOptions {
    clamp: false,
    enforce_range: true,
  };
  Ok(deno_core::webidl::WebIdlConverter::convert(
    scope,
    value,
    prefix.into(),
    deno_core::webidl::ContextFn::new(|| context.into()),
    &options,
  )?)
}
