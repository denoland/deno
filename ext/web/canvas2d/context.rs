// Copyright 2018-2026 the Deno authors. MIT license.

mod draw;
mod hit_test;
mod renderer;
mod state;
mod text;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::UnrestrictedDouble;
use deno_core::webidl::WebIdlConverter;
use deno_error::JsErrorBox;
use deno_image::image::DynamicImage;
use parley::FontContext;
use parley::LayoutContext;
pub(crate) use renderer::init_canvas_renderer;
use vello::kurbo;
use vello::kurbo::Affine;
use vello::kurbo::BezPath;
use vello::kurbo::PathEl;
use vello::kurbo::Point;
use vello::kurbo::Rect;
use vello::kurbo::Vec2;
use vello::peniko;

use self::renderer::DenoCanvasBackend;
use self::renderer::SharedRenderer;
use self::state::Canvas2DSettings;
use self::state::ClipEntry;
use self::state::DrawingBackend;
use self::state::DrawingState;
use self::state::FillStrokeStyle;
use self::state::FilterStyle;
use self::state::GlobalCompositeOperation;
use self::state::ImageSmoothingQuality;
use self::state::LineCap;
use self::state::LineJoin;
use self::state::StateStackEntry;
use self::state::TextAlign;
use self::state::TextBaseline;
use self::text::compute_text_metrics;
use crate::canvas2d::error::Canvas2DError;
use crate::canvas2d::filter::CanvasFilter;
use crate::canvas2d::filter::validate_filter_input;
use crate::canvas2d::gradient::CanvasGradient;
use crate::canvas2d::gradient::build_conic_gradient;
use crate::canvas2d::gradient::build_linear_gradient;
use crate::canvas2d::gradient::build_radial_gradient;
use crate::canvas2d::image::image_data_from_pixels;
use crate::canvas2d::image::image_data_from_premultiplied_pixels;
use crate::canvas2d::image::resolve_canvas_image_source;
use crate::canvas2d::image::unpremultiply_rgba;
use crate::canvas2d::path::arc_to_impl;
use crate::canvas2d::path::build_round_rect_path;
use crate::canvas2d::path::compute_arc_sweep;
use crate::canvas2d::path::parse_round_rect_radii;
use crate::canvas2d::pattern::CanvasPattern;
use crate::canvas2d::pattern::pad_pattern_image;
use crate::canvas2d::pattern::parse_repetition;
use crate::css::color::Color;
use crate::css::color::color_to_css_string;
use crate::css::color::parse_css_color;
use crate::css::filter::FilterValueListParser;
use crate::css::filter::ParserInput as FilterParserInput;
use crate::css::font::FontState;
use crate::css::font::parse_css_font;
use crate::css::font::parse_css_spacing;
use crate::image_data::ImageData;
use crate::text_metrics::TextMetrics;

pub const CONTEXT_ID: &str = "2d";
pub const UNSTABLE_FEATURE_NAME: &str = "canvas2d";

/// Rejects `ImageData` allocations whose backing buffer would exceed what a
/// JS typed array can address, so absurd `getImageData()`/`createImageData()`
/// requests throw instead of aborting on an out-of-memory allocation.
fn check_image_data_size(w: u32, h: u32) -> Result<(), Canvas2DError> {
  const MAX_IMAGE_DATA_BYTES: u64 = i32::MAX as u64;
  if (w as u64) * (h as u64) * 4 > MAX_IMAGE_DATA_BYTES {
    return Err(Canvas2DError::ImageDataTooLarge);
  }
  Ok(())
}

/// Validates the `CanvasLayerOptions` argument of `beginLayer()`. The layer
/// `filter` is only validated, not rendered (see `CanvasFilter`).
fn validate_begin_layer_options(
  scope: &mut v8::PinScope<'_, '_>,
  options: v8::Local<'_, v8::Value>,
) -> Result<(), Canvas2DError> {
  if options.is_null_or_undefined() {
    return Ok(());
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
    return Ok(());
  }
  if !filter.is_object() {
    // The DOMString branch of the filter union: any string (or stringifiable
    // primitive) is accepted, even when it is not a parsable CSS filter.
    return Ok(());
  }
  if deno_core::cppgc::try_unwrap_cppgc_object::<CanvasFilter>(scope, filter)
    .is_some()
  {
    return Ok(());
  }
  validate_filter_input(scope, filter)
}

