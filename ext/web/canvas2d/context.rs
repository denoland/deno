// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::UnrestrictedDouble;
use deno_core::webidl::WebIdlConverter;
use deno_error::JsErrorBox;
use deno_image::image::DynamicImage;
use deno_image::image::GenericImageView;
use deno_image::image::Rgba;
use deno_image::image::RgbaImage;
use parley::FontContext;
use parley::LayoutContext;
use parley::PositionedLayoutItem;
use vello::kurbo;
use vello::kurbo::Affine;
use vello::kurbo::BezPath;
use vello::kurbo::Cap;
use vello::kurbo::Join;
use vello::kurbo::ParamCurveNearest;
use vello::kurbo::PathEl;
use vello::kurbo::Point;
use vello::kurbo::Rect;
use vello::kurbo::Shape;
use vello::kurbo::Stroke;
use vello::kurbo::StrokeOpts;
use vello::peniko;

use super::filter::CanvasLayerFilterPrimitive;
use super::filter::parse_filter_input;
use super::renderer::CpuRenderer;
pub(crate) use super::renderer::DenoCanvasBackend;
use super::renderer::SharedRenderer;
use super::renderer::render_scene;
use super::renderer::render_scene_to_texture_view;
use super::state::Canvas2DSettings;
use super::state::ClipEntry;
use super::state::DrawingBackend;
use super::state::DrawingState;
use super::state::FillStrokeStyle;
use super::state::FilterStyle;
use super::state::GlobalCompositeOperation;
use super::state::ImageSmoothingQuality;
use super::state::LineCap;
use super::state::LineJoin;
use super::state::StateStackEntry;
use super::state::TextAlign;
use super::state::TextBaseline;
use super::text::build_text_layout;
use super::text::compute_baseline_y;
use super::text::compute_text_metrics;
use crate::canvas2d::TextMetrics;
use crate::canvas2d::error::Canvas2DError;
use crate::canvas2d::gradient::CanvasGradient;
use crate::canvas2d::gradient::build_conic_gradient;
use crate::canvas2d::gradient::build_linear_gradient;
use crate::canvas2d::gradient::build_radial_gradient;
use crate::canvas2d::image::image_data_from_pixels;
use crate::canvas2d::image::image_data_from_premultiplied_pixels;
use crate::canvas2d::image::resolve_canvas_image_source;
use crate::canvas2d::image::unpremultiply_rgba;
use crate::canvas2d::path::Path2D;
use crate::canvas2d::path::parse_round_rect_radii;
use crate::canvas2d::path::path_arc;
use crate::canvas2d::path::path_arc_to;
use crate::canvas2d::path::path_bezier_curve_to;
use crate::canvas2d::path::path_ellipse;
use crate::canvas2d::path::path_line_to;
use crate::canvas2d::path::path_move_to;
use crate::canvas2d::path::path_quadratic_curve_to;
use crate::canvas2d::path::path_rect;
use crate::canvas2d::path::path_round_rect;
use crate::canvas2d::path::transform_path;
use crate::canvas2d::pattern::CanvasPattern;
use crate::canvas2d::pattern::pad_pattern_image;
use crate::canvas2d::pattern::parse_repetition;
use crate::css::color::ParsedColor;
use crate::css::color::parse_css_color;
use crate::css::color::serialize_color_for_canvas;
use crate::css::filter::FilterValueListParser;
use crate::css::filter::ParserInput as FilterParserInput;
use crate::css::font::FontState;
use crate::css::font::TextDirection;
use crate::css::font::parse_css_font;
use crate::css::font::parse_css_spacing;
use crate::image_data::ImageData;

pub const CONTEXT_ID: &str = "2d";
pub const UNSTABLE_FEATURE_NAME: &str = "canvas2d";

/// Pixel readbacks (getImageData / putImageData / convertToBlob) tolerated
/// on the GPU backend before falling back to CPU, per Chromium's heuristic.
const GPU_READBACK_FALLBACK_THRESHOLD: u32 = 2;

/// Canvases smaller than this area render on the CPU, where they beat the
/// GPU's per-draw overhead. Matches Blink's 128 * 129 heuristic.
const MIN_GPU_ACCELERATED_AREA: u64 = 128 * 129;

pub struct OffscreenCanvasRenderingContext2D {
  canvas: v8::Global<v8::Object>,
  data: deno_webgpu::canvas::ContextData,

  drawing: RefCell<DrawingBackend>,
  renderer: SharedRenderer,

  font_ctx: Rc<RefCell<FontContext>>,
  layout_ctx: Rc<RefCell<LayoutContext<()>>>,

  state: RefCell<DrawingState>,
  state_stack: RefCell<Vec<StateStackEntry>>,
  layer_depth: std::cell::Cell<usize>,
  clip_stack: RefCell<Vec<ClipEntry>>,
  current_path: RefCell<BezPath>,

  settings: Canvas2DSettings,

  /// Pixel readbacks seen so far; drives the one-way GPU -> CPU fallback
  /// in `increment_readback_and_check_fallback`. Never reset.
  readback_count: std::cell::Cell<u32>,
}

// SAFETY: OffscreenCanvasRenderingContext2D is only accessed from the JS thread.
unsafe impl GarbageCollected for OffscreenCanvasRenderingContext2D {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OffscreenCanvasRenderingContext2D"
  }
}

#[op2]
impl OffscreenCanvasRenderingContext2D {
  #[constructor]
  #[cppgc]
  fn new() -> Result<OffscreenCanvasRenderingContext2D, Canvas2DError> {
    Err(Canvas2DError::IllegalConstructor)
  }

  #[getter]
  fn canvas(&self) -> v8::Global<v8::Object> {
    self.canvas.clone()
  }

