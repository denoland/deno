// Copyright 2018-2025 the Deno authors. MIT license.

use crate::CancelHandle;
use crate::JsBuffer;
use crate::ModuleId;
use crate::OpDecl;
use crate::OpState;
use crate::Resource;
use crate::error::CoreError;
use crate::error::CoreErrorKind;
use crate::error::ResourceError;
use crate::error::exception_to_err;
use crate::error::exception_to_err_result;
use crate::io::AdaptiveBufferStrategy;
use crate::io::BufMutView;
use crate::io::BufView;
use crate::io::ResourceId;
use crate::modules::ModuleMap;
use crate::op2;
use crate::ops_builtin_types;
use crate::ops_builtin_v8;
use crate::runtime::JsRealm;
use crate::runtime::v8_static_strings;
use bytes::BytesMut;
use deno_error::JsErrorBox;
use futures::StreamExt;
use serde_v8::ByteString;
use std::cell::RefCell;
use std::io::Write;
use std::io::stderr;
use std::io::stdout;
use std::rc::Rc;

macro_rules! builtin_ops {
  ( $($op:ident $(:: $sub:ident)*),* ) => {
    pub const BUILTIN_OPS: &'static [OpDecl] = &[
      $( $op $(:: $sub) * () ),*
    ];
  }
}

builtin_ops! {
  op_close,
  op_try_close,
  op_print,
  op_resources,
  op_wasm_streaming_feed,
  op_wasm_streaming_set_url,
  op_wasm_streaming_stream_feed,
  op_void_sync,
  op_error_async,
  op_error_async_deferred,
  op_void_async,
  op_void_async_deferred,
  op_add,
  op_add_async,
  op_read,
  op_read_all,
  op_write,
  op_read_sync,
  op_write_sync,
  op_write_all,
  op_write_type_error,
  op_shutdown,
  op_str_byte_length,
  op_panic,
  op_cancel_handle,
  op_encode_binary_string,
  op_is_terminal,
  op_import_sync,
  ops_builtin_types::op_is_any_array_buffer,
  ops_builtin_types::op_is_arguments_object,
  ops_builtin_types::op_is_array_buffer,
  ops_builtin_types::op_is_array_buffer_view,
  ops_builtin_types::op_is_async_function,
  ops_builtin_types::op_is_big_int_object,
  ops_builtin_types::op_is_boolean_object,
  ops_builtin_types::op_is_boxed_primitive,
  ops_builtin_types::op_is_data_view,
  ops_builtin_types::op_is_date,
  ops_builtin_types::op_is_generator_function,
  ops_builtin_types::op_is_generator_object,
  ops_builtin_types::op_is_map,
  ops_builtin_types::op_is_map_iterator,
  ops_builtin_types::op_is_module_namespace_object,
  ops_builtin_types::op_is_native_error,
  ops_builtin_types::op_is_number_object,
  ops_builtin_types::op_is_promise,
  ops_builtin_types::op_is_proxy,
  ops_builtin_types::op_is_reg_exp,
  ops_builtin_types::op_is_set,
  ops_builtin_types::op_is_set_iterator,
  ops_builtin_types::op_is_shared_array_buffer,
  ops_builtin_types::op_is_string_object,
  ops_builtin_types::op_is_symbol_object,
  ops_builtin_types::op_is_typed_array,
  ops_builtin_types::op_is_weak_map,
  ops_builtin_types::op_is_weak_set,
  ops_builtin_v8::op_add_main_module_handler,
  ops_builtin_v8::op_set_handled_promise_rejection_handler,
  ops_builtin_v8::op_timer_queue,
  ops_builtin_v8::op_timer_queue_system,
  ops_builtin_v8::op_timer_cancel,
  ops_builtin_v8::op_timer_ref,
  ops_builtin_v8::op_timer_unref,
  ops_builtin_v8::op_ref_op,
  ops_builtin_v8::op_unref_op,
  ops_builtin_v8::op_lazy_load_esm,
  ops_builtin_v8::op_run_microtasks,
  ops_builtin_v8::op_has_tick_scheduled,
  ops_builtin_v8::op_set_has_tick_scheduled,
  ops_builtin_v8::op_immediate_count,
  ops_builtin_v8::op_immediate_ref_count,
  ops_builtin_v8::op_immediate_set_has_outstanding,
  ops_builtin_v8::op_immediate_has_ref_count,
  ops_builtin_v8::op_eval_context,
  ops_builtin_v8::op_queue_microtask,
  ops_builtin_v8::op_encode,
  ops_builtin_v8::op_decode,
  ops_builtin_v8::op_serialize,
  ops_builtin_v8::op_deserialize,
  ops_builtin_v8::op_structured_clone,
  ops_builtin_v8::op_set_promise_hooks,
  ops_builtin_v8::op_get_promise_details,
  ops_builtin_v8::op_get_proxy_details,
  ops_builtin_v8::op_get_non_index_property_names,
  ops_builtin_v8::op_get_constructor_name,
  ops_builtin_v8::op_get_extras_binding_object,
  ops_builtin_v8::op_memory_usage,
  ops_builtin_v8::op_set_wasm_streaming_callback,
  ops_builtin_v8::op_abort_wasm_streaming,
  ops_builtin_v8::op_destructure_error,
  ops_builtin_v8::op_dispatch_exception,
  ops_builtin_v8::op_op_names,
  ops_builtin_v8::op_current_user_call_site,
  ops_builtin_v8::op_set_format_exception_callback,
  ops_builtin_v8::op_event_loop_has_more_work,
  ops_builtin_v8::op_leak_tracing_enable,
  ops_builtin_v8::op_leak_tracing_submit,
  ops_builtin_v8::op_leak_tracing_get_all,
  ops_builtin_v8::op_leak_tracing_get,
  ops_builtin_v8::op_get_ext_import_meta_proto
}

