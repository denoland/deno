// Copyright 2018-2026 the Deno authors. MIT license.

pub mod gradient;
pub mod pattern;
pub(crate) mod renderer;

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::WebIDL;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::cppgc::Visitor;
use deno_core::webidl::UnrestrictedDouble;
use deno_core::webidl::WebIdlConverter;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use deno_image::bitmap::ImageBitmap;
use deno_image::image::DynamicImage;
use deno_image::image::GenericImageView;
use deno_image::image::Rgba;
use deno_image::image::RgbaImage;
use parley::FontContext;
use parley::Layout;
use parley::LayoutContext;
use parley::PositionedLayoutItem;
use parley::StyleProperty;
use parley::style::FontFamily;
use parley::style::FontFeature;
use parley::style::FontFeatures;
use parley::style::FontWeight;
use parley::style::GenericFamily;
use vello::kurbo;
use vello::kurbo::Shape;
use vello::peniko;

use self::gradient::build_conic_gradient;
use self::gradient::build_linear_gradient;
use self::gradient::build_radial_gradient;
use self::gradient::parse_color_stop;
use self::gradient::validate_color_stop_offset;
use self::pattern::parse_repetition;
use self::renderer::DenoCanvasBackend;
use self::renderer::SharedRenderer;
use self::renderer::render_scene;
use self::renderer::render_scene_to_texture_view;
use crate::css::color::Color;
use crate::css::color::color_to_css_string;
use crate::css::color::is_color_transparent;
use crate::css::color::parse_css_color;
use crate::css::filter::CssFilterFunction;
use crate::css::filter::FilterValueListParser;
use crate::css::filter::ParserInput as FilterParserInput;
use crate::css::font::FontKerning;
use crate::css::font::FontState;
use crate::css::font::TextDirection;
use crate::css::font::parse_css_font;
use crate::css::font::parse_css_spacing;
use crate::image_data::ImageData;
use crate::text_metrics::TextMetrics;

pub const CONTEXT_ID: &str = "2d";
pub const UNSTABLE_FEATURE_NAME: &str = "canvas2d";

/// RGBA8 pixel buffer extracted from a CanvasImageSource.
pub struct ResolvedCanvasImage {
  pub width: u32,
  pub height: u32,
  pub pixels: Vec<u8>,
}

pub type SyncOffscreenCanvasPixelsFn =
  for<'a> fn(
    scope: &mut v8::PinScope<'a, 'a>,
    image: v8::Local<'a, v8::Value>,
  ) -> Result<(u32, u32, Vec<u8>), JsErrorBox>;

pub struct OffscreenCanvasPixelSync(pub SyncOffscreenCanvasPixelsFn);

pub fn set_offscreen_canvas_pixel_sync(
  state: &mut OpState,
  sync: SyncOffscreenCanvasPixelsFn,
) {
  state.put(OffscreenCanvasPixelSync(sync));
}

/// Resolves an OffscreenCanvas into RGBA8 pixels for Canvas 2D createPattern().
fn resolve_offscreen_canvas_image<'a>(
  state: &OpState,
  scope: &mut v8::PinScope<'a, 'a>,
  image: v8::Local<'a, v8::Value>,
) -> Result<ResolvedCanvasImage, JsErrorBox> {
  let sync = state
    .try_borrow::<OffscreenCanvasPixelSync>()
    .ok_or_else(|| {
      JsErrorBox::new(
        "TypeError",
        "Failed to execute 'createPattern' on 'OffscreenCanvasRenderingContext2D': parameter 1 is not of type 'CanvasImageSource'.",
      )
    })?;
  let (width, height, pixels) = sync.0(scope, image)?;
  Ok(ResolvedCanvasImage {
    width,
    height,
    pixels,
  })
}

/// Resolves an ImageBitmap or OffscreenCanvas into raw RGBA8 pixels.
pub fn resolve_canvas_image_source<'a>(
  state: &OpState,
  scope: &mut v8::PinScope<'a, 'a>,
  image: v8::Local<'a, v8::Value>,
) -> Result<ResolvedCanvasImage, JsErrorBox> {
  if image.is_null_or_undefined() {
    return Err(JsErrorBox::new(
      "TypeError",
      "Failed to execute 'createPattern' on 'OffscreenCanvasRenderingContext2D': parameter 1 is not of type 'CanvasImageSource'.",
    ));
  }

  if let Some(bitmap) =
    deno_core::cppgc::try_unwrap_cppgc_object::<ImageBitmap>(scope, image)
  {
    if bitmap.detached.get().is_some() {
      return Err(JsErrorBox::new(
        "DOMExceptionInvalidStateError",
        "The image source is detached.",
      ));
    }
    let data = bitmap.data.borrow();
    let (width, height) = data.dimensions();
    if width == 0 || height == 0 {
      return Err(JsErrorBox::new(
        "DOMExceptionInvalidStateError",
        "The image source has zero width or height.",
      ));
    }
    return Ok(ResolvedCanvasImage {
      width,
      height,
      pixels: data.as_bytes().to_vec(),
    });
  }

  resolve_offscreen_canvas_image(state, scope, image)
}

