// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;

use cosmic_text::Attrs;
use cosmic_text::Buffer;
use cosmic_text::Family;
use cosmic_text::FeatureTag;
use cosmic_text::FontFeatures;
use cosmic_text::Metrics;
use cosmic_text::Shaping;
use cosmic_text::SwashCache;
use cosmic_text::Weight;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::UnrestrictedDouble;
use deno_error::JsErrorBox;
use deno_image::image::DynamicImage;
use deno_image::image::Rgba;
use deno_image::image::RgbaImage;
use vello::kurbo;
use vello::peniko;

use crate::canvas2d_renderer::DenoCanvasBackend;
use crate::canvas2d_renderer::SharedRenderer;
use crate::canvas2d_renderer::render_scene;
use crate::canvas2d_renderer::render_scene_to_texture_view;
use crate::css::color::parse_css_color;
use crate::css::color::rgba8_to_css;
use crate::css::font::FontKerning;
use crate::css::font::FontState;
use crate::css::font::TextDirection;
use crate::css::font::parse_css_font;
use crate::css::font::parse_css_spacing;
use crate::text_metrics::TextMetrics;

pub const CONTEXT_ID: &str = "2d";
pub const UNSTABLE_FEATURE_NAME: &str = "canvas2d";

// TODO(petamoriken): move to a shared crate when canvas2d and webgpu types need to be unified.
// ext/webgpu/canvas.rs has its own PredefinedColorSpace with additional variants.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PredefinedColorSpace {
  #[default]
  Srgb,
  // TODO(petamoriken): rendering in display-p3 color space is not yet implemented.
  DisplayP3,
}

// TODO(petamoriken): move to a shared crate when canvas2d and webgpu types need to be unified.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CanvasColorType {
  #[default]
  Unorm8,
  // TODO(petamoriken): float16 rendering is not yet implemented.
  Float16,
}

/// Settings passed as the second argument to `getContext("2d", settings)`.
#[derive(Clone, Copy, Debug)]
pub struct Canvas2DSettings {
  /// When false, the canvas background is always opaque (base color is black).
  pub alpha: bool,
  // TODO(petamoriken): hint; ignored in headless context.
  pub desynchronized: bool,
  pub color_space: PredefinedColorSpace,
  pub color_type: CanvasColorType,
  // TODO(petamoriken): reserved for future getImageData optimization.
  pub will_read_frequently: bool,
}

impl Default for Canvas2DSettings {
  fn default() -> Self {
    Self {
      alpha: true,
      desynchronized: false,
      color_space: PredefinedColorSpace::Srgb,
      color_type: CanvasColorType::Unorm8,
      will_read_frequently: false,
    }
  }
}

#[derive(Clone, Copy, Default)]
pub enum TextAlign {
  #[default]
  Start,
  End,
  Left,
  Right,
  Center,
}

#[derive(Clone, Copy, Default)]
pub enum TextBaseline {
  #[default]
  Alphabetic,
  Top,
  Hanging,
  Middle,
  Ideographic,
  Bottom,
}

#[derive(Clone, Copy, Default)]
pub enum ImageSmoothingQuality {
  #[default]
  Low,
  Medium,
  High,
}

#[derive(Clone, Copy, Default)]
pub enum LineCap {
  #[default]
  Butt,
  Round,
  Square,
}

