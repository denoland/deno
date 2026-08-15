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

use super::filter::parse_filter_input;
use super::renderer::CpuRenderer;
pub(crate) use super::renderer::DenoCanvasBackend;
use super::renderer::DrawingBackend;
use super::renderer::SharedRenderer;
use super::state::Canvas2DSettings;
use super::state::CanvasFillRule;
use super::state::ClipEntry;
use super::state::DrawingState;
use super::state::FillStrokeStyle;
use super::state::GlobalCompositeOperation;
use super::state::ImageSmoothingQuality;
use super::state::LayerFilter;
use super::state::LineCap;
use super::state::LineJoin;
use super::state::MAX_CANVAS_DIMENSION;
use super::state::StateStackEntry;
use super::state::TextAlign;
use super::state::TextBaseline;
use super::text::alignment_anchor;
use super::text::build_text_layout;
use super::text::compute_baseline_y;
use super::text::compute_text_metrics;
use super::text::font_metric_offsets;
use super::v8_util::to_f64;
use crate::canvas2d::TextMetrics;
use crate::canvas2d::error::Canvas2DError;
use crate::canvas2d::font_metrics::length_resolution;
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
use crate::css::value::LengthResolution;
use crate::font::SharedLocalFontDb;
use crate::image_data::ImageData;

pub const CONTEXT_ID: &str = "2d";
pub const UNSTABLE_FEATURE_NAME: &str = "canvas2d";

/// Pixel readbacks (getImageData / putImageData / convertToBlob) tolerated
/// on the GPU backend before falling back to CPU, per Chromium's heuristic.
const GPU_READBACK_FALLBACK_THRESHOLD: u32 = 2;

/// Canvases smaller than this area render on the CPU, where they beat the
/// GPU's per-draw overhead. Matches Blink's 128 * 129 heuristic.
const MIN_GPU_ACCELERATED_AREA: u64 = 128 * 129;

/// Guards the one-time warning for drawing text with no fonts available.
static NO_FONTS_WARNING: std::sync::Once = std::sync::Once::new();

pub struct OffscreenCanvasRenderingContext2D {
  canvas: v8::Global<v8::Object>,
  data: deno_webgpu::canvas::ContextData,

  drawing: RefCell<DrawingBackend>,
  renderer: SharedRenderer,

  font_ctx: Rc<RefCell<FontContext>>,
  layout_ctx: Rc<RefCell<LayoutContext<()>>>,
  local_fonts: SharedLocalFontDb,

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
    // Parent font is the default `10px sans-serif` (no element).
    // https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font
    let resolution = self.default_font_resolution();
    if let Some(state) = parse_css_font(&value, &resolution) {
      let mut s = self.state.borrow_mut();
      // Keep non-shorthand canvas text state.
      s.font_state = FontState {
        direction: s.font_state.direction,
        font_kerning: s.font_state.font_kerning,
        letter_spacing: s.font_state.letter_spacing.clone(),
        word_spacing: s.font_state.word_spacing.clone(),
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
    if let Some(v) = TextAlign::from_str(&value) {
      self.state.borrow_mut().text_align = v;
    }
  }

  #[getter]
  #[string]
  fn text_baseline(&self) -> &'static str {
    self.state.borrow().text_baseline.as_str()
  }

  #[setter]
  fn text_baseline(&self, #[webidl] value: String) {
    if let Some(v) = TextBaseline::from_str(&value) {
      self.state.borrow_mut().text_baseline = v;
    }
  }

