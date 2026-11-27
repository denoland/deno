// Copyright 2018-2026 the Deno authors. MIT license.

use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_error::JsErrorBox;

use super::state::TextAlign;
use super::state::TextBaseline;
use super::text::TextMeasurement;

/// https://html.spec.whatwg.org/multipage/canvas.html#dictdef-textclusteroptions
#[derive(WebIDL, Default)]
#[webidl(dictionary)]
pub struct TextClusterOptions {
  #[webidl(default = None)]
  pub align: Option<TextAlign>,
  #[webidl(default = None)]
  pub baseline: Option<TextBaseline>,
  #[webidl(default = None)]
  pub x: Option<f64>,
  #[webidl(default = None)]
  pub y: Option<f64>,
}

impl TextClusterOptions {
  /// Converts an argument that `getTextClusters()` overload resolution handed
  /// over as a raw value.
  pub(super) fn from_value<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    prefix: &'static str,
    value: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<Self, WebIdlError> {
    let Some(value) = value else {
      return Ok(Self::default());
    };
    Self::convert(
      scope,
      value,
      prefix.into(),
      ContextFn::new(|| "options".into()),
      &(),
    )
  }
}

/// One minimal rendering unit of a measured piece of text.
///
/// Opaque: it keeps the whole text and the `CanvasTextDrawingStyles` of the
/// `measureText()` call that produced it, so that `fillTextCluster()` shapes it
/// in the context it was measured in rather than the current one.
///
/// https://html.spec.whatwg.org/multipage/canvas.html#textcluster
pub struct TextCluster {
  pub(super) measurement: Rc<TextMeasurement>,
  pub(super) start: u32,
  pub(super) end: u32,
  pub(super) x: f64,
  pub(super) y: f64,
  pub(super) align: TextAlign,
  pub(super) baseline: TextBaseline,
}

// SAFETY: TextCluster holds plain values and the retained measurement, neither
// of which reference the V8 heap; safe to GC.
unsafe impl GarbageCollected for TextCluster {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TextCluster"
  }
}

#[op2]
impl TextCluster {
  #[constructor]
  #[cppgc]
  fn new() -> Result<TextCluster, JsErrorBox> {
    Err(JsErrorBox::type_error("Illegal constructor"))
  }

  #[getter]
  fn x(&self) -> f64 {
    self.x
  }

  #[getter]
  fn y(&self) -> f64 {
    self.y
  }

  #[getter]
  fn start(&self) -> u32 {
    self.start
  }

  #[getter]
  fn end(&self) -> u32 {
    self.end
  }

  #[getter]
  #[string]
  fn align(&self) -> &'static str {
    self.align.as_str()
  }

  #[getter]
  #[string]
  fn baseline(&self) -> &'static str {
    self.baseline.as_str()
  }
}