#[derive(Clone, Copy, Default)]
pub enum LineJoin {
  Round,
  Bevel,
  #[default]
  Miter,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum Canvas2DError {
  #[class(type)]
  #[error("Illegal constructor")]
  IllegalConstructor,
  #[class("DOMExceptionNotSupportedError")]
  #[error("OffscreenCanvasRenderingContext2D.{0}() is not yet implemented")]
  NotSupported(&'static str),
}

// `DrawingBackend` abstracts over two unrelated vello renderer families that do
// not share a scene type today:
//
//   vello::Scene             -> vello::Renderer  (Gpu / Hybrid; GPU compute via wgpu)
//   vello_cpu::RenderContext -> vello_cpu        (Cpu; pure software, no wgpu)
//
// The sparse-strips renderers (vello_cpu / vello_hybrid) share a common
// experimental `vello_api::Scene` interface, but it lacks glyph/text rendering,
// and the GPU-compute `vello` crate does not use it at all. Once `vello_api`
// stabilizes with text support, the Cpu and Hybrid backends could be unified
// behind a single `vello_api::Scene`; revisit `DrawingBackend` then.
pub enum DrawingBackend {
  // Shared by Gpu and Hybrid
  Vello(vello::Scene),
  VelloCpu(vello_cpu::RenderContext, Box<vello_cpu::Resources>),
}

impl DrawingBackend {
  pub fn new(backend: &DenoCanvasBackend, width: u32, height: u32) -> Self {
    match backend {
      DenoCanvasBackend::Cpu(_) => DrawingBackend::VelloCpu(
        vello_cpu::RenderContext::new(width as u16, height as u16),
        Box::new(vello_cpu::Resources::new()),
      ),
      _ => DrawingBackend::Vello(vello::Scene::new()),
    }
  }

  pub fn reset(&mut self, width: u32, height: u32) {
    match self {
      DrawingBackend::Vello(scene) => scene.reset(),
      DrawingBackend::VelloCpu(ctx, resources) => {
        *ctx = vello_cpu::RenderContext::new(width as u16, height as u16);
        **resources = vello_cpu::Resources::new();
      }
    }
  }
}

pub struct OffscreenCanvasRenderingContext2D {
  pub canvas: v8::Global<v8::Object>,
  pub width: Cell<u32>,
  pub height: Cell<u32>,

  // Accumulates drawing commands for rendering.
  pub drawing: RefCell<DrawingBackend>,

  // Shared GPU renderer from OpState.
  pub renderer: SharedRenderer,

  // Shared font resources from OpState.
  pub font_system: Arc<Mutex<cosmic_text::FontSystem>>,
  pub swash_cache: Arc<Mutex<SwashCache>>,

  // Drawing state (interior mutability for setters).
  pub fill_color: Cell<[u8; 4]>,
  pub stroke_color: Cell<[u8; 4]>,
  pub global_alpha: Cell<f32>,
  pub font_state: RefCell<FontState>,
  pub text_align: Cell<TextAlign>,
  pub text_baseline: Cell<TextBaseline>,
  pub lang: RefCell<String>,

  // TODO(petamoriken): stored-only state. These are tracked to satisfy the API
  // surface but are not yet applied during rendering.
  pub global_composite_operation: RefCell<String>,
  pub filter: RefCell<String>,
  pub image_smoothing_enabled: Cell<bool>,
  pub image_smoothing_quality: Cell<ImageSmoothingQuality>,
  pub line_width: Cell<f64>,
  pub line_cap: Cell<LineCap>,
  pub line_join: Cell<LineJoin>,
  pub miter_limit: Cell<f64>,
  pub line_dash_offset: Cell<f64>,
  pub shadow_blur: Cell<f64>,
  pub shadow_color: RefCell<String>,
  pub shadow_offset_x: Cell<f64>,
  pub shadow_offset_y: Cell<f64>,

  pub settings: Canvas2DSettings,
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
  #[string]
  fn fill_style(&self) -> String {
    rgba8_to_css(self.fill_color.get())
  }

  #[setter]
  fn fill_style(&self, #[webidl] value: String) {
    if let Ok(c) = parse_css_color(&value) {
      self.fill_color.set(c);
    }
  }

  #[getter]
  #[string]
  fn stroke_style(&self) -> String {
    rgba8_to_css(self.stroke_color.get())
  }

  #[setter]
  fn stroke_style(&self, #[webidl] value: String) {
    if let Ok(c) = parse_css_color(&value) {
      self.stroke_color.set(c);
    }
  }

  #[getter]
  fn global_alpha(&self) -> f64 {
    self.global_alpha.get() as f64
  }

  #[setter]
  fn global_alpha(&self, #[webidl] value: f64) {
    self.global_alpha.set(value.clamp(0.0, 1.0) as f32);
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font>
  #[getter]
  #[string]
  fn font(&self) -> String {
    self.font_state.borrow().to_css_string()
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-font>
  #[setter]
  fn font(&self, #[webidl] value: String) {
    if let Some(state) = parse_css_font(&value) {
      let mut fstate = self.font_state.borrow_mut();
      // The font shorthand only covers style, variant-caps, weight, stretch,
      // size, line-height and family. The other text drawing styles are
      // independent attributes and must survive a font change.
      *fstate = FontState {
        direction: fstate.direction,
        font_kerning: fstate.font_kerning,
        letter_spacing: fstate.letter_spacing,
        word_spacing: fstate.word_spacing,
        text_rendering: fstate.text_rendering,
        ..state
      };
    }
  }

  #[getter]
  #[string]
  fn text_align(&self) -> &'static str {
    match self.text_align.get() {
      TextAlign::Start => "start",
      TextAlign::End => "end",
      TextAlign::Left => "left",
      TextAlign::Right => "right",
      TextAlign::Center => "center",
    }
  }

  #[setter]
  fn text_align(&self, #[webidl] value: String) {
    self.text_align.set(match value.as_str() {
      "start" => TextAlign::Start,
      "end" => TextAlign::End,
      "left" => TextAlign::Left,
      "right" => TextAlign::Right,
      "center" => TextAlign::Center,
      _ => return,
    });
  }

  #[getter]
  #[string]
  fn text_baseline(&self) -> &'static str {
    match self.text_baseline.get() {
      TextBaseline::Top => "top",
      TextBaseline::Hanging => "hanging",
      TextBaseline::Middle => "middle",
      TextBaseline::Alphabetic => "alphabetic",
      TextBaseline::Ideographic => "ideographic",
      TextBaseline::Bottom => "bottom",
    }
  }

  #[setter]
  fn text_baseline(&self, #[webidl] value: String) {
    self.text_baseline.set(match value.as_str() {
      "top" => TextBaseline::Top,
      "hanging" => TextBaseline::Hanging,
      "middle" => TextBaseline::Middle,
      "alphabetic" => TextBaseline::Alphabetic,
      "ideographic" => TextBaseline::Ideographic,
      "bottom" => TextBaseline::Bottom,
      _ => return,
    });
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-direction>
  #[getter]
  #[string]
  fn direction(&self) -> &'static str {
    match self.font_state.borrow().direction {
      crate::css::font::TextDirection::Inherit => "inherit",
      crate::css::font::TextDirection::Ltr => "ltr",
      crate::css::font::TextDirection::Rtl => "rtl",
    }
  }

  #[setter]
  fn direction(&self, #[webidl] value: String) {
    let d = match value.as_str() {
      "inherit" => crate::css::font::TextDirection::Inherit,
      "ltr" => crate::css::font::TextDirection::Ltr,
      "rtl" => crate::css::font::TextDirection::Rtl,
      _ => return,
    };
    self.font_state.borrow_mut().direction = d;
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-lang>
  #[getter]
  #[string]
  fn lang(&self) -> String {
    self.lang.borrow().clone()
  }

  #[setter]
  fn lang(&self, #[webidl] value: String) {
    *self.lang.borrow_mut() = value;
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-fontkerning>
  #[getter]
  #[string]
  fn font_kerning(&self) -> &'static str {
    match self.font_state.borrow().font_kerning {
      crate::css::font::FontKerning::Auto => "auto",
      crate::css::font::FontKerning::Normal => "normal",
      crate::css::font::FontKerning::None => "none",
    }
  }

  #[setter]
  fn font_kerning(&self, #[webidl] value: String) {
    let k = match value.as_str() {
      "auto" => crate::css::font::FontKerning::Auto,
      "normal" => crate::css::font::FontKerning::Normal,
      "none" => crate::css::font::FontKerning::None,
      _ => return,
    };
    self.font_state.borrow_mut().font_kerning = k;
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-fontstretch>
  #[getter]
  #[string]
  fn font_stretch(&self) -> &'static str {
    match self.font_state.borrow().stretch {
      cosmic_text::Stretch::UltraCondensed => "ultra-condensed",
      cosmic_text::Stretch::ExtraCondensed => "extra-condensed",
      cosmic_text::Stretch::Condensed => "condensed",
      cosmic_text::Stretch::SemiCondensed => "semi-condensed",
      cosmic_text::Stretch::Normal => "normal",
      cosmic_text::Stretch::SemiExpanded => "semi-expanded",
      cosmic_text::Stretch::Expanded => "expanded",
      cosmic_text::Stretch::ExtraExpanded => "extra-expanded",
      cosmic_text::Stretch::UltraExpanded => "ultra-expanded",
    }
  }

  #[setter]
  fn font_stretch(&self, #[webidl] value: String) {
    if let Some(s) = crate::css::font::parse_css_stretch(&value) {
      self.font_state.borrow_mut().stretch = s;
    }
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-fontvariantcaps>
  #[getter]
  #[string]
  fn font_variant_caps(&self) -> &'static str {
    match self.font_state.borrow().font_variant_caps {
      crate::css::font::FontVariantCaps::Normal => "normal",
      crate::css::font::FontVariantCaps::SmallCaps => "small-caps",
      crate::css::font::FontVariantCaps::AllSmallCaps => "all-small-caps",
      crate::css::font::FontVariantCaps::PetiteCaps => "petite-caps",
      crate::css::font::FontVariantCaps::AllPetiteCaps => "all-petite-caps",
      crate::css::font::FontVariantCaps::Unicase => "unicase",
      crate::css::font::FontVariantCaps::TitlingCaps => "titling-caps",
    }
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
    self.font_state.borrow_mut().font_variant_caps = v;
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-letterspacing>
  #[getter]
  #[string]
  fn letter_spacing(&self) -> String {
    self.font_state.borrow().letter_spacing.to_css_string()
  }

  #[setter]
  fn letter_spacing(&self, #[webidl] value: String) {
    if let Some(spacing) = parse_css_spacing(&value) {
      self.font_state.borrow_mut().letter_spacing = spacing;
    }
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-wordspacing>
  #[getter]
  #[string]
  fn word_spacing(&self) -> String {
    self.font_state.borrow().word_spacing.to_css_string()
  }

  #[setter]
  fn word_spacing(&self, #[webidl] value: String) {
    if let Some(spacing) = parse_css_spacing(&value) {
      self.font_state.borrow_mut().word_spacing = spacing;
    }
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-textrendering>
  #[getter]
  #[string]
  fn text_rendering(&self) -> &'static str {
    match self.font_state.borrow().text_rendering {
      crate::css::font::TextRendering::Auto => "auto",
      crate::css::font::TextRendering::OptimizeSpeed => "optimizeSpeed",
      crate::css::font::TextRendering::OptimizeLegibility => {
        "optimizeLegibility"
      }
      crate::css::font::TextRendering::GeometricPrecision => {
        "geometricPrecision"
      }
    }
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
    self.font_state.borrow_mut().text_rendering = r;
  }

  #[fast]
  fn fill_rect(&self, x: f64, y: f64, w: f64, h: f64) {
    if w == 0.0 || h == 0.0 {
      return;
    }
    let [r, g, b, a] = self.fill_color.get();
    let alpha =
      (a as f32 / 255.0 * self.global_alpha.get() * 255.0).round() as u8;
    let color = peniko::Color::from_rgba8(r, g, b, alpha);
    let rect = kurbo::Rect::new(x, y, x + w, y + h);
    match &mut *self.drawing.borrow_mut() {
      DrawingBackend::Vello(scene) => {
        scene.fill(
          peniko::Fill::NonZero,
          kurbo::Affine::IDENTITY,
          color,
          None,
          &rect,
        );
      }
      DrawingBackend::VelloCpu(ctx, _) => {
        ctx.set_paint(color);
        ctx.fill_rect(&rect);
      }
    }
  }

  #[fast]
  fn clear_rect(&self, x: f64, y: f64, w: f64, h: f64) {
    if w == 0.0 || h == 0.0 {
      return;
    }
    // When alpha=false, clearing restores to the opaque black background.
    let clear_color = if self.settings.alpha {
      peniko::Color::TRANSPARENT
    } else {
      peniko::Color::from_rgb8(0, 0, 0)
    };
    let rect = kurbo::Rect::new(x, y, x + w, y + h);
    match &mut *self.drawing.borrow_mut() {
      DrawingBackend::Vello(scene) => {
        scene.fill(
          peniko::Fill::NonZero,
          kurbo::Affine::IDENTITY,
          clear_color,
          None,
          &rect,
        );
      }
      DrawingBackend::VelloCpu(ctx, _) => {
        ctx.set_paint(clear_color);
        ctx.fill_rect(&rect);
      }
    }
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-filltext>
  #[required(3)]
  fn fill_text(
    &self,
    #[webidl] text: String,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] max_width: Option<UnrestrictedDouble>,
  ) {
    self.draw_text(&text, *x, *y, max_width.map(|v| *v));
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-stroketext>
  #[required(3)]
  fn stroke_text(
    &self,
    #[webidl] text: String,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] max_width: Option<UnrestrictedDouble>,
  ) {
    // For now, stroke text uses the same rendering path as fill.
    self.draw_text(&text, *x, *y, max_width.map(|v| *v));
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-measuretext>
  #[cppgc]
  fn measure_text(&self, #[string] text: &str) -> TextMetrics {
    compute_text_metrics(
      text,
      &self.font_state.borrow(),
      self.text_align.get(),
      &self.font_system,
    )
  }

  // TODO(petamoriken): the following accessors only store their values; they are
  // not yet honored during rendering.
  #[getter]
  #[string]
  fn global_composite_operation(&self) -> String {
    self.global_composite_operation.borrow().clone()
  }

  #[setter]
  fn global_composite_operation(&self, #[webidl] value: String) {
    if matches!(
      value.as_str(),
      "color"
        | "color-burn"
        | "color-dodge"
        | "copy"
        | "darken"
        | "destination-atop"
        | "destination-in"
        | "destination-out"
        | "destination-over"
        | "difference"
        | "exclusion"
        | "hard-light"
        | "hue"
        | "lighten"
        | "lighter"
        | "luminosity"
        | "multiply"
        | "overlay"
        | "saturation"
        | "screen"
        | "soft-light"
        | "source-atop"
        | "source-in"
        | "source-out"
        | "source-over"
        | "xor"
    ) {
      *self.global_composite_operation.borrow_mut() = value;
    }
  }

  #[getter]
  #[string]
  fn filter(&self) -> String {
    self.filter.borrow().clone()
  }

  #[setter]
  fn filter(&self, #[webidl] value: String) {
    *self.filter.borrow_mut() = value;
  }

  #[getter]
  fn image_smoothing_enabled(&self) -> bool {
    self.image_smoothing_enabled.get()
  }

  #[setter]
  fn image_smoothing_enabled(&self, #[webidl] value: bool) {
    self.image_smoothing_enabled.set(value);
  }

  #[getter]
  #[string]
  fn image_smoothing_quality(&self) -> &'static str {
    match self.image_smoothing_quality.get() {
      ImageSmoothingQuality::Low => "low",
      ImageSmoothingQuality::Medium => "medium",
      ImageSmoothingQuality::High => "high",
    }
  }

  #[setter]
  fn image_smoothing_quality(&self, #[webidl] value: String) {
    self.image_smoothing_quality.set(match value.as_str() {
      "low" => ImageSmoothingQuality::Low,
      "medium" => ImageSmoothingQuality::Medium,
      "high" => ImageSmoothingQuality::High,
      _ => return,
    });
  }

  #[getter]
  fn line_width(&self) -> f64 {
    self.line_width.get()
  }

  #[setter]
  fn line_width(&self, #[webidl] value: f64) {
    if value.is_finite() && value > 0.0 {
      self.line_width.set(value);
    }
  }

  #[getter]
  #[string]
  fn line_cap(&self) -> &'static str {
    match self.line_cap.get() {
      LineCap::Butt => "butt",
      LineCap::Round => "round",
      LineCap::Square => "square",
    }
  }

  #[setter]
  fn line_cap(&self, #[webidl] value: String) {
    self.line_cap.set(match value.as_str() {
      "butt" => LineCap::Butt,
      "round" => LineCap::Round,
      "square" => LineCap::Square,
      _ => return,
    });
  }

  #[getter]
  #[string]
  fn line_join(&self) -> &'static str {
    match self.line_join.get() {
      LineJoin::Round => "round",
      LineJoin::Bevel => "bevel",
      LineJoin::Miter => "miter",
    }
  }

  #[setter]
  fn line_join(&self, #[webidl] value: String) {
    self.line_join.set(match value.as_str() {
      "round" => LineJoin::Round,
      "bevel" => LineJoin::Bevel,
      "miter" => LineJoin::Miter,
      _ => return,
    });
  }

  #[getter]
  fn miter_limit(&self) -> f64 {
    self.miter_limit.get()
  }

  #[setter]
  fn miter_limit(&self, #[webidl] value: f64) {
    if value.is_finite() && value > 0.0 {
      self.miter_limit.set(value);
    }
  }

  #[getter]
  fn line_dash_offset(&self) -> f64 {
    self.line_dash_offset.get()
  }

  #[setter]
  fn line_dash_offset(&self, #[webidl] value: f64) {
    if value.is_finite() {
      self.line_dash_offset.set(value);
    }
  }

  #[getter]
  fn shadow_blur(&self) -> f64 {
    self.shadow_blur.get()
  }

  #[setter]
  fn shadow_blur(&self, #[webidl] value: f64) {
    if value.is_finite() && value >= 0.0 {
      self.shadow_blur.set(value);
    }
  }

  #[getter]
  #[string]
  fn shadow_color(&self) -> String {
    self.shadow_color.borrow().clone()
  }

  #[setter]
  fn shadow_color(&self, #[webidl] value: String) {
    if parse_css_color(&value).is_ok() {
      *self.shadow_color.borrow_mut() = value;
    }
  }

  #[getter]
  fn shadow_offset_x(&self) -> f64 {
    self.shadow_offset_x.get()
  }

  #[setter]
  fn shadow_offset_x(&self, #[webidl] value: f64) {
    if value.is_finite() {
      self.shadow_offset_x.set(value);
    }
  }

  #[getter]
  fn shadow_offset_y(&self) -> f64 {
    self.shadow_offset_y.get()
  }

  #[setter]
  fn shadow_offset_y(&self, #[webidl] value: f64) {
    if value.is_finite() {
      self.shadow_offset_y.set(value);
    }
  }

  // TODO(petamoriken): the following methods are not yet implemented and throw a
  // NotSupportedError. Replace each stub body with a real implementation.
  #[fast]
  fn save(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("save"))
  }

  #[fast]
  fn restore(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("restore"))
  }

  #[fast]
  fn reset(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("reset"))
  }

  #[fast]
  fn is_context_lost(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("isContextLost"))
  }

  #[fast]
  fn stroke_rect(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("strokeRect"))
  }

