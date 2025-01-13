// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::op2;
use deno_core::webidl::WebIdlInterfaceConverter;
use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_error::JsErrorBox;

pub struct GPUQuerySet {
  pub id: wgpu_core::id::QuerySetId,
  pub r#type: GPUQueryType,
  pub count: u32,
  pub label: String,
}

impl WebIdlInterfaceConverter for GPUQuerySet {
  const NAME: &'static str = "GPUQuerySet";
}

impl GarbageCollected for GPUQuerySet {}

#[op2]
impl GPUQuerySet {
  crate::with_label!();

  #[fast]
  fn destroy(&self) -> Result<(), JsErrorBox> {
    Err(JsErrorBox::generic(
      "This operation is currently not supported",
    ))
  }

  #[getter]
  #[string]
  fn r#type(&self) -> &'static str {
    self.r#type.as_str()
  }

  #[getter]
  fn count(&self) -> u32 {
    self.count
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUQuerySetDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub r#type: GPUQueryType,
  #[options(enforce_range = true)]
  pub count: u32,
}

#[derive(WebIDL, Clone)]
#[webidl(enum)]
pub(crate) enum GPUQueryType {
  Occlusion,
  Timestamp,
}
impl From<GPUQueryType> for wgpu_types::QueryType {
  fn from(value: GPUQueryType) -> Self {
    match value {
      GPUQueryType::Occlusion => Self::Occlusion,
      GPUQueryType::Timestamp => Self::Timestamp,
    }
  }
}