#[op2(fast)]
pub fn op_panic(#[string] message: String) {
  #[allow(clippy::print_stderr)]
  {
    eprintln!("JS PANIC: {}", message);
  }
  panic!("JS PANIC: {}", message);
}

/// Return map of resources with id as key
/// and string representation as value.
#[op2]
pub fn op_resources(state: &mut OpState) -> Vec<(ResourceId, String)> {
  state
    .resource_table
    .names()
    .map(|(rid, name)| (rid, name.to_string()))
    .collect()
}

#[op2(fast)]
fn op_add(a: i32, b: i32) -> i32 {
  a + b
}

#[allow(clippy::unused_async)]
#[op2]
pub async fn op_add_async(a: i32, b: i32) -> i32 {
  a + b
}

#[op2(fast)]
pub fn op_void_sync() {}

#[allow(clippy::unused_async)]
#[op2]
pub async fn op_void_async() {}

#[allow(clippy::unused_async)]
#[op2]
pub async fn op_error_async() -> Result<(), JsErrorBox> {
  Err(JsErrorBox::generic("error"))
}

#[allow(clippy::unused_async)]
#[op2(async(deferred), fast)]
pub async fn op_error_async_deferred() -> Result<(), JsErrorBox> {
  Err(JsErrorBox::generic("error"))
}

#[allow(clippy::unused_async)]
#[op2(async(deferred), fast)]
pub async fn op_void_async_deferred() {}

/// Remove a resource from the resource table.
#[op2(fast)]
pub fn op_close(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), ResourceError> {
  let resource = state.borrow_mut().resource_table.take_any(rid)?;
  resource.close();
  Ok(())
}

/// Try to remove a resource from the resource table. If there is no resource
/// with the specified `rid`, this is a no-op.
#[op2(fast)]
pub fn op_try_close(state: Rc<RefCell<OpState>>, #[smi] rid: ResourceId) {
  if let Ok(resource) = state.borrow_mut().resource_table.take_any(rid) {
    resource.close();
  }
}

/// Builtin utility to print to stdout/stderr
#[op2(fast)]
pub fn op_print(
  #[string] msg: &str,
  is_err: bool,
) -> Result<(), std::io::Error> {
  if is_err {
    stderr().write_all(msg.as_bytes())?;
    stderr().flush().unwrap();
  } else {
    stdout().write_all(msg.as_bytes())?;
    stdout().flush().unwrap();
  }
  Ok(())
}

pub struct WasmStreamingResource(pub(crate) RefCell<v8::WasmStreaming<false>>);