/// Builds a peniko ImageData from resolved RGBA8 pixels.
pub fn image_data_from_pixels(
  pixels: Vec<u8>,
  width: u32,
  height: u32,
) -> peniko::ImageData {
  let bytes: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(pixels);
  peniko::ImageData {
    data: peniko::Blob::new(bytes),
    format: peniko::ImageFormat::Rgba8,
    alpha_type: peniko::ImageAlphaType::Alpha,
    width,
    height,
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum Canvas2DError {
  #[class(type)]
  #[error("Illegal constructor")]
  IllegalConstructor,
  #[class("DOMExceptionNotSupportedError")]
  #[error("OffscreenCanvasRenderingContext2D.{0}() is not yet implemented")]
  NotSupported(&'static str),
  #[class("DOMExceptionIndexSizeError")]
  #[error("{0}")]
  IndexSize(String),
  #[class("DOMExceptionInvalidStateError")]
  #[error("{0}")]
  InvalidState(String),
  #[class(type)]
  #[error("The provided value is non-finite.")]
  NonFinite,
  #[class(type)]
  #[error("{0}")]
  TypeMismatch(String),
  #[class(generic)]
  #[error("{0}")]
  Render(#[from] self::renderer::RenderError),
  #[class(generic)]
  #[error("canvas2d not initialized")]
  NotInitialized,
  #[class(inherit)]
  #[error(transparent)]
  ColorStop(#[from] self::gradient::ColorStopError),
  #[class(inherit)]
  #[error(transparent)]
  Pattern(#[from] self::pattern::PatternError),
  #[class(inherit)]
  #[error(transparent)]
  Geometry(#[from] crate::geometry::GeometryError),
  #[class(inherit)]
  #[error(transparent)]
  WebIdl(#[from] deno_core::webidl::WebIdlError),
}

#[derive(Clone)]
enum FillStrokeStyle {
  Color(Color),
  Gradient(v8::Global<v8::Object>),
  Pattern(v8::Global<v8::Object>),
}

// TODO(petamoriken): move to a shared crate when canvas2d and webgpu types need to be unified.
// ext/webgpu/canvas.rs has its own PredefinedColorSpace with additional variants.
#[derive(WebIDL, Default)]
#[webidl(enum)]
enum PredefinedColorSpace {
  #[default]
  #[webidl(rename = "srgb")]
  Srgb,
  // TODO(petamoriken): rendering in display-p3 color space is not yet implemented.
  #[webidl(rename = "display-p3")]
  DisplayP3,
}

// TODO(petamoriken): move to a shared crate when canvas2d and webgpu types need to be unified.
#[derive(WebIDL, Default)]
#[webidl(enum)]
enum CanvasColorType {
  #[default]
  #[webidl(rename = "unorm8")]
  Unorm8,
  // TODO(petamoriken): float16 rendering is not yet implemented.
  #[webidl(rename = "float16")]
  Float16,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
enum TextAlign {
  #[default]
  Start,
  End,
  Left,
  Right,
  Center,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
enum TextBaseline {
  #[default]
  Alphabetic,
  Top,
  Hanging,
  Middle,
  Ideographic,
  Bottom,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
enum ImageSmoothingQuality {
  #[default]
  Low,
  Medium,
  High,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
enum LineCap {
  #[default]
  Butt,
  Round,
  Square,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
enum LineJoin {
  Round,
  Bevel,
  #[default]
  Miter,
}

#[derive(WebIDL, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[webidl(enum)]
enum GlobalCompositeOperation {
  #[default]
  SourceOver,
  SourceIn,
  SourceOut,
  SourceAtop,
  DestinationOver,
  DestinationIn,
  DestinationOut,
  DestinationAtop,
  Lighter,
  Copy,
  Xor,
  Multiply,
  Screen,
  Overlay,
  Darken,
  Lighten,
  ColorDodge,
  ColorBurn,
  HardLight,
  SoftLight,
  Difference,
  Exclusion,
  Hue,
  Saturation,
  Color,
  Luminosity,
}

impl GlobalCompositeOperation {
  fn to_blend_mode(self) -> peniko::BlendMode {
    use peniko::Compose;
    use peniko::Mix;
    match self {
      Self::SourceOver => peniko::BlendMode::default(),
      Self::SourceIn => Compose::SrcIn.into(),
      Self::SourceOut => Compose::SrcOut.into(),
      Self::SourceAtop => Compose::SrcAtop.into(),
      Self::DestinationOver => Compose::DestOver.into(),
      Self::DestinationIn => Compose::DestIn.into(),
      Self::DestinationOut => Compose::DestOut.into(),
      Self::DestinationAtop => Compose::DestAtop.into(),
      Self::Lighter => Compose::PlusLighter.into(),
      Self::Copy => Compose::Copy.into(),
      Self::Xor => Compose::Xor.into(),
      Self::Multiply => Mix::Multiply.into(),
      Self::Screen => Mix::Screen.into(),
      Self::Overlay => Mix::Overlay.into(),
      Self::Darken => Mix::Darken.into(),
      Self::Lighten => Mix::Lighten.into(),
      Self::ColorDodge => Mix::ColorDodge.into(),
      Self::ColorBurn => Mix::ColorBurn.into(),
      Self::HardLight => Mix::HardLight.into(),
      Self::SoftLight => Mix::SoftLight.into(),
      Self::Difference => Mix::Difference.into(),
      Self::Exclusion => Mix::Exclusion.into(),
      Self::Hue => Mix::Hue.into(),
      Self::Saturation => Mix::Saturation.into(),
      Self::Color => Mix::Color.into(),
      Self::Luminosity => Mix::Luminosity.into(),
    }
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
#[allow(
  dead_code,
  reason = "fields are parsed from WebIDL but not all are used yet"
)]
struct Canvas2DSettings {
  #[webidl(default = true)]
  alpha: bool,
  #[webidl(default = false)]
  desynchronized: bool,
  #[webidl(default = PredefinedColorSpace::Srgb)]
  color_space: PredefinedColorSpace,
  #[webidl(default = CanvasColorType::Unorm8)]
  color_type: CanvasColorType,
  #[webidl(default = false)]
  will_read_frequently: bool,
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
enum DrawingBackend {
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

#[derive(Clone)]
struct DrawingState {
  fill_style: FillStrokeStyle,
  stroke_style: FillStrokeStyle,
  global_alpha: f32,
  font_state: FontState,
  text_align: TextAlign,
  text_baseline: TextBaseline,
  lang: String,
  global_composite_operation: GlobalCompositeOperation,
  filter_string: String,
  filter: Vec<CssFilterFunction>,
  image_smoothing_enabled: bool,
  image_smoothing_quality: ImageSmoothingQuality,
  line_width: f64,
  line_cap: LineCap,
  line_join: LineJoin,
  miter_limit: f64,
  line_dash_offset: f64,
  line_dash: Vec<f64>,
  shadow_blur: f64,
  shadow_color: String,
  shadow_color_rgba: Color,
  shadow_offset_x: f64,
  shadow_offset_y: f64,
  transform: kurbo::Affine,
  clip_depth: usize,
}

impl Default for DrawingState {
  fn default() -> Self {
    Self {
      fill_style: FillStrokeStyle::Color(Color::BLACK),
      stroke_style: FillStrokeStyle::Color(Color::BLACK),
      global_alpha: 1.0,
      font_state: FontState::default(),
      text_align: TextAlign::default(),
      text_baseline: TextBaseline::default(),
      lang: String::from("inherit"),
      global_composite_operation: GlobalCompositeOperation::default(),
      filter_string: String::from("none"),
      filter: Vec::new(),
      image_smoothing_enabled: true,
      image_smoothing_quality: ImageSmoothingQuality::default(),
      line_width: 1.0,
      line_cap: LineCap::default(),
      line_join: LineJoin::default(),
      miter_limit: 10.0,
      line_dash_offset: 0.0,
      line_dash: Vec::new(),
      shadow_blur: 0.0,
      shadow_color: String::from("rgba(0, 0, 0, 0)"),
      shadow_color_rgba: Color::TRANSPARENT,
      shadow_offset_x: 0.0,
      shadow_offset_y: 0.0,
      transform: kurbo::Affine::IDENTITY,
      clip_depth: 0,
    }
  }
}

pub struct OffscreenCanvasRenderingContext2D {
  canvas: v8::Global<v8::Object>,
  data: deno_webgpu::canvas::ContextData,

  drawing: RefCell<DrawingBackend>,

  renderer: SharedRenderer,

  font_ctx: Arc<Mutex<FontContext>>,
  layout_ctx: Arc<Mutex<LayoutContext<()>>>,

  state: RefCell<DrawingState>,
  state_stack: RefCell<Vec<DrawingState>>,

  current_path: RefCell<kurbo::BezPath>,

  settings: Canvas2DSettings,
}

// SAFETY: OffscreenCanvasRenderingContext2D is only accessed from the JS thread.
unsafe impl GarbageCollected for OffscreenCanvasRenderingContext2D {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"OffscreenCanvasRenderingContext2D"
  }
}

/// Opaque CanvasGradient object returned by createLinearGradient and friends.
pub struct CanvasGradient {
  gradient: RefCell<peniko::Gradient>,
}

// SAFETY: CanvasGradient is only accessed from the JS thread.
unsafe impl GarbageCollected for CanvasGradient {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"CanvasGradient"
  }
}

#[op2]
impl CanvasGradient {
  #[constructor]
  #[cppgc]
  fn new() -> Result<CanvasGradient, Canvas2DError> {
    Err(Canvas2DError::IllegalConstructor)
  }

  fn add_color_stop(
    &self,
    #[webidl] offset: f64,
    #[webidl] color: String,
  ) -> Result<(), Canvas2DError> {
    let offset = validate_color_stop_offset(offset)?;
    let stop = parse_color_stop(offset, &color)?;
    self.gradient.borrow_mut().stops.push(stop);
    Ok(())
  }
}

/// Opaque CanvasPattern object returned by createPattern.
pub struct CanvasPattern {
  image: peniko::ImageData,
  x_extend: peniko::Extend,
  y_extend: peniko::Extend,
  transform: RefCell<kurbo::Affine>,
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

// Path2D stub (core path drawing on context uses current_path; full Path2D object support pending).
pub struct Path2D {
  path: RefCell<kurbo::BezPath>,
}

// SAFETY: Path2D is only accessed from the JS thread (same as context).
unsafe impl GarbageCollected for Path2D {
  fn trace(&self, _visitor: &mut Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Path2D"
  }
}

#[op2]
impl Path2D {
  #[constructor]
  #[cppgc]
  fn new(
    scope: &mut v8::PinScope<'_, '_>,
    path: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<Path2D, Canvas2DError> {
    let bez = match path {
      Some(v) if v.is_string() => {
        let s = v.to_rust_string_lossy(scope);
        kurbo::BezPath::from_svg(&s).unwrap_or_else(|_| kurbo::BezPath::new())
      }
      Some(v) => {
        if let Some(p) =
          deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v)
        {
          p.path.borrow().clone()
        } else {
          kurbo::BezPath::new()
        }
      }
      None => kurbo::BezPath::new(),
    };
    Ok(Path2D {
      path: RefCell::new(bez),
    })
  }

  // CanvasPath methods (duplicated logic from context for Path2D; can be refactored later)
  #[fast]
  fn close_path(&self) {
    self.path.borrow_mut().close_path();
  }

  fn move_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if x.is_finite() && y.is_finite() {
      self.path.borrow_mut().move_to((*x, *y));
    }
  }

  fn line_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if x.is_finite() && y.is_finite() {
      self.path.borrow_mut().line_to((*x, *y));
    }
  }

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
      self
        .path
        .borrow_mut()
        .curve_to((*cp1x, *cp1y), (*cp2x, *cp2y), (*x, *y));
    }
  }

  fn quadratic_curve_to(
    &self,
    #[webidl] cpx: UnrestrictedDouble,
    #[webidl] cpy: UnrestrictedDouble,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if cpx.is_finite() && cpy.is_finite() && x.is_finite() && y.is_finite() {
      self.path.borrow_mut().quad_to((*cpx, *cpy), (*x, *y));
    }
  }

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
    if *radius < 0.0 {
      return Err(Canvas2DError::IndexSize(format!(
        "Failed to execute 'arc': The radius provided ({}) is negative.",
        *radius
      )));
    }
    if !x.is_finite()
      || !y.is_finite()
      || !radius.is_finite()
      || !start_angle.is_finite()
      || !end_angle.is_finite()
    {
      return Ok(());
    }
    let delta = compute_arc_sweep(*start_angle, *end_angle, counterclockwise);
    let mut path = self.path.borrow_mut();
    let arc = kurbo::Arc {
      center: kurbo::Point::new(*x, *y),
      radii: kurbo::Vec2::new(*radius, *radius),
      start_angle: *start_angle,
      sweep_angle: delta,
      x_rotation: 0.0,
    };
    let start_pt = arc.center
      + kurbo::Vec2::new(
        *radius * start_angle.cos(),
        *radius * start_angle.sin(),
      );
    if path.is_empty() {
      path.move_to(start_pt);
    } else {
      path.line_to(start_pt);
    }
    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
      path.curve_to(p1, p2, p3);
    });
    Ok(())
  }

  fn arc_to(
    &self,
    #[webidl] x1: UnrestrictedDouble,
    #[webidl] y1: UnrestrictedDouble,
    #[webidl] x2: UnrestrictedDouble,
    #[webidl] y2: UnrestrictedDouble,
    #[webidl] radius: UnrestrictedDouble,
  ) -> Result<(), Canvas2DError> {
    if *radius < 0.0 {
      return Err(Canvas2DError::IndexSize(format!(
        "Failed to execute 'arcTo': The radius provided ({}) is negative.",
        *radius
      )));
    }
    if !x1.is_finite()
      || !y1.is_finite()
      || !x2.is_finite()
      || !y2.is_finite()
      || !radius.is_finite()
    {
      return Ok(());
    }
    let mut path = self.path.borrow_mut();
    if path.is_empty() {
      path.move_to((*x1, *y1));
      return Ok(());
    }
    arc_to_impl(&mut path, *x1, *y1, *x2, *y2, *radius);
    Ok(())
  }

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
    if *radius_x < 0.0 {
      return Err(Canvas2DError::IndexSize(format!(
        "Failed to execute 'ellipse': The major-axis radius provided ({}) is negative.",
        *radius_x
      )));
    }
    if *radius_y < 0.0 {
      return Err(Canvas2DError::IndexSize(format!(
        "Failed to execute 'ellipse': The minor-axis radius provided ({}) is negative.",
        *radius_y
      )));
    }
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
    let delta = compute_arc_sweep(*start_angle, *end_angle, counterclockwise);
    let mut path = self.path.borrow_mut();
    let arc = kurbo::Arc {
      center: kurbo::Point::new(*x, *y),
      radii: kurbo::Vec2::new(*radius_x, *radius_y),
      start_angle: *start_angle,
      sweep_angle: delta,
      x_rotation: *rotation,
    };
    let dx = *radius_x * start_angle.cos();
    let dy = *radius_y * start_angle.sin();
    let cos_r = rotation.cos();
    let sin_r = rotation.sin();
    let start_pt = kurbo::Point::new(
      *x + dx * cos_r - dy * sin_r,
      *y + dx * sin_r + dy * cos_r,
    );
    if path.is_empty() {
      path.move_to(start_pt);
    } else {
      path.line_to(start_pt);
    }
    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
      path.curve_to(p1, p2, p3);
    });
    Ok(())
  }

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
    let mut p = self.path.borrow_mut();
    p.move_to((*x, *y));
    p.line_to((*x + *w, *y));
    p.line_to((*x + *w, *y + *h));
    p.line_to((*x, *y + *h));
    p.close_path();
  }

  fn round_rect(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] w: UnrestrictedDouble,
    #[webidl] h: UnrestrictedDouble,
    radii: Option<f64>,
  ) {
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
      return;
    }
    let r = radii.unwrap_or(0.0);
    if r <= 0.0 {
      let mut p = self.path.borrow_mut();
      p.move_to((*x, *y));
      p.line_to((*x + *w, *y));
      p.line_to((*x + *w, *y + *h));
      p.line_to((*x, *y + *h));
      p.close_path();
      return;
    }
    let mut path = self.path.borrow_mut();
    let rr = kurbo::RoundedRect::new(*x, *y, *x + *w, *y + *h, r);
    path.extend(rr.path_elements(0.1));
  }

  #[fast]
  fn add_path(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    other: v8::Local<'_, v8::Value>,
  ) {
    if let Some(p) =
      deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, other)
    {
      let other_path = p.path.borrow();
      self.path.borrow_mut().extend(other_path.iter());
    }
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

  #[setter]
  fn fill_style<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
    value: v8::Local<'a, v8::Value>,
  ) {
    if let Some(style) = Self::parse_fill_stroke_style(scope, value) {
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

  #[setter]
  fn stroke_style<'a>(
    &self,
    scope: &mut v8::PinScope<'a, 'a>,
    value: v8::Local<'a, v8::Value>,
  ) {
    if let Some(style) = Self::parse_fill_stroke_style(scope, value) {
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
    let shadow = Self::has_shadow(&state);
    let shadow_brush = if shadow {
      Some(Self::shadow_brush(&state))
    } else {
      None
    };
    let shadow_xform = if shadow {
      Some(Self::shadow_transform(&state, state.transform))
    } else {
      None
    };
    let (brush, brush_transform) =
      self.resolve_brush(scope, &state.fill_style, 1.0, state.transform);
    let transform = state.transform;
    drop(state);
    let rect = kurbo::Rect::new(*x, *y, *x + *w, *y + *h);
    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      Self::push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let (Some(sb), Some(st)) = (shadow_brush, shadow_xform) {
      Self::fill_on(&mut drawing, &rect, peniko::Fill::NonZero, st, sb, None);
    }
    Self::fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      transform,
      brush,
      brush_transform,
    );
    if has_layer {
      Self::pop_compositing_layer(&mut drawing);
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
    let transform = self.state.borrow().transform;
    let rect = kurbo::Rect::new(x, y, x + w, y + h);
    match &mut *self.drawing.borrow_mut() {
      DrawingBackend::Vello(scene) => {
        scene.fill(peniko::Fill::NonZero, transform, clear_color, None, &rect);
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
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] text: String,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] max_width: Option<UnrestrictedDouble>,
  ) {
    self.draw_text(scope, &text, *x, *y, max_width.map(|v| *v), false);
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-stroketext>
  #[required(3)]
  fn stroke_text(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] text: String,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] max_width: Option<UnrestrictedDouble>,
  ) {
    self.draw_text(scope, &text, *x, *y, max_width.map(|v| *v), true);
  }

  /// See <https://html.spec.whatwg.org/multipage/canvas.html#dom-context-2d-measuretext>
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
  #[string]
  fn filter(&self) -> String {
    self.state.borrow().filter_string.clone()
  }

  #[setter]
  fn filter(&self, #[webidl] value: String) {
    let functions = {
      let mut parser_input = FilterParserInput::new(&value);
      let result: Result<Vec<_>, _> =
        FilterValueListParser::new(&mut parser_input).collect();
      result.ok()
    };
    if let Some(functions) = functions {
      let mut state = self.state.borrow_mut();
      state.filter_string = value;
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
      state.shadow_color = value;
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
  fn save(&self) {
    self
      .state_stack
      .borrow_mut()
      .push(self.state.borrow().clone());
  }

  #[fast]
  fn restore(&self) {
    if let Some(saved) = self.state_stack.borrow_mut().pop() {
      *self.state.borrow_mut() = saved;
    }
  }

  #[fast]
  fn reset(&self) {
    *self.state.borrow_mut() = DrawingState::default();
    self.state_stack.borrow_mut().clear();
    self.current_path.borrow_mut().truncate(0);
    // TODO(petamoriken): pop clip layers on backend for full clip support
    let (width, height) = self.data.dimensions();
    self.drawing.borrow_mut().reset(width, height);
  }

  #[fast]
  fn is_context_lost(&self) -> bool {
    false
  }

  #[fast]
  fn begin_path(&self) {
    self.current_path.borrow_mut().truncate(0);
  }

  #[fast]
  fn close_path(&self) {
    self.current_path.borrow_mut().close_path();
  }

  fn move_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if x.is_finite() && y.is_finite() {
      self.current_path.borrow_mut().move_to((*x, *y));
    }
  }

  fn line_to(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if x.is_finite() && y.is_finite() {
      self.current_path.borrow_mut().line_to((*x, *y));
    }
  }

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
      self.current_path.borrow_mut().curve_to(
        (*cp1x, *cp1y),
        (*cp2x, *cp2y),
        (*x, *y),
      );
    }
  }

  fn quadratic_curve_to(
    &self,
    #[webidl] cpx: UnrestrictedDouble,
    #[webidl] cpy: UnrestrictedDouble,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if cpx.is_finite() && cpy.is_finite() && x.is_finite() && y.is_finite() {
      self
        .current_path
        .borrow_mut()
        .quad_to((*cpx, *cpy), (*x, *y));
    }
  }

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
    if *radius < 0.0 {
      return Err(Canvas2DError::IndexSize(format!(
        "Failed to execute 'arc': The radius provided ({}) is negative.",
        *radius
      )));
    }
    if !x.is_finite()
      || !y.is_finite()
      || !radius.is_finite()
      || !start_angle.is_finite()
      || !end_angle.is_finite()
    {
      return Ok(());
    }

    let delta = compute_arc_sweep(*start_angle, *end_angle, counterclockwise);

    let mut path = self.current_path.borrow_mut();
    let arc = kurbo::Arc {
      center: kurbo::Point::new(*x, *y),
      radii: kurbo::Vec2::new(*radius, *radius),
      start_angle: *start_angle,
      sweep_angle: delta,
      x_rotation: 0.0,
    };

    let start_pt = arc.center
      + kurbo::Vec2::new(
        *radius * start_angle.cos(),
        *radius * start_angle.sin(),
      );
    if path.is_empty() {
      path.move_to(start_pt);
    } else {
      path.line_to(start_pt);
    }
    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
      path.curve_to(p1, p2, p3);
    });
    Ok(())
  }

  fn arc_to(
    &self,
    #[webidl] x1: UnrestrictedDouble,
    #[webidl] y1: UnrestrictedDouble,
    #[webidl] x2: UnrestrictedDouble,
    #[webidl] y2: UnrestrictedDouble,
    #[webidl] radius: UnrestrictedDouble,
  ) -> Result<(), Canvas2DError> {
    if *radius < 0.0 {
      return Err(Canvas2DError::IndexSize(format!(
        "Failed to execute 'arcTo': The radius provided ({}) is negative.",
        *radius
      )));
    }
    if !x1.is_finite()
      || !y1.is_finite()
      || !x2.is_finite()
      || !y2.is_finite()
      || !radius.is_finite()
    {
      return Ok(());
    }
    let mut path = self.current_path.borrow_mut();
    if path.is_empty() {
      path.move_to((*x1, *y1));
      return Ok(());
    }
    arc_to_impl(&mut path, *x1, *y1, *x2, *y2, *radius);
    Ok(())
  }

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
    if *radius_x < 0.0 {
      return Err(Canvas2DError::IndexSize(format!(
        "Failed to execute 'ellipse': The major-axis radius provided ({}) is negative.",
        *radius_x
      )));
    }
    if *radius_y < 0.0 {
      return Err(Canvas2DError::IndexSize(format!(
        "Failed to execute 'ellipse': The minor-axis radius provided ({}) is negative.",
        *radius_y
      )));
    }
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

    let delta = compute_arc_sweep(*start_angle, *end_angle, counterclockwise);

    let mut path = self.current_path.borrow_mut();
    let arc = kurbo::Arc {
      center: kurbo::Point::new(*x, *y),
      radii: kurbo::Vec2::new(*radius_x, *radius_y),
      start_angle: *start_angle,
      sweep_angle: delta,
      x_rotation: *rotation,
    };

    let dx = *radius_x * start_angle.cos();
    let dy = *radius_y * start_angle.sin();
    let cos_r = rotation.cos();
    let sin_r = rotation.sin();
    let start_pt = kurbo::Point::new(
      *x + dx * cos_r - dy * sin_r,
      *y + dx * sin_r + dy * cos_r,
    );
    if path.is_empty() {
      path.move_to(start_pt);
    } else {
      path.line_to(start_pt);
    }
    arc.to_cubic_beziers(0.1, |p1, p2, p3| {
      path.curve_to(p1, p2, p3);
    });
    Ok(())
  }

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
    let mut path = self.current_path.borrow_mut();
    path.move_to((*x, *y));
    path.line_to((*x + *w, *y));
    path.line_to((*x + *w, *y + *h));
    path.line_to((*x, *y + *h));
    path.close_path();
  }

  fn round_rect(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
    #[webidl] w: UnrestrictedDouble,
    #[webidl] h: UnrestrictedDouble,
    radii: Option<f64>,
  ) {
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || !h.is_finite() {
      return;
    }
    let r = radii.unwrap_or(0.0);
    if r <= 0.0 {
      let mut p = self.current_path.borrow_mut();
      p.move_to((*x, *y));
      p.line_to((*x + *w, *y));
      p.line_to((*x + *w, *y + *h));
      p.line_to((*x, *y + *h));
      p.close_path();
      return;
    }
    let mut path = self.current_path.borrow_mut();
    let rr = kurbo::RoundedRect::new(*x, *y, *x + *w, *y + *h, r);
    path.extend(rr.path_elements(0.1));
  }

  fn stroke_rect(
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
    let rect = kurbo::Rect::new(*x, *y, *x + *w, *y + *h);
    self.stroke_shape(scope, &rect);
  }

  fn fill(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    first: Option<v8::Local<'_, v8::Value>>,
    #[string] second: Option<String>,
  ) {
    let (path, rule) = self.resolve_path_and_fill_rule(scope, first, second);
    if path.is_empty() {
      return;
    }
    self.draw_path_fill(scope, path, rule);
  }

  #[fast]
  fn stroke(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path: Option<v8::Local<'_, v8::Value>>,
  ) {
    let path = self.resolve_optional_path(scope, path);
    if path.is_empty() {
      return;
    }
    self.draw_path_stroke(scope, path);
  }

  fn clip(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    first: Option<v8::Local<'_, v8::Value>>,
    #[string] second: Option<String>,
  ) {
    let (path, rule) = self.resolve_path_and_fill_rule(scope, first, second);
    if path.is_empty() {
      return;
    }
    self.apply_clip(path, rule);
  }

  fn is_point_in_path(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    a: Option<v8::Local<'_, v8::Value>>,
    b: Option<v8::Local<'_, v8::Value>>,
    c: Option<v8::Local<'_, v8::Value>>,
    #[string] d: Option<String>,
  ) -> Result<bool, Canvas2DError> {
    let (path, x, y, rule) =
      self.resolve_point_in_path_args(scope, a, b, c, d)?;
    if !x.is_finite() || !y.is_finite() {
      return Ok(false);
    }
    let transform = self.state.borrow().transform;
    let p = transform.inverse() * kurbo::Point::new(x, y);
    Ok(self.test_point_in_path(path, p.x, p.y, rule))
  }

  #[fast]
  fn is_point_in_stroke(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    a: Option<v8::Local<'_, v8::Value>>,
    b: Option<v8::Local<'_, v8::Value>>,
    c: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<bool, Canvas2DError> {
    let (path, x, y) = self.resolve_point_in_stroke_args(scope, a, b, c)?;
    if !x.is_finite() || !y.is_finite() {
      return Ok(false);
    }
    let transform = self.state.borrow().transform;
    let p = transform.inverse() * kurbo::Point::new(x, y);
    Ok(self.test_point_in_stroke(path, p.x, p.y))
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

  fn set_transform<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    a_or_init: Option<v8::Local<'s, v8::Value>>,
    #[webidl] b: Option<UnrestrictedDouble>,
    #[webidl] c: Option<UnrestrictedDouble>,
    #[webidl] d: Option<UnrestrictedDouble>,
    #[webidl] e: Option<UnrestrictedDouble>,
    #[webidl] f_val: Option<UnrestrictedDouble>,
  ) -> Result<(), Canvas2DError> {
    let (a, b, c, d, e, f) = match a_or_init {
      Some(v) if v.is_number() => {
        let a = v.number_value(scope).unwrap_or(f64::NAN);
        (
          a,
          b.map(|x| *x).unwrap_or(0.0),
          c.map(|x| *x).unwrap_or(0.0),
          d.map(|x| *x).unwrap_or(0.0),
          e.map(|x| *x).unwrap_or(0.0),
          f_val.map(|x| *x).unwrap_or(0.0),
        )
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
    self.state.borrow_mut().transform = kurbo::Affine::new([a, b, c, d, e, f]);
    Ok(())
  }

  #[fast]
  fn reset_transform(&self) {
    self.state.borrow_mut().transform = kurbo::Affine::IDENTITY;
  }

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
    let m = kurbo::Affine::new([*a, *b, *c, *d, *e, *f]);
    let mut state = self.state.borrow_mut();
    state.transform *= m;
  }

  fn scale(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() {
      return;
    }
    let mut state = self.state.borrow_mut();
    state.transform *= kurbo::Affine::scale_non_uniform(*x, *y);
  }

  fn rotate(&self, #[webidl] angle: UnrestrictedDouble) {
    if !angle.is_finite() {
      return;
    }
    let mut state = self.state.borrow_mut();
    state.transform *= kurbo::Affine::rotate(*angle);
  }

  fn translate(
    &self,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) {
    if !x.is_finite() || !y.is_finite() {
      return;
    }
    let mut state = self.state.borrow_mut();
    state.transform *= kurbo::Affine::translate((*x, *y));
  }

  #[required(4)]
  #[cppgc]
  fn create_linear_gradient(
    &self,
    _scope: &mut v8::PinScope<'_, '_>,
    #[webidl] x0: UnrestrictedDouble,
    #[webidl] y0: UnrestrictedDouble,
    #[webidl] x1: UnrestrictedDouble,
    #[webidl] y1: UnrestrictedDouble,
  ) -> Result<CanvasGradient, Canvas2DError> {
    Self::require_finite(&[x0, y0, x1, y1])?;
    let gradient = build_linear_gradient(*x0, *y0, *x1, *y1);
    Ok(CanvasGradient {
      gradient: RefCell::new(gradient),
    })
  }

  #[required(6)]
  #[cppgc]
  fn create_radial_gradient(
    &self,
    _scope: &mut v8::PinScope<'_, '_>,
    #[webidl] x0: UnrestrictedDouble,
    #[webidl] y0: UnrestrictedDouble,
    #[webidl] r0: UnrestrictedDouble,
    #[webidl] x1: UnrestrictedDouble,
    #[webidl] y1: UnrestrictedDouble,
    #[webidl] r1: UnrestrictedDouble,
  ) -> Result<CanvasGradient, Canvas2DError> {
    Self::require_finite(&[x0, y0, r0, x1, y1, r1])?;
    let gradient = build_radial_gradient(*x0, *y0, *r0, *x1, *y1, *r1);
    Ok(CanvasGradient {
      gradient: RefCell::new(gradient),
    })
  }

  #[required(3)]
  #[cppgc]
  fn create_conic_gradient(
    &self,
    _scope: &mut v8::PinScope<'_, '_>,
    #[webidl] start_angle: UnrestrictedDouble,
    #[webidl] x: UnrestrictedDouble,
    #[webidl] y: UnrestrictedDouble,
  ) -> Result<CanvasGradient, Canvas2DError> {
    Self::require_finite(&[start_angle, x, y])?;
    let gradient = build_conic_gradient(*start_angle, *x, *y);
    Ok(CanvasGradient {
      gradient: RefCell::new(gradient),
    })
  }

  #[cppgc]
  fn create_pattern<'a>(
    &self,
    state: &OpState,
    scope: &mut v8::PinScope<'a, 'a>,
    #[varargs] args: Option<&v8::FunctionCallbackArguments<'a>>,
  ) -> Result<CanvasPattern, Canvas2DError> {
    let num_args = args.map(|a| a.length()).unwrap_or(0);
    if num_args < 1 {
      return Err(Canvas2DError::TypeMismatch(
        "Failed to execute 'createPattern' on 'OffscreenCanvasRenderingContext2D': 1 argument required, but only 0 present.".to_string(),
      ));
    }
    let args = args.expect("checked above");
    let image = args.get(0);
    let repetition = match num_args {
      1 => String::new(),
      _ => {
        let rep = args.get(1);
        if rep.is_undefined() {
          return Err(self::pattern::PatternError::Syntax.into());
        }
        if rep.is_null() {
          String::new()
        } else {
          rep.to_rust_string_lossy(scope)
        }
      }
    };
    let repetition = parse_repetition(&repetition)?;

    let resolved =
      resolve_canvas_image_source(state, scope, image).map_err(|e| match e
        .get_class()
        .as_ref()
      {
        "TypeError" => Canvas2DError::TypeMismatch(e.get_message().to_string()),
        "DOMExceptionInvalidStateError" => {
          Canvas2DError::InvalidState(e.get_message().to_string())
        }
        _ => Canvas2DError::TypeMismatch(e.get_message().to_string()),
      })?;

    let image_data =
      image_data_from_pixels(resolved.pixels, resolved.width, resolved.height);

    Ok(CanvasPattern {
      image: image_data,
      x_extend: repetition.x_extend,
      y_extend: repetition.y_extend,
      transform: RefCell::new(kurbo::Affine::IDENTITY),
    })
  }

  #[fast]
  fn draw_image(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("drawImage"))
  }

  #[fast]
  fn create_image_data(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("createImageData"))
  }

  #[cppgc]
  fn get_image_data(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    #[webidl] sx: i32,
    #[webidl] sy: i32,
    #[webidl] sw: i32,
    #[webidl] sh: i32,
  ) -> Result<ImageData, JsErrorBox> {
    if sw == 0 || sh == 0 {
      return Err(JsErrorBox::from_err(Canvas2DError::IndexSize(
        "The source width or height is zero.".to_string(),
      )));
    }

    let full = self.render_to_bytes().map_err(JsErrorBox::from_err)?;
    let (canvas_w, canvas_h) = self.data.dimensions();

    let (sx, sw) = if sw < 0 { (sx + sw, -sw) } else { (sx, sw) };
    let (sy, sh) = if sh < 0 { (sy + sh, -sh) } else { (sy, sh) };
    let out_w = sw as u32;
    let out_h = sh as u32;

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

    let img = ImageData::new_rgba_unorm8(scope, out_w, out_h, &sub)
      .map_err(JsErrorBox::from_err)?;

    Ok(img)
  }

  #[fast]
  fn put_image_data(&self) -> Result<(), Canvas2DError> {
    Err(Canvas2DError::NotSupported("putImageData"))
  }

  fn get_line_dash(&self) -> Vec<f64> {
    self.state.borrow().line_dash.clone()
  }

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

