// Copyright 2018-2025 the Deno authors. MIT license.
#![cfg(not(target_arch = "wasm32"))]
#![warn(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use deno_core::OpState;
pub use wgpu_core;
pub use wgpu_types;
use wgpu_types::PowerPreference;

mod adapter;
mod bind_group;
mod bind_group_layout;
pub mod buffer;
mod byow;
mod command_buffer;
mod command_encoder;
mod compute_pass;
mod compute_pipeline;
mod device;
pub mod error;
mod pipeline_layout;
mod query_set;
mod queue;
mod render_bundle;
mod render_pass;
mod render_pipeline;
mod sampler;
mod shader;
mod surface;
pub mod texture;
mod webidl;

pub const UNSTABLE_FEATURE_NAME: &str = "webgpu";

#[allow(clippy::print_stdout)]
pub fn print_linker_flags(name: &str) {
  if cfg!(windows) {
    // these dls load slowly, so delay loading them
    let dlls = [
      // webgpu
      "d3dcompiler_47",
      "OPENGL32",
      // network related functions
      "iphlpapi",
    ];
    for dll in dlls {
      println!("cargo:rustc-link-arg-bin={name}=/delayload:{dll}.dll");
    }
    // enable delay loading
    println!("cargo:rustc-link-arg-bin={name}=delayimp.lib");
  }
}

pub type Instance = Arc<wgpu_core::global::Global>;

deno_core::extension!(
  deno_webgpu,
  deps = [deno_webidl, deno_web],
  ops = [
    op_create_gpu,
    device::op_webgpu_device_start_capture,
    device::op_webgpu_device_stop_capture,
  ],
  objects = [
    GPU,
    adapter::GPUAdapter,
    adapter::GPUAdapterInfo,
    bind_group::GPUBindGroup,
    bind_group_layout::GPUBindGroupLayout,
    buffer::GPUBuffer,
    command_buffer::GPUCommandBuffer,
    command_encoder::GPUCommandEncoder,
    compute_pass::GPUComputePassEncoder,
    compute_pipeline::GPUComputePipeline,
    device::GPUDevice,
    device::GPUDeviceLostInfo,
    pipeline_layout::GPUPipelineLayout,
    query_set::GPUQuerySet,
    queue::GPUQueue,
    render_bundle::GPURenderBundle,
    render_bundle::GPURenderBundleEncoder,
    render_pass::GPURenderPassEncoder,
    render_pipeline::GPURenderPipeline,
    sampler::GPUSampler,
    shader::GPUShaderModule,
    adapter::GPUSupportedFeatures,
    adapter::GPUSupportedLimits,
    texture::GPUTexture,
    texture::GPUTextureView,
    byow::UnsafeWindowSurface,
    surface::GPUCanvasContext,
  ],
  esm = ["00_init.js", "02_surface.js"],
  lazy_loaded_esm = ["01_webgpu.js"],
);

#[op2]
#[cppgc]
pub fn op_create_gpu(
  state: &mut OpState,
  scope: &mut v8::HandleScope,
  webidl_brand: v8::Local<v8::Value>,
  set_event_target_data: v8::Local<v8::Value>,
  error_event_class: v8::Local<v8::Value>,
) -> GPU {
  state.put(EventTargetSetup {
    brand: v8::Global::new(scope, webidl_brand),
    set_event_target_data: v8::Global::new(scope, set_event_target_data),
  });
  state.put(ErrorEventClass(v8::Global::new(scope, error_event_class)));
  GPU
}

struct EventTargetSetup {
  brand: v8::Global<v8::Value>,
  set_event_target_data: v8::Global<v8::Value>,
}
struct ErrorEventClass(v8::Global<v8::Value>);

pub struct GPU;

impl GarbageCollected for GPU {}

#[op2]
impl GPU {
  #[async_method]
  #[cppgc]
  async fn request_adapter(
    &self,
    state: Rc<RefCell<OpState>>,
    #[webidl] options: adapter::GPURequestAdapterOptions,
  ) -> Option<adapter::GPUAdapter> {
    let mut state = state.borrow_mut();

    let backends = std::env::var("DENO_WEBGPU_BACKEND").map_or_else(
      |_| wgpu_types::Backends::all(),
      |s| wgpu_types::Backends::from_comma_list(&s),
    );
    let instance = if let Some(instance) = state.try_borrow::<Instance>() {
      instance
    } else {
      state.put(Arc::new(wgpu_core::global::Global::new(
        "webgpu",
        &wgpu_types::InstanceDescriptor {
          backends,
          flags: wgpu_types::InstanceFlags::from_build_config(),
          backend_options: wgpu_types::BackendOptions {
            dx12: wgpu_types::Dx12BackendOptions {
              shader_compiler: wgpu_types::Dx12Compiler::Fxc,
            },
            gl: wgpu_types::GlBackendOptions::default(),
          },
        },
      )));
      state.borrow::<Instance>()
    };

    let descriptor = wgpu_core::instance::RequestAdapterOptions {
      power_preference: options
        .power_preference
        .map(|pp| match pp {
          adapter::GPUPowerPreference::LowPower => PowerPreference::LowPower,
          adapter::GPUPowerPreference::HighPerformance => {
            PowerPreference::HighPerformance
          }
        })
        .unwrap_or_default(),
      force_fallback_adapter: options.force_fallback_adapter,
      compatible_surface: None, // windowless
    };
    let id = instance.request_adapter(&descriptor, backends, None).ok()?;

    Some(adapter::GPUAdapter {
      instance: instance.clone(),
      features: SameObject::new(),
      limits: SameObject::new(),
      info: Rc::new(SameObject::new()),
      id,
    })
  }

  #[string]
  fn getPreferredCanvasFormat(&self) -> &'static str {
    // https://github.com/mozilla/gecko-dev/blob/b75080bb8b11844d18cb5f9ac6e68a866ef8e243/dom/webgpu/Instance.h#L42-L47
    if cfg!(target_os = "android") {
      texture::GPUTextureFormat::Rgba8unorm.as_str()
    } else {
      texture::GPUTextureFormat::Bgra8unorm.as_str()
    }
  }
}

fn transform_label<'a>(label: String) -> Option<std::borrow::Cow<'a, str>> {
  if label.is_empty() {
    None
  } else {
    Some(std::borrow::Cow::Owned(label))
  }
}