impl Resource for WasmStreamingResource {
  fn close(self: Rc<Self>) {
    // At this point there are no clones of Rc<WasmStreamingResource> on the
    // resource table, and no one should own a reference outside of the stack.
    // Therefore, we can be sure `self` is the only reference.
    match Rc::try_unwrap(self) {
      Ok(wsr) => {
        wsr.0.into_inner().finish();
      }
      _ => {
        panic!("Couldn't consume WasmStreamingResource.");
      }
    }
  }
}

/// Feed bytes to WasmStreamingResource.
#[op2(fast)]
pub fn op_wasm_streaming_feed(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] bytes: &[u8],
) -> Result<(), ResourceError> {
  let wasm_streaming = state
    .borrow_mut()
    .resource_table
    .get::<WasmStreamingResource>(rid)?;

  wasm_streaming.0.borrow_mut().on_bytes_received(bytes);

  Ok(())
}

#[op2(fast)]
pub fn op_wasm_streaming_set_url(
  state: &mut OpState,
  #[smi] rid: ResourceId,
  #[string] url: &str,
) -> Result<(), ResourceError> {
  let wasm_streaming =
    state.resource_table.get::<WasmStreamingResource>(rid)?;

  wasm_streaming.0.borrow_mut().set_url(url);

  Ok(())
}

#[op2]
async fn op_wasm_streaming_stream_feed(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[smi] stream_rid: ResourceId,
  auto_close: bool,
) -> Result<(), JsErrorBox> {
  let wasm_streaming = state
    .borrow_mut()
    .resource_table
    .get::<WasmStreamingResource>(rid)
    .map_err(|_| JsErrorBox::type_error("stream not found"))?;

  loop {
    let resource = state
      .borrow()
      .resource_table
      .get_any(stream_rid)
      .map_err(|_| JsErrorBox::type_error("stream not found"))?;
    let view = deno_core::BufMutView::new(65536);
    let (bytes, view) = resource.read_byob(view).await?;

    /* EOF */
    if bytes == 0 {
      break;
    }

    wasm_streaming
      .0
      .borrow_mut()
      .on_bytes_received(&view[..bytes]);
  }

  if auto_close {
    let _ = state.borrow_mut().resource_table.take_any(stream_rid);
  }

  Ok(())
}

// Get a resource from the resource table and
// handle unrefing the current task.
fn get_resource(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  promise_id: i32,
) -> Result<Rc<dyn Resource>, JsErrorBox> {
  let op_state = state.borrow();
  let resource = op_state
    .resource_table
    .get_any(rid)
    .map_err(JsErrorBox::from_err)?;

  if op_state.unrefed_resources.contains(&rid) {
    op_state.unrefed_ops.borrow_mut().insert(promise_id);
  }

  Ok(resource)
}

#[op2(promise_id)]
async fn op_read(
  state: Rc<RefCell<OpState>>,
  #[smi] promise_id: i32,
  #[smi] rid: ResourceId,
  #[buffer] buf: JsBuffer,
) -> Result<u32, JsErrorBox> {
  let resource = get_resource(state, rid, promise_id)?;

  let view = BufMutView::from(buf);
  resource.read_byob(view).await.map(|(n, _)| n as u32)
}

#[op2(promise_id)]
#[buffer]
async fn op_read_all(
  state: Rc<RefCell<OpState>>,
  #[smi] promise_id: i32,
  #[smi] rid: ResourceId,
) -> Result<BytesMut, JsErrorBox> {
  let resource = get_resource(state, rid, promise_id)?;

  let (min, maybe_max) = resource.size_hint();
  let mut buffer_strategy =
    AdaptiveBufferStrategy::new_from_hint_u64(min, maybe_max);
  let mut buf = BufMutView::new(buffer_strategy.buffer_size());

  loop {
    #[allow(deprecated)]
    buf.maybe_grow(buffer_strategy.buffer_size()).unwrap();

    let (n, new_buf) = resource.clone().read_byob(buf).await?;
    buf = new_buf;
    buf.advance_cursor(n);
    if n == 0 {
      break;
    }

    buffer_strategy.notify_read(n);
  }

  let nread = buf.reset_cursor();
  // If the buffer is larger than the amount of data read, shrink it to the
  // amount of data read.
  buf.truncate(nread);

  Ok(buf.maybe_unwrap_bytes().unwrap())
}