  #[getter]
  #[string]
  fn direction(&self) -> &'static str {
    self.state.borrow().font_state.direction.as_str()
  }

  #[setter]
  fn direction(&self, #[webidl] value: String) {
    if let Some(d) = crate::css::font::TextDirection::from_str(&value) {
      self.state.borrow_mut().font_state.direction = d;
    }
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
    if let Some(k) = crate::css::font::FontKerning::from_str(&value) {
      self.state.borrow_mut().font_state.font_kerning = k;
    }
  }

  #[getter]
  #[string]
  fn font_stretch(&self) -> &'static str {
    crate::css::font::width_to_css_str(self.state.borrow().font_state.width)
  }

  #[setter]
  fn font_stretch(&self, #[webidl] value: String) {
    if let Some(w) = crate::css::font::parse_css_width_keyword(&value) {
      self.state.borrow_mut().font_state.width = w;
    }
  }

  #[getter]
  #[string]
  fn font_variant_caps(&self) -> &'static str {
    self.state.borrow().font_state.font_variant_caps.as_str()
  }

  #[setter]
  fn font_variant_caps(&self, #[webidl] value: String) {
    if let Some(v) = crate::css::font::FontVariantCaps::from_str(&value) {
      self.state.borrow_mut().font_state.font_variant_caps = v;
    }
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
    let resolution = self.current_font_resolution();
    if let Some(spacing) = parse_css_spacing(&value, &resolution) {
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
    let resolution = self.current_font_resolution();
    if let Some(spacing) = parse_css_spacing(&value, &resolution) {
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
    if let Some(r) = crate::css::font::TextRendering::from_str(&value) {
      self.state.borrow_mut().font_state.text_rendering = r;
    }
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

    let paint = {
      let state = self.state.borrow();
      paint_from_style(scope, &state, &state.fill_style, state.transform)
    };
    let rect = Rect::new(*x, *y, *x + *w, *y + *h);
    self.paint(paint, |d, transform, brush, brush_transform| {
      d.fill(
        peniko::Fill::NonZero,
        transform,
        brush,
        brush_transform,
        &rect,
      );
    });
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
    drawing.fill(
      peniko::Fill::NonZero,
      transform,
      peniko::Brush::Solid(peniko::Color::BLACK),
      None,
      &rect,
    );
    drawing.pop_layer();
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
    self.sync_system_fonts();
    let state = self.state.borrow();
    compute_text_metrics(
      text,
      &state.font_state,
      state.text_align,
      &state.lang,
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
    if let Some(op) = GlobalCompositeOperation::from_str(&value) {
      self.state.borrow_mut().global_composite_operation = op;
    }
  }

  // TODO(petamoriken): apply CSS filters once Vello GPU supports filter effects
  #[getter]
  fn filter<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Value> {
    let state = self.state.borrow();
    v8::String::new(scope, &state.filter_style).unwrap().into()
  }

  #[reentrant]
  #[setter]
  fn filter(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
  ) {
    // A failed ToString() leaves its exception pending; an invalid filter
    // string is ignored.
    let Some(value) = value.to_string(scope) else {
      return;
    };

    let value = value.to_rust_string_lossy(scope);
    // Filter lengths resolve at set time against the default `font`.
    // https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-filter
    let resolution = self.default_font_resolution();
    let functions = {
      let mut parser_input = FilterParserInput::new(&value);
      let result: Result<Vec<_>, _> =
        FilterValueListParser::new(&mut parser_input, resolution).collect();
      result.ok()
    };
    if let Some(functions) = functions {
      let mut state = self.state.borrow_mut();
      state.filter_style = value;
      state.layer_filter = LayerFilter::Css(functions);
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
    if let Some(v) = ImageSmoothingQuality::from_str(&value) {
      self.state.borrow_mut().image_smoothing_quality = v;
    }
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
    if let Some(v) = LineCap::from_str(&value) {
      self.state.borrow_mut().line_cap = v;
    }
  }

  #[getter]
  #[string]
  fn line_join(&self) -> &'static str {
    self.state.borrow().line_join.as_str()
  }

  #[setter]
  fn line_join(&self, #[webidl] value: String) {
    if let Some(v) = LineJoin::from_str(&value) {
      self.state.borrow_mut().line_join = v;
    }
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
            self.drawing.borrow_mut().pop_layer();
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
    // A layer filter is a `<filter-value-list>` too, so its relative lengths
    // resolve the same way `ctx.filter` does.
    let layer_filter = parse_begin_layer_options(
      scope,
      options,
      self.default_font_resolution(),
    )?;

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
      state.filter_style = String::from("none");
      state.layer_filter = layer_filter;
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
      self.drawing.borrow_mut().pop_layer();
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
    // Ignore non-finite values; reject finite negative radii.
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
    // A non-finite corner radius is ignored, matching the x/y/w/h check above.
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

    // Explicit rect path so degenerate sizes still stroke. Built in user
    // space; `draw_path_stroke` applies the CTM.
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

    // Path2D tests in user space: scale tolerance by the CTM. The default
    // path is already in device space.
    let (p, scale) = if is_path2d {
      (
        transform.inverse() * Point::new(x, y),
        transform.determinant().abs().sqrt(),
      )
    } else {
      (Point::new(x, y), 1.0)
    };
    Ok(test_point_in_path(path, p.x, p.y, rule, scale))
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

    let paint = {
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
      // Sample the fractional source rect directly.
      let image_transform = ds.transform
        * Affine::translate((dx, dy))
        * Affine::scale_non_uniform(dw / sw, dh / sh)
        * Affine::translate((-sx, -sy));
      paint_from_brush(
        &ds,
        peniko::Brush::Image(image_brush),
        None,
        image_transform,
      )
    };

    let rect = Rect::new(sx, sy, sx + sw, sy + sh);
    self.paint(paint, |d, transform, brush, _| {
      d.fill(peniko::Fill::NonZero, transform, brush, None, &rect);
    });
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
  #[inline]
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

  fn base_color(&self) -> peniko::Color {
    if self.settings.alpha {
      peniko::Color::TRANSPARENT
    } else {
      peniko::Color::from_rgb8(0, 0, 0)
    }
  }

  fn with_clips_popped<T>(
    &self,
    f: impl FnOnce(&mut DrawingBackend) -> T,
  ) -> T {
    let clip_depth = self.state.borrow().clip_depth;
    let mut drawing = self.drawing.borrow_mut();
    for _ in 0..clip_depth {
      drawing.pop_layer();
    }
    let result = f(&mut drawing);
    replay_clips(&mut drawing, &self.clip_stack.borrow(), clip_depth);
    result
  }

  fn paint(
    &self,
    paint: PaintSnapshot,
    mut draw: impl FnMut(&mut DrawingBackend, Affine, peniko::Brush, Option<Affine>),
  ) {
    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer = push_compositing_layer(
      &mut drawing,
      paint.op,
      paint.alpha,
      width,
      height,
    );
    if let Some(st) = paint.shadow_xform {
      draw_shadow(&mut drawing, width, height, paint.shadow_color, |d| {
        draw(d, st, paint.brush.clone(), paint.brush_transform);
      });
      if has_layer {
        drawing.pop_layer();
        push_compositing_layer(
          &mut drawing,
          paint.op,
          paint.alpha,
          width,
          height,
        );
      }
    }
    draw(
      &mut drawing,
      paint.transform,
      paint.brush,
      paint.brush_transform,
    );
    if has_layer {
      drawing.pop_layer();
    }
  }

  /// Renders the scene to RGBA8 bytes.
  pub fn render_to_bytes(&self) -> Result<Vec<u8>, Canvas2DError> {
    let (width, height) = self.data.dimensions();
    let base_color = self.base_color();
    self.with_clips_popped(|drawing| {
      let pixels = match drawing.render_to_rgba(
        self.renderer.get().and_then(Option::as_ref),
        width,
        height,
        base_color,
      )? {
        Some(mut buf) => {
          if !self.settings.alpha && !drawing.is_gpu() {
            for pixel in buf.chunks_exact_mut(4) {
              pixel[3] = 255;
            }
          }
          buf
        }
        None => vec![0u8; (width * height * 4) as usize],
      };
      Ok(pixels)
    })
  }

  /// User-facing pixel readback. After `GPU_READBACK_FALLBACK_THRESHOLD`
  /// switches GPU → CPU (Chromium heuristic).
  #[inline]
  pub fn increment_readback_and_check_fallback(&self) {
    if !self.drawing.borrow().is_gpu() {
      return;
    }
    self.readback_count.set(self.readback_count.get() + 1);
    // Can't flatten the scene while layers are open; retry next readback.
    if self.readback_count.get() >= GPU_READBACK_FALLBACK_THRESHOLD
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
    drawing.fill(peniko::Fill::NonZero, Affine::IDENTITY, brush, None, &rect);
    replay_clips(
      drawing,
      &self.clip_stack.borrow(),
      self.state.borrow().clip_depth,
    );
  }

  /// Renders the scene into a TextureView owned by this renderer's device.
  pub fn render_to_texture_view(
    &self,
    view: &super::renderer::wgpu::TextureView,
  ) -> Result<(), Canvas2DError> {
    let (width, height) = self.data.dimensions();
    self.drawing.borrow().render_to_texture_view(
      self.renderer.get().and_then(Option::as_ref),
      view,
      width,
      height,
      self.base_color(),
    )
  }

  /// Renders the accumulated scene into a DynamicImage.
  pub fn flush_to_image(&self, image: &mut DynamicImage) {
    let (width, height) = image.dimensions();
    let base_color = self.base_color();
    let buf = self.with_clips_popped(|drawing| {
      match drawing.render_to_rgba(
        self.renderer.get().and_then(Option::as_ref),
        width,
        height,
        base_color,
      ) {
        Ok(buf) => buf,
        Err(e) => {
          log::warn!("Failed to render the canvas: {e}");
          None
        }
      }
      .map(|mut buf| {
        // Convert premultiplied alpha to straight alpha for the host bitmap.
        if self.settings.alpha {
          unpremultiply_rgba(&mut buf);
        } else {
          for pixel in buf.chunks_exact_mut(4) {
            pixel[3] = 255;
          }
        }
        buf
      })
    });
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
  ) -> (BezPath, CanvasFillRule, bool) {
    if let Some(v) = path_or_fill_rule {
      if v.is_string() {
        let rule = parse_fill_rule(&v.to_rust_string_lossy(scope));
        return (self.current_path.borrow().clone(), rule, false);
      }
      if let Some(p) =
        deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v)
      {
        return (
          p.path.borrow().clone(),
          parse_fill_rule_opt(fill_rule),
          true,
        );
      }
    }
    (
      self.current_path.borrow().clone(),
      parse_fill_rule_opt(fill_rule),
      false,
    )
  }

  fn draw_path_fill(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path: BezPath,
    rule: CanvasFillRule,
    transform: Affine,
  ) {
    if path.is_empty() {
      return;
    }
    let fill = rule.to_peniko();
    let paint = {
      let state = self.state.borrow();
      paint_from_style(scope, &state, &state.fill_style, transform)
    };
    self.paint(paint, |d, transform, brush, brush_transform| {
      d.fill(fill, transform, brush, brush_transform, &path);
    });
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
    let (paint, stroke) = {
      let state = self.state.borrow();
      let paint =
        paint_from_style(scope, &state, &state.stroke_style, transform);
      (paint, build_stroke(&state))
    };
    let path = if is_path2d {
      path
    } else {
      transform_path(&path, transform.inverse())
    };
    self.paint(paint, |d, transform, brush, brush_transform| {
      d.stroke(&stroke, transform, brush, brush_transform, &path);
    });
  }

  fn apply_clip(&self, path: BezPath, rule: CanvasFillRule, transform: Affine) {
    // Per spec, clipping with an empty path shrinks the clip region to
    // nothing; represent that with a zero-area shape at the identity transform.
    let (path, transform) = if path.is_empty() {
      (Shape::to_path(&Rect::ZERO, 0.1), Affine::IDENTITY)
    } else {
      (path, transform)
    };
    self
      .drawing
      .borrow_mut()
      .push_clip(rule.to_peniko(), transform, &path);
    let mut state = self.state.borrow_mut();
    self.clip_stack.borrow_mut().truncate(state.clip_depth);
    self.clip_stack.borrow_mut().push(ClipEntry {
      path,
      rule,
      transform,
    });
    state.clip_depth += 1;
  }

  /// Load system fonts if registerLocalFonts ran anywhere in the process.
  /// Checked per op (workers never call it themselves); load is idempotent.
  fn load_system_fonts_if_enabled(&self) -> bool {
    let enabled = self.local_fonts.system_fonts_enabled();
    if enabled {
      self.font_ctx.borrow_mut().collection.load_system_fonts();
    }
    enabled
  }

  /// Metrics of the canvas default font, which font-relative lengths in the
  /// `font` shorthand resolve against (there is no parent element).
  fn default_font_resolution(&self) -> LengthResolution {
    self.load_system_fonts_if_enabled();
    length_resolution(&mut self.font_ctx.borrow_mut(), &FontState::default())
  }

  /// Metrics of the font in effect, which font-relative lengths in
  /// `letterSpacing` / `wordSpacing` resolve against.
  fn current_font_resolution(&self) -> LengthResolution {
    self.load_system_fonts_if_enabled();
    let state = self.state.borrow();
    length_resolution(&mut self.font_ctx.borrow_mut(), &state.font_state)
  }

  /// Same as [`Self::load_system_fonts_if_enabled`], plus the one-time warning
  /// for drawing text with nothing to draw it with.
  fn sync_system_fonts(&self) {
    if !self.load_system_fonts_if_enabled() && !NO_FONTS_WARNING.is_completed()
    {
      let mut font_ctx = self.font_ctx.borrow_mut();
      if font_ctx.collection.family_names().next().is_none() {
        NO_FONTS_WARNING.call_once(|| {
          log::warn!(
            "Canvas 2D text will not render because no fonts are available. \
             Call Deno.registerLocalFonts() with --allow-sys=localFonts to \
             use the system fonts, or register one with new FontFace()."
          );
        });
      }
    }
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

    self.sync_system_fonts();
    let mut fc = self.font_ctx.borrow_mut();
    let mut lc = self.layout_ctx.borrow_mut();
    let (layout, paint, text_align, text_baseline, direction) = {
      let state = self.state.borrow();
      let layout = build_text_layout(
        &mut fc,
        &mut lc,
        text,
        &state.font_state,
        &state.lang,
      );
      let style = if stroke {
        &state.stroke_style
      } else {
        &state.fill_style
      };
      let paint = paint_from_style(scope, &state, style, state.transform);
      (
        layout,
        paint,
        state.text_align,
        state.text_baseline,
        state.font_state.direction,
      )
    };

    let metric_offsets =
      font_metric_offsets(&layout, self.state.borrow().font_state.size as f64);
    let baseline_y = compute_baseline_y(y, text_baseline, &metric_offsets);

    let layout_baseline = layout
      .lines()
      .next()
      .map(|line| line.metrics().baseline)
      .unwrap_or(0.0);

    // Line width for text-align (trailing spaces kept; no collapse).
    // https://html.spec.whatwg.org/multipage/canvas.html#text-preparation-algorithm
    let line_width: f32 = layout
      .lines()
      .next()
      .map(|line| line.metrics().advance)
      .unwrap_or(0.0);

    // Scale advances + outlines when wider than maxWidth.
    let x_scale: f32 = match max_width {
      Some(max_width) if (line_width as f64) > max_width => {
        (max_width / line_width as f64) as f32
      }
      _ => 1.0,
    };
    let scaled_width = line_width * x_scale;

    let rtl = direction == TextDirection::Rtl;
    let draw_x =
      x as f32 - alignment_anchor(scaled_width as f64, text_align, rtl) as f32;
    let baseline_y = baseline_y as f32;

    self.paint(paint, |d, transform, brush, brush_transform| {
      fill_layout_glyphs(
        d,
        &layout,
        transform,
        &brush,
        brush_transform,
        draw_x,
        baseline_y,
        layout_baseline,
        x_scale,
      );
    });
  }

  fn resolve_point_in_path_args(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path_or_x: Option<v8::Local<'_, v8::Value>>,
    x_or_y: Option<v8::Local<'_, v8::Value>>,
    y_or_fill_rule: Option<v8::Local<'_, v8::Value>>,
    fill_rule: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<(BezPath, f64, f64, CanvasFillRule, bool), Canvas2DError> {
    const PREFIX: &str = "Failed to execute 'isPointInPath' on 'OffscreenCanvasRenderingContext2D'";

    let parse_required_fill_rule =
      |context: &'static str,
       rule: &str|
       -> Result<CanvasFillRule, Canvas2DError> {
        CanvasFillRule::from_str(rule).ok_or_else(|| {
          Canvas2DError::WebIdl(deno_core::webidl::WebIdlError {
            prefix: PREFIX.into(),
            context: context.into(),
            kind: deno_core::webidl::WebIdlErrorKind::InvalidEnumVariant {
              converter: "CanvasFillRule",
              variant: rule.to_string(),
            },
          })
        })
      };

    let Some(path_or_x) = path_or_x else {
      if fill_rule.is_some() {
        // 4 args: isPointInPath(path, x, y, fillRule) — null/undefined is not Path2D
        return Err(type_error_not_path2d(PREFIX, "parameter 1"));
      }
      if x_or_y.is_some() {
        // 2-3 args with null/undefined first: isPointInPath(x, y [, fillRule])
        let y = x_or_y.map(|v| to_f64(scope, v)).unwrap_or(f64::NAN);
        let rule = y_or_fill_rule
          .map(|v| v.to_rust_string_lossy(scope))
          .unwrap_or_else(|| "nonzero".into());
        let rule = parse_required_fill_rule("parameter 3", &rule)?;
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
      let x = to_f64(scope, x_or_y);
      let y = to_f64(scope, y_or_fill_rule);
      // Non-nullable enum: explicit `null` becomes `"null"`, not the default.
      let rule = fill_rule
        .map(|v| v.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "nonzero".into());
      let rule = parse_required_fill_rule("parameter 4", &rule)?;
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
      let x = to_f64(scope, path_or_x);
      let y = to_f64(scope, x_or_y);
      let rule = y_or_fill_rule
        .map(|v| v.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "nonzero".into());
      let rule = parse_required_fill_rule("parameter 3", &rule)?;
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
        let y = x_or_y.map(|v| to_f64(scope, v)).unwrap_or(f64::NAN);
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
      let x = x_or_y.map(|v| to_f64(scope, v)).unwrap_or(f64::NAN);
      let y = y.map(|v| to_f64(scope, v)).unwrap_or(f64::NAN);
      return Ok((p.path.borrow().clone(), x, y, true));
    }
    if path_or_x.is_number() {
      // isPointInStroke(x, y)
      let x = to_f64(scope, path_or_x);
      let y = x_or_y.map(|v| to_f64(scope, v)).unwrap_or(f64::NAN);
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
    // Stroke in user space (lineWidth/lineDash). Map the default path back
    // from canvas space; Path2D is already user-space.
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
  resolution: LengthResolution,
) -> Result<LayerFilter, Canvas2DError> {
  if options.is_null_or_undefined() {
    return Ok(LayerFilter::Css(Vec::new()));
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
    return Ok(LayerFilter::Css(Vec::new()));
  }
  if !filter.is_object() {
    // Any (stringifiable) value is accepted here, even if not a parsable
    // CSS filter.
    let Some(value) = filter.to_string(scope) else {
      return Ok(LayerFilter::Css(Vec::new()));
    };
    let value = value.to_rust_string_lossy(scope);
    let mut parser_input = FilterParserInput::new(&value);
    let result: Result<Vec<_>, _> =
      FilterValueListParser::new(&mut parser_input, resolution).collect();
    return Ok(LayerFilter::Css(result.ok().unwrap_or_default()));
  }
  Ok(LayerFilter::Object(parse_filter_input(scope, filter)?))
}

#[inline]
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
  // A thrown toString() exception is left pending to propagate; an invalid
  // color string leaves the style unchanged.
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
      // Sort by offset; keep call order for equal offsets.
      g.stops.sort_by(|a, b| {
        a.offset
          .partial_cmp(&b.offset)
          .unwrap_or(std::cmp::Ordering::Equal)
      });
      // Degenerate gradients paint nothing; use a transparent brush.
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
      // Undo `pad_pattern_image()` padding so content stays in place.
      let pattern_transform = *pattern.transform.borrow()
        * Affine::translate(-pattern.content_offset);
      (peniko::Brush::Image(image_brush), Some(pattern_transform))
    }
  }
}

struct PaintSnapshot {
  op: GlobalCompositeOperation,
  alpha: f32,
  shadow_color: peniko::Color,
  shadow_xform: Option<Affine>,
  brush: peniko::Brush,
  brush_transform: Option<Affine>,
  transform: Affine,
}

fn paint_from_style(
  scope: &mut v8::PinScope<'_, '_>,
  state: &DrawingState,
  style: &FillStrokeStyle,
  transform: Affine,
) -> PaintSnapshot {
  let (brush, brush_transform) = resolve_brush(scope, style, 1.0);
  paint_from_brush(state, brush, brush_transform, transform)
}

fn paint_from_brush(
  state: &DrawingState,
  brush: peniko::Brush,
  brush_transform: Option<Affine>,
  transform: Affine,
) -> PaintSnapshot {
  PaintSnapshot {
    op: state.global_composite_operation,
    alpha: state.global_alpha,
    shadow_color: state.shadow_color.to_srgb8(),
    shadow_xform: has_shadow(state).then(|| shadow_transform(state, transform)),
    brush,
    brush_transform,
    transform,
  }
}

#[inline]
fn parse_fill_rule(rule: &str) -> CanvasFillRule {
  CanvasFillRule::from_str(rule).unwrap_or_default()
}

#[inline]
fn parse_fill_rule_opt(rule: Option<String>) -> CanvasFillRule {
  rule
    .as_deref()
    .map_or(CanvasFillRule::Nonzero, parse_fill_rule)
}

fn replay_clips(
  drawing: &mut DrawingBackend,
  clips: &[ClipEntry],
  clip_depth: usize,
) {
  for clip in clips.iter().take(clip_depth) {
    drawing.push_clip(clip.rule.to_peniko(), clip.transform, &clip.path);
  }
}

#[allow(clippy::too_many_arguments, reason = "glyph placement parameters")]
fn fill_layout_glyphs(
  drawing: &mut DrawingBackend,
  layout: &parley::Layout<()>,
  transform: Affine,
  brush: &peniko::Brush,
  brush_transform: Option<Affine>,
  draw_x: f32,
  baseline_y: f32,
  layout_baseline: f32,
  x_scale: f32,
) {
  for line in layout.lines() {
    for item in line.items() {
      let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
        continue;
      };
      let font = peniko::FontData::clone(glyph_run.run().font());
      // Keep outlines in step with condensed advances.
      let font_size = glyph_run.run().font_size() * x_scale;
      let glyphs: Vec<_> = glyph_run
        .positioned_glyphs()
        .map(|g| {
          (
            g.id,
            draw_x + g.x * x_scale,
            baseline_y + g.y - layout_baseline,
          )
        })
        .collect();
      drawing.fill_glyphs(
        &font,
        font_size,
        transform,
        brush,
        brush_transform,
        &glyphs,
      );
    }
  }
}

#[inline]
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
  drawing.push_layer(op.to_blend_mode(), alpha, width, height);
  true
}

/// Draws a shadow using the source content's alpha mask.
fn draw_shadow(
  drawing: &mut DrawingBackend,
  width: u32,
  height: u32,
  shadow_color: peniko::Color,
  draw_source: impl FnOnce(&mut DrawingBackend),
) {
  drawing.push_layer(
    peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcOver),
    1.0,
    width,
    height,
  );
  draw_source(drawing);
  drawing.push_layer(
    peniko::BlendMode::new(peniko::Mix::Normal, peniko::Compose::SrcIn),
    1.0,
    width,
    height,
  );
  let canvas_rect = Rect::new(0.0, 0.0, width as f64, height as f64);
  drawing.fill(
    peniko::Fill::NonZero,
    Affine::IDENTITY,
    peniko::Brush::Solid(shadow_color),
    None,
    &canvas_rect,
  );
  drawing.pop_layer(); // pop the SrcIn tint layer
  drawing.pop_layer(); // pop the SrcOver isolation layer
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

#[inline]
fn has_shadow(state: &DrawingState) -> bool {
  !state.shadow_color.is_transparent()
    && (state.shadow_blur > 0.0
      || state.shadow_offset_x != 0.0
      || state.shadow_offset_y != 0.0)
}

#[inline]
fn shadow_transform(state: &DrawingState, transform: Affine) -> Affine {
  // TODO(petamoriken): apply shadowBlur once Vello GPU supports filter effects
  Affine::translate((state.shadow_offset_x, state.shadow_offset_y)) * transform
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

/// Close every subpath. Hit-testing / fill / clip treat them as closed.
#[inline]
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

/// True if `pt` is on a segment (boundary counts). `scale` keeps tolerance
/// in device pixels for user-space Path2D tests.
#[inline]
fn point_on_path_boundary(path: &BezPath, pt: Point, scale: f64) -> bool {
  const EPSILON_SQ: f64 = 1e-9;
  let epsilon_sq = EPSILON_SQ / (scale * scale);
  path
    .segments()
    .any(|seg| seg.nearest(pt, 1e-6).distance_sq <= epsilon_sq)
}

fn test_point_in_path(
  path: BezPath,
  x: f64,
  y: f64,
  rule: CanvasFillRule,
  scale: f64,
) -> bool {
  let path = close_all_subpaths(&path);
  let pt = Point::new(x, y);
  if point_on_path_boundary(&path, pt, scale) {
    return true;
  }
  let w = path.winding(pt);
  match rule {
    CanvasFillRule::Evenodd => w % 2 != 0,
    CanvasFillRule::Nonzero => w != 0,
  }
}

/// Creates a drawing backend, preferring the GPU but falling back to the CPU
/// per the browser-style heuristics below.
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
) -> Result<Option<v8::Global<v8::Value>>, JsErrorBox> {
  let (width, height) = data.dimensions();
  if width > MAX_CANVAS_DIMENSION || height > MAX_CANVAS_DIMENSION {
    log::warn!(
      "The canvas size ({width}x{height}) exceeds the maximum supported \
       dimension of {MAX_CANVAS_DIMENSION}px; getContext(\"2d\") returns null."
    );
    return Ok(None);
  }
  let (renderer, font_ctx, layout_ctx, local_fonts) = {
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
    let local_fonts = state
      .try_borrow::<SharedLocalFontDb>()
      .ok_or(Canvas2DError::NotInitialized)?
      .clone();
    (renderer, font_ctx, layout_ctx, local_fonts)
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
    local_fonts,
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
  Ok(Some(v8::Global::new(scope, val)))
}
