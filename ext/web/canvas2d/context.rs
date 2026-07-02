// Copyright 2018-2026 the Deno authors. MIT license.

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
use deno_image::image::GenericImageView;
use deno_image::image::Rgba;
use deno_image::image::RgbaImage;
use parley::FontContext;
use parley::LayoutContext;
use parley::PositionedLayoutItem;
use vello::kurbo;
use vello::kurbo::Shape;
use vello::peniko;

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
use crate::canvas2d::path::arc_to_impl;
use crate::canvas2d::path::build_round_rect_path;
use crate::canvas2d::path::compute_arc_sweep;
use crate::canvas2d::path::parse_round_rect_radii;
use crate::canvas2d::pattern::CanvasPattern;
use crate::canvas2d::pattern::parse_repetition;
use crate::canvas2d::renderer::DenoCanvasBackend;
use crate::canvas2d::renderer::SharedRenderer;
use crate::canvas2d::renderer::render_scene;
use crate::canvas2d::renderer::render_scene_to_texture_view;
use crate::canvas2d::state::Canvas2DSettings;
use crate::canvas2d::state::ClipEntry;
use crate::canvas2d::state::DrawingBackend;
use crate::canvas2d::state::DrawingState;
use crate::canvas2d::state::FillStrokeStyle;
use crate::canvas2d::state::GlobalCompositeOperation;
use crate::canvas2d::state::ImageSmoothingQuality;
use crate::canvas2d::state::LineCap;
use crate::canvas2d::state::LineJoin;
use crate::canvas2d::state::StateStackEntry;
use crate::canvas2d::state::TextAlign;
use crate::canvas2d::state::TextBaseline;
use crate::canvas2d::text::build_text_layout;
use crate::canvas2d::text::compute_baseline_y;
use crate::canvas2d::text::compute_text_metrics;
use crate::css::color::Color;
use crate::css::color::color_to_css_string;
use crate::css::color::is_color_transparent;
use crate::css::color::parse_css_color;
use crate::css::filter::FilterValueListParser;
use crate::css::filter::ParserInput as FilterParserInput;
use crate::css::font::FontState;
use crate::css::font::TextDirection;
use crate::css::font::parse_css_font;
use crate::css::font::parse_css_spacing;
use crate::image_data::ImageData;
use crate::text_metrics::TextMetrics;