  #[fast]
  fn begin_path(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("beginPath"))
  }

  #[fast]
  fn close_path(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("closePath"))
  }

  #[fast]
  fn move_to(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("moveTo"))
  }

  #[fast]
  fn line_to(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("lineTo"))
  }

  #[fast]
  fn bezier_curve_to(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("bezierCurveTo"))
  }

  #[fast]
  fn quadratic_curve_to(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("quadraticCurveTo"))
  }

  #[fast]
  fn arc(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("arc"))
  }

  #[fast]
  fn arc_to(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("arcTo"))
  }

  #[fast]
  fn ellipse(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("ellipse"))
  }

  #[fast]
  fn rect(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("rect"))
  }

  #[fast]
  fn round_rect(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("roundRect"))
  }

  #[fast]
  fn fill(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("fill"))
  }

  #[fast]
  fn stroke(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("stroke"))
  }

  #[fast]
  fn clip(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("clip"))
  }

  #[fast]
  fn is_point_in_path(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("isPointInPath"))
  }

  #[fast]
  fn is_point_in_stroke(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("isPointInStroke"))
  }

  #[fast]
  fn get_transform(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("getTransform"))
  }

  #[fast]
  fn set_transform(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("setTransform"))
  }

  #[fast]
  fn reset_transform(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("resetTransform"))
  }

  #[fast]
  fn transform(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("transform"))
  }

  #[fast]
  fn scale(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("scale"))
  }

  #[fast]
  fn rotate(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("rotate"))
  }

  #[fast]
  fn translate(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("translate"))
  }

  #[fast]
  fn create_linear_gradient(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("createLinearGradient"))
  }

  #[fast]
  fn create_radial_gradient(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("createRadialGradient"))
  }

  #[fast]
  fn create_conic_gradient(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("createConicGradient"))
  }

  #[fast]
  fn create_pattern(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("createPattern"))
  }

  #[fast]
  fn draw_image(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("drawImage"))
  }

  #[fast]
  fn create_image_data(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("createImageData"))
  }

  #[fast]
  fn get_image_data(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("getImageData"))
  }

  #[fast]
  fn put_image_data(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("putImageData"))
  }

  #[fast]
  fn get_line_dash(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("getLineDash"))
  }

  #[fast]
  fn set_line_dash(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("setLineDash"))
  }
}

