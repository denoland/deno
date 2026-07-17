// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::WebIDL;
use deno_core::v8;
use deno_core::webidl::WebIdlConverter;
use vello::kurbo;
use vello::peniko;

use super::filter::CanvasLayerFilterPrimitive;
use super::renderer::DenoCanvasBackend;
use crate::css::color::ParsedColor;
use crate::css::font::FontState;

// TODO(petamoriken): move to a shared crate when canvas2d and webgpu types need to be unified.
// ext/webgpu/canvas.rs has its own PredefinedColorSpace with additional variants.
#[derive(WebIDL, Default, Clone, Copy)]
#[webidl(enum)]
pub(super) enum PredefinedColorSpace {
  #[default]
  #[webidl(rename = "srgb")]
  Srgb,
  #[webidl(rename = "display-p3")]
  DisplayP3,
}

impl PredefinedColorSpace {
  pub(super) fn to_image_data_color_space(
    self,
  ) -> crate::image_data::PredefinedColorSpace {
    match self {
      Self::Srgb => crate::image_data::PredefinedColorSpace::Srgb,
      Self::DisplayP3 => crate::image_data::PredefinedColorSpace::DisplayP3,
    }
  }
}

// TODO(petamoriken): move to a shared crate when canvas2d and webgpu types need to be unified.
#[derive(WebIDL, Default)]
#[webidl(enum)]
pub(super) enum CanvasColorType {
  #[default]
  #[webidl(rename = "unorm8")]
  Unorm8,
  // TODO(petamoriken): float16 rendering is not yet implemented.
  #[webidl(rename = "float16")]
  Float16,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
pub(super) enum TextAlign {
  #[default]
  Start,
  End,
  Left,
  Right,
  Center,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
pub(super) enum TextBaseline {
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
pub(super) enum ImageSmoothingQuality {
  #[default]
  Low,
  Medium,
  High,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
pub(super) enum LineCap {
  #[default]
  Butt,
  Round,
  Square,
}

#[derive(WebIDL, Clone, Copy, Default)]
#[webidl(enum)]
pub(super) enum LineJoin {
  Round,
  Bevel,
  #[default]
  Miter,
}

#[derive(WebIDL, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[webidl(enum)]
pub(super) enum GlobalCompositeOperation {
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
  Clear,
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
  pub(super) fn to_blend_mode(self) -> peniko::BlendMode {
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
      Self::Clear => Compose::Clear.into(),
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

pub(super) struct Canvas2DSettings {
  pub(super) alpha: bool,
  // OffscreenCanvas has no display compositor; accepted but unused.
  #[allow(dead_code, reason = "no display compositor; accepted but unused")]
  pub(super) desynchronized: bool,
  // TODO(petamoriken): rendering in display-p3 color space is not yet implemented.
  pub(super) color_space: PredefinedColorSpace,
  // TODO(petamoriken): float16 rendering is not yet implemented.
  #[allow(dead_code, reason = "float16 rendering is not yet implemented")]
  pub(super) color_type: CanvasColorType,
  pub(super) will_read_frequently: bool,
}

impl<'a> WebIdlConverter<'a> for Canvas2DSettings {
  type Options = ();

  fn convert<'b, 'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
    prefix: std::borrow::Cow<'static, str>,
    context: deno_core::webidl::ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, deno_core::webidl::WebIdlError> {
    #[derive(WebIDL)]
    #[webidl(dictionary)]
    struct RawCanvas2DSettings {
      #[webidl(default = true)]
      alpha: bool,
      #[webidl(default = false)]
      desynchronized: bool,
      #[webidl(default = String::from("srgb"))]
      color_space: String,
      #[webidl(default = String::from("unorm8"))]
      color_type: String,
      #[webidl(default = false)]
      will_read_frequently: bool,
    }

    let raw = RawCanvas2DSettings::convert(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
      options,
    )?;

    let color_space = match raw.color_space.as_str() {
      "srgb" => PredefinedColorSpace::Srgb,
      "display-p3" => PredefinedColorSpace::DisplayP3,
      _ => {
        return Err(deno_core::webidl::WebIdlError::new(
          prefix,
          context.borrowed(),
          deno_core::webidl::WebIdlErrorKind::InvalidEnumVariant {
            converter: "PredefinedColorSpace",
            variant: raw.color_space,
          },
        ));
      }
    };

    let color_type = match raw.color_type.as_str() {
      "unorm8" => CanvasColorType::Unorm8,
      "float16" => CanvasColorType::Float16,
      _ => {
        return Err(deno_core::webidl::WebIdlError::new(
          prefix,
          context.borrowed(),
          deno_core::webidl::WebIdlErrorKind::InvalidEnumVariant {
            converter: "CanvasColorType",
            variant: raw.color_type,
          },
        ));
      }
    };

    Ok(Self {
      alpha: raw.alpha,
      desynchronized: raw.desynchronized,
      color_space,
      color_type,
      will_read_frequently: raw.will_read_frequently,
    })
  }
}

#[derive(Clone)]
pub(super) enum FillStrokeStyle {
  Color(ParsedColor),
  Gradient(v8::Global<v8::Object>),
  Pattern(v8::Global<v8::Object>),
}

/// CSS filter string most recently assigned to `ctx.filter`.
#[derive(Clone)]
pub(super) enum FilterStyle {
  Css(String),
}

// `DrawingBackend` abstracts over two unrelated vello renderer families that do
// not share a scene type today:
//
//   vello::Scene             -> vello::Renderer  (Gpu; GPU compute via wgpu)
//   vello_cpu::RenderContext -> vello_cpu        (Cpu; pure software, no wgpu)
//
// Once the Vello ecosystem stabilizes and provides a unified scene interface
// across both implementations, the Cpu and Gpu backends could be unified
// behind a single scene type; revisit `DrawingBackend` then.
pub(super) enum DrawingBackend {
  // Shared by Gpu
  Vello(vello::Scene),
  VelloCpu(vello_cpu::RenderContext, Box<vello_cpu::Resources>),
}

impl DrawingBackend {
  pub(super) fn new(
    backend: &DenoCanvasBackend,
    width: u32,
    height: u32,
  ) -> Self {
    match backend {
      DenoCanvasBackend::Gpu(_) => DrawingBackend::Vello(vello::Scene::new()),
      DenoCanvasBackend::Cpu(_) => {
        let (width, height) = clamp_to_vello_cpu_dimensions(width, height);
        DrawingBackend::VelloCpu(
          vello_cpu::RenderContext::new(width, height),
          Box::new(vello_cpu::Resources::new()),
        )
      }
    }
  }

  pub(super) fn reset(&mut self, width: u32, height: u32) {
    match self {
      DrawingBackend::Vello(scene) => scene.reset(),
      DrawingBackend::VelloCpu(ctx, resources) => {
        let (width, height) = clamp_to_vello_cpu_dimensions(width, height);
        *ctx = vello_cpu::RenderContext::new(width, height);
        **resources = vello_cpu::Resources::new();
      }
    }
  }
}

/// vello_cpu uses `u16` dimensions, so clamp instead of letting an `as u16`
/// cast wrap. Canvases beyond 65535 are unsupported either way.
fn clamp_to_vello_cpu_dimensions(width: u32, height: u32) -> (u16, u16) {
  (
    u16::try_from(width).unwrap_or(u16::MAX),
    u16::try_from(height).unwrap_or(u16::MAX),
  )
}

#[derive(Clone)]
pub(super) struct DrawingState {
  pub(super) fill_style: FillStrokeStyle,
  pub(super) stroke_style: FillStrokeStyle,
  pub(super) global_alpha: f32,
  pub(super) font_state: FontState,
  pub(super) text_align: TextAlign,
  pub(super) text_baseline: TextBaseline,
  pub(super) lang: String,
  pub(super) global_composite_operation: GlobalCompositeOperation,
  pub(super) filter_style: FilterStyle,
  pub(super) filter: Vec<CanvasLayerFilterPrimitive>,
  pub(super) image_smoothing_enabled: bool,
  pub(super) image_smoothing_quality: ImageSmoothingQuality,
  pub(super) line_width: f64,
  pub(super) line_cap: LineCap,
  pub(super) line_join: LineJoin,
  pub(super) miter_limit: f64,
  pub(super) line_dash_offset: f64,
  pub(super) line_dash: Vec<f64>,
  pub(super) shadow_blur: f64,
  pub(super) shadow_color: ParsedColor,
  pub(super) shadow_offset_x: f64,
  pub(super) shadow_offset_y: f64,
  pub(super) transform: kurbo::Affine,
  pub(super) clip_depth: usize,
}

impl Default for DrawingState {
  fn default() -> Self {
    Self {
      fill_style: FillStrokeStyle::Color(ParsedColor::BLACK),
      stroke_style: FillStrokeStyle::Color(ParsedColor::BLACK),
      global_alpha: 1.0,
      font_state: FontState::default(),
      text_align: TextAlign::default(),
      text_baseline: TextBaseline::default(),
      lang: String::from("inherit"),
      global_composite_operation: GlobalCompositeOperation::default(),
      filter_style: FilterStyle::Css(String::from("none")),
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
      shadow_color: ParsedColor::TRANSPARENT,
      shadow_offset_x: 0.0,
      shadow_offset_y: 0.0,
      transform: kurbo::Affine::IDENTITY,
      clip_depth: 0,
    }
  }
}

pub(super) enum StateStackEntry {
  Save(DrawingState),
  Layer(DrawingState, bool),
}

#[derive(Clone)]
pub(super) struct ClipEntry {
  pub(super) path: kurbo::BezPath,
  pub(super) rule: String,
  pub(super) transform: kurbo::Affine,
}