pub struct OffscreenCanvasRenderingContext2D {
  canvas: v8::Global<v8::Object>,
  data: deno_webgpu::canvas::ContextData,

  drawing: RefCell<DrawingBackend>,

  renderer: SharedRenderer,

  font_ctx: Arc<Mutex<FontContext>>,
  layout_ctx: Arc<Mutex<LayoutContext<()>>>,

  state: RefCell<DrawingState>,
  state_stack: RefCell<Vec<StateStackEntry>>,

  layer_depth: std::cell::Cell<usize>,

  clip_stack: RefCell<Vec<ClipEntry>>,

  current_path: RefCell<BezPath>,

  settings: Canvas2DSettings,
}

// SAFETY: OffscreenCanvasRenderingContext2D is only accessed from the JS thread.
unsafe impl GarbageCollected for OffscreenCanvasRenderingContext2D {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OffscreenCanvasRenderingContext2D"
  }
}

impl OffscreenCanvasRenderingContext2D {
  fn transform_path(path: &BezPath, transform: Affine) -> BezPath {
    if transform == Affine::IDENTITY {
      return path.clone();
    }

    let mut transformed = BezPath::new();
    transformed.extend(path.iter().map(|el| match el {
      PathEl::MoveTo(p) => {
        PathEl::MoveTo(Self::transform_point_or_original(transform, p))
      }
      PathEl::LineTo(p) => {
        PathEl::LineTo(Self::transform_point_or_original(transform, p))
      }
      PathEl::QuadTo(p1, p2) => PathEl::QuadTo(
        Self::transform_point_or_original(transform, p1),
        Self::transform_point_or_original(transform, p2),
      ),
      PathEl::CurveTo(p1, p2, p3) => PathEl::CurveTo(
        Self::transform_point_or_original(transform, p1),
        Self::transform_point_or_original(transform, p2),
        Self::transform_point_or_original(transform, p3),
      ),
      PathEl::ClosePath => PathEl::ClosePath,
    }));
    transformed
  }

  fn transform_point_or_original(transform: Affine, point: Point) -> Point {
    let p = transform * point;
    if p.x.is_finite() && p.y.is_finite() {
      p
    } else {
      point
    }
  }

  fn append_transformed_path(&self, path: &BezPath, transform: Affine) {
    self
      .current_path
      .borrow_mut()
      .extend(Self::transform_path(path, transform).iter());
  }

  /// Appends a shape (built in the local coordinate space corresponding to
  /// `transform`, starting with a `MoveTo`) to the current default path. Per
  /// spec, `arc()`, `ellipse()`, and `arcTo()` join to an existing subpath
  /// with a straight line instead of starting a new one, so when the
  /// current default path is non-empty, the shape's leading `MoveTo` is
  /// downgraded to a `LineTo`.
  fn append_shape_path(&self, path: &BezPath, transform: Affine) {
    let transformed = Self::transform_path(path, transform);
    let mut current = self.current_path.borrow_mut();
    if current.elements().is_empty() {
      current.extend(transformed.iter());
      return;
    }
    let mut iter = transformed.iter();
    if let Some(PathEl::MoveTo(p)) = iter.next() {
      current.push(PathEl::LineTo(p));
    }
    current.extend(iter);
  }

  /// Returns the last on-path point of `path` in the same coordinate space
  /// the path is stored in, or `None` if the path has no subpath yet (i.e.
  /// it is empty or ends with `ClosePath`, which per spec is equivalent to
  /// having no current point for arcTo()'s purposes).
  fn last_path_point(path: &BezPath) -> Option<Point> {
    match path.elements().last()? {
      PathEl::MoveTo(p) => Some(*p),
      PathEl::LineTo(p) => Some(*p),
      PathEl::QuadTo(_, p) => Some(*p),
      PathEl::CurveTo(_, _, p) => Some(*p),
      PathEl::ClosePath => None,
    }
  }
  pub fn has_open_layers(&self) -> bool {
    draw::has_open_layers(self)
  }

  pub fn resize(&self) {
    draw::resize(self)
  }

  pub fn render_to_bytes(&self) -> Result<Vec<u8>, Canvas2DError> {
    draw::render_to_bytes(self)
  }

  pub fn render_to_texture_view(
    &self,
    view: &self::renderer::wgpu::TextureView,
  ) -> Result<(), Canvas2DError> {
    draw::render_to_texture_view(self, view)
  }