impl OffscreenCanvasRenderingContext2D {
  fn draw_text(&self, text: &str, x: f64, y: f64, max_width: Option<f64>) {
    // https://html.spec.whatwg.org/multipage/canvas.html#text-preparation-algorithm
    // Nothing is drawn for non-finite coordinates, or when maxWidth is
    // present but not a positive number.
    if !x.is_finite() || !y.is_finite() {
      return;
    }
    if let Some(max_width) = max_width
      && (max_width.is_nan() || max_width <= 0.0)
    {
      return;
    }
    let fstate = self.font_state.borrow();
    let mut fs = self.font_system.lock().unwrap();

    let metrics = Metrics::new(fstate.size, fstate.size * 1.2);
    let mut buf = Buffer::new(&mut fs, metrics);
    buf.set_size(None, None);

    let attrs = build_text_attrs(&fstate);
    buf.set_text(text, &attrs, Shaping::Advanced, None);
    buf.shape_until_scroll(&mut fs, false);

    let [r, g, b, a] = self.fill_color.get();
    let alpha =
      (a as f32 / 255.0 * self.global_alpha.get() * 255.0).round() as u8;
    let brush = peniko::Color::from_rgba8(r, g, b, alpha);

    let baseline_y = compute_baseline_y(y, &buf, self.text_baseline.get());

    // wordSpacing is not supported by cosmic-text, so the advance of each
    // word separator is widened manually by shifting the following glyphs.
    let word_spacing_px =
      fstate.word_spacing.resolve_to_pixels(fstate.size as f64) as f32;
    let separator_indices = word_separator_indices(text, word_spacing_px);
    let word_offset = |glyph_start: usize| -> f32 {
      word_spacing_px
        * separator_indices
          .iter()
          .take_while(|&&i| i < glyph_start)
          .count() as f32
    };

    // Compute total line width for text-align adjustment.
    let line_width: f32 = buf
      .layout_runs()
      .next()
      .and_then(|run| {
        let last = run.glyphs.last()?;
        Some(last.x + last.w)
      })
      .unwrap_or(0.0)
      + word_spacing_px * separator_indices.len() as f32;

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

    let rtl = fstate.direction == TextDirection::Rtl;
    let x_offset = match self.text_align.get() {
      TextAlign::Left => 0.0,
      TextAlign::Right => -scaled_width,
      TextAlign::Center => -scaled_width / 2.0,
      TextAlign::Start if rtl => -scaled_width,
      TextAlign::Start => 0.0,
      TextAlign::End if rtl => 0.0,
      TextAlign::End => -scaled_width,
    };
    let draw_x = x as f32 + x_offset;

    match &mut *self.drawing.borrow_mut() {
      DrawingBackend::Vello(scene) => {
        for run in buf.layout_runs() {
          if run.glyphs.is_empty() {
            continue;
          }

          // Group consecutive glyphs by font_id and render each group with the
          // correct font. A single LayoutRun may span multiple font faces when
          // fallback fonts are used for individual characters.
          let mut start = 0;
          while start < run.glyphs.len() {
            let font_id = run.glyphs[start].font_id;
            let font_size = run.glyphs[start].font_size;

            // Find the end of this same-font segment.
            let end = run.glyphs[start..]
              .iter()
              .position(|g| g.font_id != font_id)
              .map_or(run.glyphs.len(), |pos| start + pos);

            let Some(font) =
              fs.db().with_face_data(font_id, |data, face_index| {
                let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> =
                  Arc::new(data.to_vec());
                let blob = peniko::Blob::new(bytes);
                peniko::FontData::new(blob, face_index)
              })
            else {
              start = end;
              continue;
            };

            let glyphs = run.glyphs[start..end].iter().map(|g| vello::Glyph {
              id: g.glyph_id as u32,
              x: draw_x + (g.x + g.x_offset + word_offset(g.start)) * x_scale,
              y: baseline_y as f32 + g.y_offset,
            });

            scene
              .draw_glyphs(&font)
              .font_size(font_size)
              .brush(brush)
              .draw(peniko::Fill::NonZero, glyphs);

            start = end;
          }
        }
      }
      DrawingBackend::VelloCpu(ctx, resources) => {
        for run in buf.layout_runs() {
          if run.glyphs.is_empty() {
            continue;
          }

          // Group consecutive glyphs by font_id and render each group with the
          // correct font. A single LayoutRun may span multiple font faces when
          // fallback fonts are used for individual characters.
          let mut start = 0;
          while start < run.glyphs.len() {
            let font_id = run.glyphs[start].font_id;
            let font_size = run.glyphs[start].font_size;

            // Find the end of this same-font segment.
            let end = run.glyphs[start..]
              .iter()
              .position(|g| g.font_id != font_id)
              .map_or(run.glyphs.len(), |pos| start + pos);

            if let Some(font) =
              fs.db().with_face_data(font_id, |data, face_index| {
                let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> =
                  Arc::new(data.to_vec());
                let blob = peniko::Blob::new(bytes);
                peniko::FontData::new(blob, face_index)
              })
            {
              ctx.set_paint(brush);
              ctx
                .glyph_run(resources, &font)
                .font_size(font_size)
                .fill_glyphs(run.glyphs[start..end].iter().map(|g| {
                  vello_cpu::Glyph {
                    id: g.glyph_id as u32,
                    x: draw_x
                      + (g.x + g.x_offset + word_offset(g.start)) * x_scale,
                    y: baseline_y as f32 + g.y_offset,
                  }
                }));
            }

            start = end;
          }
        }
      }
    }
  }