  #[getter]
  fn fill_style<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
  ) -> v8::Local<'a, v8::Value> {
    match &self.state.borrow().fill_style {
      FillStrokeStyle::Color(c) => {
        let s = serialize_color_for_canvas(c);
        v8::String::new(scope, &s).unwrap().into()
      }
      FillStrokeStyle::Gradient(g) | FillStrokeStyle::Pattern(g) => {
        v8::Local::new(scope, g).into()
      }
    }
  }

  #[reentrant]
  #[setter]
  fn fill_style<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
    value: v8::Local<'a, v8::Value>,
  ) {
    if let Some(style) = parse_fill_stroke_style(scope, value) {
      self.state.borrow_mut().fill_style = style;
    }
  }

  #[getter]
  fn stroke_style<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
  ) -> v8::Local<'a, v8::Value> {
    match &self.state.borrow().stroke_style {
      FillStrokeStyle::Color(c) => {
        let s = serialize_color_for_canvas(c);
        v8::String::new(scope, &s).unwrap().into()
      }
      FillStrokeStyle::Gradient(g) | FillStrokeStyle::Pattern(g) => {
        v8::Local::new(scope, g).into()
      }
    }
  }

  #[reentrant]
  #[setter]
  fn stroke_style<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
    value: v8::Local<'a, v8::Value>,
  ) {
    if let Some(style) = parse_fill_stroke_style(scope, value) {
      self.state.borrow_mut().stroke_style = style;
    }
  }

  #[getter]
  fn global_alpha(&self) -> f64 {
    self.state.borrow().global_alpha as f64
  }

  #[setter]
  fn global_alpha(&self, #[webidl] value: UnrestrictedDouble) {
    if !value.is_finite() || *value < 0.0 || *value > 1.0 {
      return;
    }

    self.state.borrow_mut().global_alpha = *value as f32;
  }

  #[getter]
  #[string]
  fn font(&self) -> String {
    self.state.borrow().font_state.to_css_string()
  }

  #[setter]
  fn font(&self, #[webidl] value: String) {
    if let Some(state) = parse_css_font(&value) {
      let mut s = self.state.borrow_mut();
      // Keep non-shorthand canvas text state.
      s.font_state = FontState {
        direction: s.font_state.direction,
        font_kerning: s.font_state.font_kerning,
        letter_spacing: s.font_state.letter_spacing,
        word_spacing: s.font_state.word_spacing,
        text_rendering: s.font_state.text_rendering,
        ..state
      };
    }
  }

  #[getter]
  #[string]
  fn text_align(&self) -> &'static str {
    self.state.borrow().text_align.as_str()
  }

  #[setter]
  fn text_align(&self, #[webidl] value: String) {
    self.state.borrow_mut().text_align = match value.as_str() {
      "start" => TextAlign::Start,
      "end" => TextAlign::End,
      "left" => TextAlign::Left,
      "right" => TextAlign::Right,
      "center" => TextAlign::Center,
      _ => return,
    };
  }

  #[getter]
  #[string]
  fn text_baseline(&self) -> &'static str {
    self.state.borrow().text_baseline.as_str()
  }

  #[setter]
  fn text_baseline(&self, #[webidl] value: String) {
    self.state.borrow_mut().text_baseline = match value.as_str() {
      "top" => TextBaseline::Top,
      "hanging" => TextBaseline::Hanging,
      "middle" => TextBaseline::Middle,
      "alphabetic" => TextBaseline::Alphabetic,
      "ideographic" => TextBaseline::Ideographic,
      "bottom" => TextBaseline::Bottom,
      _ => return,
    };
  }

  #[getter]
  #[string]
  fn direction(&self) -> &'static str {
    self.state.borrow().font_state.direction.as_str()
  }

  #[setter]
  fn direction(&self, #[webidl] value: String) {
    let d = match value.as_str() {
      "inherit" => crate::css::font::TextDirection::Inherit,
      "ltr" => crate::css::font::TextDirection::Ltr,
      "rtl" => crate::css::font::TextDirection::Rtl,
      _ => return,
    };
    self.state.borrow_mut().font_state.direction = d;
  }

  #[getter]
  #[string]
  fn lang(&self) -> String {
    self.state.borrow().lang.clone()
  }

  #[setter]
  fn lang(&self, #[webidl] value: String) {
    self.state.borrow_mut().lang = value;
  }

  #[getter]
  #[string]
  fn font_kerning(&self) -> &'static str {
    self.state.borrow().font_state.font_kerning.as_str()
  }

  #[setter]
  fn font_kerning(&self, #[webidl] value: String) {
    let k = match value.as_str() {
      "auto" => crate::css::font::FontKerning::Auto,
      "normal" => crate::css::font::FontKerning::Normal,
      "none" => crate::css::font::FontKerning::None,
      _ => return,
    };
    self.state.borrow_mut().font_state.font_kerning = k;
  }

  #[getter]
  #[string]
  fn font_stretch(&self) -> &'static str {
    crate::css::font::stretch_to_css_str(self.state.borrow().font_state.stretch)
  }

  #[setter]
  fn font_stretch(&self, #[webidl] value: String) {
    if let Some(s) = crate::css::font::parse_css_stretch(&value) {
      self.state.borrow_mut().font_state.stretch = s;
    }
  }

  #[getter]
  #[string]
  fn font_variant_caps(&self) -> &'static str {
    self.state.borrow().font_state.font_variant_caps.as_str()
  }

  #[setter]
  fn font_variant_caps(&self, #[webidl] value: String) {
    let v = match value.as_str() {
      "normal" => crate::css::font::FontVariantCaps::Normal,
      "small-caps" => crate::css::font::FontVariantCaps::SmallCaps,
      "all-small-caps" => crate::css::font::FontVariantCaps::AllSmallCaps,
      "petite-caps" => crate::css::font::FontVariantCaps::PetiteCaps,
      "all-petite-caps" => crate::css::font::FontVariantCaps::AllPetiteCaps,
      "unicase" => crate::css::font::FontVariantCaps::Unicase,
      "titling-caps" => crate::css::font::FontVariantCaps::TitlingCaps,
      _ => return,
    };
    self.state.borrow_mut().font_state.font_variant_caps = v;
  }

  #[getter]
  #[string]
  fn letter_spacing(&self) -> String {
    self
      .state
      .borrow()
      .font_state
      .letter_spacing
      .to_css_string()
  }

  #[setter]
  fn letter_spacing(&self, #[webidl] value: String) {
    if let Some(spacing) = parse_css_spacing(&value) {
      self.state.borrow_mut().font_state.letter_spacing = spacing;
    }
  }

  #[getter]
  #[string]
  fn word_spacing(&self) -> String {
    self.state.borrow().font_state.word_spacing.to_css_string()
  }

  #[setter]
  fn word_spacing(&self, #[webidl] value: String) {
    if let Some(spacing) = parse_css_spacing(&value) {
      self.state.borrow_mut().font_state.word_spacing = spacing;
    }
  }

  #[getter]
  #[string]
  fn text_rendering(&self) -> &'static str {
    self.state.borrow().font_state.text_rendering.as_str()
  }

  #[setter]
  fn text_rendering(&self, #[webidl] value: String) {
    let r = match value.as_str() {
      "auto" => crate::css::font::TextRendering::Auto,
      "optimizeSpeed" => crate::css::font::TextRendering::OptimizeSpeed,
      "optimizeLegibility" => {
        crate::css::font::TextRendering::OptimizeLegibility
      }
      "geometricPrecision" => {
        crate::css::font::TextRendering::GeometricPrecision
      }
      _ => return,
    };
    self.state.borrow_mut().font_state.text_rendering = r;
  }

  #[required(4)]
  #[undefined]
  fn fill_rect(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] w: UnrestrictedDouble,
    #[webidl] h: UnrestrictedDouble,
  ) {
    if !x.is_finite()
      || !y.is_finite()
      || !w.is_finite()
      || !h.is_finite()
      || *w == 0.0
      || *h == 0.0
    {
      return;
    }

    let (
      op,
      alpha,
      shadow_color,
      shadow_xform,
      brush,
      brush_transform,
      transform,
    ) = {
      let state = self.state.borrow();
      let op = state.global_composite_operation;
      let alpha = state.global_alpha;
      let shadow = has_shadow(&state);
      let shadow_color = state.shadow_color.to_srgb8();
      let shadow_xform = if shadow {
        Some(shadow_transform(&state, state.transform))
      } else {
        None
      };
      let (brush, brush_transform) =
        resolve_brush(scope, &state.fill_style, 1.0);
      let transform = state.transform;
      (
        op,
        alpha,
        shadow_color,
        shadow_xform,
        brush,
        brush_transform,
        transform,
      )
    };

    let rect = Rect::new(*x, *y, *x + *w, *y + *h);
    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let Some(st) = shadow_xform {
      draw_shadow(&mut drawing, width, height, shadow_color, |d| {
        fill_on(
          d,
          &rect,
          peniko::Fill::NonZero,
          st,
          brush.clone(),
          brush_transform,
        );
      });
      // Composite shadow before the source shape.
      if has_layer {
        pop_compositing_layer(&mut drawing);
        push_compositing_layer(&mut drawing, op, alpha, width, height);
      }
    }
    fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      transform,
      brush,
      brush_transform,
    );
    if has_layer {
      pop_compositing_layer(&mut drawing);
    }
  }

  #[fast]
  #[undefined]
  fn clear_rect(&self, x: f64, y: f64, w: f64, h: f64) {
    if !x.is_finite()
      || !y.is_finite()
      || !w.is_finite()
      || !h.is_finite()
      || w == 0.0
      || h == 0.0
    {
      return;
    }

    let transform = self.state.borrow().transform;
    let rect = Rect::new(x, y, x + w, y + h);
    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    // clearRect ignores compositing and alpha.
    push_compositing_layer(
      &mut drawing,
      GlobalCompositeOperation::DestinationOut,
      1.0,
      width,
      height,
    );
    fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      transform,
      peniko::Brush::Solid(peniko::Color::BLACK),
      None,
    );
    pop_compositing_layer(&mut drawing);
  }

  #[required(3)]
  #[undefined]
  fn fill_text(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] text: String,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] max_width: Option<UnrestrictedDouble>,
  ) {
    // Nothing is drawn for non-finite coordinates.
    if !x.is_finite() || !y.is_finite() {
      return;
    }

    self.draw_text(scope, &text, *x, *y, max_width.map(|v| *v), false);
  }

  #[required(3)]
  #[undefined]
  fn stroke_text(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] text: String,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] max_width: Option<UnrestrictedDouble>,
  ) {
    // Nothing is drawn for non-finite coordinates.
    if !x.is_finite() || !y.is_finite() {
      return;
    }

    self.draw_text(scope, &text, *x, *y, max_width.map(|v| *v), true);
  }

  #[required(1)]
  #[cppgc]
  fn measure_text(&self, #[string] text: &str) -> TextMetrics {
    let state = self.state.borrow();
    compute_text_metrics(
      text,
      &state.font_state,
      state.text_align,
      &self.font_ctx,
      &self.layout_ctx,
    )
  }

  #[getter]
  #[string]
  fn global_composite_operation(&self) -> &'static str {
    self.state.borrow().global_composite_operation.as_str()
  }

  #[setter]
  fn global_composite_operation(&self, #[webidl] value: String) {
    let op = match value.as_str() {
      "clear" => GlobalCompositeOperation::Clear,
      "source-over" => GlobalCompositeOperation::SourceOver,
      "source-in" => GlobalCompositeOperation::SourceIn,
      "source-out" => GlobalCompositeOperation::SourceOut,
      "source-atop" => GlobalCompositeOperation::SourceAtop,
      "destination-over" => GlobalCompositeOperation::DestinationOver,
      "destination-in" => GlobalCompositeOperation::DestinationIn,
      "destination-out" => GlobalCompositeOperation::DestinationOut,
      "destination-atop" => GlobalCompositeOperation::DestinationAtop,
      "lighter" => GlobalCompositeOperation::Lighter,
      "copy" => GlobalCompositeOperation::Copy,
      "xor" => GlobalCompositeOperation::Xor,
      "multiply" => GlobalCompositeOperation::Multiply,
      "screen" => GlobalCompositeOperation::Screen,
      "overlay" => GlobalCompositeOperation::Overlay,
      "darken" => GlobalCompositeOperation::Darken,
      "lighten" => GlobalCompositeOperation::Lighten,
      "color-dodge" => GlobalCompositeOperation::ColorDodge,
      "color-burn" => GlobalCompositeOperation::ColorBurn,
      "hard-light" => GlobalCompositeOperation::HardLight,
      "soft-light" => GlobalCompositeOperation::SoftLight,
      "difference" => GlobalCompositeOperation::Difference,
      "exclusion" => GlobalCompositeOperation::Exclusion,
      "hue" => GlobalCompositeOperation::Hue,
      "saturation" => GlobalCompositeOperation::Saturation,
      "color" => GlobalCompositeOperation::Color,
      "luminosity" => GlobalCompositeOperation::Luminosity,
      _ => return,
    };
    self.state.borrow_mut().global_composite_operation = op;
  }

  // TODO(petamoriken): apply CSS filters once Vello GPU supports filter effects
  #[getter]
  fn filter<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    let state = self.state.borrow();
    let FilterStyle::Css(value) = &state.filter_style;
    v8::String::new(scope, value).unwrap().into()
  }

  #[reentrant]
  #[setter]
  fn filter(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
  ) {
    // The DOMString branch of the union: a failed ToString leaves the pending
    // exception to propagate, and an invalid filter string is ignored.
    let Some(value) = value.to_string(scope) else {
      return;
    };

    let value = value.to_rust_string_lossy(scope);
    let functions = {
      let mut parser_input = FilterParserInput::new(&value);
      let result: Result<Vec<_>, _> =
        FilterValueListParser::new(&mut parser_input).collect();
      result.ok()
    };
    if let Some(functions) = functions {
      let mut state = self.state.borrow_mut();
      state.filter_style = FilterStyle::Css(value);
      state.filter = functions.into_iter().map(Into::into).collect();
    }
  }

  #[getter]
  fn image_smoothing_enabled(&self) -> bool {
    self.state.borrow().image_smoothing_enabled
  }

  #[setter]
  fn image_smoothing_enabled(&self, #[webidl] value: bool) {
    self.state.borrow_mut().image_smoothing_enabled = value;
  }

  #[getter]
  #[string]
  fn image_smoothing_quality(&self) -> &'static str {
    self.state.borrow().image_smoothing_quality.as_str()
  }

  #[setter]
  fn image_smoothing_quality(&self, #[webidl] value: String) {
    self.state.borrow_mut().image_smoothing_quality = match value.as_str() {
      "low" => ImageSmoothingQuality::Low,
      "medium" => ImageSmoothingQuality::Medium,
      "high" => ImageSmoothingQuality::High,
      _ => return,
    };
  }

  #[getter]
  fn line_width(&self) -> f64 {
    self.state.borrow().line_width
  }

  #[setter]
  fn line_width(&self, #[webidl] value: UnrestrictedDouble) {
    if !value.is_finite() || *value <= 0.0 {
      return;
    }

    self.state.borrow_mut().line_width = *value;
  }

  #[getter]
  #[string]
  fn line_cap(&self) -> &'static str {
    self.state.borrow().line_cap.as_str()
  }

  #[setter]
  fn line_cap(&self, #[webidl] value: String) {
    self.state.borrow_mut().line_cap = match value.as_str() {
      "butt" => LineCap::Butt,
      "round" => LineCap::Round,
      "square" => LineCap::Square,
      _ => return,
    };
  }

  #[getter]
  #[string]
  fn line_join(&self) -> &'static str {
    self.state.borrow().line_join.as_str()
  }

  #[setter]
  fn line_join(&self, #[webidl] value: String) {
    self.state.borrow_mut().line_join = match value.as_str() {
      "round" => LineJoin::Round,
      "bevel" => LineJoin::Bevel,
      "miter" => LineJoin::Miter,
      _ => return,
    };
  }

  #[getter]
  fn miter_limit(&self) -> f64 {
    self.state.borrow().miter_limit
  }

  #[setter]
  fn miter_limit(&self, #[webidl] value: UnrestrictedDouble) {
    if !value.is_finite() || *value <= 0.0 {
      return;
    }

    self.state.borrow_mut().miter_limit = *value;
  }

  #[getter]
  fn line_dash_offset(&self) -> f64 {
    self.state.borrow().line_dash_offset
  }

  #[setter]
  fn line_dash_offset(&self, #[webidl] value: UnrestrictedDouble) {
    if !value.is_finite() {
      return;
    }

    self.state.borrow_mut().line_dash_offset = *value;
  }

  #[getter]
  fn shadow_blur(&self) -> f64 {
    self.state.borrow().shadow_blur
  }

  #[setter]
  fn shadow_blur(&self, #[webidl] value: UnrestrictedDouble) {
    if !value.is_finite() || *value < 0.0 {
      return;
    }

    self.state.borrow_mut().shadow_blur = *value;
  }

  #[getter]
  #[string]
  fn shadow_color(&self) -> String {
    serialize_color_for_canvas(&self.state.borrow().shadow_color)
  }

  #[setter]
  fn shadow_color(&self, #[webidl] value: String) {
    if let Ok(parsed) = parse_css_color(&value) {
      self.state.borrow_mut().shadow_color = parsed;
    }
  }

  #[getter]
  fn shadow_offset_x(&self) -> f64 {
    self.state.borrow().shadow_offset_x
  }

  #[setter]
  fn shadow_offset_x(&self, #[webidl] value: UnrestrictedDouble) {
    if !value.is_finite() {
      return;
    }

    self.state.borrow_mut().shadow_offset_x = *value;
  }

  #[getter]
  fn shadow_offset_y(&self) -> f64 {
    self.state.borrow().shadow_offset_y
  }

  #[setter]
  fn shadow_offset_y(&self, #[webidl] value: UnrestrictedDouble) {
    if !value.is_finite() {
      return;
    }

    self.state.borrow_mut().shadow_offset_y = *value;
  }

  #[fast]
  #[undefined]
  fn save(&self) {
    self
      .state_stack
      .borrow_mut()
      .push(StateStackEntry::Save(self.state.borrow().clone()));
  }

  #[fast]
  #[undefined]
  fn restore(&self) -> Result<(), Canvas2DError> {
    let mut stack = self.state_stack.borrow_mut();
    match stack.last() {
      None => Ok(()),
      // A beginLayer() with no matching endLayer() sits on top of the
      // stack: restore() must not reach past it to an earlier save().
      Some(StateStackEntry::Layer(..)) => Err(Canvas2DError::InvalidState(
        "restore called with an unclosed layer on top of the stack".into(),
      )),
      Some(StateStackEntry::Save(_)) => {
        let current_clip_depth = self.state.borrow().clip_depth;
        if let Some(StateStackEntry::Save(saved)) = stack.pop() {
          let saved_clip_depth = saved.clip_depth;
          *self.state.borrow_mut() = saved;
          for _ in saved_clip_depth..current_clip_depth {
            pop_compositing_layer(&mut self.drawing.borrow_mut());
          }
        }
        Ok(())
      }
    }
  }

  #[fast]
  #[undefined]
  fn reset(&self) {
    *self.state.borrow_mut() = DrawingState::default();
    self.state_stack.borrow_mut().clear();
    self.layer_depth.set(0);
    self.clip_stack.borrow_mut().clear();
    self.current_path.borrow_mut().truncate(0);
    let (width, height) = self.data.dimensions();
    self.drawing.borrow_mut().reset(width, height);
  }

  #[fast]
  #[reentrant]
  #[undefined]
  fn begin_layer<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
    options: v8::Local<'a, v8::Value>,
  ) -> Result<(), Canvas2DError> {
    let layer_filter = parse_begin_layer_options(scope, options)?;

    let current_state = self.state.borrow().clone();
    let op = current_state.global_composite_operation;
    let alpha = current_state.global_alpha;

    self.layer_depth.set(self.layer_depth.get() + 1);

    {
      let mut state = self.state.borrow_mut();
      state.global_alpha = 1.0;
      state.global_composite_operation = GlobalCompositeOperation::SourceOver;
      state.shadow_color = ParsedColor::TRANSPARENT;
      state.shadow_offset_x = 0.0;
      state.shadow_offset_y = 0.0;
      state.shadow_blur = 0.0;
      state.filter_style = FilterStyle::Css(String::from("none"));
      state.filter = layer_filter;
    }

    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let pushed = push_compositing_layer(&mut drawing, op, alpha, width, height);

    self
      .state_stack
      .borrow_mut()
      .push(StateStackEntry::Layer(current_state, pushed));

    Ok(())
  }

  #[fast]
  #[undefined]
  fn end_layer(&self) -> Result<(), Canvas2DError> {
    let depth = self.layer_depth.get();
    if depth == 0 {
      return Err(Canvas2DError::InvalidState(
        "endLayer called without matching beginLayer".into(),
      ));
    }

    // A save() with no matching restore() sits on top of the stack:
    // endLayer() must not reach past it to an earlier beginLayer().
    let mut stack = self.state_stack.borrow_mut();
    match stack.last() {
      Some(StateStackEntry::Layer(..)) => {}
      _ => {
        return Err(Canvas2DError::InvalidState(
          "endLayer called without matching beginLayer".into(),
        ));
      }
    }
    let Some(StateStackEntry::Layer(saved_state, pushed)) = stack.pop() else {
      unreachable!("just matched Layer above");
    };

    *self.state.borrow_mut() = saved_state;
    self.layer_depth.set(depth - 1);
    if pushed {
      pop_compositing_layer(&mut self.drawing.borrow_mut());
    }
    Ok(())
  }

  #[fast]
  fn is_context_lost(&self) -> bool {
    false
  }

  #[fast]
  #[undefined]
  fn begin_path(&self) {
    self.current_path.borrow_mut().truncate(0);
  }

  #[fast]
  #[undefined]
  fn close_path(&self) {
    let mut path = self.current_path.borrow_mut();
    if !path.elements().is_empty() {
      path.close_path();
    }
  }

  #[required(2)]
  #[undefined]
  fn move_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() {
      return;
    }

    let transform = self.state.borrow().transform;
    path_move_to(&mut self.current_path.borrow_mut(), transform, *x, *y);
  }

  #[required(2)]
  #[undefined]
  fn line_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() {
      return;
    }

    let transform = self.state.borrow().transform;
    path_line_to(&mut self.current_path.borrow_mut(), transform, *x, *y);
  }

  #[required(6)]
  #[undefined]
  fn bezier_curve_to(
    &self,
    #[webidl] cp1x: UnrestrictedDouble,
    #[webidl] cp1y: UnrestrictedDouble,
    #[webidl] cp2x: UnrestrictedDouble,
    #[webidl] cp2y: UnrestrictedDouble,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if !cp1x.is_finite()
      || !cp1y.is_finite()
      || !cp2x.is_finite()
      || !cp2y.is_finite()
      || !x.is_finite()
      || !y.is_finite()
    {
      return;
    }

    let transform = self.state.borrow().transform;
    path_bezier_curve_to(
      &mut self.current_path.borrow_mut(),
      transform,
      *cp1x,
      *cp1y,
      *cp2x,
      *cp2y,
      *x,
      *y,
    );
  }

  #[required(4)]
  #[undefined]
  fn quadratic_curve_to(
    &self,
    #[webidl] cpx: UnrestrictedDouble,
    #[webidl] cpy: UnrestrictedDouble,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if !cpx.is_finite() || !cpy.is_finite() || !x.is_finite() || !y.is_finite()
    {
      return;
    }

    let transform = self.state.borrow().transform;
    path_quadratic_curve_to(
      &mut self.current_path.borrow_mut(),
      transform,
      *cpx,
      *cpy,
      *x,
      *y,
    );
  }

  #[required(5)]
  #[undefined]
  fn arc(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] radius: UnrestrictedDouble,
    #[webidl] start_angle: UnrestrictedDouble,
    #[webidl] end_angle: UnrestrictedDouble,
    counterclockwise: Option<bool>,
  ) -> Result<(), Canvas2DError> {
    let counterclockwise = counterclockwise.unwrap_or(false);
    // Ignore non-finite values; reject finite negative radii.
    if !x.is_finite()
      || !y.is_finite()
      || !radius.is_finite()
      || !start_angle.is_finite()
      || !end_angle.is_finite()
    {
      return Ok(());
    }
    if *radius < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius));
    }

    let transform = self.state.borrow().transform;
    path_arc(
      &mut self.current_path.borrow_mut(),
      transform,
      *x,
      *y,
      *radius,
      *start_angle,
      *end_angle,
      counterclockwise,
    );
    Ok(())
  }

  #[required(5)]
  #[undefined]
  fn arc_to(
    &self,
    #[webidl] x1: UnrestrictedDouble,
    #[webidl] y1: UnrestrictedDouble,
    #[webidl] x2: UnrestrictedDouble,
    #[webidl] y2: UnrestrictedDouble,
    #[webidl] radius: UnrestrictedDouble,
  ) -> Result<(), Canvas2DError> {
    // Ignore non-finite values; reject finite negative radii.
    if !x1.is_finite()
      || !y1.is_finite()
      || !x2.is_finite()
      || !y2.is_finite()
      || !radius.is_finite()
    {
      return Ok(());
    }
    if *radius < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius));
    }

    let transform = self.state.borrow().transform;
    path_arc_to(
      &mut self.current_path.borrow_mut(),
      transform,
      *x1,
      *y1,
      *x2,
      *y2,
      *radius,
    );
    Ok(())
  }

  #[required(7)]
  #[undefined]
  fn ellipse(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] radius_x: UnrestrictedDouble,
    #[webidl] radius_y: UnrestrictedDouble,
    #[webidl] rotation: UnrestrictedDouble,
    #[webidl] start_angle: UnrestrictedDouble,
    #[webidl] end_angle: UnrestrictedDouble,
    counterclockwise: Option<bool>,
  ) -> Result<(), Canvas2DError> {
    let counterclockwise = counterclockwise.unwrap_or(false);
    // Per spec, non-finite arguments are silently ignored; only a finite
    // negative radius throws IndexSizeError.
    if !x.is_finite()
      || !y.is_finite()
      || !radius_x.is_finite()
      || !radius_y.is_finite()
      || !rotation.is_finite()
      || !start_angle.is_finite()
      || !end_angle.is_finite()
    {
      return Ok(());
    }
    if *radius_x < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius_x));
    }
    if *radius_y < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius_y));
    }

    let transform = self.state.borrow().transform;
    path_ellipse(
      &mut self.current_path.borrow_mut(),
      transform,
      *x,
      *y,
      *radius_x,
      *radius_y,
      *rotation,
      *start_angle,
      *end_angle,
      counterclockwise,
    );
    Ok(())
  }

  #[required(4)]
  #[undefined]
  fn rect(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] w: UnrestrictedDouble,
    #[webidl] h: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
      return;
    }

    let transform = self.state.borrow().transform;
    path_rect(
      &mut self.current_path.borrow_mut(),
      transform,
      *x,
      *y,
      *w,
      *h,
    );
  }

  #[reentrant]
  #[required(4)]
  #[undefined]
  fn round_rect(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] w: UnrestrictedDouble,
    #[webidl] h: UnrestrictedDouble,
    radii: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<(), Canvas2DError> {
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
      return Ok(());
    }

    let radii_val = radii.unwrap_or_else(|| v8::undefined(scope).into());
    // Per spec, a non-finite radius (unlike a negative one) is silently
    // ignored rather than throwing, matching the x/y/w/h check above.
    let corner_radii = match parse_round_rect_radii(scope, radii_val) {
      Ok(radii) => radii,
      Err(Canvas2DError::NonFinite) => return Ok(()),
      Err(e) => return Err(e),
    };
    let transform = self.state.borrow().transform;
    path_round_rect(
      &mut self.current_path.borrow_mut(),
      transform,
      *x,
      *y,
      *w,
      *h,
      &corner_radii,
    );
    Ok(())
  }

  #[required(4)]
  #[undefined]
  fn stroke_rect(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] w: UnrestrictedDouble,
    #[webidl] h: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
      return;
    }
    if *w == 0.0 && *h == 0.0 {
      return;
    }

    // Stroke an explicit rect path so degenerate dimensions still render.
    // This temporary path is built in user space (Affine::IDENTITY); the
    // real transform is applied later by draw_path_stroke.
    let mut path = BezPath::new();
    path_rect(&mut path, Affine::IDENTITY, *x, *y, *w, *h);
    let transform = self.state.borrow().transform;
    self.draw_path_stroke(scope, path, transform, true);
  }

  #[undefined]
  fn fill(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path_or_fill_rule: Option<v8::Local<'_, v8::Value>>,
    #[string] fill_rule: Option<String>,
  ) {
    let (path, rule, is_path2d) =
      self.resolve_path_and_fill_rule(scope, path_or_fill_rule, fill_rule);
    if path.is_empty() {
      return;
    }

    let transform = if is_path2d {
      self.state.borrow().transform
    } else {
      Affine::IDENTITY
    };
    self.draw_path_fill(scope, path, rule, transform);
  }

  #[fast]
  #[undefined]
  fn stroke(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path: Option<v8::Local<'_, v8::Value>>,
  ) {
    let (path, is_path2d) = self.resolve_optional_path(scope, path);
    if path.is_empty() {
      return;
    }

    let transform = self.state.borrow().transform;
    self.draw_path_stroke(scope, path, transform, is_path2d);
  }

  #[undefined]
  fn clip(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path_or_fill_rule: Option<v8::Local<'_, v8::Value>>,
    #[string] fill_rule: Option<String>,
  ) {
    let (path, rule, is_path2d) =
      self.resolve_path_and_fill_rule(scope, path_or_fill_rule, fill_rule);
    // Empty paths clip everything.
    let transform = if is_path2d {
      self.state.borrow().transform
    } else {
      Affine::IDENTITY
    };
    self.apply_clip(path, rule, transform);
  }

  #[fast]
  fn is_point_in_path(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path_or_x: Option<v8::Local<'_, v8::Value>>,
    x_or_y: Option<v8::Local<'_, v8::Value>>,
    y_or_fill_rule: v8::Local<'_, v8::Value>,
    fill_rule: v8::Local<'_, v8::Value>,
  ) -> Result<bool, Canvas2DError> {
    // Preserve explicit null for CanvasFillRule conversion.
    let y_or_fill_rule =
      (!y_or_fill_rule.is_undefined()).then_some(y_or_fill_rule);
    let fill_rule = (!fill_rule.is_undefined()).then_some(fill_rule);
    let (path, x, y, rule, is_path2d) = self.resolve_point_in_path_args(
      scope,
      path_or_x,
      x_or_y,
      y_or_fill_rule,
      fill_rule,
    )?;
    if !x.is_finite() || !y.is_finite() {
      return Ok(false);
    }
    // No inverse CTM means no hit.
    let transform = self.state.borrow().transform;
    if transform.determinant() == 0.0 {
      return Ok(false);
    }

    let p = if is_path2d {
      transform.inverse() * Point::new(x, y)
    } else {
      Point::new(x, y)
    };
    Ok(test_point_in_path(path, p.x, p.y, rule))
  }

  #[fast]
  fn is_point_in_stroke(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path_or_x: Option<v8::Local<'_, v8::Value>>,
    x_or_y: Option<v8::Local<'_, v8::Value>>,
    y: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<bool, Canvas2DError> {
    let (path, x, y, is_path2d) =
      self.resolve_point_in_stroke_args(scope, path_or_x, x_or_y, y)?;
    if !x.is_finite() || !y.is_finite() {
      return Ok(false);
    }
    // No inverse CTM means no hit.
    let transform = self.state.borrow().transform;
    if transform.determinant() == 0.0 {
      return Ok(false);
    }

    // Stroke hit-testing runs in user space.
    let p = transform.inverse() * Point::new(x, y);
    Ok(self.test_point_in_stroke(path, p.x, p.y, transform, is_path2d))
  }

  fn get_transform<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Object> {
    let [a, b, c, d, e, f] = self.state.borrow().transform.as_coeffs();
    let obj = deno_core::cppgc::make_cppgc_empty_object::<
      crate::geometry::DOMMatrix,
    >(scope);
    deno_core::cppgc::wrap_object(
      scope,
      obj,
      crate::geometry::DOMMatrix::new_2d(a, b, c, d, e, f),
    )
  }

  #[undefined]
  fn set_transform<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    a_or_init: Option<v8::Local<'s, v8::Value>>,
    #[webidl] b: Option<UnrestrictedDouble>,
    #[webidl] c: Option<UnrestrictedDouble>,
    #[webidl] d: Option<UnrestrictedDouble>,
    #[webidl] e: Option<UnrestrictedDouble>,
    #[webidl] f: Option<UnrestrictedDouble>,
  ) -> Result<(), Canvas2DError> {
    let transform = match a_or_init {
      Some(v) if v.is_number() => {
        let a = v.number_value(scope).unwrap_or(f64::NAN);
        let provided = 1
          + b.is_some() as u32
          + c.is_some() as u32
          + d.is_some() as u32
          + e.is_some() as u32
          + f.is_some() as u32;
        let (Some(b), Some(c), Some(d), Some(e), Some(f)) = (b, c, d, e, f)
        else {
          return Err(Canvas2DError::MissingArgument {
            required: 6,
            provided,
          });
        };
        [a, *b, *c, *d, *e, *f]
      }
      arg => {
        let v = arg.unwrap_or_else(|| v8::undefined(scope).into());
        let init = crate::geometry::DOMMatrix2DInit::convert(
          scope,
          v,
          Default::default(),
          (|| "".into()).into(),
          &Default::default(),
        )?;
        init.to_affine()?
      }
    };
    if transform.iter().any(|v| !v.is_finite()) {
      return Ok(());
    }
    self.state.borrow_mut().transform = Affine::new(transform);
    Ok(())
  }

  #[fast]
  #[undefined]
  fn reset_transform(&self) {
    self.state.borrow_mut().transform = Affine::IDENTITY;
  }

  #[required(6)]
  #[undefined]
  fn transform(
    &self,
    #[webidl] a: UnrestrictedDouble,
    #[webidl] b: UnrestrictedDouble,
    #[webidl] c: UnrestrictedDouble,
    #[webidl] d: UnrestrictedDouble,
    #[webidl] e: UnrestrictedDouble,
    #[webidl] f: UnrestrictedDouble,
  ) {
    if !a.is_finite()
      || !b.is_finite()
      || !c.is_finite()
      || !d.is_finite()
      || !e.is_finite()
      || !f.is_finite()
    {
      return;
    }

    let m = Affine::new([*a, *b, *c, *d, *e, *f]);
    let mut state = self.state.borrow_mut();
    state.transform *= m;
  }

  #[required(2)]
  #[undefined]
  fn scale(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() {
      return;
    }

    let mut state = self.state.borrow_mut();
    state.transform *= Affine::scale_non_uniform(*x, *y);
  }

  #[required(1)]
  #[undefined]
  fn rotate(&self, #[webidl] angle: UnrestrictedDouble) {
    if !angle.is_finite() {
      return;
    }

    let mut state = self.state.borrow_mut();
    state.transform *= Affine::rotate(*angle);
  }

  #[required(2)]
  #[undefined]
  fn translate(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() {
      return;
    }

    let mut state = self.state.borrow_mut();
    state.transform *= Affine::translate((*x, *y));
  }

  #[required(4)]
  #[cppgc]
  fn create_linear_gradient(
    &self,
    #[webidl] x0: UnrestrictedDouble,
    #[webidl] y0: UnrestrictedDouble,
    #[webidl] x1: UnrestrictedDouble,
    #[webidl] y1: UnrestrictedDouble,
  ) -> Result<CanvasGradient, Canvas2DError> {
    if !x0.is_finite() || !y0.is_finite() || !x1.is_finite() || !y1.is_finite()
    {
      return Err(Canvas2DError::NonFinite);
    }

    let gradient = build_linear_gradient(*x0, *y0, *x1, *y1);
    Ok(CanvasGradient {
      gradient: RefCell::new(gradient),
    })
  }

  #[required(6)]
  #[cppgc]
  fn create_radial_gradient(
    &self,
    #[webidl] x0: UnrestrictedDouble,
    #[webidl] y0: UnrestrictedDouble,
    #[webidl] r0: UnrestrictedDouble,
    #[webidl] x1: UnrestrictedDouble,
    #[webidl] y1: UnrestrictedDouble,
    #[webidl] r1: UnrestrictedDouble,
  ) -> Result<CanvasGradient, Canvas2DError> {
    if !x0.is_finite()
      || !y0.is_finite()
      || !r0.is_finite()
      || !x1.is_finite()
      || !y1.is_finite()
      || !r1.is_finite()
    {
      return Err(Canvas2DError::NonFinite);
    }
    if *r0 < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*r0));
    }
    if *r1 < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*r1));
    }

    let gradient = build_radial_gradient(*x0, *y0, *r0, *x1, *y1, *r1);
    Ok(CanvasGradient {
      gradient: RefCell::new(gradient),
    })
  }

  #[required(3)]
  #[cppgc]
  fn create_conic_gradient(
    &self,
    #[webidl] start_angle: UnrestrictedDouble,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) -> Result<CanvasGradient, Canvas2DError> {
    if !start_angle.is_finite() || !x.is_finite() || !y.is_finite() {
      return Err(Canvas2DError::NonFinite);
    }

    let gradient = build_conic_gradient(*start_angle, *x, *y);
    Ok(CanvasGradient {
      gradient: RefCell::new(gradient),
    })
  }

  #[required(2)]
  #[cppgc]
  fn create_pattern<'a>(
    &self,
    state: &OpState,
    scope: &mut v8::PinScope<'a, 'a>,
    image: v8::Local<'a, v8::Value>,
    rep: v8::Local<'a, v8::Value>,
  ) -> Result<CanvasPattern, Canvas2DError> {
    if self.layer_depth.get() > 0 {
      return Err(Canvas2DError::InvalidState(
        "createPattern called while layers are open".into(),
      ));
    }

    let repetition = if rep.is_undefined() {
      return Err(Canvas2DError::PatternSyntax);
    } else if rep.is_null() {
      String::new()
    } else {
      rep.to_rust_string_lossy(scope)
    };
    let repetition = parse_repetition(&repetition)?;
    let resolved = resolve_canvas_image_source(state, scope, image)?;

    let pad_x = repetition.x_extend == peniko::Extend::Pad;
    let pad_y = repetition.y_extend == peniko::Extend::Pad;
    let (pixels, width, height, content_offset) = pad_pattern_image(
      &resolved.pixels,
      resolved.width,
      resolved.height,
      pad_x,
      pad_y,
    );
    let image_data = image_data_from_pixels(pixels, width, height);

    Ok(CanvasPattern {
      image: image_data,
      x_extend: repetition.x_extend,
      y_extend: repetition.y_extend,
      transform: RefCell::new(Affine::IDENTITY),
      content_offset,
    })
  }

  #[required(3)]
  #[undefined]
  fn draw_image<'a>(
    &self,
    state: &OpState,
    scope: &mut v8::PinScope<'a, 'a>,
    image: v8::Local<'a, v8::Value>,
    #[webidl] sx_or_dx: UnrestrictedDouble,
    #[webidl] sy_or_dy: UnrestrictedDouble,
    sw_or_dw: Option<v8::Local<'a, v8::Value>>,
    sh_or_dh: Option<v8::Local<'a, v8::Value>>,
    dx: Option<v8::Local<'a, v8::Value>>,
    dy: Option<v8::Local<'a, v8::Value>>,
    dw: Option<v8::Local<'a, v8::Value>>,
    dh: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<(), Canvas2DError> {
    let resolved = resolve_canvas_image_source(state, scope, image)?;

    let has_sw_or_dw = sw_or_dw
      .as_ref()
      .map(|v| !v.is_undefined())
      .unwrap_or(false);
    let has_dx = dx.as_ref().map(|v| !v.is_undefined()).unwrap_or(false);

    let (sx, sy, sw, sh, dx, dy, dw, dh) = if has_dx {
      // 9-arg: (image, sx, sy, sw, sh, dx, dy, dw, dh)
      let sx = *sx_or_dx;
      let sy = *sy_or_dy;
      let sw = sw_or_dw
        .and_then(|v| v.number_value(scope))
        .unwrap_or(f64::NAN);
      let sh = sh_or_dh
        .and_then(|v| v.number_value(scope))
        .unwrap_or(f64::NAN);
      let dx = dx.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let dy = dy.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let dw = dw.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let dh = dh.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      if !sx.is_finite()
        || !sy.is_finite()
        || !sw.is_finite()
        || !sh.is_finite()
        || !dx.is_finite()
        || !dy.is_finite()
        || !dw.is_finite()
        || !dh.is_finite()
      {
        return Ok(());
      }
      if sw == 0.0 || sh == 0.0 {
        return Ok(());
      }

      (sx, sy, sw, sh, dx, dy, dw, dh)
    } else if has_sw_or_dw {
      // 5-arg: (image, dx, dy, dw, dh)
      let dx = *sx_or_dx;
      let dy = *sy_or_dy;
      let dw = sw_or_dw
        .and_then(|v| v.number_value(scope))
        .unwrap_or(f64::NAN);
      let dh = sh_or_dh
        .and_then(|v| v.number_value(scope))
        .unwrap_or(f64::NAN);
      if !dx.is_finite()
        || !dy.is_finite()
        || !dw.is_finite()
        || !dh.is_finite()
      {
        return Ok(());
      }

      let iw = resolved.width as f64;
      let ih = resolved.height as f64;
      (0.0, 0.0, iw, ih, dx, dy, dw, dh)
    } else {
      // 3-arg: (image, dx, dy)
      let dx = *sx_or_dx;
      let dy = *sy_or_dy;
      if !dx.is_finite() || !dy.is_finite() {
        return Ok(());
      }

      let iw = resolved.width as f64;
      let ih = resolved.height as f64;
      (0.0, 0.0, iw, ih, dx, dy, iw, ih)
    };

    if sw == 0.0 || sh == 0.0 || dw == 0.0 || dh == 0.0 {
      return Ok(());
    }

    // Negative sizes move the origin; they do not mirror the image.
    let (sx, sw) = if sw < 0.0 { (sx + sw, -sw) } else { (sx, sw) };
    let (sy, sh) = if sh < 0.0 { (sy + sh, -sh) } else { (sy, sh) };
    let (dx, dw) = if dw < 0.0 { (dx + dw, -dw) } else { (dx, dw) };
    let (dy, dh) = if dh < 0.0 { (dy + dh, -dh) } else { (dy, dh) };

    let img =
      image_data_from_pixels(resolved.pixels, resolved.width, resolved.height);

    let (brush, image_transform, op, alpha, shadow_color, shadow_xform) = {
      let ds = self.state.borrow();
      let quality = if ds.image_smoothing_enabled {
        match ds.image_smoothing_quality {
          ImageSmoothingQuality::Low => peniko::ImageQuality::Low,
          ImageSmoothingQuality::Medium => peniko::ImageQuality::Medium,
          ImageSmoothingQuality::High => peniko::ImageQuality::High,
        }
      } else {
        peniko::ImageQuality::Low
      };

      let image_brush = peniko::ImageBrush::new(img).with_quality(quality);
      let brush = peniko::Brush::Image(image_brush);

      // Sample the fractional source rect directly.
      let scale_x = dw / sw;
      let scale_y = dh / sh;
      let image_transform = ds.transform
        * Affine::translate((dx, dy))
        * Affine::scale_non_uniform(scale_x, scale_y)
        * Affine::translate((-sx, -sy));

      let op = ds.global_composite_operation;
      let alpha = ds.global_alpha;
      let shadow = has_shadow(&ds);
      let shadow_color = ds.shadow_color.to_srgb8();
      let shadow_xform = if shadow {
        Some(shadow_transform(&ds, image_transform))
      } else {
        None
      };
      (
        brush,
        image_transform,
        op,
        alpha,
        shadow_color,
        shadow_xform,
      )
    };

    let rect = Rect::new(sx, sy, sx + sw, sy + sh);
    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let Some(st) = shadow_xform {
      draw_shadow(&mut drawing, width, height, shadow_color, |d| {
        fill_on(d, &rect, peniko::Fill::NonZero, st, brush.clone(), None);
      });
      // Composite shadow before the source image.
      if has_layer {
        pop_compositing_layer(&mut drawing);
        push_compositing_layer(&mut drawing, op, alpha, width, height);
      }
    }
    fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      image_transform,
      brush,
      None,
    );
    if has_layer {
      pop_compositing_layer(&mut drawing);
    }
    Ok(())
  }

  #[required(1)]
  #[cppgc]
  fn create_image_data<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
    sw_or_image_data: v8::Local<'a, v8::Value>,
    sh: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<ImageData, Canvas2DError> {
    if let Some(imagedata) = deno_core::cppgc::try_unwrap_cppgc_object::<
      ImageData,
    >(scope, sw_or_image_data)
    {
      let w = imagedata.get_width();
      let h = imagedata.get_height();
      let pixels = vec![0u8; w as usize * h as usize * 4];
      return Ok(ImageData::new_rgba_unorm8(scope, w, h, &pixels)?);
    }

    let Some(sh) = sh.filter(|v| !v.is_undefined()) else {
      return Err(Canvas2DError::MissingArgument {
        required: 2,
        provided: 1,
      });
    };

    let sw = require_long(scope, sw_or_image_data)?;
    let sh = require_long(scope, sh)?;

    let w = sw.unsigned_abs();
    let h = sh.unsigned_abs();

    if w == 0 || h == 0 {
      return Err(Canvas2DError::ZeroSourceSize);
    }
    check_image_data_size(w, h)?;

    let pixels = vec![0u8; w as usize * h as usize * 4];
    Ok(ImageData::new_rgba_unorm8(scope, w, h, &pixels)?)
  }

  #[cppgc]
  fn get_image_data(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] sx: f64,
    #[webidl] sy: f64,
    #[webidl] sw: f64,
    #[webidl] sh: f64,
  ) -> Result<ImageData, Canvas2DError> {
    let sx = sx as i32;
    let sy = sy as i32;
    let sw = sw as i32;
    let sh = sh as i32;
    if self.layer_depth.get() > 0 {
      return Err(Canvas2DError::InvalidState(
        "getImageData called while layers are open".into(),
      ));
    }
    if sw == 0 || sh == 0 {
      return Err(Canvas2DError::ZeroSourceSize);
    }

    self.increment_readback_and_check_fallback();
    let full = self.render_to_bytes()?;
    let (canvas_w, canvas_h) = self.data.dimensions();

    let (sx, sw) = if sw < 0 { (sx + sw, -sw) } else { (sx, sw) };
    let (sy, sh) = if sh < 0 { (sy + sh, -sh) } else { (sy, sh) };
    let out_w = sw as u32;
    let out_h = sh as u32;
    check_image_data_size(out_w, out_h)?;

    let mut sub = vec![0u8; (out_w as usize) * (out_h as usize) * 4];
    for row in 0..out_h {
      let src_y = sy + row as i32;
      if src_y < 0 || src_y >= canvas_h as i32 {
        continue;
      }
      for col in 0..out_w {
        let src_x = sx + col as i32;
        if src_x < 0 || src_x >= canvas_w as i32 {
          continue;
        }
        let src_idx = (src_y as u32 * canvas_w + src_x as u32) as usize * 4;
        let dst_idx = (row * out_w + col) as usize * 4;
        sub[dst_idx..dst_idx + 4].copy_from_slice(&full[src_idx..src_idx + 4]);
      }
    }

    unpremultiply_rgba(&mut sub);
    let cs = self.settings.color_space.to_image_data_color_space();
    Ok(ImageData::new_rgba_unorm8_with_color_space(
      scope, out_w, out_h, &sub, cs,
    )?)
  }

  #[required(3)]
  #[undefined]
  fn put_image_data(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    imagedata_val: v8::Local<'_, v8::Value>,
    #[webidl] dx: f64,
    #[webidl] dy: f64,
    dirty_x_arg: Option<v8::Local<'_, v8::Value>>,
    dirty_y_arg: Option<v8::Local<'_, v8::Value>>,
    dirty_w_arg: Option<v8::Local<'_, v8::Value>>,
    dirty_h_arg: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<(), Canvas2DError> {
    let dx = dx as i32;
    let dy = dy as i32;
    if self.layer_depth.get() > 0 {
      return Err(Canvas2DError::InvalidState(
        "putImageData called while layers are open".into(),
      ));
    }

    let imagedata = deno_core::cppgc::try_unwrap_cppgc_object::<ImageData>(
      scope,
      imagedata_val,
    )
    .ok_or(Canvas2DError::NotImageData)?;

    let src_w = imagedata.get_width() as i32;
    let src_h = imagedata.get_height() as i32;

    let has_dirty = dirty_x_arg
      .as_ref()
      .map(|v| !v.is_undefined())
      .unwrap_or(false);

    let (mut dirty_x, mut dirty_y, mut dirty_w, mut dirty_h) = if has_dirty {
      let dirty_x = require_long(scope, dirty_x_arg.unwrap())?;
      let dirty_y = require_long(
        scope,
        dirty_y_arg.unwrap_or_else(|| v8::undefined(scope).into()),
      )?;
      let dirty_w = require_long(
        scope,
        dirty_w_arg.unwrap_or_else(|| v8::undefined(scope).into()),
      )?;
      let dirty_h = require_long(
        scope,
        dirty_h_arg.unwrap_or_else(|| v8::undefined(scope).into()),
      )?;
      (dirty_x, dirty_y, dirty_w, dirty_h)
    } else {
      (0, 0, src_w, src_h)
    };

    if dirty_w < 0 {
      dirty_x += dirty_w;
      dirty_w = -dirty_w;
    }
    if dirty_h < 0 {
      dirty_y += dirty_h;
      dirty_h = -dirty_h;
    }

    if dirty_x < 0 {
      dirty_w += dirty_x;
      dirty_x = 0;
    }
    if dirty_y < 0 {
      dirty_h += dirty_y;
      dirty_y = 0;
    }
    if dirty_x + dirty_w > src_w {
      dirty_w = src_w - dirty_x;
    }
    if dirty_y + dirty_h > src_h {
      dirty_h = src_h - dirty_y;
    }
    if dirty_w <= 0 || dirty_h <= 0 {
      return Ok(());
    }

    let src_pixels = imagedata.read_pixels_rgba8(scope);
    let src_stride = imagedata.get_width() as usize;

    let (canvas_w, canvas_h) = self.data.dimensions();
    self.increment_readback_and_check_fallback();
    let mut pixels = self.render_to_bytes()?;

    for row in 0..dirty_h {
      let sy = (dirty_y + row) as usize;
      let canvas_y = dy + dirty_y + row;
      if canvas_y < 0 || canvas_y >= canvas_h as i32 {
        continue;
      }
      for col in 0..dirty_w {
        let sx = (dirty_x + col) as usize;
        let canvas_x = dx + dirty_x + col;
        if canvas_x < 0 || canvas_x >= canvas_w as i32 {
          continue;
        }
        let src_idx = (sy * src_stride + sx) * 4;
        let dst_idx =
          (canvas_y as usize * canvas_w as usize + canvas_x as usize) * 4;
        let a = src_pixels[src_idx + 3] as u32;
        if a == 255 {
          pixels[dst_idx..dst_idx + 4]
            .copy_from_slice(&src_pixels[src_idx..src_idx + 4]);
        } else if a == 0 {
          pixels[dst_idx..dst_idx + 4].copy_from_slice(&[0, 0, 0, 0]);
        } else {
          pixels[dst_idx] =
            ((src_pixels[src_idx] as u32 * a + 127) / 255) as u8;
          pixels[dst_idx + 1] =
            ((src_pixels[src_idx + 1] as u32 * a + 127) / 255) as u8;
          pixels[dst_idx + 2] =
            ((src_pixels[src_idx + 2] as u32 * a + 127) / 255) as u8;
          pixels[dst_idx + 3] = a as u8;
        }
      }
    }

    let mut drawing = self.drawing.borrow_mut();
    drawing.reset(canvas_w, canvas_h);
    self.refill_scene_from_snapshot(&mut drawing, pixels, canvas_w, canvas_h);
    Ok(())
  }

  fn get_line_dash(&self) -> Vec<f64> {
    self.state.borrow().line_dash.clone()
  }

  #[undefined]
  fn set_line_dash(&self, #[webidl] segments: Vec<UnrestrictedDouble>) {
    if segments.iter().any(|s| !s.is_finite() || **s < 0.0) {
      return;
    }

    let values: Vec<f64> = segments.iter().map(|s| **s).collect();
    let dash = if values.len() % 2 == 1 {
      let mut doubled = values.clone();
      doubled.extend_from_slice(&values);
      doubled
    } else {
      values
    };
    self.state.borrow_mut().line_dash = dash;
  }
}

