// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::v8;
use deno_core::webidl::UnrestrictedDouble;
use deno_image::image::DynamicImage;
use deno_image::image::GenericImageView;
use deno_image::image::Rgba;
use deno_image::image::RgbaImage;
use parley::PositionedLayoutItem;
use vello::kurbo::Affine;
use vello::kurbo::BezPath;
use vello::kurbo::Cap;
use vello::kurbo::Join;
use vello::kurbo::Rect;
use vello::kurbo::Shape;
use vello::kurbo::Stroke;
use vello::peniko;

use super::OffscreenCanvasRenderingContext2D;
use super::renderer::render_scene;
use super::renderer::render_scene_to_texture_view;
use super::state::ClipEntry;
use super::state::DrawingBackend;
use super::state::DrawingState;
use super::state::FillStrokeStyle;
use super::state::GlobalCompositeOperation;
use super::state::LineCap;
use super::state::LineJoin;
use super::state::TextAlign;
use super::text::build_text_layout;
use super::text::compute_baseline_y;
use crate::canvas2d::error::Canvas2DError;
use crate::canvas2d::gradient::CanvasGradient;
use crate::canvas2d::image::unpremultiply_rgba;
use crate::canvas2d::path::Path2D;
use crate::canvas2d::pattern::CanvasPattern;
use crate::css::color::is_color_transparent;
use crate::css::color::parse_css_color;
use crate::css::font::TextDirection;

