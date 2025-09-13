// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::OnceCell;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::op2;

use crate::Instance;
use crate::error::GPUGenericError;

pub struct GPUCommandBuffer {
  pub instance: Instance,
  pub id: wgpu_core::id::CommandBufferId,
  pub label: String,

  pub consumed: OnceCell<()>,
}

impl Drop for GPUCommandBuffer {
  fn drop(&mut self) {
    if self.consumed.get().is_none() {
      self.instance.command_buffer_drop(self.id);
    }
  }
}

impl deno_core::webidl::WebIdlInterfaceConverter for GPUCommandBuffer {
  const NAME: &'static str = "GPUCommandBuffer";
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUCommandBuffer {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUCommandBuffer"
  }
}

#[op2]
impl GPUCommandBuffer {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUCommandBuffer, GPUGenericError> {
    Err(GPUGenericError::InvalidConstructor)
  }

  #[getter]
  #[string]
  fn label(&self) -> String {
    self.label.clone()
  }
  #[setter]
  #[string]
  fn label(&self, #[webidl] _label: String) {
    // TODO(@crowlKats): no-op, needs wpgu to implement changing the label
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUCommandBufferDescriptor {
  #[webidl(default = String::new())]
  pub label: String,
}