#[op2(promise_id)]
async fn op_write(
  state: Rc<RefCell<OpState>>,
  #[smi] promise_id: i32,
  #[smi] rid: ResourceId,
  #[buffer] buf: JsBuffer,
) -> Result<u32, JsErrorBox> {
  let resource = get_resource(state, rid, promise_id)?;

  let view = BufView::from(buf);
  let resp = resource.write(view).await?;
  Ok(resp.nwritten() as u32)
}

#[op2(fast)]
fn op_read_sync(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] data: &mut [u8],
) -> Result<u32, JsErrorBox> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get_any(rid)
    .map_err(JsErrorBox::from_err)?;
  resource.read_byob_sync(data).map(|n| n as u32)
}

#[op2(fast)]
fn op_write_sync(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] data: &[u8],
) -> Result<u32, JsErrorBox> {
  let resource = state
    .borrow_mut()
    .resource_table
    .get_any(rid)
    .map_err(JsErrorBox::from_err)?;
  let nwritten = resource.write_sync(data)?;
  Ok(nwritten as u32)
}

#[op2]
async fn op_write_all(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[buffer] buf: JsBuffer,
) -> Result<(), JsErrorBox> {
  let resource = state
    .borrow()
    .resource_table
    .get_any(rid)
    .map_err(JsErrorBox::from_err)?;
  let view = BufView::from(buf);
  resource.write_all(view).await?;
  Ok(())
}

#[op2]
async fn op_write_type_error(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] error: String,
) -> Result<(), JsErrorBox> {
  let resource = state
    .borrow()
    .resource_table
    .get_any(rid)
    .map_err(JsErrorBox::from_err)?;
  resource.write_error(&JsErrorBox::type_error(error)).await?;
  Ok(())
}

#[op2]
async fn op_shutdown(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<(), JsErrorBox> {
  let resource = state
    .borrow()
    .resource_table
    .get_any(rid)
    .map_err(JsErrorBox::from_err)?;
  resource.shutdown().await
}

#[op2(fast)]
fn op_str_byte_length<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  value: v8::Local<'s, v8::Value>,
) -> u32 {
  if let Ok(string) = v8::Local::<v8::String>::try_from(value) {
    string.utf8_length(scope) as u32
  } else {
    0
  }
}

/// Creates a [`CancelHandle`] resource that can be used to cancel invocations of certain ops.
#[op2(fast)]
#[smi]
pub fn op_cancel_handle(state: &mut OpState) -> u32 {
  state.resource_table.add(CancelHandle::new())
}

#[op2]
#[serde]
fn op_encode_binary_string(#[buffer] s: &[u8]) -> ByteString {
  ByteString::from(s)
}

#[op2(fast)]
fn op_is_terminal(state: &mut OpState, #[smi] rid: ResourceId) -> bool {
  match state.resource_table.get_handle(rid) {
    Ok(handle) => handle.is_terminal(),
    _ => false,
  }
}

async fn do_load_job<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  module_map_rc: Rc<ModuleMap>,
  specifier: &str,
  code: Option<String>,
) -> Result<ModuleId, CoreError> {
  let mut load = ModuleMap::load_side(
    module_map_rc.clone(),
    specifier.to_string(),
    crate::modules::SideModuleKind::Sync,
    code,
  )
  .await?;

  while let Some(load_result) = load.next().await {
    let (request, info) = load_result?;
    load
      .register_and_recurse(scope, &request, info)
      .map_err(|e| e.into_error(scope, false, false))?;
  }

  let root_id = load.root_module_id.expect("Root module should be loaded");

  let module = module_map_rc
    .get_module(scope, root_id)
    .expect("Module must exist");

  match module.get_status() {
    v8::ModuleStatus::Uninstantiated => {
      module_map_rc
        .instantiate_module(scope, root_id)
        .map_err(|e| {
          let exception = v8::Local::new(scope, e);
          exception_to_err(scope, exception, false, false)
        })?;
    }
    v8::ModuleStatus::Instantiated
    | v8::ModuleStatus::Instantiating
    | v8::ModuleStatus::Evaluating => {
      return Err(
        JsErrorBox::generic(format!(
          "Cannot require() ES Module {specifier} in a cycle."
        ))
        .into(),
      );
    }
    v8::ModuleStatus::Evaluated => {
      // OK
    }
    v8::ModuleStatus::Errored => {
      return Err(
        CoreErrorKind::Js(exception_to_err(
          scope,
          module.get_exception(),
          false,
          false,
        ))
        .into_box(),
      );
    }
  }

  Ok(root_id)
}

