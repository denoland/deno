// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Mutex;

use vello::AaConfig;
use vello::AaSupport;
use vello::RendererOptions;
use vello::kurbo::Affine;
use vello::kurbo::BezPath;
use vello::kurbo::Rect;
use vello::kurbo::Shape;
use vello::kurbo::Stroke;
use vello::peniko;
pub use vello::wgpu;

use crate::canvas2d::error::Canvas2DError;
use crate::canvas2d::error::RenderError;

/// GPU compute backend — uses a real GPU hardware adapter.
pub struct GpuRenderer {
  device: wgpu::Device,
  queue: wgpu::Queue,
  renderer: Mutex<vello::Renderer>,
}

impl GpuRenderer {
  /// The largest 2D texture dimension this device supports.
  pub(crate) fn max_texture_dimension_2d(&self) -> u32 {
    self.device.limits().max_texture_dimension_2d
  }
}

/// Pure-CPU backend — uses vello_cpu::RenderContext with no wgpu dependency.
/// Always available; used as the final fallback when wgpu cannot be initialized.
pub struct CpuRenderer;

pub enum DenoCanvasBackend {
  Gpu(Box<GpuRenderer>),
  Cpu(CpuRenderer),
}

pub type SharedRenderer =
  std::sync::Arc<std::sync::OnceLock<Option<DenoCanvasBackend>>>;

// Gpu (`vello::Scene`) and Cpu (`vello_cpu::RenderContext`) do not share a
// scene type. Revisit if Vello unifies them.
pub(super) enum DrawingBackend {
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
        let (width, height) = clamp_resize_dimensions_to_u16(width, height);
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
        let (width, height) = clamp_resize_dimensions_to_u16(width, height);
        *ctx = vello_cpu::RenderContext::new(width, height);
        **resources = vello_cpu::Resources::new();
      }
    }
  }

  #[inline]
  pub(super) fn is_gpu(&self) -> bool {
    matches!(self, Self::Vello(_))
  }

  #[inline]
  pub(super) fn fill(
    &mut self,
    fill: peniko::Fill,
    transform: Affine,
    brush: peniko::Brush,
    brush_transform: Option<Affine>,
    shape: &impl Shape,
  ) {
    match self {
      Self::Vello(scene) => {
        scene.fill(fill, transform, &brush, brush_transform, shape);
      }
      Self::VelloCpu(ctx, _) => {
        apply_cpu_paint(ctx, brush, brush_transform);
        ctx.set_fill_rule(cpu_fill_rule(fill));
        ctx.set_transform(transform);
        let path: BezPath = shape.path_elements(0.1).collect();
        ctx.fill_path(&path);
      }
    }
  }

  #[inline]
  pub(super) fn stroke(
    &mut self,
    stroke: &Stroke,
    transform: Affine,
    brush: peniko::Brush,
    brush_transform: Option<Affine>,
    path: &BezPath,
  ) {
    match self {
      Self::Vello(scene) => {
        scene.stroke(stroke, transform, &brush, brush_transform, path);
      }
      Self::VelloCpu(ctx, _) => {
        apply_cpu_paint(ctx, brush, brush_transform);
        ctx.set_stroke(stroke.clone());
        ctx.set_transform(transform);
        ctx.stroke_path(path);
      }
    }
  }

  #[inline]
  pub(super) fn push_layer(
    &mut self,
    blend: peniko::BlendMode,
    alpha: f32,
    width: u32,
    height: u32,
  ) {
    match self {
      Self::Vello(scene) => {
        let clip = Rect::new(0.0, 0.0, width as f64, height as f64);
        scene.push_layer(
          peniko::Fill::NonZero,
          blend,
          alpha,
          Affine::IDENTITY,
          &clip,
        );
      }
      Self::VelloCpu(ctx, _) => {
        ctx.push_layer(None, Some(blend), Some(alpha), None, None);
      }
    }
  }

  #[inline]
  pub(super) fn pop_layer(&mut self) {
    match self {
      Self::Vello(scene) => scene.pop_layer(),
      Self::VelloCpu(ctx, _) => ctx.pop_layer(),
    }
  }

  #[inline]
  pub(super) fn push_clip(
    &mut self,
    fill: peniko::Fill,
    transform: Affine,
    path: &BezPath,
  ) {
    match self {
      Self::Vello(scene) => {
        scene.push_clip_layer(fill, transform, path);
      }
      Self::VelloCpu(ctx, _) => {
        ctx.set_fill_rule(cpu_fill_rule(fill));
        ctx.set_transform(transform);
        ctx.push_clip_layer(path);
      }
    }
  }

  pub(super) fn fill_glyphs(
    &mut self,
    font: &peniko::FontData,
    font_size: f32,
    transform: Affine,
    brush: &peniko::Brush,
    brush_transform: Option<Affine>,
    glyphs: &[(u32, f32, f32)],
  ) {
    match self {
      Self::Vello(scene) => {
        let mut glyph_draw = scene
          .draw_glyphs(font)
          .font_size(font_size)
          .transform(transform)
          .brush(brush);
        if let Some(bt) = brush_transform {
          glyph_draw = glyph_draw.brush_transform(Some(bt));
        }
        glyph_draw.draw(
          peniko::Fill::NonZero,
          glyphs
            .iter()
            .copied()
            .map(|(id, x, y)| vello::Glyph { id, x, y }),
        );
      }
      Self::VelloCpu(ctx, resources) => {
        apply_cpu_paint(ctx, brush.clone(), brush_transform);
        ctx.set_transform(transform);
        ctx
          .glyph_run(resources, font)
          .font_size(font_size)
          .fill_glyphs(
            glyphs.iter().copied().map(|(id, x, y)| vello_cpu::Glyph {
              id,
              x,
              y,
            }),
          );
      }
    }
  }

  /// Premultiplied RGBA8. `None` when the GPU backend has no renderer yet.
  pub(super) fn render_to_rgba(
    &mut self,
    renderer: Option<&DenoCanvasBackend>,
    width: u32,
    height: u32,
    base_color: peniko::Color,
  ) -> Result<Option<Vec<u8>>, Canvas2DError> {
    match self {
      Self::Vello(scene) => {
        let Some(renderer) = renderer else {
          return Ok(None);
        };
        Ok(Some(render_scene(
          renderer, scene, width, height, base_color,
        )?))
      }
      Self::VelloCpu(ctx, resources) => {
        let pixel_count = (width as usize) * (height as usize);
        let mut buf = vec![0u8; pixel_count * 4];
        ctx.render_to_buffer(
          resources,
          &mut buf,
          width as u16,
          height as u16,
          vello_cpu::RenderMode::OptimizeSpeed,
        );
        Ok(Some(buf))
      }
    }
  }

  pub(super) fn render_to_texture_view(
    &self,
    renderer: Option<&DenoCanvasBackend>,
    view: &wgpu::TextureView,
    width: u32,
    height: u32,
    base_color: peniko::Color,
  ) -> Result<(), Canvas2DError> {
    match self {
      Self::Vello(scene) => {
        if let Some(renderer) = renderer {
          render_scene_to_texture_view(
            renderer, scene, view, width, height, base_color,
          )?;
        }
        Ok(())
      }
      // Surface 2D needs GPU. If wired up, skip CPU fallback for these.
      Self::VelloCpu(_, _) => {
        unreachable!("render_to_texture_view called on Cpu backend")
      }
    }
  }
}