impl OffscreenCanvasRenderingContext2D {
  /// Clears the accumulated scene and updates the canvas dimensions.
  pub fn has_open_layers(&self) -> bool {
    self.layer_depth.get() > 0
  }

  /// Called when OffscreenCanvas.width or .height is changed.
  pub fn resize(&self) {
    *self.state.borrow_mut() = DrawingState::default();
    self.state_stack.borrow_mut().clear();
    self.layer_depth.set(0);
    self.clip_stack.borrow_mut().clear();
    self.current_path.borrow_mut().truncate(0);
    let (width, height) = self.data.dimensions();

    // Content is discarded per spec, so re-derive the backend: the size
    // heuristics may pick differently for the new dimensions.
    *self.drawing.borrow_mut() = create_drawing_backend(
      &self.renderer,
      self.settings.will_read_frequently,
      self.readback_count.get(),
      width,
      height,
    );
  }

  /// Renders the scene to RGBA8 bytes.
  pub fn render_to_bytes(&self) -> Result<Vec<u8>, Canvas2DError> {
    let (width, height) = self.data.dimensions();
    let base_color = if self.settings.alpha {
      peniko::Color::TRANSPARENT
    } else {
      peniko::Color::from_rgb8(0, 0, 0)
    };
    let clip_depth = self.state.borrow().clip_depth;
    let mut drawing = self.drawing.borrow_mut();
    for _ in 0..clip_depth {
      pop_compositing_layer(&mut drawing);
    }
    let result = match &mut *drawing {
      DrawingBackend::Vello(scene) => {
        if let Some(Some(renderer)) = self.renderer.get() {
          Ok(render_scene(renderer, scene, width, height, base_color)?)
        } else {
          Ok(vec![0u8; (width * height * 4) as usize])
        }
      }
      DrawingBackend::VelloCpu(ctx, resources) => {
        let pixel_count = (width as usize) * (height as usize);
        let mut buf = vec![0u8; pixel_count * 4];
        ctx.render_to_buffer(
          resources,
          &mut buf,
          width as u16,
          height as u16,
          vello_cpu::RenderMode::OptimizeSpeed,
        );
        if !self.settings.alpha {
          for pixel in buf.chunks_exact_mut(4) {
            pixel[3] = 255;
          }
        }
        Ok(buf)
      }
    };
    let clip_stack = self.clip_stack.borrow();
    for clip in clip_stack.iter().take(clip_depth) {
      let fill = if clip.rule == "evenodd" {
        peniko::Fill::EvenOdd
      } else {
        peniko::Fill::NonZero
      };
      push_clip(&mut drawing, fill, clip.transform, &clip.path);
    }
    result
  }