#[allow(dead_code, reason = "text drawing helpers used by fillText/strokeText")]
impl OffscreenCanvasRenderingContext2D {
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
    let fstate = self.state.borrow().font_state.clone();
    let mut fc = self.font_ctx.lock().unwrap();
    let mut lc = self.layout_ctx.lock().unwrap();
    let layout = build_text_layout(&mut fc, &mut lc, text, &fstate);

    let state = self.state.borrow();
    let style = if stroke {
      &state.stroke_style
    } else {
      &state.fill_style
    };
    let op = state.global_composite_operation;
    let global_alpha = state.global_alpha;
    // TODO(petamoriken): apply text shadow once Vello GPU supports filter effects
    let (brush, brush_transform) =
      self.resolve_brush(scope, style, 1.0, state.transform);
    let text_align = state.text_align;
    let text_baseline = state.text_baseline;
    let transform = state.transform;
    drop(state);

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

    let rtl = fstate.direction == TextDirection::Rtl;
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
    let has_layer = Self::push_compositing_layer(
      &mut drawing,
      op,
      global_alpha,
      canvas_w,
      canvas_h,
    );
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

            Self::apply_cpu_paint(ctx, brush.clone(), brush_transform);
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
      Self::pop_compositing_layer(&mut drawing);
    }
  }

  /// Clears the accumulated scene and updates the canvas dimensions.
  /// Called when OffscreenCanvas.width or .height is changed.
  pub fn resize(&self) {
    let (width, height) = self.data.dimensions();
    self.drawing.borrow_mut().reset(width, height);
  }

  /// Renders the accumulated scene to raw RGBA8 bytes.
  ///
  /// Returns a blank zero-filled buffer when no GPU backend is available.
  pub fn render_to_bytes(&self) -> Result<Vec<u8>, Canvas2DError> {
    let (width, height) = self.data.dimensions();
    let base_color = if self.settings.alpha {
      peniko::Color::TRANSPARENT
    } else {
      peniko::Color::from_rgb8(0, 0, 0)
    };
    match &mut *self.drawing.borrow_mut() {
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
    }
  }

  /// Renders the accumulated scene directly to an external TextureView.
  ///
  /// The view must be created from a texture belonging to the same wgpu device
  /// as this context's renderer. Does nothing when no backend is available.
  pub fn render_to_texture_view(
    &self,
    view: &self::renderer::wgpu::TextureView,
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
    let (width, height) = image.dimensions();
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

fn build_text_layout(
  font_ctx: &mut FontContext,
  layout_ctx: &mut LayoutContext<()>,
  text: &str,
  fstate: &FontState,
) -> Layout<()> {
  use std::borrow::Cow;

  use parley::style::FontFamilyName;

  let mut builder = layout_ctx.ranged_builder(font_ctx, text, 1.0, true);

  let family: FontFamily<'_> = match fstate.families.first().map(|s| s.as_str())
  {
    Some("serif") => GenericFamily::Serif.into(),
    Some("sans-serif") | None => GenericFamily::SansSerif.into(),
    Some("monospace") => GenericFamily::Monospace.into(),
    Some("cursive") => GenericFamily::Cursive.into(),
    Some("fantasy") => GenericFamily::Fantasy.into(),
    Some(name) => {
      FontFamily::Single(FontFamilyName::Named(Cow::Borrowed(name)))
    }
  };
  builder.push_default(StyleProperty::FontFamily(family));
  builder.push_default(StyleProperty::FontSize(fstate.size));
  builder.push_default(StyleProperty::FontWeight(FontWeight::new(
    fstate.weight as f32,
  )));
  builder.push_default(StyleProperty::FontStyle(fstate.style.to_parley()));
  builder.push_default(StyleProperty::FontWidth(fstate.stretch.to_parley()));

  let letter_spacing_px =
    fstate.letter_spacing.resolve_to_pixels(fstate.size as f64) as f32;
  if letter_spacing_px != 0.0 {
    builder.push_default(StyleProperty::LetterSpacing(letter_spacing_px));
  }

  let word_spacing_px =
    fstate.word_spacing.resolve_to_pixels(fstate.size as f64) as f32;
  if word_spacing_px != 0.0 {
    builder.push_default(StyleProperty::WordSpacing(word_spacing_px));
  }

  if fstate.font_kerning == FontKerning::None {
    let kern_off = FontFeature::new(parley::setting::Tag::new(b"kern"), 0);
    builder.push_default(StyleProperty::FontFeatures(FontFeatures::List(
      Cow::Owned(vec![kern_off]),
    )));
  }

  let mut layout = builder.build(text);
  layout.break_all_lines(None);
  layout.align(
    parley::Alignment::Start,
    parley::AlignmentOptions::default(),
  );
  layout
}

/// Adjusts the canvas-space y for textBaseline alignment.
fn compute_baseline_y(
  fill_y: f64,
  layout: &Layout<()>,
  baseline: TextBaseline,
) -> f64 {
  let (ascent, descent) = if let Some(line) = layout.lines().next() {
    let m = line.metrics();
    (m.ascent as f64, m.descent as f64)
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
  font_ctx: &Arc<Mutex<FontContext>>,
  layout_ctx: &Arc<Mutex<LayoutContext<()>>>,
) -> TextMetrics {
  let mut fc = font_ctx.lock().unwrap();
  let mut lc = layout_ctx.lock().unwrap();
  let layout = build_text_layout(&mut fc, &mut lc, text, fstate);

  let mut width = 0.0f64;
  let mut font_bb_ascent = 0.0f64;
  let mut font_bb_descent = 0.0f64;

  for line in layout.lines() {
    let m = line.metrics();
    width = width.max((m.advance - m.trailing_whitespace) as f64);
    font_bb_ascent = font_bb_ascent.max(m.ascent as f64);
    font_bb_descent = font_bb_descent.max(m.descent as f64);
  }

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
  prefix: &'static str,
  context: &'static str,
) -> Result<v8::Global<v8::Value>, JsErrorBox> {
  let (width, height) = data.dimensions();
  let (renderer, font_ctx, layout_ctx) = {
    let state = state.borrow();
    let renderer = state
      .try_borrow::<SharedRenderer>()
      .ok_or_else(|| JsErrorBox::from_err(Canvas2DError::NotInitialized))?
      .clone();
    let font_ctx = state
      .try_borrow::<Arc<Mutex<FontContext>>>()
      .ok_or_else(|| JsErrorBox::from_err(Canvas2DError::NotInitialized))?
      .clone();
    let layout_ctx = state
      .try_borrow::<Arc<Mutex<LayoutContext<()>>>>()
      .ok_or_else(|| JsErrorBox::from_err(Canvas2DError::NotInitialized))?
      .clone();
    (renderer, font_ctx, layout_ctx)
  };

  let settings = Canvas2DSettings::convert(
    scope,
    options,
    prefix.into(),
    (|| context.into()).into(),
    &(),
  )
  .map_err(JsErrorBox::from_err)?;

  let ctx = OffscreenCanvasRenderingContext2D {
    canvas,
    data,
    drawing: RefCell::new({
      match renderer.get() {
        Some(Some(backend)) => DrawingBackend::new(backend, width, height),
        _ => DrawingBackend::Vello(vello::Scene::new()),
      }
    }),
    renderer,
    font_ctx,
    layout_ctx,
    state: RefCell::new(DrawingState::default()),
    state_stack: RefCell::new(Vec::new()),
    current_path: RefCell::new(kurbo::BezPath::new()),
    settings,
  };

  let obj = deno_core::cppgc::make_cppgc_object(scope, ctx);
  let val: v8::Local<v8::Value> = obj.cast();
  Ok(v8::Global::new(scope, val))
}

/// Placeholder init op (reserved for future initialization).
#[op2(fast)]
pub fn op_canvas2d_init(_state: &mut OpState) {}

// --- Internal helpers for Phase 2 paths ---

#[allow(dead_code, reason = "path resolution helpers used by fill/stroke/clip")]
impl OffscreenCanvasRenderingContext2D {
  fn resolve_optional_path(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    arg: Option<v8::Local<'_, v8::Value>>,
  ) -> kurbo::BezPath {
    if let Some(v) = arg
      && let Some(p) =
        deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v)
    {
      return p.path.borrow().clone();
    }
    self.current_path.borrow().clone()
  }

  fn resolve_path_and_fill_rule(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    first: Option<v8::Local<'_, v8::Value>>,
    second: Option<String>,
  ) -> (kurbo::BezPath, String) {
    // first may be Path2D or fillRule string
    if let Some(v) = first {
      if v.is_string() {
        let rule = v.to_rust_string_lossy(scope);
        return (self.current_path.borrow().clone(), rule);
      }
      if let Some(p) =
        deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v)
      {
        let rule = second.unwrap_or_else(|| "nonzero".to_string());
        return (p.path.borrow().clone(), rule);
      }
    }
    let rule = second.unwrap_or_else(|| "nonzero".to_string());
    (self.current_path.borrow().clone(), rule)
  }

  fn draw_path_fill(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path: kurbo::BezPath,
    rule: String,
  ) {
    if path.is_empty() {
      return;
    }
    let state = self.state.borrow();
    let op = state.global_composite_operation;
    let alpha = state.global_alpha;
    let shadow = Self::has_shadow(&state);
    let shadow_brush = if shadow {
      Some(Self::shadow_brush(&state))
    } else {
      None
    };
    let shadow_xform = if shadow {
      Some(Self::shadow_transform(&state, state.transform))
    } else {
      None
    };
    let (brush, brush_transform) =
      self.resolve_brush(scope, &state.fill_style, 1.0, state.transform);
    let transform = state.transform;
    let fill = if rule == "evenodd" {
      peniko::Fill::EvenOdd
    } else {
      peniko::Fill::NonZero
    };
    drop(state);

    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      Self::push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let (Some(sb), Some(st)) = (shadow_brush, shadow_xform) {
      Self::fill_on(&mut drawing, &path, fill, st, sb, None);
    }
    Self::fill_on(&mut drawing, &path, fill, transform, brush, brush_transform);
    if has_layer {
      Self::pop_compositing_layer(&mut drawing);
    }
  }

  fn draw_path_stroke(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    path: kurbo::BezPath,
  ) {
    if path.is_empty() {
      return;
    }
    let state = self.state.borrow();
    let op = state.global_composite_operation;
    let alpha = state.global_alpha;
    let shadow = Self::has_shadow(&state);
    let shadow_brush = if shadow {
      Some(Self::shadow_brush(&state))
    } else {
      None
    };
    let shadow_xform = if shadow {
      Some(Self::shadow_transform(&state, state.transform))
    } else {
      None
    };
    let (brush, brush_transform) =
      self.resolve_brush(scope, &state.stroke_style, 1.0, state.transform);
    let transform = state.transform;

    let mut stroke =
      kurbo::Stroke::new(state.line_width).with_miter_limit(state.miter_limit);
    match state.line_join {
      LineJoin::Round => {
        stroke.join = kurbo::Join::Round;
      }
      LineJoin::Bevel => {
        stroke.join = kurbo::Join::Bevel;
      }
      LineJoin::Miter => {
        stroke.join = kurbo::Join::Miter;
      }
    }
    match state.line_cap {
      LineCap::Butt => {
        stroke.start_cap = kurbo::Cap::Butt;
        stroke.end_cap = kurbo::Cap::Butt;
      }
      LineCap::Round => {
        stroke.start_cap = kurbo::Cap::Round;
        stroke.end_cap = kurbo::Cap::Round;
      }
      LineCap::Square => {
        stroke.start_cap = kurbo::Cap::Square;
        stroke.end_cap = kurbo::Cap::Square;
      }
    }
    if !state.line_dash.is_empty() {
      stroke = stroke
        .with_dashes(state.line_dash_offset, state.line_dash.iter().copied());
    }
    drop(state);

    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let has_layer =
      Self::push_compositing_layer(&mut drawing, op, alpha, width, height);
    if let (Some(sb), Some(st)) = (shadow_brush, shadow_xform) {
      Self::stroke_on(&mut drawing, &path, &stroke, st, sb, None);
    }
    Self::stroke_on(
      &mut drawing,
      &path,
      &stroke,
      transform,
      brush,
      brush_transform,
    );
    if has_layer {
      Self::pop_compositing_layer(&mut drawing);
    }
  }

  fn stroke_shape(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    shape: &impl kurbo::Shape,
  ) {
    let path: kurbo::BezPath = shape.path_elements(0.1).collect();
    self.draw_path_stroke(scope, path);
  }

  fn require_finite(
    values: &[UnrestrictedDouble],
  ) -> Result<(), Canvas2DError> {
    if values.iter().any(|v| !v.is_finite()) {
      return Err(Canvas2DError::NonFinite);
    }
    Ok(())
  }

  fn parse_fill_stroke_style(
    scope: &mut v8::PinScope<'_, '_>,
    value: v8::Local<'_, v8::Value>,
  ) -> Option<FillStrokeStyle> {
    if value.is_string() {
      let s = value.to_rust_string_lossy(scope);
      return parse_css_color(&s).ok().map(FillStrokeStyle::Color);
    }
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
    None
  }

  fn resolve_brush(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    style: &FillStrokeStyle,
    global_alpha: f32,
    _ctm: kurbo::Affine,
  ) -> (peniko::Brush, Option<kurbo::Affine>) {
    match style {
      FillStrokeStyle::Color(c) => {
        let rgba = c.to_rgba8();
        let alpha =
          (rgba.a as f32 / 255.0 * global_alpha * 255.0).round() as u8;
        let color = peniko::Color::from_rgba8(rgba.r, rgba.g, rgba.b, alpha);
        (peniko::Brush::Solid(color), None)
      }
      FillStrokeStyle::Gradient(obj) => {
        let local = v8::Local::new(scope, obj);
        let gradient = deno_core::cppgc::try_unwrap_cppgc_object::<
          CanvasGradient,
        >(scope, local.into())
        .expect("fillStyle gradient reference must be valid");
        let g = gradient.gradient.borrow().clone();
        (peniko::Brush::Gradient(g), Some(kurbo::Affine::IDENTITY))
      }
      FillStrokeStyle::Pattern(obj) => {
        let local = v8::Local::new(scope, obj);
        let pattern =
          deno_core::cppgc::try_unwrap_cppgc_object::<CanvasPattern>(
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
        let pattern_transform = *pattern.transform.borrow();
        (peniko::Brush::Image(image_brush), Some(pattern_transform))
      }
    }
  }

  fn apply_cpu_paint(
    ctx: &mut vello_cpu::RenderContext,
    brush: peniko::Brush,
    brush_transform: Option<kurbo::Affine>,
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
    let blend = op.to_blend_mode();
    match drawing {
      DrawingBackend::Vello(scene) => {
        let clip = kurbo::Rect::new(0.0, 0.0, width as f64, height as f64);
        scene.push_layer(
          peniko::Fill::NonZero,
          blend,
          alpha,
          kurbo::Affine::IDENTITY,
          &clip,
        );
      }
      DrawingBackend::VelloCpu(ctx, _) => {
        ctx.push_layer(None, Some(blend), Some(alpha), None, None);
      }
    }
    true
  }

  fn pop_compositing_layer(drawing: &mut DrawingBackend) {
    match drawing {
      DrawingBackend::Vello(scene) => scene.pop_layer(),
      DrawingBackend::VelloCpu(ctx, _) => ctx.pop_layer(),
    }
  }

  fn has_shadow(state: &DrawingState) -> bool {
    !is_color_transparent(state.shadow_color_rgba)
      && (state.shadow_blur > 0.0
        || state.shadow_offset_x != 0.0
        || state.shadow_offset_y != 0.0)
  }

  fn shadow_brush(state: &DrawingState) -> peniko::Brush {
    peniko::Brush::Solid(state.shadow_color_rgba)
  }

  fn shadow_transform(
    state: &DrawingState,
    transform: kurbo::Affine,
  ) -> kurbo::Affine {
    // TODO(petamoriken): apply shadowBlur once Vello GPU supports filter effects
    kurbo::Affine::translate((state.shadow_offset_x, state.shadow_offset_y))
      * transform
  }

  fn fill_on(
    drawing: &mut DrawingBackend,
    shape: &impl kurbo::Shape,
    fill: peniko::Fill,
    transform: kurbo::Affine,
    brush: peniko::Brush,
    brush_transform: Option<kurbo::Affine>,
  ) {
    match drawing {
      DrawingBackend::Vello(scene) => {
        scene.fill(fill, transform, &brush, brush_transform, shape);
      }
      DrawingBackend::VelloCpu(ctx, _) => {
        Self::apply_cpu_paint(ctx, brush, brush_transform);
        ctx.set_fill_rule(if fill == peniko::Fill::EvenOdd {
          vello_cpu::peniko::Fill::EvenOdd
        } else {
          vello_cpu::peniko::Fill::NonZero
        });
        ctx.set_transform(transform);
        let path: kurbo::BezPath = shape.path_elements(0.1).collect();
        ctx.fill_path(&path);
      }
    }
  }

  fn stroke_on(
    drawing: &mut DrawingBackend,
    path: &kurbo::BezPath,
    stroke: &kurbo::Stroke,
    transform: kurbo::Affine,
    brush: peniko::Brush,
    brush_transform: Option<kurbo::Affine>,
  ) {
    match drawing {
      DrawingBackend::Vello(scene) => {
        scene.stroke(stroke, transform, &brush, brush_transform, path);
      }
      DrawingBackend::VelloCpu(ctx, _) => {
        Self::apply_cpu_paint(ctx, brush, brush_transform);
        ctx.set_stroke(stroke.clone());
        ctx.set_transform(transform);
        ctx.stroke_path(path);
      }
    }
  }

  fn apply_clip(&self, path: kurbo::BezPath, rule: String) {
    if path.is_empty() {
      return;
    }
    let fill = if rule == "evenodd" {
      peniko::Fill::EvenOdd
    } else {
      peniko::Fill::NonZero
    };
    let transform = self.state.borrow().transform;

    match &mut *self.drawing.borrow_mut() {
      DrawingBackend::Vello(scene) => {
        scene.push_clip_layer(fill, transform, &path);
      }
      DrawingBackend::VelloCpu(ctx, _) => {
        ctx.push_clip_layer(&path);
      }
    }
    self.state.borrow_mut().clip_depth += 1;
  }

  #[inline]
  fn v8_to_f64(
    scope: &mut v8::PinScope<'_, '_>,
    v: v8::Local<'_, v8::Value>,
  ) -> f64 {
    v.number_value(scope).unwrap_or(f64::NAN)
  }

  #[inline]
  fn type_error_not_path2d(
    prefix: &'static str,
    context: &'static str,
  ) -> Canvas2DError {
    Canvas2DError::WebIdl(deno_core::webidl::WebIdlError {
      prefix: prefix.into(),
      context: context.into(),
      kind: deno_core::webidl::WebIdlErrorKind::ConvertToConverterType(
        "Path2D",
      ),
    })
  }

  #[inline]
  fn resolve_point_in_path_args(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    a: Option<v8::Local<'_, v8::Value>>,
    b: Option<v8::Local<'_, v8::Value>>,
    c: Option<v8::Local<'_, v8::Value>>,
    d: Option<String>,
  ) -> Result<(kurbo::BezPath, f64, f64, String), Canvas2DError> {
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

    let Some(a) = a else {
      if d.is_some() {
        // 4 args: isPointInPath(path, x, y, fillRule) — null/undefined is not Path2D
        return Err(Self::type_error_not_path2d(PREFIX, "parameter 1"));
      }
      if b.is_some() {
        // 2-3 args with null/undefined first: isPointInPath(x, y [, fillRule])
        let y =
          b.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
        let rule = c
          .map(|v| v.to_rust_string_lossy(scope))
          .unwrap_or_else(|| "nonzero".into());
        validate_fill_rule("parameter 3", &rule)?;
        return Ok((
          self.current_path.borrow().clone(),
          f64::NAN,
          y,
          rule,
        ));
      }
      return Ok((
        self.current_path.borrow().clone(),
        f64::NAN,
        f64::NAN,
        "nonzero".into(),
      ));
    };
    if let Some(p) =
      deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, a)
    {
      // isPointInPath(path, x, y [, fillRule])
      let x = b.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      let y = c.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      let rule = d.unwrap_or_else(|| "nonzero".into());
      validate_fill_rule("parameter 4", &rule)?;
      return Ok((p.path.borrow().clone(), x, y, rule));
    }
    if a.is_number() {
      // isPointInPath(x, y [, fillRule])
      let x = Self::v8_to_f64(scope, a);
      let y = b.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      let rule = c
        .map(|v| v.to_rust_string_lossy(scope))
        .unwrap_or_else(|| "nonzero".into());
      validate_fill_rule("parameter 3", &rule)?;
      return Ok((self.current_path.borrow().clone(), x, y, rule));
    }
    Err(Self::type_error_not_path2d(PREFIX, "parameter 1"))
  }

  #[inline]
  fn resolve_point_in_stroke_args(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
    a: Option<v8::Local<'_, v8::Value>>,
    b: Option<v8::Local<'_, v8::Value>>,
    c: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<(kurbo::BezPath, f64, f64), Canvas2DError> {
    const PREFIX: &str = "Failed to execute 'isPointInStroke' on 'OffscreenCanvasRenderingContext2D'";
    let Some(a) = a else {
      if c.is_some() {
        // 3 args: isPointInStroke(path, x, y) — null/undefined is not Path2D
        return Err(Self::type_error_not_path2d(PREFIX, "parameter 1"));
      }
      if b.is_some() {
        // 2 args with null/undefined first: isPointInStroke(x, y)
        let y =
          b.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
        return Ok((self.current_path.borrow().clone(), f64::NAN, y));
      }
      return Ok((
        self.current_path.borrow().clone(),
        f64::NAN,
        f64::NAN,
      ));
    };
    if let Some(p) =
      deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, a)
    {
      // isPointInStroke(path, x, y)
      let x = b.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      let y = c.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      return Ok((p.path.borrow().clone(), x, y));
    }
    if a.is_number() {
      // isPointInStroke(x, y)
      let x = Self::v8_to_f64(scope, a);
      let y = b.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
      return Ok((self.current_path.borrow().clone(), x, y));
    }
    Err(Self::type_error_not_path2d(PREFIX, "parameter 1"))
  }

  #[inline]
  fn test_point_in_path(
    &self,
    path: kurbo::BezPath,
    x: f64,
    y: f64,
    rule: String,
  ) -> bool {
    use kurbo::Shape;
    let pt = kurbo::Point::new(x, y);
    let w = path.winding(pt);
    match rule.as_str() {
      "evenodd" => w % 2 != 0,
      _ => w != 0,
    }
  }

  #[inline]
  fn test_point_in_stroke(&self, path: kurbo::BezPath, x: f64, y: f64) -> bool {
    if path.is_empty() {
      return false;
    }
    // Approximate: stroke the path and test contains on outline.
    let state = self.state.borrow();
    let stroke = kurbo::Stroke::new(state.line_width.max(1.0));
    drop(state);
    let outline = kurbo::stroke(
      path.path_elements(0.1),
      &stroke,
      &kurbo::StrokeOpts::default(),
      0.1,
    );
    outline.contains(kurbo::Point::new(x, y))
  }
}

