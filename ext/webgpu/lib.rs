// Copyright 2018-2026 the Deno authors. MIT license.

#![cfg(not(target_arch = "wasm32"))]
#![warn(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::v8;
use serde::Deserialize as _;
use serde::de::IntoDeserializer;
pub use wgpu_core;
pub use wgpu_types;
use wgpu_types::PowerPreference;

use crate::error::GPUGenericError;

pub mod adapter;
mod bind_group;
mod bind_group_layout;
pub mod buffer;
pub mod canvas;
mod command_buffer;
mod command_encoder;
mod compute_pass;
mod compute_pipeline;
pub mod device;
pub mod error;
mod pipeline_layout;
mod query_set;
mod queue;
mod render_bundle;
mod render_pass;
mod render_pipeline;
mod sampler;
mod shader;
pub mod texture;
mod webidl;

pub const UNSTABLE_FEATURE_NAME: &str = "webgpu";

pub const DX12_COMPILER_ENV_VAR: &str = "DENO_WEBGPU_DX12_COMPILER";

#[allow(clippy::print_stdout, reason = "prints linker flags for build scripts")]
pub fn print_linker_flags(name: &str) {
  if cfg!(windows) {
    // these dls load slowly, so delay loading them
    let dlls = [
      // webgpu
      "d3dcompiler_47",
      "OPENGL32",
      // network related functions
      "iphlpapi",
      // restart manager (only needed on the `deno clean` locked-file path)
      "rstrtmgr",
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
    WGSLLanguageFeatures,
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
    shader::GPUCompilationInfo,
    shader::GPUCompilationMessage,
    shader::GPUShaderModule,
    adapter::GPUSupportedFeatures,
    adapter::GPUSupportedLimits,
    texture::GPUTexture,
    texture::GPUTextureView,
    texture::GPUExternalTexture,
    canvas::GPUCanvasContext,
  ],
  lazy_loaded_esm = ["01_webgpu.js"],
  lazy_loaded_js = ["00_init.js"],
);

#[op2]
#[cppgc]
pub fn op_create_gpu(
  state: &mut OpState,
  scope: &mut v8::PinScope<'_, '_>,
  webidl_brand: v8::Local<v8::Value>,
  set_event_target_data: v8::Local<v8::Value>,
  uncaptured_error_event_class: v8::Local<v8::Value>,
  pipeline_error_class: v8::Local<v8::Value>,
) -> GPU {
  state.put(EventTargetSetup {
    brand: v8::Global::new(scope, webidl_brand),
    set_event_target_data: v8::Global::new(scope, set_event_target_data),
  });
  state.put(ErrorEventClass(v8::Global::new(
    scope,
    uncaptured_error_event_class,
  )));
  state.put(PipelineErrorClass(v8::Global::new(
    scope,
    pipeline_error_class,
  )));
  GPU {
    wgsl_language_features: SameObject::new(),
  }
}

struct EventTargetSetup {
  brand: v8::Global<v8::Value>,
  set_event_target_data: v8::Global<v8::Value>,
}
struct ErrorEventClass(v8::Global<v8::Value>);
struct PipelineErrorClass(v8::Global<v8::Value>);

pub struct GPU {
  pub wgsl_language_features: SameObject<WGSLLanguageFeatures>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPU {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPU"
  }
}

#[op2]
impl GPU {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPU, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[cppgc]
  async fn request_adapter(
    &self,
    state: Rc<RefCell<OpState>>,
    #[webidl] options: adapter::GPURequestAdapterOptions,
  ) -> Option<adapter::GPUAdapter> {
    let mut state = state.borrow_mut();

    let (backends, instance) = get_or_init_instance(&mut state, &options)?;

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
      instance,
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

  #[getter]
  fn wgslLanguageFeatures(
    &self,
    scope: &mut v8::PinScope<'_, '_>,
  ) -> v8::Global<v8::Object> {
    self
      .wgsl_language_features
      .get(scope, WGSLLanguageFeatures::new)
  }
}

pub struct WGSLLanguageFeatures(v8::Global<v8::Value>);

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for WGSLLanguageFeatures {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"WGSLLanguageFeatures"
  }
}

impl WGSLLanguageFeatures {
  pub fn new(scope: &mut v8::PinScope<'_, '_>) -> Self {
    use wgpu_core::naga::front::wgsl::ImplementedLanguageExtension;

    let set = v8::Set::new(scope);
    for ext in ImplementedLanguageExtension::all() {
      let key = v8::String::new(scope, ext.to_ident()).unwrap();
      set.add(scope, key.into());
    }
    Self(v8::Global::new(scope, <v8::Local<v8::Value>>::from(set)))
  }
}

#[op2]
impl WGSLLanguageFeatures {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<WGSLLanguageFeatures, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[symbol("setlike_set")]
  fn set(&self) -> v8::Global<v8::Value> {
    self.0.clone()
  }
}

fn transform_label<'a>(label: String) -> Option<std::borrow::Cow<'a, str>> {
  if label.is_empty() {
    None
  } else {
    Some(std::borrow::Cow::Owned(label))
  }
}

pub fn get_or_init_instance(
  state: &mut OpState,
  options: &adapter::GPURequestAdapterOptions,
) -> Option<(wgpu_types::Backends, Instance)> {
  let dx12_compiler = std::env::var(DX12_COMPILER_ENV_VAR)
    .ok()
    .and_then(|s| s.parse().ok());
  let backends = std::env::var("DENO_WEBGPU_BACKEND").map_or_else(
    |_| wgpu_types::Backends::all(),
    |s| wgpu_types::Backends::from_comma_list(&s),
  );

  let instance = if let Some(instance) = state.try_borrow::<Instance>() {
    instance.clone()
  } else {
    state.put(Arc::new(wgpu_core::global::Global::new(
      "webgpu",
      wgpu_types::InstanceDescriptor {
        backends,
        flags: wgpu_types::InstanceFlags::from_build_config(),
        memory_budget_thresholds: wgpu_types::MemoryBudgetThresholds {
          for_resource_creation: Some(97),
          for_device_loss: Some(99),
        },
        backend_options: wgpu_types::BackendOptions {
          dx12: wgpu_types::Dx12BackendOptions {
            shader_compiler: dx12_compiler
              .unwrap_or(wgpu_types::Dx12Compiler::Fxc),
            ..Default::default()
          },
          gl: wgpu_types::GlBackendOptions::default(),
          noop: wgpu_types::NoopBackendOptions::default(),
        },
        display: None,
      },
      None,
    )));
    state.borrow::<Instance>().clone()
  };

  // Check that the feature level string is valid.
  // `wgpu` does not support compatibility-level adapters. As permitted
  // by the spec, we always return a core-level adapter.
  wgpu_types::FeatureLevel::deserialize(IntoDeserializer::<
    serde::de::value::Error,
  >::into_deserializer(
    options.feature_level.as_str()
  ))
  .ok()?;

  Some((backends, instance))
}