  /// Called whenever getImageData / putImageData / convertToBlob reads
  /// pixels back. Readbacks are expensive on the GPU backend, so after
  /// `GPU_READBACK_FALLBACK_THRESHOLD` of them the context switches to the
  /// CPU backend, mirroring Chromium's heuristic.
  pub fn increment_readback_and_check_fallback(&self) {
    if !matches!(*self.drawing.borrow(), DrawingBackend::Vello(_)) {
      return;
    }
    self.readback_count.set(self.readback_count.get() + 1);
    if self.readback_count.get() >= GPU_READBACK_FALLBACK_THRESHOLD
      // Can't flatten the scene while layers are open; retry next readback.
      && self.layer_depth.get() == 0
    {
      self.switch_to_cpu_backend();
    }
  }

  /// Renders the current content once, then rebuilds it on the CPU backend.
  fn switch_to_cpu_backend(&self) {
    let Ok(pixels) = self.render_to_bytes() else {
      return;
    };
    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    *drawing =
      DrawingBackend::new(&DenoCanvasBackend::Cpu(CpuRenderer), width, height);
    self.refill_scene_from_snapshot(&mut drawing, pixels, width, height);
  }

  /// Rebuilds the scene from a full-canvas premultiplied RGBA8 snapshot as
  /// a single image fill, then replays the active clip stack on top.
  fn refill_scene_from_snapshot(
    &self,
    drawing: &mut DrawingBackend,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
  ) {
    let img = image_data_from_premultiplied_pixels(pixels, width, height);
    let image_brush = peniko::ImageBrush::new(img);
    let brush = peniko::Brush::Image(image_brush);
    let rect = Rect::new(0.0, 0.0, width as f64, height as f64);
    fill_on(
      drawing,
      &rect,
      peniko::Fill::NonZero,
      Affine::IDENTITY,
      brush,
      None,
    );

    let clip_depth = self.state.borrow().clip_depth;
    let clip_stack = self.clip_stack.borrow();
    for clip in clip_stack.iter().take(clip_depth) {
      let fill = if clip.rule == "evenodd" {
        peniko::Fill::EvenOdd
      } else {
        peniko::Fill::NonZero
      };
      push_clip(drawing, fill, clip.transform, &clip.path);
    }
  }

