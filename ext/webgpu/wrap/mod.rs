// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::cppgc::SameObject;
use deno_core::op2;
use deno_core::GarbageCollected;
use deno_core::OpState;
use wgpu_types::PowerPreference;

use crate::Instance;

pub mod adapter;
pub mod bind_group;
pub mod bind_group_layout;
pub mod buffer;
pub mod command_buffer;
pub mod command_encoder;
pub mod compute_pass;
pub mod compute_pipeline;
pub mod device;
pub mod error;
pub mod pipeline_layout;
pub mod query_set;
pub mod queue;
pub mod render_bundle;
pub mod render_pass;
pub mod render_pipeline;
pub mod sampler;
pub mod shader;
pub mod texture;
pub mod webidl;

#[op2]
#[cppgc]
pub fn create_gpu() -> GPU {
  GPU
}

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
      |s| wgpu_core::instance::parse_backends_from_comma_list(&s),
    );
    let instance = if let Some(instance) = state.try_borrow::<Instance>() {
      instance
    } else {
      state.put(Arc::new(wgpu_core::global::Global::new(
        "webgpu",
        wgpu_types::InstanceDescriptor {
          backends,
          flags: wgpu_types::InstanceFlags::from_build_config(),
          dx12_shader_compiler: wgpu_types::Dx12Compiler::Fxc,
          gles_minor_version: wgpu_types::Gles3MinorVersion::default(),
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