  /// Clears the accumulated scene and updates the canvas dimensions.
  /// Called when OffscreenCanvas.width or .height is changed.
  pub fn resize(&self, width: u32, height: u32) {
    self.width.set(width);
    self.height.set(height);
    self.drawing.borrow_mut().reset(width, height);
  }

  /// Renders the accumulated scene to raw RGBA8 bytes.
  ///
  /// Returns a blank zero-filled buffer when no GPU backend is available.
  pub fn render_to_bytes(&self) -> Result<Vec<u8>, JsErrorBox> {
    let width = self.width.get();
    let height = self.height.get();
    let base_color = if self.settings.alpha {
      peniko::Color::TRANSPARENT
    } else {
      peniko::Color::from_rgb8(0, 0, 0)
    };
    match &mut *self.drawing.borrow_mut() {
      DrawingBackend::Vello(scene) => {
        if let Some(Some(renderer)) = self.renderer.get() {
          render_scene(renderer, scene, width, height, base_color)
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
        // For alpha:false, composite over opaque black: set all alpha channels to 255.
        // (vello_cpu outputs premultiplied RGBA; compositing premul over black keeps RGB and
        // makes alpha 255. The result is already straight-alpha since a=255 means no division.)
        if !self.settings.alpha {
          for pixel in buf.chunks_exact_mut(4) {
            pixel[3] = 255;
          }
        }
        Ok(buf)
      }
    }
  }

  /// Renders the accumulated scene directly to an external TextureView.
  ///
  /// The view must be created from a texture belonging to the same wgpu device
  /// as this context's renderer. Does nothing when no backend is available.
  pub fn render_to_texture_view(
    &self,
    view: &crate::canvas2d_renderer::wgpu::TextureView,
  ) -> Result<(), JsErrorBox> {
    let width = self.width.get();
    let height = self.height.get();
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
      // VelloCpu is never used with UnsafeWindowSurface: getContext("2d") on a
      // surface always calls init_canvas2d_present_state which requires a GPU
      // adapter, so context creation fails before reaching here.
      DrawingBackend::VelloCpu(_, _) => {
        unreachable!("render_to_texture_view called on Cpu backend")
      }
    }
  }