  /// Renders the scene into a TextureView owned by this renderer's device.
  pub fn render_to_texture_view(
    &self,
    view: &super::renderer::wgpu::TextureView,
  ) -> Result<(), Canvas2DError> {
    let (width, height) = self.data.dimensions();
    let base_color = if self.settings.alpha {
      peniko::Color::TRANSPARENT
    } else {
      peniko::Color::from_rgb8(0, 0, 0)
    };
    match &*self.drawing.borrow() {
      DrawingBackend::Vello(scene) => {
        if let Some(Some(renderer)) = self.renderer.get() {
          render_scene_to_texture_view(
            renderer, scene, view, width, height, base_color,
          )?;
        }
        Ok(())
      }
      // Surface-backed 2D contexts require a GPU backend. If this path is
      // ever wired up, exclude such contexts from the CPU fallback in
      // `increment_readback_and_check_fallback`.
      DrawingBackend::VelloCpu(_, _) => {
        unreachable!("render_to_texture_view called on Cpu backend")
      }
    }
  }

  /// Renders the accumulated scene into a DynamicImage.
  /// Called by ext/canvas when convertToBlob / transferToImageBitmap is invoked.
  pub fn flush_to_image(&self, image: &mut DynamicImage) {
    let (width, height) = image.dimensions();
    let base_color = if self.settings.alpha {
      peniko::Color::TRANSPARENT
    } else {
      peniko::Color::from_rgb8(0, 0, 0)
    };
    let clip_depth = self.state.borrow().clip_depth;
    let mut drawing = self.drawing.borrow_mut();
    for _ in 0..clip_depth {
      pop_compositing_layer(&mut drawing);
    }
    let buf = match &mut *drawing {
      DrawingBackend::Vello(scene) => {
        if let Some(Some(renderer)) = self.renderer.get() {
          render_scene(renderer, scene, width, height, base_color)
            .map_err(|e| {
              log::warn!("canvas2d: render error: {e}");
            })
            .ok()
            .map(|mut buf| {
              // render_scene returns premultiplied alpha; DynamicImage expects
              // straight alpha, same as the VelloCpu branch below.
              if self.settings.alpha {
                unpremultiply_rgba(&mut buf);
              } else {
                for pixel in buf.chunks_exact_mut(4) {
                  pixel[3] = 255;
                }
              }
              buf
            })
        } else {
          None
        }
      }
      DrawingBackend::VelloCpu(ctx, resources) => {
        let pixel_count = (width as usize) * (height as usize);
        let mut buf = vec![0u8; pixel_count * 4];
        ctx.render_to_buffer(
          resources,
          &mut buf,
          width as u16,
          height as u16,
          vello_cpu::RenderMode::OptimizeSpeed,
        );
        if !self.settings.alpha {
          for pixel in buf.chunks_exact_mut(4) {
            pixel[3] = 255;
          }
        } else {
          unpremultiply_rgba(&mut buf);
        }
        Some(buf)
      }
    };
    let clip_stack = self.clip_stack.borrow();
    for clip in clip_stack.iter().take(clip_depth) {
      let fill = if clip.rule == "evenodd" {
        peniko::Fill::EvenOdd
      } else {
        peniko::Fill::NonZero
      };
      push_clip(&mut drawing, fill, clip.transform, &clip.path);
    }
    let rgba = buf
      .and_then(|b| RgbaImage::from_raw(width, height, b))
      .unwrap_or_else(|| {
        if self.settings.alpha {
          RgbaImage::new(width, height)
        } else {
          RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]))
        }
      });
    *image = DynamicImage::ImageRgba8(rgba);
  }

  fn resolve_optional_path(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    arg: Option<v8::Local<'_, v8::Value>>,
  ) -> (BezPath, bool) {
    if let Some(v) = arg
      && let Some(p) =
        deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v)
    {
      return (p.path.borrow().clone(), true);
    }
    (self.current_path.borrow().clone(), false)
  }

  fn resolve_path_and_fill_rule(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path_or_fill_rule: Option<v8::Local<'_, v8::Value>>,
    fill_rule: Option<String>,
  ) -> (BezPath, String, bool) {
    // path_or_fill_rule may be Path2D or fillRule string
    if let Some(v) = path_or_fill_rule {
      if v.is_string() {
        let rule = v.to_rust_string_lossy(scope);
        return (self.current_path.borrow().clone(), rule, false);
      }
      if let Some(p) =
        deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v)
      {
        let rule = fill_rule.unwrap_or_else(|| "nonzero".to_string());
        return (p.path.borrow().clone(), rule, true);
      }
    }
    let rule = fill_rule.unwrap_or_else(|| "nonzero".to_string());
    (self.current_path.borrow().clone(), rule, false)
  }

  fn draw_path_fill(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path: BezPath,
    rule: String,
    transform: Affine,
  ) {
    if path.is_empty() {
      return;
    }
    let (op, alpha, shadow_color, shadow_xform, brush, brush_transform, fill) = {
      let state = self.state.borrow();
      let op = state.global_composite_operation;
      let alpha = state.global_alpha;
      let shadow = has_shadow(&state);
      let shadow_color = state.shadow_color.to_srgb8();
      let shadow_xform = if shadow {
        Some(shadow_transform(&state, transform))
      } else {
        None
      };
      let (brush, brush_transform) =
        resolve_brush(scope, &state.fill_style, 1.0);
      let fill = if rule == "evenodd" {
        peniko::Fill::EvenOdd
      } else {
        peniko::Fill::NonZero
      };
      (
        op,
        alpha,
        shadow_color,
        shadow_xform,
        brush,
        brush_transform,
        fill,
      )
    };

    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let Some(st) = shadow_xform {
      draw_shadow(&mut drawing, width, height, shadow_color, |d| {
        fill_on(d, &path, fill, st, brush.clone(), brush_transform);
      });
      // Per spec, the shadow is composited first, then the source shape is
      // composited on top as a separate step.
      if has_layer {
        pop_compositing_layer(&mut drawing);
        push_compositing_layer(&mut drawing, op, alpha, width, height);
      }
    }
    fill_on(&mut drawing, &path, fill, transform, brush, brush_transform);
    if has_layer {
      pop_compositing_layer(&mut drawing);
    }
  }

  fn draw_path_stroke(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path: BezPath,
    transform: Affine,
    is_path2d: bool,
  ) {
    if path.is_empty() {
      return;
    }
    let (op, alpha, shadow_color, shadow_xform, brush, brush_transform, stroke) = {
      let state = self.state.borrow();
      let op = state.global_composite_operation;
      let alpha = state.global_alpha;
      let shadow = has_shadow(&state);
      let shadow_color = state.shadow_color.to_srgb8();
      let shadow_xform = if shadow {
        Some(shadow_transform(&state, transform))
      } else {
        None
      };
      let (brush, brush_transform) =
        resolve_brush(scope, &state.stroke_style, 1.0);
      let stroke = build_stroke(&state);
      (
        op,
        alpha,
        shadow_color,
        shadow_xform,
        brush,
        brush_transform,
        stroke,
      )
    };

    let path = if is_path2d {
      path
    } else {
      transform_path(&path, transform.inverse())
    };

    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let Some(st) = shadow_xform {
      draw_shadow(&mut drawing, width, height, shadow_color, |d| {
        stroke_on(d, &path, &stroke, st, brush.clone(), brush_transform);
      });
      // Per spec, the shadow is composited first, then the source shape is
      // composited on top as a separate step.
      if has_layer {
        pop_compositing_layer(&mut drawing);
        push_compositing_layer(&mut drawing, op, alpha, width, height);
      }
    }
    stroke_on(
      &mut drawing,
      &path,
      &stroke,
      transform,
      brush,
      brush_transform,
    );
    if has_layer {
      pop_compositing_layer(&mut drawing);
    }
  }

  fn apply_clip(&self, path: BezPath, rule: String, transform: Affine) {
    // Per spec, clipping with an empty path shrinks the clip region to
    // nothing (rather than leaving it unchanged), so subsequent drawing is
    // fully clipped out. Use a zero-area shape with the identity transform
    // to represent that.
    let (path, transform) = if path.is_empty() {
      (Shape::to_path(&Rect::ZERO, 0.1), Affine::IDENTITY)
    } else {
      (path, transform)
    };
    let fill = if rule == "evenodd" {
      peniko::Fill::EvenOdd
    } else {
      peniko::Fill::NonZero
    };
    push_clip(&mut self.drawing.borrow_mut(), fill, transform, &path);
    let mut state = self.state.borrow_mut();
    self.clip_stack.borrow_mut().truncate(state.clip_depth);
    self.clip_stack.borrow_mut().push(ClipEntry {
      path,
      rule,
      transform,
    });
    state.clip_depth += 1;
  }

  fn draw_text(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    text: &str,
    x: f64,
    y: f64,
    max_width: Option<f64>,
    stroke: bool,
  ) {
    // https://html.spec.whatwg.org/multipage/canvas.html#text-preparation-algorithm
    // Nothing is drawn when maxWidth is present but not a positive number.
    if let Some(max_width) = max_width
      && (max_width.is_nan() || max_width <= 0.0)
    {
      return;
    }

    let mut fc = self.font_ctx.borrow_mut();
    let mut lc = self.layout_ctx.borrow_mut();
    let (
      layout,
      op,
      global_alpha,
      shadow_color,
      shadow_xform,
      brush,
      brush_transform,
      text_align,
      text_baseline,
      transform,
      direction,
    ) = {
      let state = self.state.borrow();
      let layout = build_text_layout(&mut fc, &mut lc, text, &state.font_state);

      let style = if stroke {
        &state.stroke_style
      } else {
        &state.fill_style
      };
      let op = state.global_composite_operation;
      let global_alpha = state.global_alpha;
      let shadow = has_shadow(&state);
      let shadow_color = state.shadow_color.to_srgb8();
      let shadow_xform = if shadow {
        Some(shadow_transform(&state, state.transform))
      } else {
        None
      };
      let (brush, brush_transform) = resolve_brush(scope, style, 1.0);
      let text_align = state.text_align;
      let text_baseline = state.text_baseline;
      let transform = state.transform;
      let direction = state.font_state.direction;
      (
        layout,
        op,
        global_alpha,
        shadow_color,
        shadow_xform,
        brush,
        brush_transform,
        text_align,
        text_baseline,
        transform,
        direction,
      )
    };

    let baseline_y = compute_baseline_y(y, &layout, text_baseline);

    let layout_baseline = layout
      .lines()
      .next()
      .map(|line| line.metrics().baseline)
      .unwrap_or(0.0);

    // Compute total line width for text-align adjustment.
    let line_width: f32 = layout
      .lines()
      .next()
      .map(|line| line.metrics().advance - line.metrics().trailing_whitespace)
      .unwrap_or(0.0);

    // Condense the text horizontally when it is wider than maxWidth.
    // TODO(petamoriken): only the glyph advances are compressed for now,
    // the glyph outlines themselves are not horizontally scaled.
    let x_scale: f32 = match max_width {
      Some(max_width) if (line_width as f64) > max_width => {
        (max_width / line_width as f64) as f32
      }
      _ => 1.0,
    };
    let scaled_width = line_width * x_scale;

    let rtl = direction == TextDirection::Rtl;
    let x_offset = match text_align {
      TextAlign::Left => 0.0,
      TextAlign::Right => -scaled_width,
      TextAlign::Center => -scaled_width / 2.0,
      TextAlign::Start if rtl => -scaled_width,
      TextAlign::Start => 0.0,
      TextAlign::End if rtl => 0.0,
      TextAlign::End => -scaled_width,
    };
    let draw_x = x as f32 + x_offset;

    let (canvas_w, canvas_h) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer = push_compositing_layer(
      &mut drawing,
      op,
      global_alpha,
      canvas_w,
      canvas_h,
    );
    if let Some(st) = shadow_xform {
      draw_shadow(
        &mut drawing,
        canvas_w,
        canvas_h,
        shadow_color,
        |d| match d {
          DrawingBackend::Vello(scene) => {
            for line in layout.lines() {
              for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                  continue;
                };
                let font = peniko::FontData::clone(glyph_run.run().font());
                let font_size = glyph_run.run().font_size();
                let glyphs =
                  glyph_run.positioned_glyphs().map(|g| vello::Glyph {
                    id: g.id,
                    x: draw_x + g.x * x_scale,
                    y: baseline_y as f32 + g.y - layout_baseline,
                  });
                let mut glyph_draw = scene
                  .draw_glyphs(&font)
                  .font_size(font_size)
                  .transform(st)
                  .brush(&brush);
                if let Some(bt) = brush_transform {
                  glyph_draw = glyph_draw.brush_transform(Some(bt));
                }
                glyph_draw.draw(peniko::Fill::NonZero, glyphs);
              }
            }
          }
          DrawingBackend::VelloCpu(ctx, resources) => {
            for line in layout.lines() {
              for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                  continue;
                };
                let font = peniko::FontData::clone(glyph_run.run().font());
                let font_size = glyph_run.run().font_size();
                apply_cpu_paint(ctx, brush.clone(), brush_transform);
                ctx.set_transform(st);
                ctx
                  .glyph_run(resources, &font)
                  .font_size(font_size)
                  .fill_glyphs(glyph_run.positioned_glyphs().map(|g| {
                    vello_cpu::Glyph {
                      id: g.id,
                      x: draw_x + g.x * x_scale,
                      y: baseline_y as f32 + g.y - layout_baseline,
                    }
                  }));
              }
            }
          }
        },
      );
      // Per spec, the shadow is composited first, then the source text is
      // composited on top as a separate step.
      if has_layer {
        pop_compositing_layer(&mut drawing);
        push_compositing_layer(
          &mut drawing,
          op,
          global_alpha,
          canvas_w,
          canvas_h,
        );
      }
    }
    match &mut *drawing {
      DrawingBackend::Vello(scene) => {
        for line in layout.lines() {
          for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
              continue;
            };
            let font = peniko::FontData::clone(glyph_run.run().font());
            let font_size = glyph_run.run().font_size();

            let glyphs = glyph_run.positioned_glyphs().map(|g| vello::Glyph {
              id: g.id,
              x: draw_x + g.x * x_scale,
              y: baseline_y as f32 + g.y - layout_baseline,
            });

            let mut glyph_draw = scene
              .draw_glyphs(&font)
              .font_size(font_size)
              .transform(transform)
              .brush(&brush);
            if let Some(bt) = brush_transform {
              glyph_draw = glyph_draw.brush_transform(Some(bt));
            }
            glyph_draw.draw(peniko::Fill::NonZero, glyphs);
          }
        }
      }
      DrawingBackend::VelloCpu(ctx, resources) => {
        for line in layout.lines() {
          for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
              continue;
            };
            let font = peniko::FontData::clone(glyph_run.run().font());
            let font_size = glyph_run.run().font_size();

            apply_cpu_paint(ctx, brush.clone(), brush_transform);
            ctx.set_transform(transform);
            ctx
              .glyph_run(resources, &font)
              .font_size(font_size)
              .fill_glyphs(glyph_run.positioned_glyphs().map(|g| {
                vello_cpu::Glyph {
                  id: g.id,
                  x: draw_x + g.x * x_scale,
                  y: baseline_y as f32 + g.y - layout_baseline,
                }
              }));
          }
        }
      }
    }
    if has_layer {
      pop_compositing_layer(&mut drawing);
    }
  }

  fn resolve_point_in_path_args(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path_or_x: Option<v8::Local<'_, v8::Value>>,
    x_or_y: Option<v8::Local<'_, v8::Value>>,
    y_or_fill_rule: Option<v8::Local<'_, v8::Value>>,
    fill_rule: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<(BezPath, f64, f64, String, bool), Canvas2DError> {
    const PREFIX: &str = "Failed to execute 'isPointInPath' on 'OffscreenCanvasRenderingContext2D'";

    let validate_fill_rule =
      |context: &'static str, rule: &str| -> Result<(), Canvas2DError> {
        match rule {
          "nonzero" | "evenodd" => Ok(()),
          _ => Err(Canvas2DError::WebIdl(deno_core::webidl::WebIdlError {
            prefix: PREFIX.into(),
            context: context.into(),
            kind: deno_core::webidl::WebIdlErrorKind::InvalidEnumVariant {
              converter: "CanvasFillRule",
              variant: rule.to_string(),
            },
          })),
        }
      };

    let Some(path_or_x) = path_or_x else {
      if fill_rule.is_some() {
        // 4 args: isPointInPath(path, x, y, fillRule) — null/undefined is not Path2D
        return Err(type_error_not_path2d(PREFIX, "parameter 1"));
      }
      if x_or_y.is_some() {
        // 2-3 args with null/undefined first: isPointInPath(x, y [, fillRule])
        let y = x_or_y.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
        let rule = y_or_fill_rule
          .map(|v| v.to_rust_string_lossy(scope))
          .unwrap_or_else(|| "nonzero".into());
        validate_fill_rule("parameter 3", &rule)?;
        return Ok((
          self.current_path.borrow().clone(),
          f64::NAN,
          y,
          rule,
          false,
        ));
      }
      // Zero arguments: neither the (x, y [, fillRule]) nor the
      // (path, x, y [, fillRule]) overload has enough arguments.
      return Err(Canvas2DError::MissingArgument {
        required: 2,
        provided: 0,
      });
    };
    if let Some(p) =
      deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, path_or_x)
    {
      // isPointInPath(path, x, y [, fillRule])
      let provided =
        1 + x_or_y.is_some() as u32 + y_or_fill_rule.is_some() as u32;
      let (Some(x_or_y), Some(y_or_fill_rule)) = (x_or_y, y_or_fill_rule)
      else {
        return Err(Canvas2DError::MissingArgument {
          required: 3,
          provided,
        });
      };
      let x = v8_to_f64(scope, x_or_y);
      let y = v8_to_f64(scope, y_or_fill_rule);
      // CanvasFillRule is a non-nullable DOMString enum, so an explicit
      // `null` must be stringified to "null" (an invalid enum value)
      // rather than falling back to the "nonzero" default like an omitted
      // argument would.
      let rule = fill_rule
        .map(|v| v.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "nonzero".into());
      validate_fill_rule("parameter 4", &rule)?;
      return Ok((p.path.borrow().clone(), x, y, rule, true));
    }
    if path_or_x.is_number() {
      // isPointInPath(x, y [, fillRule])
      let Some(x_or_y) = x_or_y else {
        return Err(Canvas2DError::MissingArgument {
          required: 2,
          provided: 1,
        });
      };
      let x = v8_to_f64(scope, path_or_x);
      let y = v8_to_f64(scope, x_or_y);
      let rule = y_or_fill_rule
        .map(|v| v.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "nonzero".into());
      validate_fill_rule("parameter 3", &rule)?;
      return Ok((self.current_path.borrow().clone(), x, y, rule, false));
    }
    Err(type_error_not_path2d(PREFIX, "parameter 1"))
  }

  fn resolve_point_in_stroke_args(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path_or_x: Option<v8::Local<'_, v8::Value>>,
    x_or_y: Option<v8::Local<'_, v8::Value>>,
    y: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<(BezPath, f64, f64, bool), Canvas2DError> {
    const PREFIX: &str = "Failed to execute 'isPointInStroke' on 'OffscreenCanvasRenderingContext2D'";
    let Some(path_or_x) = path_or_x else {
      if y.is_some() {
        // 3 args: isPointInStroke(path, x, y) — null/undefined is not Path2D
        return Err(type_error_not_path2d(PREFIX, "parameter 1"));
      }
      if x_or_y.is_some() {
        // 2 args with null/undefined first: isPointInStroke(x, y)
        let y = x_or_y.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
        return Ok((self.current_path.borrow().clone(), f64::NAN, y, false));
      }
      return Ok((
        self.current_path.borrow().clone(),
        f64::NAN,
        f64::NAN,
        false,
      ));
    };
    if let Some(p) =
      deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, path_or_x)
    {
      // isPointInStroke(path, x, y)
      let x = x_or_y.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      let y = y.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      return Ok((p.path.borrow().clone(), x, y, true));
    }
    if path_or_x.is_number() {
      // isPointInStroke(x, y)
      let x = v8_to_f64(scope, path_or_x);
      let y = x_or_y.map(|v| v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      return Ok((self.current_path.borrow().clone(), x, y, false));
    }
    Err(type_error_not_path2d(PREFIX, "parameter 1"))
  }

  fn test_point_in_stroke(
    &self,
    path: BezPath,
    x: f64,
    y: f64,
    transform: Affine,
    is_path2d: bool,
  ) -> bool {
    if path.is_empty() {
      return false;
    }
    // lineWidth/lineDash are specified in user-space units, so the stroke
    // outline must be built in user space. The default path is stored in
    // canvas space (see path_move_to/path_line_to/etc. in path.rs), so map
    // it back; Path2D coordinates are already in user space.
    let path = if is_path2d {
      path
    } else {
      transform_path(&path, transform.inverse())
    };
    let stroke = {
      let state = self.state.borrow();
      build_stroke(&state)
    };
    let outline = kurbo::stroke(
      path.path_elements(0.01),
      &stroke,
      &StrokeOpts::default(),
      0.01,
    );
    outline.contains(Point::new(x, y))
  }
}