  pub fn flush_to_image(&self, image: &mut DynamicImage) {
    draw::flush_to_image(self, image)
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
        let s = color_to_css_string(*c);
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
    if let Some(style) = draw::parse_fill_stroke_style(scope, value) {
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
        let s = color_to_css_string(*c);
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
    if let Some(style) = draw::parse_fill_stroke_style(scope, value) {
      self.state.borrow_mut().stroke_style = style;
    }
  }

  #[getter]
  fn global_alpha(&self) -> f64 {
    self.state.borrow().global_alpha as f64
  }

  #[setter]
  fn global_alpha(&self, #[webidl] value: UnrestrictedDouble) {
    if value.is_finite() && *value >= 0.0 && *value <= 1.0 {
      self.state.borrow_mut().global_alpha = *value as f32;
    }
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font>
  #[getter]
  #[string]
  fn font(&self) -> String {
    self.state.borrow().font_state.to_css_string()
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font>
  #[setter]
  fn font(&self, #[webidl] value: String) {
    if let Some(state) = parse_css_font(&value) {
      let mut s = self.state.borrow_mut();
      // The font shorthand only covers style, variant-caps, weight, stretch,
      // size, line-height and family. The other text drawing styles are
      // independent attributes and must survive a font change.
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

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-direction>
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

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-lang>
  #[getter]
  #[string]
  fn lang(&self) -> String {
    self.state.borrow().lang.clone()
  }

  #[setter]
  fn lang(&self, #[webidl] value: String) {
    self.state.borrow_mut().lang = value;
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-fontkerning>
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

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-fontstretch>
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

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-fontvariantcaps>
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

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-letterspacing>
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

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-wordspacing>
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

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-textrendering>
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
    let state = self.state.borrow();
    let op = state.global_composite_operation;
    let alpha = state.global_alpha;
    let shadow = draw::has_shadow(&state);
    let shadow_color = state.shadow_color_rgba;
    let shadow_xform = if shadow {
      Some(draw::shadow_transform(&state, state.transform))
    } else {
      None
    };
    let (brush, brush_transform) =
      draw::resolve_brush(scope, &state.fill_style, 1.0);
    let transform = state.transform;
    drop(state);
    let rect = Rect::new(*x, *y, *x + *w, *y + *h);
    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      draw::push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let Some(st) = shadow_xform {
      draw::draw_shadow(&mut drawing, width, height, shadow_color, |d| {
        draw::fill_on(
          d,
          &rect,
          peniko::Fill::NonZero,
          st,
          brush.clone(),
          brush_transform,
        );
      });
      // Per spec, the shadow is composited first, then the source shape is
      // composited on top as a separate step.
      if has_layer {
        draw::pop_compositing_layer(&mut drawing);
        draw::push_compositing_layer(&mut drawing, op, alpha, width, height);
      }
    }
    draw::fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      transform,
      brush,
      brush_transform,
    );
    if has_layer {
      draw::pop_compositing_layer(&mut drawing);
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
    // Erase the rect to transparent black regardless of the current
    // globalCompositeOperation/globalAlpha, by punching it out with an
    // opaque brush under a canvas-wide "destination-out" layer. A plain
    // source-over fill with a transparent color would be a no-op instead.
    draw::push_compositing_layer(
      &mut drawing,
      GlobalCompositeOperation::DestinationOut,
      1.0,
      width,
      height,
    );
    draw::fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      transform,
      peniko::Brush::Solid(peniko::Color::BLACK),
      None,
    );
    draw::pop_compositing_layer(&mut drawing);
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-filltext>
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
    draw::draw_text(self, scope, &text, *x, *y, max_width.map(|v| *v), false);
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-stroketext>
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
    draw::draw_text(self, scope, &text, *x, *y, max_width.map(|v| *v), true);
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-measuretext>
  #[required(1)]
  #[cppgc]
  fn measure_text(&self, #[string] text: &str) -> TextMetrics {
    compute_text_metrics(
      text,
      &self.state.borrow().font_state,
      self.state.borrow().text_align,
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
    match &self.state.borrow().filter_style {
      FilterStyle::Css(value) => v8::String::new(scope, value).unwrap().into(),
      FilterStyle::Object(object) => v8::Local::new(scope, object).into(),
    }
  }

  #[reentrant]
  #[setter]
  fn filter(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
  ) {
    if deno_core::cppgc::try_unwrap_cppgc_object::<CanvasFilter>(scope, value)
      .is_some()
    {
      let mut state = self.state.borrow_mut();
      state.filter_style =
        FilterStyle::Object(v8::Global::new(scope, value.cast::<v8::Object>()));
      // Object-form filters carry no CSS filter function list.
      state.filter = Vec::new();
      return;
    }
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
      state.filter = functions;
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
    if value.is_finite() && *value > 0.0 {
      self.state.borrow_mut().line_width = *value;
    }
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
    if value.is_finite() && *value > 0.0 {
      self.state.borrow_mut().miter_limit = *value;
    }
  }

  #[getter]
  fn line_dash_offset(&self) -> f64 {
    self.state.borrow().line_dash_offset
  }

  #[setter]
  fn line_dash_offset(&self, #[webidl] value: UnrestrictedDouble) {
    if value.is_finite() {
      self.state.borrow_mut().line_dash_offset = *value;
    }
  }

  #[getter]
  fn shadow_blur(&self) -> f64 {
    self.state.borrow().shadow_blur
  }

  #[setter]
  fn shadow_blur(&self, #[webidl] value: UnrestrictedDouble) {
    if value.is_finite() && *value >= 0.0 {
      self.state.borrow_mut().shadow_blur = *value;
    }
  }

  #[getter]
  #[string]
  fn shadow_color(&self) -> String {
    self.state.borrow().shadow_color.clone()
  }

  #[setter]
  fn shadow_color(&self, #[webidl] value: String) {
    if let Ok(rgba) = parse_css_color(&value) {
      let mut state = self.state.borrow_mut();
      // The getter must return the serialized form, not the raw input.
      state.shadow_color = color_to_css_string(rgba);
      state.shadow_color_rgba = rgba;
    }
  }

  #[getter]
  fn shadow_offset_x(&self) -> f64 {
    self.state.borrow().shadow_offset_x
  }

  #[setter]
  fn shadow_offset_x(&self, #[webidl] value: UnrestrictedDouble) {
    if value.is_finite() {
      self.state.borrow_mut().shadow_offset_x = *value;
    }
  }

  #[getter]
  fn shadow_offset_y(&self) -> f64 {
    self.state.borrow().shadow_offset_y
  }

  #[setter]
  fn shadow_offset_y(&self, #[webidl] value: UnrestrictedDouble) {
    if value.is_finite() {
      self.state.borrow_mut().shadow_offset_y = *value;
    }
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
            draw::pop_compositing_layer(&mut self.drawing.borrow_mut());
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
  fn begin_layer(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    options: v8::Local<'_, v8::Value>,
  ) -> Result<(), Canvas2DError> {
    validate_begin_layer_options(scope, options)?;

    let current_state = self.state.borrow().clone();
    let op = current_state.global_composite_operation;
    let alpha = current_state.global_alpha;

    self.layer_depth.set(self.layer_depth.get() + 1);

    {
      let mut state = self.state.borrow_mut();
      state.global_alpha = 1.0;
      state.global_composite_operation = GlobalCompositeOperation::SourceOver;
      state.shadow_color = String::from("rgba(0, 0, 0, 0)");
      state.shadow_color_rgba = Color::TRANSPARENT;
      state.shadow_offset_x = 0.0;
      state.shadow_offset_y = 0.0;
      state.shadow_blur = 0.0;
      state.filter_style = FilterStyle::Css(String::from("none"));
      state.filter = Vec::new();
    }

    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let pushed =
      draw::push_compositing_layer(&mut drawing, op, alpha, width, height);

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

    let mut stack = self.state_stack.borrow_mut();
    // A save() with no matching restore() sits on top of the stack:
    // endLayer() must not reach past it to an earlier beginLayer().
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
      draw::pop_compositing_layer(&mut self.drawing.borrow_mut());
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
    if x.is_finite() && y.is_finite() {
      let transform = self.state.borrow().transform;
      let mut path = self.current_path.borrow_mut();
      path.move_to(transform * Point::new(*x, *y));
    }
  }

  #[required(2)]
  #[undefined]
  fn line_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if x.is_finite() && y.is_finite() {
      let transform = self.state.borrow().transform;
      let p = Self::transform_point_or_original(transform, Point::new(*x, *y));
      let mut path = self.current_path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to(p);
      }
      path.line_to(p);
    }
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
    if cp1x.is_finite()
      && cp1y.is_finite()
      && cp2x.is_finite()
      && cp2y.is_finite()
      && x.is_finite()
      && y.is_finite()
    {
      let transform = self.state.borrow().transform;
      let cp1 =
        Self::transform_point_or_original(transform, Point::new(*cp1x, *cp1y));
      let cp2 =
        Self::transform_point_or_original(transform, Point::new(*cp2x, *cp2y));
      let p = Self::transform_point_or_original(transform, Point::new(*x, *y));
      let mut path = self.current_path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to(cp1);
      }
      path.curve_to(cp1, cp2, p);
    }
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
    if cpx.is_finite() && cpy.is_finite() && x.is_finite() && y.is_finite() {
      let transform = self.state.borrow().transform;
      let cp =
        Self::transform_point_or_original(transform, Point::new(*cpx, *cpy));
      let p = Self::transform_point_or_original(transform, Point::new(*x, *y));
      let mut path = self.current_path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to(cp);
      }
      path.quad_to(cp, p);
    }
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
    // Per spec, non-finite arguments are silently ignored; only a finite
    // negative radius throws IndexSizeError.
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

    let delta = compute_arc_sweep(*start_angle, *end_angle, counterclockwise);

    let transform = self.state.borrow().transform;
    let mut path = BezPath::new();
    let arc = kurbo::Arc {
      center: Point::new(*x, *y),
      radii: Vec2::new(*radius, *radius),
      start_angle: *start_angle,
      sweep_angle: delta,
      x_rotation: 0.0,
    };

    let (sin_a, cos_a) = start_angle.sin_cos();
    let start_pt = arc.center + Vec2::new(*radius * cos_a, *radius * sin_a);
    path.move_to(start_pt);
    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
      path.curve_to(p1, p2, p3);
    });
    self.append_shape_path(&path, transform);
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
    // Per spec, non-finite arguments are silently ignored; only a finite
    // negative radius throws IndexSizeError.
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
    // Per spec, arcTo() behaves like moveTo(x1, y1) when there is no
    // current subpath to compute the tangent circle from.
    let Some(current_canvas_pt) =
      Self::last_path_point(&self.current_path.borrow())
    else {
      let mut path = BezPath::new();
      path.move_to((*x1, *y1));
      self.append_transformed_path(&path, transform);
      return Ok(());
    };
    // arc_to_impl's tangent-circle math assumes a Euclidean coordinate
    // system, so it must run in user space: the current point is mapped
    // back through the inverse CTM, and the resulting path is transformed
    // forward again before being joined onto the current default path.
    if transform.determinant() == 0.0 {
      return Ok(());
    }
    let user_current = transform.inverse() * current_canvas_pt;
    let mut path = BezPath::new();
    path.move_to(user_current);
    arc_to_impl(&mut path, *x1, *y1, *x2, *y2, *radius);
    self.append_shape_path(&path, transform);
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

    let delta = compute_arc_sweep(*start_angle, *end_angle, counterclockwise);

    let transform = self.state.borrow().transform;
    let mut path = BezPath::new();
    let arc = kurbo::Arc {
      center: Point::new(*x, *y),
      radii: Vec2::new(*radius_x, *radius_y),
      start_angle: *start_angle,
      sweep_angle: delta,
      x_rotation: *rotation,
    };

    let (sin_a, cos_a) = start_angle.sin_cos();
    let dx = *radius_x * cos_a;
    let dy = *radius_y * sin_a;
    let (sin_r, cos_r) = rotation.sin_cos();
    let start_pt =
      Point::new(*x + dx * cos_r - dy * sin_r, *y + dx * sin_r + dy * cos_r);
    path.move_to(start_pt);
    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
      path.curve_to(p1, p2, p3);
    });
    self.append_shape_path(&path, transform);
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
    let mut path = BezPath::new();
    path.move_to((*x, *y));
    path.line_to((*x + *w, *y));
    path.line_to((*x + *w, *y + *h));
    path.line_to((*x, *y + *h));
    path.close_path();
    self.append_transformed_path(&path, transform);
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
    let mut path = BezPath::new();
    build_round_rect_path(&mut path, *x, *y, *w, *h, &corner_radii);
    self.append_transformed_path(&path, transform);
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
    // Per spec this is equivalent to stroking a rect() path, even when one
    // dimension is zero: Rect's own path conversion may collapse a
    // zero-height/width rect to nothing, but an explicit closed 4-segment
    // path still traces out and back along the non-zero dimension, so
    // caps/joins render a degenerate "doubled-over line" as required.
    let mut path = BezPath::new();
    path.move_to((*x, *y));
    path.line_to((*x + *w, *y));
    path.line_to((*x + *w, *y + *h));
    path.line_to((*x, *y + *h));
    path.close_path();
    let transform = self.state.borrow().transform;
    draw::draw_path_stroke(self, scope, path, transform, true);
  }

  #[undefined]
  fn fill(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    first: Option<v8::Local<'_, v8::Value>>,
    #[string] second: Option<String>,
  ) {
    let (path, rule, is_path2d) =
      draw::resolve_path_and_fill_rule(self, scope, first, second);
    if path.is_empty() {
      return;
    }
    let transform = if is_path2d {
      self.state.borrow().transform
    } else {
      Affine::IDENTITY
    };
    draw::draw_path_fill(self, scope, path, rule, transform);
  }

  #[fast]
  #[undefined]
  fn stroke(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path: Option<v8::Local<'_, v8::Value>>,
  ) {
    let (path, is_path2d) = draw::resolve_optional_path(self, scope, path);
    if path.is_empty() {
      return;
    }
    let transform = self.state.borrow().transform;
    draw::draw_path_stroke(self, scope, path, transform, is_path2d);
  }

  #[undefined]
  fn clip(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    first: Option<v8::Local<'_, v8::Value>>,
    #[string] second: Option<String>,
  ) {
    let (path, rule, is_path2d) =
      draw::resolve_path_and_fill_rule(self, scope, first, second);
    // Note: unlike fill()/stroke(), an empty path must not be a no-op here
    // -- per spec, clipping to an empty path shrinks the clip region to
    // nothing -- so apply_clip() is still called (it handles the empty
    // case itself).
    let transform = if is_path2d {
      self.state.borrow().transform
    } else {
      Affine::IDENTITY
    };
    draw::apply_clip(self, path, rule, transform);
  }

  #[fast]
  fn is_point_in_path(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    a: Option<v8::Local<'_, v8::Value>>,
    b: Option<v8::Local<'_, v8::Value>>,
    c: v8::Local<'_, v8::Value>,
    d: v8::Local<'_, v8::Value>,
  ) -> Result<bool, Canvas2DError> {
    // The op2 `Option` conversion folds an explicit `null` into `None`, but
    // the slots that can carry a CanvasFillRule (a non-nullable enum) must
    // distinguish them: `null` stringifies to "null" (an invalid variant)
    // while an omitted argument (padded to `undefined` by V8) falls back to
    // "nonzero".
    let c = (!c.is_undefined()).then_some(c);
    let d = (!d.is_undefined()).then_some(d);
    let (path, x, y, rule, is_path2d) =
      hit_test::resolve_point_in_path_args(self, scope, a, b, c, d)?;
    if !x.is_finite() || !y.is_finite() {
      return Ok(false);
    }
    // Per spec, isPointInPath() returns false outright when the current
    // transformation matrix has no inverse.
    let transform = self.state.borrow().transform;
    if transform.determinant() == 0.0 {
      return Ok(false);
    }
    let p = if is_path2d {
      transform.inverse() * Point::new(x, y)
    } else {
      Point::new(x, y)
    };
    Ok(hit_test::test_point_in_path(path, p.x, p.y, rule))
  }

  #[fast]
  fn is_point_in_stroke(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    a: Option<v8::Local<'_, v8::Value>>,
    b: Option<v8::Local<'_, v8::Value>>,
    c: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<bool, Canvas2DError> {
    let (path, x, y, is_path2d) =
      hit_test::resolve_point_in_stroke_args(self, scope, a, b, c)?;
    if !x.is_finite() || !y.is_finite() {
      return Ok(false);
    }
    // Per spec, isPointInStroke() returns false outright when the current
    // transformation matrix has no inverse.
    let transform = self.state.borrow().transform;
    if transform.determinant() == 0.0 {
      return Ok(false);
    }
    // The stroke outline is always built in user space (see
    // test_point_in_stroke), so the query point must land there too,
    // regardless of whether it is being tested against the default path or
    // an explicit Path2D.
    let p = transform.inverse() * Point::new(x, y);
    Ok(hit_test::test_point_in_stroke(
      self, path, p.x, p.y, transform, is_path2d,
    ))
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
    draw::require_finite(&[x0, y0, x1, y1])?;
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
    draw::require_finite(&[x0, y0, r0, x1, y1, r1])?;
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
    draw::require_finite(&[start_angle, x, y])?;
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
    #[webidl] a1: UnrestrictedDouble,
    #[webidl] a2: UnrestrictedDouble,
    a3: Option<v8::Local<'a, v8::Value>>,
    a4: Option<v8::Local<'a, v8::Value>>,
    a5: Option<v8::Local<'a, v8::Value>>,
    a6: Option<v8::Local<'a, v8::Value>>,
    a7: Option<v8::Local<'a, v8::Value>>,
    a8: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<(), Canvas2DError> {
    let resolved = resolve_canvas_image_source(state, scope, image)?;

    let has_a3 = a3.as_ref().map(|v| !v.is_undefined()).unwrap_or(false);
    let has_a5 = a5.as_ref().map(|v| !v.is_undefined()).unwrap_or(false);

    let (sx, sy, sw, sh, dx, dy, dw, dh) = if has_a5 {
      // 9-arg: (image, sx, sy, sw, sh, dx, dy, dw, dh)
      let sx = *a1;
      let sy = *a2;
      let sw = a3.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let sh = a4.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let dx = a5.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let dy = a6.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let dw = a7.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let dh = a8.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
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
    } else if has_a3 {
      // 5-arg: (image, dx, dy, dw, dh)
      let dx = *a1;
      let dy = *a2;
      let dw = a3.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
      let dh = a4.and_then(|v| v.number_value(scope)).unwrap_or(f64::NAN);
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
      let dx = *a1;
      let dy = *a2;
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

    // Normalize both rectangles' negative width/height by flipping the
    // origin and taking the absolute size -- per spec this only changes
    // *which* pixels are selected/where they land, it must not mirror the
    // rendered image (unlike a negative scale would).
    let (sx, sw) = if sw < 0.0 { (sx + sw, -sw) } else { (sx, sw) };
    let (sy, sh) = if sh < 0.0 { (sy + sh, -sh) } else { (sy, sh) };
    let (dx, dw) = if dw < 0.0 { (dx + dw, -dw) } else { (dx, dw) };
    let (dy, dh) = if dh < 0.0 { (dy + dh, -dh) } else { (dy, dh) };

    let img =
      image_data_from_pixels(resolved.pixels, resolved.width, resolved.height);

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

    // The shape is the (possibly fractional) source rect in the image's own
    // pixel space; the transform maps that rect onto the destination rect,
    // so the image is sampled at full precision without an intermediate
    // pixel-copy/crop step (which previously truncated fractional source
    // coordinates to integers).
    let scale_x = dw / sw;
    let scale_y = dh / sh;
    let image_transform = ds.transform
      * Affine::translate((dx, dy))
      * Affine::scale_non_uniform(scale_x, scale_y)
      * Affine::translate((-sx, -sy));

    let op = ds.global_composite_operation;
    let alpha = ds.global_alpha;
    let shadow = draw::has_shadow(&ds);
    let shadow_color = ds.shadow_color_rgba;
    let shadow_xform = if shadow {
      Some(draw::shadow_transform(&ds, image_transform))
    } else {
      None
    };
    drop(ds);

    let rect = Rect::new(sx, sy, sx + sw, sy + sh);
    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      draw::push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let Some(st) = shadow_xform {
      draw::draw_shadow(&mut drawing, width, height, shadow_color, |d| {
        draw::fill_on(d, &rect, peniko::Fill::NonZero, st, brush.clone(), None);
      });
      // Per spec, the shadow is composited first, then the source image is
      // composited on top as a separate step.
      if has_layer {
        draw::pop_compositing_layer(&mut drawing);
        draw::push_compositing_layer(&mut drawing, op, alpha, width, height);
      }
    }
    draw::fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      image_transform,
      brush,
      None,
    );
    if has_layer {
      draw::pop_compositing_layer(&mut drawing);
    }
    Ok(())
  }

  #[required(1)]
  #[cppgc]
  fn create_image_data<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
    arg0: v8::Local<'a, v8::Value>,
    arg1: Option<v8::Local<'a, v8::Value>>,
  ) -> Result<ImageData, Canvas2DError> {
    if let Some(imagedata) =
      deno_core::cppgc::try_unwrap_cppgc_object::<ImageData>(scope, arg0)
    {
      let w = imagedata.get_width();
      let h = imagedata.get_height();
      let pixels = vec![0u8; w as usize * h as usize * 4];
      return Ok(ImageData::new_rgba_unorm8(scope, w, h, &pixels)?);
    }

    let Some(arg1) = arg1.filter(|v| !v.is_undefined()) else {
      return Err(Canvas2DError::MissingArgument {
        required: 2,
        provided: 1,
      });
    };

    let sw = draw::require_long(scope, arg0)?;
    let sh = draw::require_long(scope, arg1)?;

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
    dirty_x_val: Option<v8::Local<'_, v8::Value>>,
    dirty_y_val: Option<v8::Local<'_, v8::Value>>,
    dirty_w_val: Option<v8::Local<'_, v8::Value>>,
    dirty_h_val: Option<v8::Local<'_, v8::Value>>,
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

    let has_dirty = dirty_x_val
      .as_ref()
      .map(|v| !v.is_undefined())
      .unwrap_or(false);

    let (mut dirty_x, mut dirty_y, mut dirty_w, mut dirty_h) = if has_dirty {
      let dirty_x = draw::require_long(scope, dirty_x_val.unwrap())?;
      let dirty_y = draw::require_long(
        scope,
        dirty_y_val.unwrap_or_else(|| v8::undefined(scope).into()),
      )?;
      let dirty_w = draw::require_long(
        scope,
        dirty_w_val.unwrap_or_else(|| v8::undefined(scope).into()),
      )?;
      let dirty_h = draw::require_long(
        scope,
        dirty_h_val.unwrap_or_else(|| v8::undefined(scope).into()),
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

    let img = image_data_from_premultiplied_pixels(pixels, canvas_w, canvas_h);
    let image_brush = peniko::ImageBrush::new(img);
    let brush = peniko::Brush::Image(image_brush);
    let rect = Rect::new(0.0, 0.0, canvas_w as f64, canvas_h as f64);

    let clip_depth = self.state.borrow().clip_depth;
    let mut drawing = self.drawing.borrow_mut();
    drawing.reset(canvas_w, canvas_h);
    draw::fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      Affine::IDENTITY,
      brush,
      None,
    );
    let clip_stack = self.clip_stack.borrow();
    for clip in clip_stack.iter().take(clip_depth) {
      let fill = if clip.rule == "evenodd" {
        peniko::Fill::EvenOdd
      } else {
        peniko::Fill::NonZero
      };
      draw::push_clip(&mut drawing, fill, clip.transform, &clip.path);
    }
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

/// Creates an OffscreenCanvasRenderingContext2D cppgc object.
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
      .try_borrow::<Arc<Mutex<FontContext>>>()
      .ok_or(Canvas2DError::NotInitialized)?
      .clone();
    let layout_ctx = state
      .try_borrow::<Arc<Mutex<LayoutContext<()>>>>()
      .ok_or(Canvas2DError::NotInitialized)?
      .clone();
    (renderer, font_ctx, layout_ctx)
  };

  // Per spec, a non-object `options` value (e.g. a number, string, or
  // symbol) is not a WebIDL dictionary conversion failure here -- it is
  // simply ignored, as though no options were passed at all.
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

  let ctx = OffscreenCanvasRenderingContext2D {
    canvas,
    data,
    drawing: RefCell::new({
      if settings.will_read_frequently {
        DrawingBackend::new(
          &DenoCanvasBackend::Cpu(self::renderer::CpuRenderer),
          width,
          height,
        )
      } else {
        match renderer.get() {
          Some(Some(backend)) => DrawingBackend::new(backend, width, height),
          _ => DrawingBackend::Vello(vello::Scene::new()),
        }
      }
    }),
    renderer,
    font_ctx,
    layout_ctx,
    state: RefCell::new(DrawingState::default()),
    state_stack: RefCell::new(Vec::new()),
    layer_depth: std::cell::Cell::new(0),
    clip_stack: RefCell::new(Vec::new()),
    current_path: RefCell::new(BezPath::new()),
    settings,
  };

  let obj = deno_core::cppgc::make_cppgc_object(scope, ctx);
  let val: v8::Local<v8::Value> = obj.cast();
  Ok(v8::Global::new(scope, val))
}

/// Placeholder init op (reserved for future initialization).
#[op2(fast)]
pub fn op_canvas2d_init(_state: &mut OpState) {}
