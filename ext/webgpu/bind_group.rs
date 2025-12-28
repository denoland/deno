// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::cppgc::Ref;
use deno_core::op2;
use deno_core::v8::Local;
use deno_core::v8::PinScope;
use deno_core::v8::Value;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlInterfaceConverter;

use crate::Instance;
use crate::buffer::GPUBuffer;
use crate::error::GPUGenericError;
use crate::sampler::GPUSampler;
use crate::texture::GPUTextureView;

pub struct GPUBindGroup {
  pub instance: Instance,
  pub id: wgpu_core::id::BindGroupId,
  pub label: String,
}

impl Drop for GPUBindGroup {
  fn drop(&mut self) {
    self.instance.bind_group_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUBindGroup {
  const NAME: &'static str = "GPUBindGroup";
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUBindGroup {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUBindGroup"
  }
}

#[op2]
impl GPUBindGroup {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUBindGroup, GPUGenericError> {
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
pub(crate) struct GPUBindGroupDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub layout: Ref<super::bind_group_layout::GPUBindGroupLayout>,
  pub entries: Vec<GPUBindGroupEntry>,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUBindGroupEntry {
  #[options(enforce_range = true)]
  pub binding: u32,
  pub resource: GPUBindingResource,
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUBufferBinding {
  pub buffer: Ref<GPUBuffer>,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  pub offset: u64,
  #[options(enforce_range = true)]
  pub size: Option<u64>,
}

pub(crate) enum GPUBindingResource {
  Sampler(Ref<GPUSampler>),
  TextureView(Ref<GPUTextureView>),
  BufferBinding(GPUBufferBinding),
}

impl<'a> WebIdlConverter<'a> for GPUBindingResource {
  type Options = ();

  fn convert<'b>(
    scope: &mut PinScope<'a, '_>,
    value: Local<'a, Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    <Ref<GPUSampler>>::convert(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
      options,
    )
    .map(Self::Sampler)
    .or_else(|_| {
      <Ref<GPUTextureView>>::convert(
        scope,
        value,
        prefix.clone(),
        context.borrowed(),
        options,
      )
      .map(Self::TextureView)
    })
    .or_else(|_| {
      GPUBufferBinding::convert(scope, value, prefix, context, options)
        .map(Self::BufferBinding)
    })
  }
}
