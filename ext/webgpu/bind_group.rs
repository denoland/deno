// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_core::cppgc::Ptr;
use deno_core::op2;
use deno_core::v8::HandleScope;
use deno_core::v8::Local;
use deno_core::v8::Value;
use deno_core::webidl::ContextFn;
use deno_core::webidl::WebIdlConverter;
use deno_core::webidl::WebIdlError;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;

use crate::buffer::GPUBuffer;
use crate::sampler::GPUSampler;
use crate::texture::GPUTextureView;
use crate::Instance;

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

impl GarbageCollected for GPUBindGroup {}

#[op2]
impl GPUBindGroup {
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

  pub layout: Ptr<super::bind_group_layout::GPUBindGroupLayout>,
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
  pub buffer: Ptr<GPUBuffer>,
  #[webidl(default = 0)]
  #[options(enforce_range = true)]
  pub offset: u64,
  #[options(enforce_range = true)]
  pub size: Option<u64>,
}

pub(crate) enum GPUBindingResource {
  Sampler(Ptr<GPUSampler>),
  TextureView(Ptr<GPUTextureView>),
  BufferBinding(GPUBufferBinding),
}

impl<'a> WebIdlConverter<'a> for GPUBindingResource {
  type Options = ();

  fn convert<'b>(
    scope: &mut HandleScope<'a>,
    value: Local<'a, Value>,
    prefix: Cow<'static, str>,
    context: ContextFn<'b>,
    options: &Self::Options,
  ) -> Result<Self, WebIdlError> {
    <Ptr<GPUSampler>>::convert(
      scope,
      value,
      prefix.clone(),
      context.borrowed(),
      options,
    )
    .map(Self::Sampler)
    .or_else(|_| {
      <Ptr<GPUTextureView>>::convert(
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