/// Rejects ImageData buffers too large for JS typed arrays.
#[inline]
fn check_image_data_size(w: u32, h: u32) -> Result<(), Canvas2DError> {
  const MAX_IMAGE_DATA_BYTES: u64 = i32::MAX as u64;
  if (w as u64) * (h as u64) * 4 > MAX_IMAGE_DATA_BYTES {
    return Err(Canvas2DError::ImageDataTooLarge);
  }
  Ok(())
}

/// Parses `CanvasLayerOptions` for `beginLayer()`.
#[inline]
fn parse_begin_layer_options<'a>(
  scope: &mut v8::PinScope<'a, 'a>,
  options: v8::Local<'a, v8::Value>,
) -> Result<Vec<CanvasLayerFilterPrimitive>, Canvas2DError> {
  if options.is_null_or_undefined() {
    return Ok(Vec::new());
  }
  if !options.is_object() {
    return Err(Canvas2DError::InvalidBeginLayerOptions);
  }

  let obj = options.cast::<v8::Object>();
  let filter_key = v8::String::new(scope, "filter").unwrap();
  let Some(filter) = obj.get(scope, filter_key.into()) else {
    return Err(Canvas2DError::InvalidBeginLayerOptions);
  };
  if filter.is_null_or_undefined() {
    return Ok(Vec::new());
  }
  if !filter.is_object() {
    // The DOMString branch of the filter union: any string (or stringifiable
    // primitive) is accepted, even when it is not a parsable CSS filter.
    let Some(value) = filter.to_string(scope) else {
      return Ok(Vec::new());
    };
    let value = value.to_rust_string_lossy(scope);
    let mut parser_input = FilterParserInput::new(&value);
    let result: Result<Vec<_>, _> =
      FilterValueListParser::new(&mut parser_input).collect();
    return Ok(
      result
        .ok()
        .unwrap_or_default()
        .into_iter()
        .map(Into::into)
        .collect(),
    );
  }
  parse_filter_input(scope, filter)
}