/// Clamp to `u16` so the cast cannot wrap. Only `resize()` reaches here.
#[inline]
fn clamp_resize_dimensions_to_u16(width: u32, height: u32) -> (u16, u16) {
  (
    u16::try_from(width).unwrap_or(u16::MAX),
    u16::try_from(height).unwrap_or(u16::MAX),
  )
}

#[inline]
fn cpu_fill_rule(fill: peniko::Fill) -> vello_cpu::peniko::Fill {
  if fill == peniko::Fill::EvenOdd {
    vello_cpu::peniko::Fill::EvenOdd
  } else {
    vello_cpu::peniko::Fill::NonZero
  }
}

#[inline]
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

/// Initializes the best available canvas rendering backend.
/// Always returns `Some` — falls back to pure-CPU if wgpu is unavailable.
pub fn init_canvas_renderer() -> Option<DenoCanvasBackend> {
  try_init_gpu()
    .map(|renderer| DenoCanvasBackend::Gpu(Box::new(renderer)))
    .or(Some(DenoCanvasBackend::Cpu(CpuRenderer)))
}

fn try_init_gpu() -> Option<GpuRenderer> {
  if wgpu::Instance::enabled_backend_features().is_empty() {
    return None;
  }
  futures::executor::block_on(async {
    let instance = wgpu::Instance::default();
    let adapter = instance.request_adapter(&Default::default()).await.ok()?;
    if !adapter
      .get_downlevel_capabilities()
      .flags
      .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
    {
      return None;
    }
    if !adapter
      .limits()
      .check_limits(&wgpu::Limits::downlevel_defaults())
    {
      return None;
    }
    let (device, queue) = adapter
      .request_device(&wgpu::DeviceDescriptor::default())
      .await
      .ok()?;
    let renderer = vello::Renderer::new(
      &device,
      RendererOptions {
        use_cpu: false,
        antialiasing_support: AaSupport::area_only(),
        ..Default::default()
      },
    )
    .ok()?;
    Some(GpuRenderer {
      device,
      queue,
      renderer: Mutex::new(renderer),
    })
  })
}