  /// Renders the accumulated scene into a DynamicImage.
  /// Called by ext/canvas when convertToBlob / transferToImageBitmap is invoked.
  pub fn flush_to_image(&self, image: &mut DynamicImage) {
    let width = self.width.get();
    let height = self.height.get();
    let base_color = if self.settings.alpha {
      peniko::Color::TRANSPARENT
    } else {
      peniko::Color::from_rgb8(0, 0, 0)
    };
    match &mut *self.drawing.borrow_mut() {
      DrawingBackend::Vello(scene) => {
        if let Some(Some(renderer)) = self.renderer.get() {
          match render_scene(renderer, scene, width, height, base_color) {
            Ok(pixels) => {
              if let Some(rgba) = RgbaImage::from_raw(width, height, pixels) {
                *image = DynamicImage::ImageRgba8(rgba);
                return;
              }
            }
            Err(e) => {
              log::warn!("canvas2d: render error: {e}");
            }
          }
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
        // For alpha:false, composite over opaque black and convert to straight alpha.
        if !self.settings.alpha {
          for pixel in buf.chunks_exact_mut(4) {
            pixel[3] = 255;
          }
        } else {
          // Convert premultiplied to straight alpha.
          for pixel in buf.chunks_exact_mut(4) {
            let a = pixel[3];
            if a != 0 && a != 255 {
              let inv = 255.0 / a as f32;
              pixel[0] = (pixel[0] as f32 * inv).min(255.0) as u8;
              pixel[1] = (pixel[1] as f32 * inv).min(255.0) as u8;
              pixel[2] = (pixel[2] as f32 * inv).min(255.0) as u8;
            }
          }
        }
        if let Some(rgba) = RgbaImage::from_raw(width, height, buf) {
          *image = DynamicImage::ImageRgba8(rgba);
          return;
        }
      }
    }
    // Fallback: blank image when renderer is unavailable; respect alpha setting.
    let fallback = if self.settings.alpha {
      RgbaImage::new(width, height)
    } else {
      RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]))
    };
    *image = DynamicImage::ImageRgba8(fallback);
  }
}