pub const CONTEXT_ID: &str = "2d";
pub const UNSTABLE_FEATURE_NAME: &str = "canvas2d";

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
        if let (Some(sb), Some(st)) = (&shadow_brush, &shadow_xform) {
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
              scene
                .draw_glyphs(&font)
                .font_size(font_size)
                .transform(*st)
                .brush(sb)
                .draw(peniko::Fill::NonZero, glyphs);
            }
          }
        }
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
        if let (Some(sb), Some(st)) = (&shadow_brush, &shadow_xform) {
          for line in layout.lines() {
            for item in line.items() {
              let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
              };
              let font = peniko::FontData::clone(glyph_run.run().font());
              let font_size = glyph_run.run().font_size();
              Self::apply_cpu_paint(ctx, sb.clone(), None);
              ctx.set_transform(*st);
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
        for line in layout.lines() {
          for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
              continue;
            };
            let font = peniko::FontData::clone(glyph_run.run().font());
            let font_size = glyph_run.run().font_size();

            Self::apply_cpu_paint(ctx, brush.clone(), brush_transform);
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
      Self::pop_compositing_layer(&mut drawing);
    }
  }

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
    let clip_depth = self.state.borrow().clip_depth;
    let mut drawing = self.drawing.borrow_mut();
    for _ in 0..clip_depth {
      Self::pop_compositing_layer(&mut drawing);
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
      match &mut *drawing {
        DrawingBackend::Vello(scene) => {
          scene.push_clip_layer(fill, clip.transform, &clip.path);
        }
        DrawingBackend::VelloCpu(ctx, _) => {
          ctx.push_clip_layer(&clip.path);
        }
      }
    }
    result
  }

  /// Renders the accumulated scene directly to an external TextureView.
  ///
  /// The view must be created from a texture belonging to the same wgpu device
  /// as this context's renderer. Does nothing when no backend is available.
  pub fn render_to_texture_view(
    &self,
    view: &crate::canvas2d::renderer::wgpu::TextureView,
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
    let clip_depth = self.state.borrow().clip_depth;
    let mut drawing = self.drawing.borrow_mut();
    for _ in 0..clip_depth {
      Self::pop_compositing_layer(&mut drawing);
    }
    let buf = match &mut *drawing {
      DrawingBackend::Vello(scene) => {
        if let Some(Some(renderer)) = self.renderer.get() {
          render_scene(renderer, scene, width, height, base_color)
            .map_err(|e| {
              log::warn!("canvas2d: render error: {e}");
            })
            .ok()
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
      match &mut *drawing {
        DrawingBackend::Vello(scene) => {
          scene.push_clip_layer(fill, clip.transform, &clip.path);
        }
        DrawingBackend::VelloCpu(ctx, _) => {
          ctx.push_clip_layer(&clip.path);
        }
      }
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

  fn extract_sub_image(
    pixels: &[u8],
    img_w: u32,
    img_h: u32,
    sx: f64,
    sy: f64,
    sw: f64,
    sh: f64,
  ) -> (Vec<u8>, u32, u32) {
    let (sx, sw) = if sw < 0.0 { (sx + sw, -sw) } else { (sx, sw) };
    let (sy, sh) = if sh < 0.0 { (sy + sh, -sh) } else { (sy, sh) };

    let x0 = (sx.max(0.0) as u32).min(img_w);
    let y0 = (sy.max(0.0) as u32).min(img_h);
    let x1 = ((sx + sw) as u32).min(img_w);
    let y1 = ((sy + sh) as u32).min(img_h);
    let out_w = x1.saturating_sub(x0);
    let out_h = y1.saturating_sub(y0);

    if out_w == 0 || out_h == 0 {
      return (vec![], 0, 0);
    }

    let mut sub = vec![0u8; out_w as usize * out_h as usize * 4];
    for row in 0..out_h {
      let src_offset = ((y0 + row) as usize * img_w as usize + x0 as usize) * 4;
      let dst_offset = row as usize * out_w as usize * 4;
      let len = out_w as usize * 4;
      sub[dst_offset..dst_offset + len]
        .copy_from_slice(&pixels[src_offset..src_offset + len]);
    }
    (sub, out_w, out_h)
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
    let mut state = self.state.borrow_mut();
    self.clip_stack.borrow_mut().truncate(state.clip_depth);
    self.clip_stack.borrow_mut().push(ClipEntry {
      path,
      rule,
      transform,
    });
    state.clip_depth += 1;
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
        let y = b.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
        let rule = c
          .map(|v| v.to_rust_string_lossy(scope))
          .unwrap_or_else(|| "nonzero".into());
        validate_fill_rule("parameter 3", &rule)?;
        return Ok((self.current_path.borrow().clone(), f64::NAN, y, rule));
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
        let y = b.map(|v| Self::v8_to_f64(scope, v)).unwrap_or(f64::NAN);
        return Ok((self.current_path.borrow().clone(), f64::NAN, y));
      }
      return Ok((self.current_path.borrow().clone(), f64::NAN, f64::NAN));
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
  #[undefined]
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
  #[undefined]
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
  #[undefined]
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
  #[undefined]
  fn save(&self) {
    self
      .state_stack
      .borrow_mut()
      .push(StateStackEntry::Save(self.state.borrow().clone()));
  }

  #[fast]
  #[undefined]
  fn restore(&self) {
    let mut stack = self.state_stack.borrow_mut();
    if let Some(StateStackEntry::Save(_)) = stack.last() {
      let current_clip_depth = self.state.borrow().clip_depth;
      if let Some(StateStackEntry::Save(saved)) = stack.pop() {
        let saved_clip_depth = saved.clip_depth;
        *self.state.borrow_mut() = saved;
        for _ in saved_clip_depth..current_clip_depth {
          Self::pop_compositing_layer(&mut self.drawing.borrow_mut());
        }
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
  #[undefined]
  fn begin_layer(
    &self,
    options: Option<v8::Local<'_, v8::Value>>,
  ) -> Result<(), Canvas2DError> {
    if let Some(opts) = options
      && !opts.is_undefined()
      && !opts.is_null()
      && !opts.is_object()
    {
      return Err(Canvas2DError::WebIdl(deno_core::webidl::WebIdlError {
        prefix: "beginLayer".into(),
        context: "Argument 1".into(),
        kind: deno_core::webidl::WebIdlErrorKind::ConvertToConverterType(
          "BeginLayerOptions",
        ),
      }));
    }

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
      state.filter_string = String::from("none");
      state.filter = Vec::new();
    }

    let (width, height) = self.data.dimensions();
    let mut drawing = self.drawing.borrow_mut();
    let pushed =
      Self::push_compositing_layer(&mut drawing, op, alpha, width, height);

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
    loop {
      match stack.pop() {
        Some(StateStackEntry::Layer(saved_state, pushed)) => {
          *self.state.borrow_mut() = saved_state;
          self.layer_depth.set(depth - 1);
          if pushed {
            Self::pop_compositing_layer(&mut self.drawing.borrow_mut());
          }
          return Ok(());
        }
        Some(StateStackEntry::Save(_)) => {
          continue;
        }
        None => {
          return Err(Canvas2DError::InvalidState(
            "endLayer called without matching beginLayer".into(),
          ));
        }
      }
    }
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
      self.current_path.borrow_mut().move_to((*x, *y));
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
      let mut path = self.current_path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to((*x, *y));
      } else {
        path.line_to((*x, *y));
      }
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
      let mut path = self.current_path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to((*cp1x, *cp1y));
      }
      path.curve_to((*cp1x, *cp1y), (*cp2x, *cp2y), (*x, *y));
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
      let mut path = self.current_path.borrow_mut();
      if path.elements().is_empty() {
        path.move_to((*cpx, *cpy));
      }
      path.quad_to((*cpx, *cpy), (*x, *y));
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
    if *radius < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius));
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
    if *radius < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius));
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
    if *radius_x < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius_x));
    }
    if *radius_y < 0.0 {
      return Err(Canvas2DError::NegativeRadius(*radius_y));
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
    let mut path = self.current_path.borrow_mut();
    path.move_to((*x, *y));
    path.line_to((*x + *w, *y));
    path.line_to((*x + *w, *y + *h));
    path.line_to((*x, *y + *h));
    path.close_path();
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
    let corner_radii = parse_round_rect_radii(scope, radii_val)?;
    let mut path = self.current_path.borrow_mut();
    build_round_rect_path(&mut path, *x, *y, *w, *h, &corner_radii);
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

  #[undefined]
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
  #[undefined]
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

  #[undefined]
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

  #[undefined]
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
        let provided = 1
          + b.is_some() as u32
          + c.is_some() as u32
          + d.is_some() as u32
          + e.is_some() as u32
          + f_val.is_some() as u32;
        let (Some(b), Some(c), Some(d), Some(e), Some(f_val)) =
          (b, c, d, e, f_val)
        else {
          return Err(Canvas2DError::MissingArgument {
            required: 6,
            provided,
          });
        };
        (a, *b, *c, *d, *e, *f_val)
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
    if [a, b, c, d, e, f].iter().any(|v| !v.is_finite()) {
      return Ok(());
    }
    self.state.borrow_mut().transform = kurbo::Affine::new([a, b, c, d, e, f]);
    Ok(())
  }

  #[fast]
  #[undefined]
  fn reset_transform(&self) {
    self.state.borrow_mut().transform = kurbo::Affine::IDENTITY;
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
    let m = kurbo::Affine::new([*a, *b, *c, *d, *e, *f]);
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
    state.transform *= kurbo::Affine::scale_non_uniform(*x, *y);
  }

  #[required(1)]
  #[undefined]
  fn rotate(&self, #[webidl] angle: UnrestrictedDouble) {
    if !angle.is_finite() {
      return;
    }
    let mut state = self.state.borrow_mut();
    state.transform *= kurbo::Affine::rotate(*angle);
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
    state.transform *= kurbo::Affine::translate((*x, *y));
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

    let image_data =
      image_data_from_pixels(resolved.pixels, resolved.width, resolved.height);

    Ok(CanvasPattern {
      image: image_data,
      x_extend: repetition.x_extend,
      y_extend: repetition.y_extend,
      transform: RefCell::new(kurbo::Affine::IDENTITY),
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

    if dw == 0.0 || dh == 0.0 {
      return Ok(());
    }

    let (src_pixels, src_w, src_h) = Self::extract_sub_image(
      &resolved.pixels,
      resolved.width,
      resolved.height,
      sx,
      sy,
      sw,
      sh,
    );
    if src_w == 0 || src_h == 0 {
      return Ok(());
    }

    let img = image_data_from_pixels(src_pixels, src_w, src_h);

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

    let scale_x = dw / src_w as f64;
    let scale_y = dh / src_h as f64;
    let image_transform = ds.transform
      * kurbo::Affine::translate((dx, dy))
      * kurbo::Affine::scale_non_uniform(scale_x, scale_y);

    let op = ds.global_composite_operation;
    let alpha = ds.global_alpha;
    let shadow = Self::has_shadow(&ds);
    let shadow_brush = if shadow {
      Some(Self::shadow_brush(&ds))
    } else {
      None
    };
    let shadow_xform = if shadow {
      Some(Self::shadow_transform(&ds, image_transform))
    } else {
      None
    };
    drop(ds);

    let rect = kurbo::Rect::new(0.0, 0.0, src_w as f64, src_h as f64);
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
      image_transform,
      brush,
      None,
    );
    if has_layer {
      Self::pop_compositing_layer(&mut drawing);
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

    let sw = Self::require_long(scope, arg0)?;
    let sh = Self::require_long(scope, arg1)?;

    let w = sw.unsigned_abs();
    let h = sh.unsigned_abs();

    if w == 0 || h == 0 {
      return Err(Canvas2DError::ZeroSourceSize);
    }

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
      let dirty_x = Self::require_long(scope, dirty_x_val.unwrap())?;
      let dirty_y = Self::require_long(
        scope,
        dirty_y_val.unwrap_or_else(|| v8::undefined(scope).into()),
      )?;
      let dirty_w = Self::require_long(
        scope,
        dirty_w_val.unwrap_or_else(|| v8::undefined(scope).into()),
      )?;
      let dirty_h = Self::require_long(
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
    let rect = kurbo::Rect::new(0.0, 0.0, canvas_w as f64, canvas_h as f64);

    let clip_depth = self.state.borrow().clip_depth;
    let mut drawing = self.drawing.borrow_mut();
    drawing.reset(canvas_w, canvas_h);
    Self::fill_on(
      &mut drawing,
      &rect,
      peniko::Fill::NonZero,
      kurbo::Affine::IDENTITY,
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
      match &mut *drawing {
        DrawingBackend::Vello(scene) => {
          scene.push_clip_layer(fill, clip.transform, &clip.path);
        }
        DrawingBackend::VelloCpu(ctx, _) => {
          ctx.push_clip_layer(&clip.path);
        }
      }
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
          &DenoCanvasBackend::Cpu(crate::canvas2d::renderer::CpuRenderer),
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