/// Render a vello Scene to a TextureView on this backend's device.
/// No CPU readback. GPU backends only.
pub fn render_scene_to_texture_view(
  backend: &DenoCanvasBackend,
  scene: &vello::Scene,
  view: &wgpu::TextureView,
  width: u32,
  height: u32,
  base_color: peniko::Color,
) -> Result<(), RenderError> {
  let (device, queue, renderer) = wgpu_renderer(backend);
  render_wgpu_to_view(
    device, queue, renderer, scene, view, width, height, base_color,
  )
}

/// Render a vello Scene to RGBA8. GPU backends only.
pub fn render_scene(
  backend: &DenoCanvasBackend,
  scene: &vello::Scene,
  width: u32,
  height: u32,
  base_color: peniko::Color,
) -> Result<Vec<u8>, RenderError> {
  let (device, queue, renderer) = wgpu_renderer(backend);
  render_wgpu(device, queue, renderer, scene, width, height, base_color)
}

fn wgpu_renderer(
  backend: &DenoCanvasBackend,
) -> (&wgpu::Device, &wgpu::Queue, &Mutex<vello::Renderer>) {
  match backend {
    DenoCanvasBackend::Gpu(r) => (&r.device, &r.queue, &r.renderer),
    DenoCanvasBackend::Cpu(_) => {
      unreachable!("wgpu_renderer called on Cpu backend")
    }
  }
}

#[allow(
  clippy::too_many_arguments,
  reason = "rendering function requires all parameters"
)]
fn render_wgpu_to_view(
  device: &wgpu::Device,
  queue: &wgpu::Queue,
  renderer: &Mutex<vello::Renderer>,
  scene: &vello::Scene,
  view: &wgpu::TextureView,
  width: u32,
  height: u32,
  base_color: peniko::Color,
) -> Result<(), RenderError> {
  renderer
    .lock()
    .unwrap()
    .render_to_texture(
      device,
      queue,
      scene,
      view,
      &vello::RenderParams {
        base_color,
        width,
        height,
        antialiasing_method: AaConfig::Area,
      },
    )
    .map_err(RenderError::from)
}

fn render_wgpu(
  device: &wgpu::Device,
  queue: &wgpu::Queue,
  renderer: &Mutex<vello::Renderer>,
  scene: &vello::Scene,
  width: u32,
  height: u32,
  base_color: peniko::Color,
) -> Result<Vec<u8>, RenderError> {
  let texture = device.create_texture(&wgpu::TextureDescriptor {
    label: Some("canvas2d_render_target"),
    size: wgpu::Extent3d {
      width,
      height,
      depth_or_array_layers: 1,
    },
    mip_level_count: 1,
    sample_count: 1,
    dimension: wgpu::TextureDimension::D2,
    format: wgpu::TextureFormat::Rgba8Unorm,
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
      | wgpu::TextureUsages::COPY_SRC
      | wgpu::TextureUsages::STORAGE_BINDING,
    view_formats: &[],
  });
  let view = texture.create_view(&Default::default());

  renderer
    .lock()
    .unwrap()
    .render_to_texture(
      device,
      queue,
      scene,
      &view,
      &vello::RenderParams {
        base_color,
        width,
        height,
        antialiasing_method: AaConfig::Area,
      },
    )
    .map_err(RenderError::from)?;

  // bytes_per_row must be aligned to COPY_BYTES_PER_ROW_ALIGNMENT (256).
  let unaligned_bytes_per_row = width * 4;
  let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
  let bytes_per_row = unaligned_bytes_per_row.div_ceil(align) * align;

  let buffer_size = (bytes_per_row * height) as u64;
  let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("canvas2d_readback"),
    size: buffer_size,
    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
  });

  let mut encoder =
    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
      label: Some("canvas2d_readback_encoder"),
    });
  encoder.copy_texture_to_buffer(
    texture.as_image_copy(),
    wgpu::TexelCopyBufferInfo {
      buffer: &readback_buffer,
      layout: wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(bytes_per_row),
        rows_per_image: None,
      },
    },
    wgpu::Extent3d {
      width,
      height,
      depth_or_array_layers: 1,
    },
  );
  queue.submit(Some(encoder.finish()));

  let slice = readback_buffer.slice(..);
  slice.map_async(wgpu::MapMode::Read, |_| {});
  let _ = device.poll(wgpu::PollType::wait_indefinitely());

  let data = slice.get_mapped_range();
  // Strip row padding if bytes_per_row was rounded up.
  if bytes_per_row == unaligned_bytes_per_row {
    Ok(data.to_vec())
  } else {
    let mut out =
      Vec::with_capacity((unaligned_bytes_per_row * height) as usize);
    for row in 0..height {
      let start = (row * bytes_per_row) as usize;
      let end = start + unaligned_bytes_per_row as usize;
      out.extend_from_slice(&data[start..end]);
    }
    Ok(out)
  }
}