fn compute_arc_sweep(
  start_angle: f64,
  end_angle: f64,
  counterclockwise: bool,
) -> f64 {
  let two_pi = 2.0 * std::f64::consts::PI;
  let mut delta = end_angle - start_angle;

  if counterclockwise {
    if delta >= two_pi {
      delta = two_pi;
    } else if delta <= 0.0 {
      while delta <= 0.0 {
        delta += two_pi;
      }
      if delta > two_pi {
        delta = two_pi;
      }
    }
  } else {
    if delta <= -two_pi {
      delta = -two_pi;
    } else if delta >= 0.0 {
      while delta >= 0.0 {
        delta -= two_pi;
      }
      if delta < -two_pi {
        delta = -two_pi;
      }
    }
  }
  delta
}

fn arc_to_impl(
  path: &mut kurbo::BezPath,
  x1: f64,
  y1: f64,
  x2: f64,
  y2: f64,
  radius: f64,
) {
  let current = match path.elements().last() {
    Some(kurbo::PathEl::MoveTo(p)) => *p,
    Some(kurbo::PathEl::LineTo(p)) => *p,
    Some(kurbo::PathEl::QuadTo(_, p)) => *p,
    Some(kurbo::PathEl::CurveTo(_, _, p)) => *p,
    Some(kurbo::PathEl::ClosePath) => return,
    None => return,
  };

  let p0 = current;
  let p1 = kurbo::Point::new(x1, y1);
  let p2 = kurbo::Point::new(x2, y2);

  if p0 == p1 || p1 == p2 || radius == 0.0 {
    path.line_to(p1);
    return;
  }

  let v0 = p0 - p1;
  let v1 = p2 - p1;

  let cross = v0.x * v1.y - v0.y * v1.x;
  if cross.abs() < 1e-10 {
    path.line_to(p1);
    return;
  }

  let d0 = v0.hypot();
  let d1 = v1.hypot();
  let u0 = kurbo::Vec2::new(v0.x / d0, v0.y / d0);
  let u1 = kurbo::Vec2::new(v1.x / d1, v1.y / d1);

  let cos_half = ((1.0 + u0.dot(u1)) / 2.0).sqrt();
  if cos_half == 0.0 {
    path.line_to(p1);
    return;
  }
  let d = radius / ((1.0 - cos_half * cos_half).sqrt() / cos_half);

  let t0 = kurbo::Point::new(p1.x + u0.x * d, p1.y + u0.y * d);
  let t1 = kurbo::Point::new(p1.x + u1.x * d, p1.y + u1.y * d);

  let cx_dir = kurbo::Vec2::new(u0.x + u1.x, u0.y + u1.y);
  let cx_len = cx_dir.hypot();
  if cx_len == 0.0 {
    path.line_to(p1);
    return;
  }

  let sign = if cross < 0.0 { 1.0 } else { -1.0 };
  let center = kurbo::Point::new(
    p1.x + cx_dir.x / cx_len * (d * d + radius * radius).sqrt(),
    p1.y + cx_dir.y / cx_len * (d * d + radius * radius).sqrt(),
  );

  let start_angle = (t0.y - center.y).atan2(t0.x - center.x);
  let end_angle = (t1.y - center.y).atan2(t1.x - center.x);

  let counterclockwise = sign > 0.0;
  let sweep = compute_arc_sweep(start_angle, end_angle, counterclockwise);

  path.line_to(t0);
  let arc = kurbo::Arc {
    center,
    radii: kurbo::Vec2::new(radius, radius),
    start_angle,
    sweep_angle: sweep,
    x_rotation: 0.0,
  };
  arc.to_cubic_beziers(0.1, |p1, p2, p3| {
    path.curve_to(p1, p2, p3);
  });
}