/// Parses a CSS `<length>` value that uses only `px` units, returning pixels.
/// Returns `None` for invalid or unsupported values.
/// Builds cosmic-text shaping attributes from the current font state.
fn build_text_attrs(fstate: &FontState) -> Attrs<'_> {
  let family = match fstate.families.first().map(|s| s.as_str()) {
    Some("serif") => Family::Serif,
    Some("sans-serif") | None => Family::SansSerif,
    Some("monospace") => Family::Monospace,
    Some("cursive") => Family::Cursive,
    Some("fantasy") => Family::Fantasy,
    Some(name) => Family::Name(name),
  };
  let mut attrs = Attrs::new()
    .family(family)
    .weight(Weight(fstate.weight))
    .style(fstate.style)
    .stretch(fstate.stretch);

  // cosmic-text letter spacing is specified in em units.
  let letter_spacing_px =
    fstate.letter_spacing.resolve_to_pixels(fstate.size as f64) as f32;
  if letter_spacing_px != 0.0 && fstate.size > 0.0 {
    attrs = attrs.letter_spacing(letter_spacing_px / fstate.size);
  }

  if fstate.font_kerning == FontKerning::None {
    let mut features = FontFeatures::new();
    features.disable(FeatureTag::KERNING);
    attrs = attrs.font_features(features);
  }

  attrs
}

/// Returns the byte indices of word separators in `text`, used to apply
/// wordSpacing manually. Returns an empty list when spacing is zero.
/// See <https://html.spec.whatwg.org/multipage/canvas.html#text-preparation-algorithm>
fn word_separator_indices(text: &str, word_spacing_px: f32) -> Vec<usize> {
  if word_spacing_px == 0.0 {
    return Vec::new();
  }
  text
    .char_indices()
    .filter(|(_, c)| {
      matches!(
        c,
        '\u{0020}'
          | '\u{00A0}'
          | '\u{1361}'
          | '\u{10100}'
          | '\u{10101}'
          | '\u{1039F}'
          | '\u{1091F}'
      )
    })
    .map(|(i, _)| i)
    .collect()
}

/// Adjusts the canvas-space y for textBaseline alignment.
fn compute_baseline_y(
  fill_y: f64,
  buf: &Buffer,
  baseline: TextBaseline,
) -> f64 {
  let first_run = buf.layout_runs().next();
  let (ascent, descent) = if let Some(run) = first_run {
    let asc = (run.line_y - run.line_top) as f64;
    let desc = (run.line_top + run.line_height - run.line_y) as f64;
    (asc, desc)
  } else {
    (0.0, 0.0)
  };

  match baseline {
    TextBaseline::Alphabetic => fill_y,
    TextBaseline::Top => fill_y + ascent,
    TextBaseline::Bottom => fill_y - descent,
    TextBaseline::Middle => fill_y + (ascent - descent) / 2.0,
    TextBaseline::Hanging => fill_y + ascent * 0.8,
    TextBaseline::Ideographic => fill_y - descent,
  }
}