pub(super) fn draw_text(
  context: &OffscreenCanvasRenderingContext2D,
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
  let fstate = context.state.borrow().font_state.clone();
  let mut fc = context.font_ctx.lock().unwrap();
  let mut lc = context.layout_ctx.lock().unwrap();
  let layout = build_text_layout(&mut fc, &mut lc, text, &fstate);

  let state = context.state.borrow();
  let style = if stroke {
    &state.stroke_style
  } else {
    &state.fill_style
  };
  let op = state.global_composite_operation;
  let global_alpha = state.global_alpha;
  let shadow = has_shadow(&state);
  let shadow_color = state.shadow_color_rgba;
  let shadow_xform = if shadow {
    Some(shadow_transform(&state, state.transform))
  } else {
    None
  };
  let (brush, brush_transform) = resolve_brush(scope, style, 1.0);
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

  let (canvas_w, canvas_h) = context.data.dimensions();
  let mut drawing = context.drawing.borrow_mut();
  let has_layer =
    push_compositing_layer(&mut drawing, op, global_alpha, canvas_w, canvas_h);
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

/// Clears the accumulated scene and updates the canvas dimensions.
pub(super) fn has_open_layers(
  context: &OffscreenCanvasRenderingContext2D,
) -> bool {
  context.layer_depth.get() > 0
}

/// Called when OffscreenCanvas.width or .height is changed.
pub(super) fn resize(context: &OffscreenCanvasRenderingContext2D) {
  *context.state.borrow_mut() = DrawingState::default();
  context.state_stack.borrow_mut().clear();
  context.layer_depth.set(0);
  context.clip_stack.borrow_mut().clear();
  context.current_path.borrow_mut().truncate(0);
  let (width, height) = context.data.dimensions();
  context.drawing.borrow_mut().reset(width, height);
}

/// Renders the accumulated scene to raw RGBA8 bytes.
///
/// Returns a blank zero-filled buffer when no GPU backend is available.
pub(super) fn render_to_bytes(
  context: &OffscreenCanvasRenderingContext2D,
) -> Result<Vec<u8>, Canvas2DError> {
  let (width, height) = context.data.dimensions();
  let base_color = if context.settings.alpha {
    peniko::Color::TRANSPARENT
  } else {
    peniko::Color::from_rgb8(0, 0, 0)
  };
  let clip_depth = context.state.borrow().clip_depth;
  let mut drawing = context.drawing.borrow_mut();
  for _ in 0..clip_depth {
    pop_compositing_layer(&mut drawing);
  }
  let result = match &mut *drawing {
    DrawingBackend::Vello(scene) => {
      if let Some(Some(renderer)) = context.renderer.get() {
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
      if !context.settings.alpha {
        for pixel in buf.chunks_exact_mut(4) {
          pixel[3] = 255;
        }
      }
      Ok(buf)
    }
  };
  let clip_stack = context.clip_stack.borrow();
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

/// Renders the accumulated scene directly to an external TextureView.
///
/// The view must be created from a texture belonging to the same wgpu device
/// as this context's renderer. Does nothing when no backend is available.
pub(super) fn render_to_texture_view(
  context: &OffscreenCanvasRenderingContext2D,
  view: &super::renderer::wgpu::TextureView,
) -> Result<(), Canvas2DError> {
  let (width, height) = context.data.dimensions();
  let base_color = if context.settings.alpha {
    peniko::Color::TRANSPARENT
  } else {
    peniko::Color::from_rgb8(0, 0, 0)
  };
  match &*context.drawing.borrow() {
    DrawingBackend::Vello(scene) => {
      if let Some(Some(renderer)) = context.renderer.get() {
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
pub(super) fn flush_to_image(
  context: &OffscreenCanvasRenderingContext2D,
  image: &mut DynamicImage,
) {
  let (width, height) = image.dimensions();
  let base_color = if context.settings.alpha {
    peniko::Color::TRANSPARENT
  } else {
    peniko::Color::from_rgb8(0, 0, 0)
  };
  let clip_depth = context.state.borrow().clip_depth;
  let mut drawing = context.drawing.borrow_mut();
  for _ in 0..clip_depth {
    pop_compositing_layer(&mut drawing);
  }
  let buf = match &mut *drawing {
    DrawingBackend::Vello(scene) => {
      if let Some(Some(renderer)) = context.renderer.get() {
        render_scene(renderer, scene, width, height, base_color)
          .map_err(|e| {
            log::warn!("canvas2d: render error: {e}");
          })
          .ok()
          .map(|mut buf| {
            // render_scene returns premultiplied alpha; DynamicImage expects
            // straight alpha, same as the VelloCpu branch below.
            if context.settings.alpha {
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
      if !context.settings.alpha {
        for pixel in buf.chunks_exact_mut(4) {
          pixel[3] = 255;
        }
      } else {
        unpremultiply_rgba(&mut buf);
      }
      Some(buf)
    }
  };
  let clip_stack = context.clip_stack.borrow();
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
      if context.settings.alpha {
        RgbaImage::new(width, height)
      } else {
        RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]))
      }
    });
  *image = DynamicImage::ImageRgba8(rgba);
}

pub(super) fn resolve_optional_path(
  context: &OffscreenCanvasRenderingContext2D,
  scope: &mut v8::PinScope<'_, '_>,
  arg: Option<v8::Local<'_, v8::Value>>,
) -> (BezPath, bool) {
  if let Some(v) = arg
    && let Some(p) =
      deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v)
  {
    return (p.path.borrow().clone(), true);
  }
  (context.current_path.borrow().clone(), false)
}

pub(super) fn resolve_path_and_fill_rule(
  context: &OffscreenCanvasRenderingContext2D,
  scope: &mut v8::PinScope<'_, '_>,
  first: Option<v8::Local<'_, v8::Value>>,
  second: Option<String>,
) -> (BezPath, String, bool) {
  // first may be Path2D or fillRule string
  if let Some(v) = first {
    if v.is_string() {
      let rule = v.to_rust_string_lossy(scope);
      return (context.current_path.borrow().clone(), rule, false);
    }
    if let Some(p) =
      deno_core::cppgc::try_unwrap_cppgc_object::<Path2D>(scope, v)
    {
      let rule = second.unwrap_or_else(|| "nonzero".to_string());
      return (p.path.borrow().clone(), rule, true);
    }
  }
  let rule = second.unwrap_or_else(|| "nonzero".to_string());
  (context.current_path.borrow().clone(), rule, false)
}

pub(super) fn draw_path_fill(
  context: &OffscreenCanvasRenderingContext2D,
  scope: &mut v8::PinScope<'_, '_>,
  path: BezPath,
  rule: String,
  transform: Affine,
) {
  if path.is_empty() {
    return;
  }
  let state = context.state.borrow();
  let op = state.global_composite_operation;
  let alpha = state.global_alpha;
  let shadow = has_shadow(&state);
  let shadow_color = state.shadow_color_rgba;
  let shadow_xform = if shadow {
    Some(shadow_transform(&state, transform))
  } else {
    None
  };
  let (brush, brush_transform) = resolve_brush(scope, &state.fill_style, 1.0);
  let fill = if rule == "evenodd" {
    peniko::Fill::EvenOdd
  } else {
    peniko::Fill::NonZero
  };
  drop(state);

  let (width, height) = context.data.dimensions();
  let mut drawing = context.drawing.borrow_mut();
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

pub(super) fn draw_path_stroke(
  context: &OffscreenCanvasRenderingContext2D,
  scope: &mut v8::PinScope<'_, '_>,
  path: BezPath,
  transform: Affine,
  is_path2d: bool,
) {
  if path.is_empty() {
    return;
  }
  let state = context.state.borrow();
  let op = state.global_composite_operation;
  let alpha = state.global_alpha;
  let shadow = has_shadow(&state);
  let shadow_color = state.shadow_color_rgba;
  let shadow_xform = if shadow {
    Some(shadow_transform(&state, transform))
  } else {
    None
  };
  let (brush, brush_transform) = resolve_brush(scope, &state.stroke_style, 1.0);
  let stroke = build_stroke(&state);
  drop(state);

  let path = if is_path2d {
    path
  } else {
    OffscreenCanvasRenderingContext2D::transform_path(
      &path,
      transform.inverse(),
    )
  };

  let (width, height) = context.data.dimensions();
  let mut drawing = context.drawing.borrow_mut();
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

pub(super) fn require_finite(
  values: &[UnrestrictedDouble],
) -> Result<(), Canvas2DError> {
  if values.iter().any(|v| !v.is_finite()) {
    return Err(Canvas2DError::NonFinite);
  }
  Ok(())
}

pub(super) fn require_long(
  scope: &mut v8::PinScope<'_, '_>,
  val: v8::Local<'_, v8::Value>,
) -> Result<i32, Canvas2DError> {
  let n = val.number_value(scope).unwrap_or(f64::NAN);
  if !n.is_finite() {
    return Err(Canvas2DError::NonFinite);
  }
  Ok(n as i32)
}

pub(super) fn parse_fill_stroke_style(
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

// Note: vello (GPU and CPU) applies brush transforms relative to the shape
// transform (the encoded transform is `shape_transform * brush_transform`),
// so gradients use IDENTITY and patterns pass only their own transform to
// end up in user space as the spec requires. The canvas CTM does not need
// to be threaded into the brush transform separately.
pub(super) fn resolve_brush(
  scope: &mut v8::PinScope<'_, '_>,
  style: &FillStrokeStyle,
  global_alpha: f32,
) -> (peniko::Brush, Option<Affine>) {
  match style {
    FillStrokeStyle::Color(c) => {
      let rgba = c.to_rgba8();
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

pub(super) fn apply_cpu_paint(
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

pub(super) fn push_compositing_layer(
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

/// Unconditionally pushes a full-canvas-rect compositing layer with the
/// given blend mode/alpha (unlike push_compositing_layer, which skips
/// pushing anything for the common source-over/alpha=1 case).
pub(super) fn push_full_canvas_layer(
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

pub(super) fn pop_compositing_layer(drawing: &mut DrawingBackend) {
  match drawing {
    DrawingBackend::Vello(scene) => scene.pop_layer(),
    DrawingBackend::VelloCpu(ctx, _) => ctx.pop_layer(),
  }
}

/// Draws a shadow whose alpha follows the source content's own per-pixel
/// alpha, per spec (shadow alpha = source alpha * shadowColor alpha),
/// rather than a solid silhouette of the shape. `draw_source` renders the
/// real content (with its real brush/image) offset by the shadow
/// transform; the result is then tinted with the solid shadow color via
/// a SrcIn layer.
pub(super) fn draw_shadow(
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

/// Pushes a single clip layer with the given fill rule and transform. The
/// CPU backend's `push_clip_layer` has no fill-rule/transform parameters
/// of its own -- it uses whatever was last set via `set_fill_rule()` /
/// `set_transform()` -- so those must be applied first to keep it in sync
/// with the GPU backend.
pub(super) fn push_clip(
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

/// Builds a Stroke reflecting the current line style state
/// (width, cap, join, miter limit, dash pattern). Shared by actual
/// stroke rendering and isPointInStroke() hit-testing so the two stay in
/// sync.
#[inline]
pub(super) fn build_stroke(state: &DrawingState) -> Stroke {
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
pub(super) fn has_shadow(state: &DrawingState) -> bool {
  !is_color_transparent(state.shadow_color_rgba)
    && (state.shadow_blur > 0.0
      || state.shadow_offset_x != 0.0
      || state.shadow_offset_y != 0.0)
}

#[inline]
pub(super) fn shadow_transform(
  state: &DrawingState,
  transform: Affine,
) -> Affine {
  // TODO(petamoriken): apply shadowBlur once Vello GPU supports filter effects
  Affine::translate((state.shadow_offset_x, state.shadow_offset_y)) * transform
}

pub(super) fn fill_on(
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

pub(super) fn stroke_on(
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

pub(super) fn apply_clip(
  context: &OffscreenCanvasRenderingContext2D,
  path: BezPath,
  rule: String,
  transform: Affine,
) {
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
  push_clip(&mut context.drawing.borrow_mut(), fill, transform, &path);
  let mut state = context.state.borrow_mut();
  context.clip_stack.borrow_mut().truncate(state.clip_depth);
  context.clip_stack.borrow_mut().push(ClipEntry {
    path,
    rule,
    transform,
  });
  state.clip_depth += 1;
}
