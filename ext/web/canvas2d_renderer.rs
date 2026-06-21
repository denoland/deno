// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Mutex;

use vello::AaConfig;
use vello::AaSupport;
use vello::RendererOptions;
use vello::peniko;
pub use vello::wgpu;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RenderError(#[from] vello::Error);

/// GPU compute backend — uses a real GPU hardware adapter.
pub struct GpuRenderer {
  device: wgpu::Device,
  queue: wgpu::Queue,
  renderer: Mutex<vello::Renderer>,
}

/// Hybrid software backend — runs vello's GPU compute pipeline on a wgpu
/// software adapter (not a CPU rasterizer; the GPU shaders are emulated).
///
/// This requires wgpu to be available and find a software adapter.
/// On macOS, Metal has no software fallback, so this path is effectively
/// Windows-only (WARP) or Linux with lavapipe/llvmpipe.
pub struct HybridRenderer {
  device: wgpu::Device,
  queue: wgpu::Queue,
  // TODO(petamoriken): Replace this software-adapter approach with the dedicated
  // `vello_hybrid::Renderer` (a sparse-strips renderer that offloads work to the
  // GPU) once it stabilizes.
  renderer: Mutex<vello::Renderer>,
}

/// Pure-CPU backend — uses vello_cpu::RenderContext with no wgpu dependency.
/// Always available; used as the final fallback when wgpu cannot be initialized.
pub struct CpuRenderer;

pub enum DenoCanvasBackend {
  Gpu(GpuRenderer),
  Hybrid(HybridRenderer),
  Cpu(CpuRenderer),
}

pub type SharedRenderer =
  std::sync::Arc<std::sync::OnceLock<Option<DenoCanvasBackend>>>;

/// Initializes the best available canvas rendering backend.
/// Always returns `Some` — falls back to pure-CPU if wgpu is unavailable.
pub fn init_canvas_renderer() -> Option<DenoCanvasBackend> {
  try_init_gpu()
    .map(DenoCanvasBackend::Gpu)
    .or_else(|| try_init_hybrid().map(DenoCanvasBackend::Hybrid))
    .or(Some(DenoCanvasBackend::Cpu(CpuRenderer)))
}

fn try_init_gpu() -> Option<GpuRenderer> {
  if wgpu::Instance::enabled_backend_features().is_empty() {
    return None;
  }
  futures::executor::block_on(async {
    let instance = wgpu::Instance::default();
    let adapter = instance
      .request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: false,
        ..Default::default()
      })
      .await
      .ok()?;
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

fn try_init_hybrid() -> Option<HybridRenderer> {
  if wgpu::Instance::enabled_backend_features().is_empty() {
    return None;
  }
  futures::executor::block_on(async {
    let instance = wgpu::Instance::default();
    let adapter = instance
      .request_adapter(&wgpu::RequestAdapterOptions {
        force_fallback_adapter: true,
        ..Default::default()
      })
      .await
      .ok()?;
    let (device, queue) = adapter
      .request_device(&wgpu::DeviceDescriptor::default())
      .await
      .ok()?;
    let renderer = vello::Renderer::new(
      &device,
      RendererOptions {
        use_cpu: true,
        antialiasing_support: AaSupport::area_only(),
        ..Default::default()
      },
    )
    .ok()?;
    Some(HybridRenderer {
      device,
      queue,
      renderer: Mutex::new(renderer),
    })
  })
}

/// Renders a vello Scene directly to a caller-provided TextureView.
///
/// Unlike [`render_scene`], this does not perform a CPU readback.
/// The view must have been created from a texture that belongs to the same
/// wgpu device as the backend.
/// Only valid for `Gpu` and `Hybrid` backends.
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

/// Renders a vello Scene to RGBA8 bytes.
/// Only valid for `Gpu` and `Hybrid` backends; the `Cpu` backend renders
/// directly via `vello_cpu::RenderContext::render_to_buffer` (see the
/// `DrawingBackend::VelloCpu` arm in `canvas2d.rs`).
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
    DenoCanvasBackend::Hybrid(r) => (&r.device, &r.queue, &r.renderer),
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