fn compute_text_metrics(
  text: &str,
  fstate: &FontState,
  text_align: TextAlign,
  font_system: &Arc<Mutex<cosmic_text::FontSystem>>,
) -> TextMetrics {
  let mut fs = font_system.lock().unwrap();
  let metrics = Metrics::new(fstate.size, fstate.size * 1.2);
  let mut buf = Buffer::new(&mut fs, metrics);
  buf.set_size(None, None);

  let attrs = build_text_attrs(fstate);
  buf.set_text(text, &attrs, Shaping::Advanced, None);
  buf.shape_until_scroll(&mut fs, false);

  let word_spacing_px =
    fstate.word_spacing.resolve_to_pixels(fstate.size as f64) as f32;
  let separator_count = word_separator_indices(text, word_spacing_px).len();

  let mut width = 0.0f64;
  let mut font_bb_ascent = 0.0f64;
  let mut font_bb_descent = 0.0f64;

  for run in buf.layout_runs() {
    width = width.max(run.line_w as f64);
    let ascent = (run.line_y - run.line_top) as f64;
    let descent = (run.line_top + run.line_height - run.line_y) as f64;
    font_bb_ascent = font_bb_ascent.max(ascent);
    font_bb_descent = font_bb_descent.max(descent);
  }
  width += (word_spacing_px as f64) * separator_count as f64;

  let em_ascent = fstate.size as f64 * 0.8;
  let em_descent = fstate.size as f64 * 0.2;

  // actualBoundingBoxLeft/Right are signed distances from the alignment
  // anchor given by textAlign and direction.
  // See <https://html.spec.whatwg.org/multipage/canvas.html#dom-textmetrics-actualboundingboxleft>
  let rtl = fstate.direction == TextDirection::Rtl;
  let anchor = match text_align {
    TextAlign::Left => 0.0,
    TextAlign::Right => width,
    TextAlign::Center => width / 2.0,
    TextAlign::Start if rtl => width,
    TextAlign::Start => 0.0,
    TextAlign::End if rtl => 0.0,
    TextAlign::End => width,
  };

  TextMetrics {
    width,
    actual_bounding_box_left: anchor,
    actual_bounding_box_right: width - anchor,
    font_bounding_box_ascent: font_bb_ascent,
    font_bounding_box_descent: font_bb_descent,
    actual_bounding_box_ascent: font_bb_ascent,
    actual_bounding_box_descent: font_bb_descent,
    em_height_ascent: em_ascent,
    em_height_descent: em_descent,
    hanging_baseline: em_ascent * 0.8,
    alphabetic_baseline: 0.0,
    ideographic_baseline: -em_descent,
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
  _prefix: &'static str,
  _context: &'static str,
) -> Result<v8::Global<v8::Value>, JsErrorBox> {
  let (width, height) = match &data {
    deno_webgpu::canvas::ContextData::Canvas(image) => {
      let d = image.borrow();
      (d.width(), d.height())
    }
    deno_webgpu::canvas::ContextData::Surface(surface) => {
      let d = surface.borrow();
      (d.width, d.height)
    }
  };
  let (renderer, font_system, swash_cache) = {
    let state = state.borrow();
    let renderer = state
      .try_borrow::<SharedRenderer>()
      .ok_or_else(|| JsErrorBox::generic("canvas2d not initialized"))?
      .clone();
    let font_system = state
      .try_borrow::<Arc<Mutex<cosmic_text::FontSystem>>>()
      .ok_or_else(|| JsErrorBox::generic("canvas2d not initialized"))?
      .clone();
    let swash_cache = state
      .try_borrow::<Arc<Mutex<SwashCache>>>()
      .ok_or_else(|| JsErrorBox::generic("canvas2d not initialized"))?
      .clone();
    (renderer, font_system, swash_cache)
  };

  let settings = parse_canvas2d_settings(scope, Some(options));

  let ctx = OffscreenCanvasRenderingContext2D {
    canvas,
    width: Cell::new(width),
    height: Cell::new(height),
    drawing: RefCell::new({
      match renderer.get() {
        Some(Some(backend)) => DrawingBackend::new(backend, width, height),
        _ => DrawingBackend::Vello(vello::Scene::new()),
      }
    }),
    renderer,
    font_system,
    swash_cache,
    fill_color: Cell::new([0, 0, 0, 255]),
    stroke_color: Cell::new([0, 0, 0, 255]),
    global_alpha: Cell::new(1.0),
    font_state: RefCell::new(FontState::default()),
    lang: RefCell::new(String::from("inherit")),
    text_align: Cell::new(TextAlign::default()),
    text_baseline: Cell::new(TextBaseline::default()),
    global_composite_operation: RefCell::new(String::from("source-over")),
    filter: RefCell::new(String::from("none")),
    image_smoothing_enabled: Cell::new(true),
    image_smoothing_quality: Cell::new(ImageSmoothingQuality::default()),
    line_width: Cell::new(1.0),
    line_cap: Cell::new(LineCap::default()),
    line_join: Cell::new(LineJoin::default()),
    miter_limit: Cell::new(10.0),
    line_dash_offset: Cell::new(0.0),
    shadow_blur: Cell::new(0.0),
    shadow_color: RefCell::new(String::from("rgba(0, 0, 0, 0)")),
    shadow_offset_x: Cell::new(0.0),
    shadow_offset_y: Cell::new(0.0),
    settings,
  };

  let obj = deno_core::cppgc::make_cppgc_object(scope, ctx);
  let val: v8::Local<v8::Value> = obj.cast();
  Ok(v8::Global::new(scope, val))
}

fn get_v8_key<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  obj: v8::Local<'s, v8::Object>,
  key: &str,
) -> Option<v8::Local<'s, v8::Value>> {
  let k = v8::String::new(scope, key)?;
  let v = obj.get(scope, k.into())?;
  if v.is_undefined() || v.is_null() {
    None
  } else {
    Some(v)
  }
}

fn parse_canvas2d_settings<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  options: Option<v8::Local<'s, v8::Value>>,
) -> Canvas2DSettings {
  let mut s = Canvas2DSettings::default();
  let Some(options) = options else {
    return s;
  };
  let Ok(obj) = options.try_cast::<v8::Object>() else {
    return s;
  };

  if let Some(v) = get_v8_key(scope, obj, "alpha")
    && v.is_boolean()
  {
    s.alpha = v.boolean_value(scope);
  }
  if let Some(v) = get_v8_key(scope, obj, "desynchronized")
    && v.is_boolean()
  {
    s.desynchronized = v.boolean_value(scope);
  }
  if let Some(v) = get_v8_key(scope, obj, "colorSpace")
    && let Some(str_v) = v.to_string(scope)
  {
    s.color_space = match str_v.to_rust_string_lossy(scope).as_str() {
      "display-p3" => PredefinedColorSpace::DisplayP3,
      _ => PredefinedColorSpace::Srgb,
    };
  }
  if let Some(v) = get_v8_key(scope, obj, "colorType")
    && let Some(str_v) = v.to_string(scope)
  {
    s.color_type = match str_v.to_rust_string_lossy(scope).as_str() {
      "float16" => CanvasColorType::Float16,
      _ => CanvasColorType::Unorm8,
    };
  }
  if let Some(v) = get_v8_key(scope, obj, "willReadFrequently")
    && v.is_boolean()
  {
    s.will_read_frequently = v.boolean_value(scope);
  }

  s
}

/// Placeholder init op (reserved for future initialization).
#[op2(fast)]
pub fn op_canvas2d_init(_state: &mut OpState) {}