/// Wrap module with another module that also exports `__esModule=true` in order
/// to maintain compat with node, which does this to maintain compat with babel.
fn wrap_module<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  module: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
  const SOURCE: &str = "
  export * from 'original';
  export {default} from 'original';
  export const __esModule = true;";

  let source = v8::String::new(scope, SOURCE)?;
  let origin = v8::ScriptOrigin::new(
    scope,
    source.into(),
    0,
    0,
    false,
    0,
    None,
    true,
    false,
    true,
    None,
  );

  let mut source = v8::script_compiler::Source::new(source, Some(&origin));
  let wrapper_module = v8::script_compiler::compile_module(scope, &mut source)?;

  let global_module = v8::Global::new(scope, module);
  scope.set_slot(global_module);

  #[allow(clippy::unnecessary_wraps)]
  fn resolve_callback<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    _: v8::Local<'s, v8::FixedArray>,
    _: v8::Local<'s, v8::Module>,
  ) -> Option<v8::Local<'s, v8::Module>> {
    // SAFETY: It is safe to open a CallbackScope from a context in this callback.
    v8::callback_scope!(unsafe scope, context);
    debug_assert_eq!(specifier.to_rust_string_lossy(scope), "original");
    let module = scope.remove_slot::<v8::Global<v8::Module>>().unwrap();
    Some(v8::Local::new(scope, module))
  }

  wrapper_module.instantiate_module(scope, resolve_callback)?;

  let result = wrapper_module.evaluate(scope)?;
  if let Ok(promise) = result.try_cast::<v8::Promise>() {
    promise.mark_as_handled();
    if promise.state() == v8::PromiseState::Rejected {
      let exception_state = JsRealm::exception_state_from_scope(scope);
      // TODO: remove after crrev.com/c/7595271
      exception_state.track_promise_rejection(
        scope,
        promise,
        v8::PromiseRejectEvent::PromiseHandlerAddedAfterReject,
        None,
      );
    }
  }

  Some(wrapper_module)
}

#[op2(reentrant)]
fn op_import_sync<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  #[string] specifier: &str,
  #[string] code: Option<String>,
) -> Result<v8::Local<'s, v8::Value>, CoreError> {
  let module_map_rc = JsRealm::module_map_from(scope);

  // no js execution within block_on
  let module_id = futures::executor::block_on(do_load_job(
    scope,
    module_map_rc.clone(),
    specifier,
    code,
  ))?;

  let module = module_map_rc
    .get_module(scope, module_id)
    .expect("Module must exist");

  match module.get_status() {
    v8::ModuleStatus::Uninstantiated
    | v8::ModuleStatus::Instantiating
    | v8::ModuleStatus::Evaluating => {
      return Err(
        JsErrorBox::generic(format!(
          "Cannot require() ES Module {specifier} in a cycle."
        ))
        .into(),
      );
    }
    v8::ModuleStatus::Instantiated => {
      module_map_rc.mod_evaluate_sync(scope, module_id)?;
    }
    v8::ModuleStatus::Evaluated => {
      // OK
    }
    v8::ModuleStatus::Errored => {
      return Err(
        CoreErrorKind::Js(exception_to_err(
          scope,
          module.get_exception(),
          false,
          false,
        ))
        .into_box(),
      );
    }
  }

  let namespace = module.get_module_namespace().cast::<v8::Object>();

  v8::tc_scope!(let scope, scope);

  let default = v8_static_strings::DEFAULT.v8_string(scope).unwrap();
  let es_module = v8_static_strings::ESMODULE.v8_string(scope).unwrap();
  // If the module has a default export and no __esModule export, wrap it.
  if namespace.has_own_property(scope, default.into()) == Some(true)
    && namespace.has_own_property(scope, es_module.into()) == Some(false)
  {
    let Some(module) = wrap_module(scope, module) else {
      let exception = scope.exception().unwrap();
      return exception_to_err_result(scope, exception, false, false)
        .map_err(|e| CoreErrorKind::Js(e).into_box());
    };
    Ok(v8::Local::new(scope, module.get_module_namespace()))
  } else {
    Ok(v8::Local::new(scope, namespace).into())
  }
}
