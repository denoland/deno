// Copyright 2018-2025 the Deno authors. MIT license.

use deno_core::GarbageCollected;
use deno_core::WebIDL;
use deno_core::cppgc::make_cppgc_object;
use deno_core::op2;
use deno_core::v8;
use deno_core::webidl::WebIdlInterfaceConverter;
use wgpu_core::pipeline;

use crate::Instance;
use crate::error::GPUGenericError;

pub struct GPUShaderModule {
  pub instance: Instance,
  pub id: wgpu_core::id::ShaderModuleId,
  pub label: String,
  pub compilation_info: v8::Global<v8::Object>,
}

impl Drop for GPUShaderModule {
  fn drop(&mut self) {
    self.instance.shader_module_drop(self.id);
  }
}

impl WebIdlInterfaceConverter for GPUShaderModule {
  const NAME: &'static str = "GPUShaderModule";
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUShaderModule {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUShaderModule"
  }
}

#[op2]
impl GPUShaderModule {
  #[constructor]
  #[cppgc]
  fn constructor(_: bool) -> Result<GPUShaderModule, GPUGenericError> {
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

  fn get_compilation_info<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Promise> {
    let resolver = v8::PromiseResolver::new(scope).unwrap();
    let info = v8::Local::new(scope, self.compilation_info.clone());
    resolver.resolve(scope, info.into()).unwrap();
    resolver.get_promise(scope)
  }
}

#[derive(WebIDL)]
#[webidl(dictionary)]
pub(crate) struct GPUShaderModuleDescriptor {
  #[webidl(default = String::new())]
  pub label: String,

  pub code: String,
}

pub struct GPUCompilationMessage {
  message: String,
  r#type: GPUCompilationMessageType,
  line_num: u64,
  line_pos: u64,
  offset: u64,
  length: u64,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUCompilationMessage {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUCompilationMessage"
  }
}

#[op2]
impl GPUCompilationMessage {
  #[getter]
  #[string]
  fn message(&self) -> String {
    self.message.clone()
  }

  #[getter]
  #[string]
  #[rename("type")]
  fn r#type(&self) -> &'static str {
    self.r#type.as_str()
  }

  #[getter]
  #[number]
  fn line_num(&self) -> u64 {
    self.line_num
  }

  #[getter]
  #[number]
  fn line_pos(&self) -> u64 {
    self.line_pos
  }

  #[getter]
  #[number]
  fn offset(&self) -> u64 {
    self.offset
  }

  #[getter]
  #[number]
  fn length(&self) -> u64 {
    self.length
  }
}

impl GPUCompilationMessage {
  fn new(error: &pipeline::CreateShaderModuleError, source: &str) -> Self {
    let message = error.to_string();

    let loc = match error {
      pipeline::CreateShaderModuleError::Parsing(e) => e.inner.location(source),
      pipeline::CreateShaderModuleError::Validation(e) => {
        e.inner.location(source)
      }
      _ => None,
    };

    match loc {
      Some(loc) => {
        let len_utf16 = |s: &str| s.chars().map(|c| c.len_utf16() as u64).sum();

        let start = loc.offset as usize;

        // Naga reports a `line_pos` using UTF-8 bytes, so we cannot use it.
        let line_start =
          source[0..start].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
        let line_pos = len_utf16(&source[line_start..start]) + 1;

        Self {
          message,
          r#type: GPUCompilationMessageType::Error,
          line_num: loc.line_number.into(),
          line_pos,
          offset: len_utf16(&source[0..start]),
          length: len_utf16(&source[start..start + loc.length as usize]),
        }
      }
      _ => Self {
        message,
        r#type: GPUCompilationMessageType::Error,
        line_num: 0,
        line_pos: 0,
        offset: 0,
        length: 0,
      },
    }
  }
}

pub struct GPUCompilationInfo {
  messages: v8::Global<v8::Object>,
}
// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for GPUCompilationInfo {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"GPUCompilationInfo"
  }
}

#[op2]
impl GPUCompilationInfo {
  #[getter]
  #[global]
  fn messages(&self) -> v8::Global<v8::Object> {
    self.messages.clone()
  }
}

impl GPUCompilationInfo {
  pub fn new<'args, 'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    messages: impl ExactSizeIterator<
      Item = &'args pipeline::CreateShaderModuleError,
    >,
    source: &'args str,
  ) -> Self {
    let array = v8::Array::new(scope, messages.len().try_into().unwrap());
    for (i, message) in messages.enumerate() {
      let message_object =
        make_cppgc_object(scope, GPUCompilationMessage::new(message, source));
      array.set_index(scope, i.try_into().unwrap(), message_object.into());
    }

    let object: v8::Local<v8::Object> = array.into();
    object
      .set_integrity_level(scope, v8::IntegrityLevel::Frozen)
      .unwrap();

    Self {
      messages: v8::Global::new(scope, object),
    }
  }
}

#[derive(WebIDL, Clone)]
#[webidl(enum)]
pub(crate) enum GPUCompilationMessageType {
  Error,
  Warning,
  Info,
}
