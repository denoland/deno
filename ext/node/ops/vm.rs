// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::v8;

use super::vm_internal as i;

pub use i::create_v8_context;
pub use i::init_global_template;
pub use i::ContextInitMode;
pub use i::VM_CONTEXT_INDEX;

pub use i::DEFINER_MAP_FN;
pub use i::DELETER_MAP_FN;
pub use i::DESCRIPTOR_MAP_FN;
pub use i::ENUMERATOR_MAP_FN;
pub use i::GETTER_MAP_FN;
pub use i::SETTER_MAP_FN;

pub use i::INDEXED_DEFINER_MAP_FN;
pub use i::INDEXED_DELETER_MAP_FN;
pub use i::INDEXED_DESCRIPTOR_MAP_FN;
pub use i::INDEXED_GETTER_MAP_FN;
pub use i::INDEXED_SETTER_MAP_FN;

pub struct Script {
  inner: i::ContextifyScript,
}

impl deno_core::GcResource for Script {}

impl Script {
  fn new(
    scope: &mut v8::HandleScope,
    source: v8::Local<v8::String>,
  ) -> Result<Self, AnyError> {
    Ok(Self {
      inner: i::ContextifyScript::new(scope, source)?,
    })
  }

  fn run_in_this_context<'s>(
    &self,
    scope: &'s mut v8::HandleScope,
  ) -> Result<v8::Local<'s, v8::Value>, AnyError> {
    let context = scope.get_current_context();

    let context_scope = &mut v8::ContextScope::new(scope, context);
    let mut scope = v8::EscapableHandleScope::new(context_scope);
    let result = self
      .inner
      .eval_machine(&mut scope, context)
      .unwrap_or_else(|| v8::undefined(&mut scope).into());
    Ok(scope.escape(result))
  }

  fn run_in_context<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    sandbox: v8::Local<'s, v8::Value>,
  ) -> Result<v8::Local<'s, v8::Value>, AnyError> {
    let context = if let Ok(sandbox_obj) = sandbox.try_into() {
      let context = i::ContextifyContext::from_sandbox_obj(scope, sandbox_obj)
        .ok_or_else(|| type_error("Invalid sandbox object"))?;
      context.context(scope)
    } else {
      scope.get_current_context()
    };

    let context_scope = &mut v8::ContextScope::new(scope, context);
    let mut scope = v8::EscapableHandleScope::new(context_scope);
    let result = self
      .inner
      .eval_machine(&mut scope, context)
      .unwrap_or_else(|| v8::undefined(&mut scope).into());
    Ok(scope.escape(result))
  }
}

#[op2]
pub fn op_vm_create_script<'a>(
  scope: &mut v8::HandleScope<'a>,
  source: v8::Local<'a, v8::String>,
) -> Result<v8::Local<'a, v8::Object>, AnyError> {
  let script = Script::new(scope, source)?;
  Ok(deno_core::cppgc::make_cppgc_object(scope, script))
}

#[op2(reentrant)]
pub fn op_vm_script_run_in_context<'a>(
  scope: &mut v8::HandleScope<'a>,
  #[cppgc] script: &Script,
  sandbox: v8::Local<'a, v8::Value>,
) -> Result<v8::Local<'a, v8::Value>, AnyError> {
  script.run_in_context(scope, sandbox)
}

#[op2(reentrant)]
pub fn op_vm_script_run_in_this_context<'a>(
  scope: &'a mut v8::HandleScope,
  #[cppgc] script: &Script,
) -> Result<v8::Local<'a, v8::Value>, AnyError> {
  script.run_in_this_context(scope)
}

#[op2]
pub fn op_vm_create_context(
  scope: &mut v8::HandleScope,
  sandbox_obj: v8::Local<v8::Object>,
) {
  // Don't allow contextifying a sandbox multiple times.
  assert!(!i::ContextifyContext::is_contextify_context(
    scope,
    sandbox_obj
  ));

  i::ContextifyContext::attach(scope, sandbox_obj);
}

#[op2]
pub fn op_vm_is_context(
  scope: &mut v8::HandleScope,
  sandbox_obj: v8::Local<v8::Value>,
) -> bool {
  sandbox_obj
    .try_into()
    .map(|sandbox_obj| {
      i::ContextifyContext::is_contextify_context(scope, sandbox_obj)
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_core::v8;

  #[test]
  fn test_run_in_this_context() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    let isolate = &mut v8::Isolate::new(Default::default());

    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);

    let source = v8::String::new(scope, "1 + 2").unwrap();
    let script = Script::new(scope, source).unwrap();

    let result = script.run_in_this_context(scope).unwrap();
    assert!(result.is_number());
  }
}