fn require_long(
  scope: &mut v8::PinScope<'_, '_>,
  val: v8::Local<'_, v8::Value>,
) -> Result<i32, Canvas2DError> {
  let n = val.number_value(scope).unwrap_or(f64::NAN);
  if !n.is_finite() {
    return Err(Canvas2DError::NonFinite);
  }
  Ok(n as i32)
}

fn parse_fill_stroke_style(
  scope: &mut v8::PinScope<'_, '_>,
  value: v8::Local<'_, v8::Value>,
) -> Option<FillStrokeStyle> {
  if deno_core::cppgc::try_unwrap_cppgc_object::<CanvasGradient>(scope, value)
    .is_some()
  {
    return Some(FillStrokeStyle::Gradient(v8::Global::new(
      scope,
      value.cast::<v8::Object>(),
    )));
  }
  if deno_core::cppgc::try_unwrap_cppgc_object::<CanvasPattern>(scope, value)
    .is_some()
  {
    return Some(FillStrokeStyle::Pattern(v8::Global::new(
      scope,
      value.cast::<v8::Object>(),
    )));
  }
  // The DOMString branch of the union: ToString may invoke a user-supplied
  // toString() (whose thrown exception is left pending to propagate); an
  // invalid color string leaves the style unchanged.
  let s = value.to_string(scope)?;
  let s = s.to_rust_string_lossy(scope);
  parse_css_color(&s).ok().map(FillStrokeStyle::Color)
}

fn resolve_brush(
  scope: &mut v8::PinScope<'_, '_>,
  style: &FillStrokeStyle,
  global_alpha: f32,
) -> (peniko::Brush, Option<Affine>) {
  match style {
    FillStrokeStyle::Color(c) => {
      let rgba = c.to_srgb8().to_rgba8();
      let alpha = (rgba.a as f32 / 255.0 * global_alpha * 255.0).round() as u8;
      let color = peniko::Color::from_rgba8(rgba.r, rgba.g, rgba.b, alpha);
      (peniko::Brush::Solid(color), None)
    }
    FillStrokeStyle::Gradient(obj) => {
      let local = v8::Local::new(scope, obj);
      let gradient =
        deno_core::cppgc::try_unwrap_cppgc_object::<CanvasGradient>(
          scope,
          local.into(),
        )
        .expect("fillStyle gradient reference must be valid");
      let mut g = gradient.gradient.borrow().clone();
      // Stops are pushed in addColorStop() call order, not offset order;
      // sort (stably, so same-offset stops keep their relative order) so
      // the ramp is built correctly.
      g.stops.sort_by(|a, b| {
        a.offset
          .partial_cmp(&b.offset)
          .unwrap_or(std::cmp::Ordering::Equal)
      });
      // Degenerate gradients (per spec) paint nothing: a linear gradient
      // whose two points coincide, or a radial gradient whose two circles
      // are identical. A solid transparent brush is used to represent
      // "nothing" so this can be returned like any other brush.
      let degenerate = match g.kind {
        peniko::GradientKind::Linear(pos) => pos.start == pos.end,
        peniko::GradientKind::Radial(pos) => {
          pos.start_center == pos.end_center
            && pos.start_radius == pos.end_radius
        }
        peniko::GradientKind::Sweep(_) => false,
      };
      if degenerate || g.stops.is_empty() {
        return (peniko::Brush::Solid(peniko::Color::TRANSPARENT), None);
      }
      if g.stops.len() == 1 {
        let color = g.stops[0].color.to_alpha_color::<peniko::color::Srgb>();
        return (peniko::Brush::Solid(color), None);
      }
      (peniko::Brush::Gradient(g), Some(Affine::IDENTITY))
    }
    FillStrokeStyle::Pattern(obj) => {
      let local = v8::Local::new(scope, obj);
      let pattern = deno_core::cppgc::try_unwrap_cppgc_object::<CanvasPattern>(
        scope,
        local.into(),
      )
      .expect("fillStyle pattern reference must be valid");
      let mut image_brush = peniko::ImageBrush::new(pattern.image.clone())
        .with_x_extend(pattern.x_extend)
        .with_y_extend(pattern.y_extend);
      if global_alpha != 1.0 {
        image_brush = image_brush.multiply_alpha(global_alpha);
      }
      // Compensate for the transparent border pad_pattern_image() may
      // have added: shift brush-local space so the real image content
      // still lands where an unpadded image's content would have.
      let pattern_transform = *pattern.transform.borrow()
        * Affine::translate(-pattern.content_offset);
      (peniko::Brush::Image(image_brush), Some(pattern_transform))
    }
  }
}

