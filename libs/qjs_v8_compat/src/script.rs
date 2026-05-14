// Copyright 2018-2026 the Deno authors. MIT license.
//
// Script and ScriptOrigin. Maps to JS_Eval / JS_EvalThis.

use crate::primitives::String as JsString;
use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

crate::value_type!(Script);
crate::value_type!(UnboundScript);
crate::value_type!(UnboundModuleScript);

/// V8 carries a `ScriptOrigin` for source maps, filename, line offsets.
pub struct ScriptOrigin<'s> {
  filename: Option<String>,
  _scope: std::marker::PhantomData<&'s ()>,
}

impl<'s> ScriptOrigin<'s> {
  pub fn new(
    scope: &mut HandleScope<'s>,
    resource_name: Local<'s, Value>,
    _resource_line_offset: i32,
    _resource_column_offset: i32,
    _resource_is_shared_cross_origin: bool,
    _script_id: i32,
    _source_map_url: Local<'s, Value>,
    _resource_is_opaque: bool,
    _is_wasm: bool,
    _is_module: bool,
    _host_defined_options: Option<Local<'s, Value>>,
  ) -> Self {
    Self {
      filename: sys::to_string_lossy(scope.ctx(), resource_name.raw()),
      _scope: std::marker::PhantomData,
    }
  }
  pub fn filename(&self) -> Option<&str> {
    self.filename.as_deref()
  }
}

impl Script {
  pub fn compile<'s>(
    scope: &mut HandleScope<'s>,
    source: Local<'s, JsString>,
    origin: Option<&ScriptOrigin<'s>>,
  ) -> Option<Local<'s, Script>> {
    let src = sys::to_string_lossy(scope.ctx(), source.raw())?;
    let filename = origin
      .and_then(|o| o.filename().map(str::to_owned))
      .unwrap_or_else(|| "<anonymous>".into());
    let raw = sys::eval(
      scope.ctx(),
      &src,
      &filename,
      crate::ffi::JS_EVAL_TYPE_GLOBAL | crate::ffi::JS_EVAL_FLAG_COMPILE_ONLY,
    );
    if sys::jsv_is_exception(&raw) {
      return None;
    }
    scope.track_owned(raw);
    Some(Local::from_raw(raw))
  }
}

impl<'s> Local<'s, Script> {
  pub fn run(&self, _scope: &mut HandleScope<'s>) -> Option<Local<'s, Value>> {
    // QJS-DIVERGE: compile-only in QuickJS produces a function bytecode
    // value; we'd need JS_EvalFunction to actually run it. Wired in the
    // follow-up that connects deno_core's eval pipeline.
    None
  }
}