fn apply_cpu_paint(
  ctx: &mut vello_cpu::RenderContext,
  brush: peniko::Brush,
  brush_transform: Option<Affine>,
) {
  match brush {
    peniko::Brush::Solid(color) => {
      ctx.reset_paint_transform();
      ctx.set_paint(color);
    }
    peniko::Brush::Gradient(gradient) => {
      if let Some(t) = brush_transform {
        ctx.set_paint_transform(t);
      } else {
        ctx.reset_paint_transform();
      }
      ctx.set_paint(vello_cpu::PaintType::Gradient(gradient));
    }
    peniko::Brush::Image(image_brush) => {
      let source =
        vello_cpu::ImageSource::from_peniko_image_data(&image_brush.image);
      let cpu_brush = peniko::ImageBrush {
        image: source,
        sampler: image_brush.sampler,
      };
      if let Some(t) = brush_transform {
        ctx.set_paint_transform(t);
      } else {
        ctx.reset_paint_transform();
      }
      ctx.set_paint(vello_cpu::PaintType::Image(cpu_brush));
    }
  }
}

fn push_compositing_layer(
  drawing: &mut DrawingBackend,
  op: GlobalCompositeOperation,
  alpha: f32,
  width: u32,
  height: u32,
) -> bool {
  if op == GlobalCompositeOperation::SourceOver && alpha == 1.0 {
    return false;
  }
  push_full_canvas_layer(drawing, op.to_blend_mode(), alpha, width, height);
  true
}

/// Pushes a full-canvas compositing layer.
fn push_full_canvas_layer(
  drawing: &mut DrawingBackend,
  blend: peniko::BlendMode,
  alpha: f32,
  width: u32,
  height: u32,
) {
  match drawing {
    DrawingBackend::Vello(scene) => {
      let clip = Rect::new(0.0, 0.0, width as f64, height as f64);
      scene.push_layer(
        peniko::Fill::NonZero,
        blend,
        alpha,
        Affine::IDENTITY,
        &clip,
      );
    }
    DrawingBackend::VelloCpu(ctx, _) => {
      ctx.push_layer(None, Some(blend), Some(alpha), None, None);
    }
  }
}

fn pop_compositing_layer(drawing: &mut DrawingBackend) {
  match drawing {
    DrawingBackend::Vello(scene) => scene.pop_layer(),
    DrawingBackend::VelloCpu(ctx, _) => ctx.pop_layer(),
  }
}

/// Draws a shadow using the source content's alpha mask.
fn draw_shadow(
  drawing: &mut DrawingBackend,
  width: u32,
  height: u32,
  shadow_color: peniko::Color,
  draw_source: impl FnOnce(&mut DrawingBackend),
) {
  push_full_canvas_layer(
    drawing,
    peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcOver),
    1.0,
    width,
    height,
  );
  draw_source(drawing);
  push_full_canvas_layer(
    drawing,
    peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn),
    1.0,
    width,
    height,
  );
  let canvas_rect = Rect::new(0.0, 0.0, width as f64, height as f64);
  fill_on(
    drawing,
    &canvas_rect,
    peniko::Fill::NonZero,
    Affine::IDENTITY,
    peniko::Brush::Solid(shadow_color),
    None,
  );
  pop_compositing_layer(drawing); // pop the SrcIn tint layer
  pop_compositing_layer(drawing); // pop the SrcOver isolation layer
}

/// Pushes a clip layer after syncing fill rule and transform.
fn push_clip(
  drawing: &mut DrawingBackend,
  fill: peniko::Fill,
  transform: Affine,
  path: &BezPath,
) {
  match drawing {
    DrawingBackend::Vello(scene) => {
      scene.push_clip_layer(fill, transform, path);
    }
    DrawingBackend::VelloCpu(ctx, _) => {
      ctx.set_fill_rule(if fill == peniko::Fill::EvenOdd {
        vello_cpu::peniko::Fill::EvenOdd
      } else {
        vello_cpu::peniko::Fill::NonZero
      });
      ctx.set_transform(transform);
      ctx.push_clip_layer(path);
    }
  }
}

fn build_stroke(state: &DrawingState) -> Stroke {
  let mut stroke =
    Stroke::new(state.line_width).with_miter_limit(state.miter_limit);
  match state.line_join {
    LineJoin::Round => {
      stroke.join = Join::Round;
    }
    LineJoin::Bevel => {
      stroke.join = Join::Bevel;
    }
    LineJoin::Miter => {
      stroke.join = Join::Miter;
    }
  }
  match state.line_cap {
    LineCap::Butt => {
      stroke.start_cap = Cap::Butt;
      stroke.end_cap = Cap::Butt;
    }
    LineCap::Round => {
      stroke.start_cap = Cap::Round;
      stroke.end_cap = Cap::Round;
    }
    LineCap::Square => {
      stroke.start_cap = Cap::Square;
      stroke.end_cap = Cap::Square;
    }
  }
  if !state.line_dash.is_empty() {
    stroke = stroke
      .with_dashes(state.line_dash_offset, state.line_dash.iter().copied());
  }
  stroke
}

fn has_shadow(state: &DrawingState) -> bool {
  !state.shadow_color.is_transparent()
    && (state.shadow_blur > 0.0
      || state.shadow_offset_x != 0.0
      || state.shadow_offset_y != 0.0)
}

fn shadow_transform(state: &DrawingState, transform: Affine) -> Affine {
  // TODO(petamoriken): apply shadowBlur once Vello GPU supports filter effects
  Affine::translate((state.shadow_offset_x, state.shadow_offset_y)) * transform
}

fn fill_on(
  drawing: &mut DrawingBackend,
  shape: &impl Shape,
  fill: peniko::Fill,
  transform: Affine,
  brush: peniko::Brush,
  brush_transform: Option<Affine>,
) {
  match drawing {
    DrawingBackend::Vello(scene) => {
      scene.fill(fill, transform, &brush, brush_transform, shape);
    }
    DrawingBackend::VelloCpu(ctx, _) => {
      apply_cpu_paint(ctx, brush, brush_transform);
      ctx.set_fill_rule(if fill == peniko::Fill::EvenOdd {
        vello_cpu::peniko::Fill::EvenOdd
      } else {
        vello_cpu::peniko::Fill::NonZero
      });
      ctx.set_transform(transform);
      let path: BezPath = shape.path_elements(0.1).collect();
      ctx.fill_path(&path);
    }
  }
}

fn stroke_on(
  drawing: &mut DrawingBackend,
  path: &BezPath,
  stroke: &Stroke,
  transform: Affine,
  brush: peniko::Brush,
  brush_transform: Option<Affine>,
) {
  match drawing {
    DrawingBackend::Vello(scene) => {
      scene.stroke(stroke, transform, &brush, brush_transform, path);
    }
    DrawingBackend::VelloCpu(ctx, _) => {
      apply_cpu_paint(ctx, brush, brush_transform);
      ctx.set_stroke(stroke.clone());
      ctx.set_transform(transform);
      ctx.stroke_path(path);
    }
  }
}

fn v8_to_f64(
  scope: &mut v8::PinScope<'_, '_>,
  v: v8::Local<'_, v8::Value>,
) -> f64 {
  v.number_value(scope).unwrap_or(f64::NAN)
}

fn type_error_not_path2d(
  prefix: &'static str,
  context: &'static str,
) -> Canvas2DError {
  Canvas2DError::WebIdl(deno_core::webidl::WebIdlError {
    prefix: prefix.into(),
    context: context.into(),
    kind: deno_core::webidl::WebIdlErrorKind::ConvertToConverterType("Path2D"),
  })
}

/// Returns a copy of `path` with every subpath explicitly closed. Per
/// spec, isPointInPath()/isPointInStroke() (and fill()/clip()) treat each
/// subpath as though it had been closed, regardless of whether
/// closePath() was actually called.
fn close_all_subpaths(path: &BezPath) -> BezPath {
  let mut closed = BezPath::new();
  let mut subpath_open = false;
  for el in path.iter() {
    match el {
      PathEl::MoveTo(_) => {
        if subpath_open {
          closed.push(PathEl::ClosePath);
        }
        subpath_open = true;
      }
      PathEl::ClosePath => subpath_open = false,
      _ => {}
    }
    closed.push(el);
  }
  if subpath_open {
    closed.push(PathEl::ClosePath);
  }
  closed
}

/// Returns whether `pt` lies on (within floating-point tolerance of) any
/// segment of `path`. Per spec, points exactly on the path's boundary
/// count as inside for isPointInPath().
fn point_on_path_boundary(path: &BezPath, pt: Point) -> bool {
  const EPSILON_SQ: f64 = 1e-9;
  path
    .segments()
    .any(|seg| seg.nearest(pt, 1e-6).distance_sq <= EPSILON_SQ)
}

fn test_point_in_path(path: BezPath, x: f64, y: f64, rule: String) -> bool {
  let path = close_all_subpaths(&path);
  let pt = Point::new(x, y);
  if point_on_path_boundary(&path, pt) {
    return true;
  }
  let w = path.winding(pt);
  match rule.as_str() {
    "evenodd" => w % 2 != 0,
    _ => w != 0,
  }
}

/// Creates a drawing backend, preferring the GPU but choosing the CPU when
/// browser-style heuristics say so: `willReadFrequently`, repeated pixel
/// readbacks, a canvas too small to benefit from the GPU, or one too large
/// for a GPU texture.
fn create_drawing_backend(
  renderer: &SharedRenderer,
  will_read_frequently: bool,
  readback_count: u32,
  width: u32,
  height: u32,
) -> DrawingBackend {
  let cpu = DenoCanvasBackend::Cpu(CpuRenderer);
  let Some(Some(backend)) = renderer.get() else {
    return DrawingBackend::new(&cpu, width, height);
  };
  let use_cpu = match backend {
    DenoCanvasBackend::Cpu(_) => true,
    DenoCanvasBackend::Gpu(gpu) => {
      will_read_frequently
        || readback_count >= GPU_READBACK_FALLBACK_THRESHOLD
        || (width as u64) * (height as u64) < MIN_GPU_ACCELERATED_AREA
        || width.max(height) > gpu.max_texture_dimension_2d()
    }
  };
  DrawingBackend::new(if use_cpu { &cpu } else { backend }, width, height)
}

/// Creates an OffscreenCanvasRenderingContext2D object.
#[allow(
  clippy::too_many_arguments,
  reason = "matches CreateCanvasContext signature"
)]
pub fn create_context<'s>(
  state: std::rc::Rc<std::cell::RefCell<OpState>>,
  _instance: Option<deno_webgpu::Instance>,
  canvas: v8::Global<v8::Object>,
  data: deno_webgpu::canvas::ContextData,
  scope: &mut v8::PinScope<'s, '_>,
  options: v8::Local<'s, v8::Value>,
  prefix: &'static str,
  context: &'static str,
) -> Result<v8::Global<v8::Value>, JsErrorBox> {
  let (width, height) = data.dimensions();
  let (renderer, font_ctx, layout_ctx) = {
    let state = state.borrow();
    let renderer = state
      .try_borrow::<SharedRenderer>()
      .ok_or(Canvas2DError::NotInitialized)?
      .clone();
    let font_ctx = state
      .try_borrow::<Rc<RefCell<FontContext>>>()
      .ok_or(Canvas2DError::NotInitialized)?
      .clone();
    let layout_ctx = state
      .try_borrow::<Rc<RefCell<LayoutContext<()>>>>()
      .ok_or(Canvas2DError::NotInitialized)?
      .clone();
    (renderer, font_ctx, layout_ctx)
  };

  // Non-object options are ignored.
  let options = if options.is_object() || options.is_null_or_undefined() {
    options
  } else {
    v8::undefined(scope).into()
  };
  let settings = Canvas2DSettings::convert(
    scope,
    options,
    prefix.into(),
    (|| context.into()).into(),
    &(),
  )
  .map_err(Canvas2DError::from)?;
  renderer.get_or_init(super::renderer::init_canvas_renderer);

  let ctx = OffscreenCanvasRenderingContext2D {
    canvas,
    data,
    drawing: RefCell::new(create_drawing_backend(
      &renderer,
      settings.will_read_frequently,
      0,
      width,
      height,
    )),
    renderer,
    font_ctx,
    layout_ctx,
    state: RefCell::new(DrawingState::default()),
    state_stack: RefCell::new(Vec::new()),
    layer_depth: std::cell::Cell::new(0),
    clip_stack: RefCell::new(Vec::new()),
    current_path: RefCell::new(BezPath::new()),
    settings,
    readback_count: std::cell::Cell::new(0),
  };

  let obj = deno_core::cppgc::make_cppgc_object(scope, ctx);
  let val: v8::Local<v8::Value> = obj.cast();
  Ok(v8::Global::new(scope, val))
}
