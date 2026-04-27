// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroI32;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use deno_core::JsBuffer;
use deno_core::op2;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::v8::MapFnTo;

use crate::create_host_defined_options;

pub const PRIVATE_SYMBOL_NAME: v8::OneByteConst =
  v8::String::create_external_onebyte_const(b"node:contextify:context");

/// An unbounded script that can be run in a context.
pub struct ContextifyScript {
  script: v8::TracedReference<v8::UnboundScript>,
}

// SAFETY: we're sure this can be GCed
unsafe impl v8::cppgc::GarbageCollected for ContextifyScript {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.script);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ContextifyScript"
  }
}

impl ContextifyScript {
  #[allow(clippy::too_many_arguments, reason = "internal code")]
  fn create<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    source: v8::Local<'s, v8::String>,
    filename: v8::Local<'s, v8::Value>,
    line_offset: i32,
    column_offset: i32,
    cached_data: Option<JsBuffer>,
    produce_cached_data: bool,
    parsing_context: Option<v8::Local<'s, v8::Object>>,
  ) -> Option<CompileResult<'s>> {
    let context = if let Some(parsing_context) = parsing_context {
      let Some(context) =
        ContextifyContext::from_sandbox_obj(scope, parsing_context)
      else {
        let message = v8::String::new_external_onebyte_static(
          scope,
          b"Invalid sandbox object",
        )
        .unwrap();
        let exception = v8::Exception::type_error(scope, message);
        scope.throw_exception(exception);
        return None;
      };
      context.context(scope)
    } else {
      scope.get_current_context()
    };

    let scope = &mut v8::ContextScope::new(scope, context);
    let host_defined_options = create_host_defined_options(scope);
    let origin = v8::ScriptOrigin::new(
      scope,
      filename,
      line_offset,
      column_offset,
      true,
      -1,
      None,
      false,
      false,
      false,
      Some(host_defined_options),
    );

    let mut source = if let Some(cached_data) = cached_data {
      let cached_data = v8::script_compiler::CachedData::new(&cached_data);
      v8::script_compiler::Source::new_with_cached_data(
        source,
        Some(&origin),
        cached_data,
      )
    } else {
      v8::script_compiler::Source::new(source, Some(&origin))
    };

    let options = if source.get_cached_data().is_some() {
      v8::script_compiler::CompileOptions::ConsumeCodeCache
    } else {
      v8::script_compiler::CompileOptions::NoCompileOptions
    };

    v8::tc_scope!(scope, scope);

    let Some(unbound_script) = v8::script_compiler::compile_unbound_script(
      scope,
      &mut source,
      options,
      v8::script_compiler::NoCacheReason::NoReason,
    ) else {
      if !scope.has_terminated() {
        scope.rethrow();
      }
      return None;
    };

    let cached_data = if produce_cached_data {
      unbound_script.create_code_cache()
    } else {
      None
    };

    let script = v8::TracedReference::new(scope, unbound_script);
    let this = deno_core::cppgc::make_cppgc_object(scope, Self { script });

    Some(CompileResult {
      value: serde_v8::Value {
        v8_value: this.into(),
      },
      cached_data: cached_data.as_ref().map(|c| {
        let backing_store =
          v8::ArrayBuffer::new_backing_store_from_vec(c.to_vec());
        v8::ArrayBuffer::with_backing_store(scope, &backing_store.make_shared())
          .into()
      }),
      cached_data_rejected: source
        .get_cached_data()
        .map(|c| c.rejected())
        .unwrap_or(false),
      cached_data_produced: cached_data.is_some(),
    })
  }

  fn run_in_context<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
    sandbox: Option<v8::Local<'s, v8::Object>>,
    timeout: i64,
    display_errors: bool,
    break_on_sigint: bool,
  ) -> Option<v8::Local<'s, v8::Value>> {
    let (context, microtask_queue) = if let Some(sandbox) = sandbox {
      let Some(context) = ContextifyContext::from_sandbox_obj(scope, sandbox)
      else {
        let message = v8::String::new_external_onebyte_static(
          scope,
          b"Invalid sandbox object",
        )
        .unwrap();
        let exception = v8::Exception::type_error(scope, message);
        scope.throw_exception(exception);
        return None;
      };
      (context.context(scope), context.microtask_queue())
    } else {
      (scope.get_current_context(), None)
    };

    self.eval_machine(
      scope,
      context,
      timeout,
      display_errors,
      break_on_sigint,
      microtask_queue,
    )
  }

  pub fn eval_machine<'s, 'i>(
    &self,
    scope: &mut v8::PinScope<'s, 'i>,
    context: v8::Local<'s, v8::Context>,
    timeout: i64,
    _display_errors: bool,
    _break_on_sigint: bool,
    microtask_queue: Option<&v8::MicrotaskQueue>,
  ) -> Option<v8::Local<'s, v8::Value>> {
    let context_scope = &mut v8::ContextScope::new(scope, context);
    let scope_storage =
      std::pin::pin!(v8::EscapableHandleScope::new(context_scope));
    let scope = &mut scope_storage.init();
    v8::tc_scope!(scope, scope);

    let unbound_script = self.script.get(scope).unwrap();
    let script = unbound_script.bind_to_current_context(scope);

    let handle = scope.thread_safe_handle();

    let mut run = || {
      let r = script.run(scope);
      if r.is_some()
        && let Some(mtask_queue) = microtask_queue
      {
        mtask_queue.perform_checkpoint(scope);
      }
      r
    };

    #[allow(
      clippy::disallowed_types,
      reason = "isolated here and sending to separate thread"
    )]
    let timed_out = std::sync::Arc::new(AtomicBool::new(false));
    let result = if timeout != -1 {
      let timed_out = timed_out.clone();
      let (tx, rx) = std::sync::mpsc::channel();
      deno_core::unsync::spawn_blocking(move || {
        if rx
          .recv_timeout(Duration::from_millis(timeout as _))
          .is_err()
        {
          timed_out.store(true, Ordering::Relaxed);
          handle.terminate_execution();
        }
      });
      let r = run();
      let _ = tx.send(());
      r
    } else {
      run()
    };

    if timed_out.load(Ordering::Relaxed) {
      if scope.has_terminated() {
        scope.cancel_terminate_execution();
      }
      let message = v8::String::new(
        scope,
        &format!("Script execution timed out after {timeout}ms"),
      )
      .unwrap();
      let exception = v8::Exception::error(scope, message);
      let code_str =
        v8::String::new_external_onebyte_static(scope, b"code").unwrap();
      let code = v8::String::new_external_onebyte_static(
        scope,
        b"ERR_SCRIPT_EXECUTION_TIMEOUT",
      )
      .unwrap();
      exception
        .cast::<v8::Object>()
        .set(scope, code_str.into(), code.into());
      scope.throw_exception(exception);
    }

    if scope.has_caught() {
      // If there was an exception thrown during script execution, re-throw it.
      if !scope.has_terminated() {
        scope.rethrow();
      }

      return None;
    }

    Some(scope.escape(result?))
  }
}

pub struct ContextifyContext {
  microtask_queue: *mut v8::MicrotaskQueue,
  context: v8::TracedReference<v8::Context>,
  sandbox: v8::TracedReference<v8::Object>,
}

// SAFETY: we're sure this can be GCed
unsafe impl deno_core::GarbageCollected for ContextifyContext {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.context);
    visitor.trace(&self.sandbox);
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ContextifyContext"
  }
}

impl Drop for ContextifyContext {
  fn drop(&mut self) {
    if !self.microtask_queue.is_null() {
      // SAFETY: If this isn't null, it is a valid MicrotaskQueue.
      unsafe {
        std::ptr::drop_in_place(self.microtask_queue);
      }
    }
  }
}

struct AllowCodeGenWasm(bool);

extern "C" fn allow_wasm_code_gen(
  context: v8::Local<v8::Context>,
  _source: v8::Local<v8::String>,
) -> bool {
  match context.get_slot::<AllowCodeGenWasm>() {
    Some(b) => b.0,
    None => true,
  }
}

impl ContextifyContext {
  pub fn attach(
    scope: &mut v8::PinScope<'_, '_>,
    sandbox_obj: v8::Local<v8::Object>,
    _name: String,
    _origin: String,
    allow_code_gen_strings: bool,
    allow_code_gen_wasm: bool,
    own_microtask_queue: bool,
  ) {
    let main_context = scope.get_current_context();

    let tmp = init_global_template(scope, ContextInitMode::UseSnapshot);

    let microtask_queue = if own_microtask_queue {
      v8::MicrotaskQueue::new(scope, v8::MicrotasksPolicy::Explicit).into_raw()
    } else {
      std::ptr::null_mut()
    };

    let context = create_v8_context(
      scope,
      tmp,
      ContextInitMode::UseSnapshot,
      microtask_queue,
    );

    let context_state = main_context.get_aligned_pointer_from_embedder_data(
      deno_core::CONTEXT_STATE_SLOT_INDEX,
    );
    let module_map = main_context
      .get_aligned_pointer_from_embedder_data(deno_core::MODULE_MAP_SLOT_INDEX);

    context.set_security_token(main_context.get_security_token(scope));
    // SAFETY: set embedder data from the creation context
    unsafe {
      context.set_aligned_pointer_in_embedder_data(
        deno_core::CONTEXT_STATE_SLOT_INDEX,
        context_state,
      );
      context.set_aligned_pointer_in_embedder_data(
        deno_core::MODULE_MAP_SLOT_INDEX,
        module_map,
      );
    }

    scope.set_allow_wasm_code_generation_callback(allow_wasm_code_gen);
    context.set_allow_generation_from_strings(allow_code_gen_strings);
    context.set_slot(Rc::new(AllowCodeGenWasm(allow_code_gen_wasm)));

    let wrapper = {
      let context = v8::TracedReference::new(scope, context);
      let sandbox = v8::TracedReference::new(scope, sandbox_obj);
      deno_core::cppgc::make_cppgc_object(
        scope,
        Self {
          context,
          sandbox,
          microtask_queue,
        },
      )
    };
    let ptr =
      deno_core::cppgc::try_unwrap_cppgc_object::<Self>(scope, wrapper.into());

    // SAFETY: We are storing a pointer to the ContextifyContext
    // in the embedder data of the v8::Context. The contextified wrapper
    // lives longer than the execution context, so this should be safe.
    unsafe {
      context.set_aligned_pointer_in_embedder_data(
        3,
        &*ptr.unwrap() as *const ContextifyContext as _,
      );
    }

    let private_str =
      v8::String::new_from_onebyte_const(scope, &PRIVATE_SYMBOL_NAME);
    let private_symbol = v8::Private::for_api(scope, private_str);

    sandbox_obj.set_private(scope, private_symbol, wrapper.into());
  }

  pub fn attach_vanilla<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    allow_code_gen_strings: bool,
    allow_code_gen_wasm: bool,
    own_microtask_queue: bool,
  ) -> v8::Local<'s, v8::Object> {
    let main_context = scope.get_current_context();

    let microtask_queue = if own_microtask_queue {
      v8::MicrotaskQueue::new(scope, v8::MicrotasksPolicy::Explicit).into_raw()
    } else {
      std::ptr::null_mut()
    };

    // Create a vanilla V8 context without global template (no interceptors)
    let context = {
      let esc_scope = std::pin::pin!(v8::EscapableHandleScope::new(scope));
      let esc_scope = &mut esc_scope.init();
      let ctx = v8::Context::new(
        esc_scope,
        v8::ContextOptions {
          microtask_queue: Some(microtask_queue),
          ..Default::default()
        },
      );
      // SAFETY: ContextifyContexts will update this to a pointer to the native object
      unsafe {
        ctx.set_aligned_pointer_in_embedder_data(1, std::ptr::null_mut());
        ctx.set_aligned_pointer_in_embedder_data(2, std::ptr::null_mut());
        ctx.set_aligned_pointer_in_embedder_data(3, std::ptr::null_mut());
        ctx.clear_all_slots();
      };
      esc_scope.escape(ctx)
    };

    let context_state = main_context.get_aligned_pointer_from_embedder_data(
      deno_core::CONTEXT_STATE_SLOT_INDEX,
    );
    let module_map = main_context
      .get_aligned_pointer_from_embedder_data(deno_core::MODULE_MAP_SLOT_INDEX);

    context.set_security_token(main_context.get_security_token(scope));
    // SAFETY: set embedder data from the creation context
    unsafe {
      context.set_aligned_pointer_in_embedder_data(
        deno_core::CONTEXT_STATE_SLOT_INDEX,
        context_state,
      );
      context.set_aligned_pointer_in_embedder_data(
        deno_core::MODULE_MAP_SLOT_INDEX,
        module_map,
      );
    }

    scope.set_allow_wasm_code_generation_callback(allow_wasm_code_gen);
    context.set_allow_generation_from_strings(allow_code_gen_strings);
    context.set_slot(Rc::new(AllowCodeGenWasm(allow_code_gen_wasm)));

    // For vanilla contexts, the sandbox IS the global proxy
    let sandbox_obj = context.global(scope);

    let wrapper = {
      let context = v8::TracedReference::new(scope, context);
      let sandbox = v8::TracedReference::new(scope, sandbox_obj);
      deno_core::cppgc::make_cppgc_object(
        scope,
        Self {
          context,
          sandbox,
          microtask_queue,
        },
      )
    };
    let ptr =
      deno_core::cppgc::try_unwrap_cppgc_object::<Self>(scope, wrapper.into());

    // SAFETY: We are storing a pointer to the ContextifyContext
    // in the embedder data of the v8::Context. The contextified wrapper
    // lives longer than the execution context, so this should be safe.
    unsafe {
      context.set_aligned_pointer_in_embedder_data(
        3,
        &*ptr.unwrap() as *const ContextifyContext as _,
      );
    }

    let private_str =
      v8::String::new_from_onebyte_const(scope, &PRIVATE_SYMBOL_NAME);
    let private_symbol = v8::Private::for_api(scope, private_str);

    sandbox_obj.set_private(scope, private_symbol, wrapper.into());

    sandbox_obj
  }

  pub fn from_sandbox_obj<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    sandbox_obj: v8::Local<v8::Object>,
  ) -> Option<&'a Self> {
    let private_str =
      v8::String::new_from_onebyte_const(scope, &PRIVATE_SYMBOL_NAME);
    let private_symbol = v8::Private::for_api(scope, private_str);

    sandbox_obj
      .get_private(scope, private_symbol)
      .and_then(|wrapper| {
        deno_core::cppgc::try_unwrap_cppgc_object::<Self>(scope, wrapper)
          // SAFETY: the lifetime of the scope does not actually bind to
          // the lifetime of this reference at all, but the object we read
          // it from does, so it will be alive at least that long.
          .map(|r| unsafe { &*(&*r as *const _) })
      })
  }

  pub fn is_contextify_context(
    scope: &mut v8::PinScope<'_, '_>,
    object: v8::Local<v8::Object>,
  ) -> bool {
    Self::from_sandbox_obj(scope, object).is_some()
  }

  pub fn context<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> v8::Local<'a, v8::Context> {
    self.context.get(scope).unwrap()
  }

  fn global_proxy<'s>(
    &self,
    scope: &mut v8::PinScope<'s, '_>,
  ) -> v8::Local<'s, v8::Object> {
    let ctx = self.context(scope);
    ctx.global(scope)
  }

  fn sandbox<'a>(
    &self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Option<v8::Local<'a, v8::Object>> {
    self.sandbox.get(scope)
  }

  fn microtask_queue(&self) -> Option<&v8::MicrotaskQueue> {
    if self.microtask_queue.is_null() {
      None
    } else {
      // SAFETY: If this isn't null, it is a valid MicrotaskQueue.
      Some(unsafe { &*self.microtask_queue })
    }
  }

  fn get<'a, 'c>(
    scope: &mut v8::PinScope<'a, '_>,
    object: v8::Local<'a, v8::Object>,
  ) -> Option<&'c ContextifyContext> {
    let context = object.get_creation_context(scope)?;

    let context_ptr = context.get_aligned_pointer_from_embedder_data(3);
    if context_ptr.is_null() {
      return None;
    }
    // SAFETY: We are storing a pointer to the ContextifyContext
    // in the embedder data of the v8::Context during creation.
    Some(unsafe { &*(context_ptr as *const ContextifyContext) })
  }
}

pub const VM_CONTEXT_INDEX: usize = 0;

#[derive(PartialEq)]
pub enum ContextInitMode {
  ForSnapshot,
  UseSnapshot,
}

pub fn create_v8_context<'a>(
  scope: &mut v8::PinScope<'a, '_, ()>,
  object_template: v8::Local<v8::ObjectTemplate>,
  mode: ContextInitMode,
  microtask_queue: *mut v8::MicrotaskQueue,
) -> v8::Local<'a, v8::Context> {
  let scope = std::pin::pin!(v8::EscapableHandleScope::new(scope));
  let scope = &mut scope.init();

  let context = if mode == ContextInitMode::UseSnapshot {
    v8::Context::from_snapshot(
      scope,
      VM_CONTEXT_INDEX,
      v8::ContextOptions {
        microtask_queue: Some(microtask_queue),
        ..Default::default()
      },
    )
    .unwrap()
  } else {
    let ctx = v8::Context::new(
      scope,
      v8::ContextOptions {
        global_template: Some(object_template),
        microtask_queue: Some(microtask_queue),
        ..Default::default()
      },
    );
    // SAFETY: ContextifyContexts will update this to a pointer to the native object
    unsafe {
      ctx.set_aligned_pointer_in_embedder_data(1, std::ptr::null_mut());
      ctx.set_aligned_pointer_in_embedder_data(2, std::ptr::null_mut());
      ctx.set_aligned_pointer_in_embedder_data(3, std::ptr::null_mut());
      ctx.clear_all_slots();
    };
    ctx
  };

  scope.escape(context)
}

#[derive(Debug, Clone)]
struct SlotContextifyGlobalTemplate(v8::Global<v8::ObjectTemplate>);

pub fn init_global_template<'a>(
  scope: &mut v8::PinScope<'a, '_, ()>,
  mode: ContextInitMode,
) -> v8::Local<'a, v8::ObjectTemplate> {
  let maybe_object_template_slot =
    scope.get_slot::<SlotContextifyGlobalTemplate>();

  if let Some(object_template_slot) = maybe_object_template_slot {
    v8::Local::new(scope, object_template_slot.clone().0)
  } else {
    let global_object_template = init_global_template_inner(scope);

    if mode == ContextInitMode::UseSnapshot {
      let contextify_global_template_slot = SlotContextifyGlobalTemplate(
        v8::Global::new(scope, global_object_template),
      );
      scope.set_slot(contextify_global_template_slot);
    }
    global_object_template
  }
}

// Using thread_local! to get around compiler bug.
//
// See NOTE in ext/node/global.rs#L12
thread_local! {
  pub static QUERY_MAP_FN: v8::NamedPropertyQueryCallback = property_query.map_fn_to();
  pub static GETTER_MAP_FN: v8::NamedPropertyGetterCallback = property_getter.map_fn_to();
  pub static SETTER_MAP_FN: v8::NamedPropertySetterCallback = property_setter.map_fn_to();
  pub static DELETER_MAP_FN: v8::NamedPropertyDeleterCallback = property_deleter.map_fn_to();
  pub static ENUMERATOR_MAP_FN: v8::NamedPropertyEnumeratorCallback = property_enumerator.map_fn_to();
  pub static DEFINER_MAP_FN: v8::NamedPropertyDefinerCallback = property_definer.map_fn_to();
  pub static DESCRIPTOR_MAP_FN: v8::NamedPropertyDescriptorCallback = property_descriptor.map_fn_to();
}

thread_local! {
  pub static INDEXED_GETTER_MAP_FN: v8::IndexedPropertyGetterCallback = indexed_property_getter.map_fn_to();
  pub static INDEXED_SETTER_MAP_FN: v8::IndexedPropertySetterCallback = indexed_property_setter.map_fn_to();
  pub static INDEXED_DELETER_MAP_FN: v8::IndexedPropertyDeleterCallback = indexed_property_deleter.map_fn_to();
  pub static INDEXED_DEFINER_MAP_FN: v8::IndexedPropertyDefinerCallback = indexed_property_definer.map_fn_to();
  pub static INDEXED_DESCRIPTOR_MAP_FN: v8::IndexedPropertyDescriptorCallback = indexed_property_descriptor.map_fn_to();
  pub static INDEXED_ENUMERATOR_MAP_FN: v8::IndexedPropertyEnumeratorCallback = indexed_property_enumerator.map_fn_to();
  pub static INDEXED_QUERY_MAP_FN: v8::IndexedPropertyQueryCallback = indexed_property_query.map_fn_to();
}

pub fn init_global_template_inner<'a, 'b, 'i>(
  scope: &'b mut v8::PinScope<'a, 'i, ()>,
) -> v8::Local<'a, v8::ObjectTemplate> {
  let scope = std::pin::pin!(v8::EscapableHandleScope::new(scope));
  let scope = &mut scope.init();

  let global_object_template = v8::ObjectTemplate::new(scope);
  global_object_template.set_internal_field_count(3);

  let named_property_handler_config = {
    let mut config = v8::NamedPropertyHandlerConfiguration::new()
      .flags(v8::PropertyHandlerFlags::HAS_NO_SIDE_EFFECT);

    config = GETTER_MAP_FN.with(|getter| config.getter_raw(*getter));
    config = SETTER_MAP_FN.with(|setter| config.setter_raw(*setter));
    config = QUERY_MAP_FN.with(|query| config.query_raw(*query));
    config = DELETER_MAP_FN.with(|deleter| config.deleter_raw(*deleter));
    config =
      ENUMERATOR_MAP_FN.with(|enumerator| config.enumerator_raw(*enumerator));
    config = DEFINER_MAP_FN.with(|definer| config.definer_raw(*definer));
    config =
      DESCRIPTOR_MAP_FN.with(|descriptor| config.descriptor_raw(*descriptor));

    config
  };

  let indexed_property_handler_config = {
    let mut config = v8::IndexedPropertyHandlerConfiguration::new()
      .flags(v8::PropertyHandlerFlags::HAS_NO_SIDE_EFFECT);

    config = INDEXED_GETTER_MAP_FN.with(|getter| config.getter_raw(*getter));
    config = INDEXED_SETTER_MAP_FN.with(|setter| config.setter_raw(*setter));
    config = INDEXED_QUERY_MAP_FN.with(|query| config.query_raw(*query));
    config =
      INDEXED_DELETER_MAP_FN.with(|deleter| config.deleter_raw(*deleter));
    config = INDEXED_ENUMERATOR_MAP_FN
      .with(|enumerator| config.enumerator_raw(*enumerator));
    config =
      INDEXED_DEFINER_MAP_FN.with(|definer| config.definer_raw(*definer));
    config = INDEXED_DESCRIPTOR_MAP_FN
      .with(|descriptor| config.descriptor_raw(*descriptor));

    config
  };

  global_object_template
    .set_named_property_handler(named_property_handler_config);
  global_object_template
    .set_indexed_property_handler(indexed_property_handler_config);

  scope.escape(global_object_template)
}

fn property_query<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  property: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue<v8::Integer>,
) -> v8::Intercepted {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return v8::Intercepted::kNo;
  };

  let context = ctx.context(scope);
  let scope = &mut v8::ContextScope::new(scope, context);
  let Some(sandbox) = ctx.sandbox(scope) else {
    return v8::Intercepted::kNo;
  };

  // Use `Has` rather than `HasRealNamedProperty` for the sandbox so the
  // `in` operator walks the sandbox's prototype chain, matching Node's
  // behaviour. With the own-only check, a user that does
  // `Object.setPrototypeOf(sandbox, someProto)` would not see properties
  // from `someProto` reachable via `propName in window` inside the vm
  // context.
  //
  // The fallback path on the global proxy keeps using
  // `HasRealNamedProperty` to avoid recursing back into this interceptor:
  // the global proxy carries the named-property handler that brought us
  // here, so a regular `Has` would re-enter `property_query` infinitely.
  let property_value: v8::Local<v8::Value> = property.into();
  match sandbox.has(scope, property_value) {
    None => v8::Intercepted::kNo,
    Some(true) => {
      let attr = sandbox
        .get_property_attributes(scope, property_value)
        .map(|a| a.as_u32())
        .unwrap_or(0);
      rv.set_uint32(attr);
      v8::Intercepted::kYes
    }
    Some(false) => {
      match ctx
        .global_proxy(scope)
        .has_real_named_property(scope, property)
      {
        None => v8::Intercepted::kNo,
        Some(true) => {
          let Some(attr) = ctx
            .global_proxy(scope)
            .get_real_named_property_attributes(scope, property)
          else {
            return v8::Intercepted::kNo;
          };
          rv.set_uint32(attr.as_u32());
          v8::Intercepted::kYes
        }
        Some(false) => v8::Intercepted::kNo,
      }
    }
  }
}

fn property_getter<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut ret: v8::ReturnValue,
) -> v8::Intercepted {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return v8::Intercepted::kNo;
  };

  let Some(sandbox) = ctx.sandbox(scope) else {
    return v8::Intercepted::kNo;
  };

  v8::tc_scope!(tc_scope, scope);
  let maybe_rv = sandbox.get_real_named_property(tc_scope, key).or_else(|| {
    ctx
      .global_proxy(tc_scope)
      .get_real_named_property(tc_scope, key)
  });

  if let Some(mut rv) = maybe_rv {
    if tc_scope.has_caught() && !tc_scope.has_terminated() {
      tc_scope.rethrow();
    }

    if rv == sandbox {
      rv = ctx.global_proxy(tc_scope).into();
    }

    ret.set(rv);
    return v8::Intercepted::kYes;
  }

  v8::Intercepted::kNo
}

fn property_setter<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Name>,
  value: v8::Local<'s, v8::Value>,
  args: v8::PropertyCallbackArguments<'s>,
  _rv: v8::ReturnValue<()>,
) -> v8::Intercepted {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return v8::Intercepted::kNo;
  };

  let (attributes, is_declared_on_global_proxy) = match ctx
    .global_proxy(scope)
    .get_real_named_property_attributes(scope, key)
  {
    Some(attr) => (attr, true),
    None => (v8::PropertyAttribute::NONE, false),
  };
  let mut read_only = attributes.is_read_only();
  let Some(sandbox) = ctx.sandbox(scope) else {
    return v8::Intercepted::kNo;
  };
  let (attributes, is_declared_on_sandbox) =
    match sandbox.get_real_named_property_attributes(scope, key) {
      Some(attr) => (attr, true),
      None => (v8::PropertyAttribute::NONE, false),
    };
  read_only |= attributes.is_read_only();

  if read_only {
    return v8::Intercepted::kNo;
  }

  // true for x = 5
  // false for this.x = 5
  // false for Object.defineProperty(this, 'foo', ...)
  // false for vmResult.x = 5 where vmResult = vm.runInContext();
  let is_contextual_store = ctx.global_proxy(scope) != args.holder();

  // Indicator to not return before setting (undeclared) function declarations
  // on the sandbox in strict mode, i.e. args.ShouldThrowOnError() = true.
  // True for 'function f() {}', 'this.f = function() {}',
  // 'var f = function()'.
  // In effect only for 'function f() {}' because
  // var f = function(), is_declared = true
  // this.f = function() {}, is_contextual_store = false.
  let is_function = value.is_function();

  let is_declared = is_declared_on_global_proxy || is_declared_on_sandbox;
  if !is_declared
    && args.should_throw_on_error()
    && is_contextual_store
    && !is_function
  {
    return v8::Intercepted::kNo;
  }

  if !is_declared && key.is_symbol() {
    return v8::Intercepted::kNo;
  };

  if sandbox.set(scope, key.into(), value).is_none() {
    return v8::Intercepted::kNo;
  }

  if is_declared_on_sandbox
    && let Some(desc) = sandbox.get_own_property_descriptor(scope, key)
    && !desc.is_undefined()
  {
    let desc_obj: v8::Local<v8::Object> = desc.try_into().unwrap();
    // We have to specify the return value for any contextual or get/set
    // property
    let get_key =
      v8::String::new_external_onebyte_static(scope, b"get").unwrap();
    let set_key =
      v8::String::new_external_onebyte_static(scope, b"set").unwrap();
    if desc_obj
      .has_own_property(scope, get_key.into())
      .unwrap_or(false)
      || desc_obj
        .has_own_property(scope, set_key.into())
        .unwrap_or(false)
    {
      return v8::Intercepted::kYes;
    }
  }

  v8::Intercepted::kNo
}

fn property_descriptor<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) -> v8::Intercepted {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return v8::Intercepted::kNo;
  };

  let context = ctx.context(scope);
  let Some(sandbox) = ctx.sandbox(scope) else {
    return v8::Intercepted::kNo;
  };
  let scope = &mut v8::ContextScope::new(scope, context);

  if sandbox.has_own_property(scope, key).unwrap_or(false)
    && let Some(desc) = sandbox.get_own_property_descriptor(scope, key)
  {
    rv.set(desc);
    return v8::Intercepted::kYes;
  }

  v8::Intercepted::kNo
}

fn property_definer<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Name>,
  desc: &v8::PropertyDescriptor,
  args: v8::PropertyCallbackArguments<'s>,
  _: v8::ReturnValue<()>,
) -> v8::Intercepted {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return v8::Intercepted::kNo;
  };

  let context = ctx.context(scope);
  let scope = &mut v8::ContextScope::new(scope, context);

  let (attributes, is_declared) = match ctx
    .global_proxy(scope)
    .get_real_named_property_attributes(scope, key)
  {
    Some(attr) => (attr, true),
    None => (v8::PropertyAttribute::NONE, false),
  };

  let read_only = attributes.is_read_only();
  let dont_delete = attributes.is_dont_delete();

  // If the property is set on the global as read_only, don't change it on
  // the global or sandbox.
  if is_declared && read_only && dont_delete {
    return v8::Intercepted::kNo;
  }

  let Some(sandbox) = ctx.sandbox(scope) else {
    return v8::Intercepted::kNo;
  };

  let define_prop_on_sandbox =
    |scope: &mut v8::PinScope<'_, '_>,
     desc_for_sandbox: &mut v8::PropertyDescriptor| {
      if desc.has_enumerable() {
        desc_for_sandbox.set_enumerable(desc.enumerable());
      }

      if desc.has_configurable() {
        desc_for_sandbox.set_configurable(desc.configurable());
      }

      sandbox.define_property(scope, key, desc_for_sandbox);
    };

  if desc.has_get() || desc.has_set() {
    let mut desc_for_sandbox = v8::PropertyDescriptor::new_from_get_set(
      if desc.has_get() {
        desc.get()
      } else {
        v8::undefined(scope).into()
      },
      if desc.has_set() {
        desc.set()
      } else {
        v8::undefined(scope).into()
      },
    );

    define_prop_on_sandbox(scope, &mut desc_for_sandbox);
  } else {
    let value = if desc.has_value() {
      desc.value()
    } else {
      v8::undefined(scope).into()
    };

    if desc.has_writable() {
      let mut desc_for_sandbox =
        v8::PropertyDescriptor::new_from_value_writable(value, desc.writable());
      define_prop_on_sandbox(scope, &mut desc_for_sandbox);
    } else {
      let mut desc_for_sandbox = v8::PropertyDescriptor::new_from_value(value);
      define_prop_on_sandbox(scope, &mut desc_for_sandbox);
    }
  }

  v8::Intercepted::kYes
}

fn property_deleter<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue<v8::Boolean>,
) -> v8::Intercepted {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return v8::Intercepted::kNo;
  };

  let context = ctx.context(scope);
  let Some(sandbox) = ctx.sandbox(scope) else {
    return v8::Intercepted::kNo;
  };

  let context_scope = &mut v8::ContextScope::new(scope, context);
  if sandbox.delete(context_scope, key.into()).unwrap_or(false) {
    return v8::Intercepted::kNo;
  }

  rv.set_bool(false);
  v8::Intercepted::kYes
}

fn property_enumerator<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue<v8::Array>,
) {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return;
  };

  let context = ctx.context(scope);
  let Some(sandbox) = ctx.sandbox(scope) else {
    return;
  };

  let context_scope = &mut v8::ContextScope::new(scope, context);
  let args = v8::GetPropertyNamesArgsBuilder::new()
    .mode(v8::KeyCollectionMode::OwnOnly)
    .property_filter(v8::PropertyFilter::ALL_PROPERTIES)
    .index_filter(v8::IndexFilter::SkipIndices)
    .key_conversion(v8::KeyConversionMode::ConvertToString)
    .build();
  let Some(properties) = sandbox.get_property_names(context_scope, args) else {
    return;
  };

  rv.set(properties);
}

fn indexed_property_enumerator<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue<v8::Array>,
) {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return;
  };
  let context = ctx.context(scope);
  let scope = &mut v8::ContextScope::new(scope, context);
  let Some(sandbox) = ctx.sandbox(scope) else {
    return;
  };

  // Return only indexed (numeric) own properties, including non-enumerable.
  let args = v8::GetPropertyNamesArgsBuilder::new()
    .mode(v8::KeyCollectionMode::OwnOnly)
    .property_filter(v8::PropertyFilter::ALL_PROPERTIES)
    .index_filter(v8::IndexFilter::IncludeIndices)
    .key_conversion(v8::KeyConversionMode::KeepNumbers)
    .build();
  let Some(properties) = sandbox.get_property_names(scope, args) else {
    return;
  };

  let Ok(properties_vec) =
    serde_v8::from_v8::<Vec<serde_v8::Value>>(scope, properties.into())
  else {
    return;
  };

  let mut indices = vec![];
  for prop in properties_vec {
    if prop.v8_value.is_number() {
      indices.push(prop.v8_value);
    }
  }

  rv.set(v8::Array::new_with_elements(scope, &indices));
}

fn uint32_to_name<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  index: u32,
) -> v8::Local<'s, v8::Name> {
  let int = v8::Integer::new_from_unsigned(scope, index);
  let u32 = v8::Local::<v8::Uint32>::try_from(int).unwrap();
  u32.to_string(scope).unwrap().into()
}

fn indexed_property_query<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  index: u32,
  args: v8::PropertyCallbackArguments<'s>,
  rv: v8::ReturnValue<v8::Integer>,
) -> v8::Intercepted {
  let name = uint32_to_name(scope, index);
  property_query(scope, name, args, rv)
}

fn indexed_property_getter<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  index: u32,
  args: v8::PropertyCallbackArguments<'s>,
  rv: v8::ReturnValue,
) -> v8::Intercepted {
  let key = uint32_to_name(scope, index);
  property_getter(scope, key, args, rv)
}

fn indexed_property_setter<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  index: u32,
  value: v8::Local<'s, v8::Value>,
  args: v8::PropertyCallbackArguments<'s>,
  rv: v8::ReturnValue<()>,
) -> v8::Intercepted {
  let key = uint32_to_name(scope, index);
  property_setter(scope, key, value, args, rv)
}

fn indexed_property_descriptor<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  index: u32,
  args: v8::PropertyCallbackArguments<'s>,
  rv: v8::ReturnValue,
) -> v8::Intercepted {
  let key = uint32_to_name(scope, index);
  property_descriptor(scope, key, args, rv)
}

fn indexed_property_definer<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  index: u32,
  descriptor: &v8::PropertyDescriptor,
  args: v8::PropertyCallbackArguments<'s>,
  rv: v8::ReturnValue<()>,
) -> v8::Intercepted {
  let key = uint32_to_name(scope, index);
  property_definer(scope, key, descriptor, args, rv)
}

fn indexed_property_deleter<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  index: u32,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue<v8::Boolean>,
) -> v8::Intercepted {
  let Some(ctx) = ContextifyContext::get(scope, args.holder()) else {
    return v8::Intercepted::kNo;
  };

  let context = ctx.context(scope);
  let Some(sandbox) = ctx.sandbox(scope) else {
    return v8::Intercepted::kNo;
  };

  let context_scope = &mut v8::ContextScope::new(scope, context);
  if !sandbox.delete_index(context_scope, index).unwrap_or(false) {
    return v8::Intercepted::kNo;
  }

  // Delete failed on the sandbox, intercept and do not delete on
  // the global object.
  rv.set_bool(false);
  v8::Intercepted::kNo
}

#[allow(clippy::too_many_arguments, reason = "op")]
#[op2]
#[serde]
pub fn op_vm_create_script<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  source: v8::Local<'a, v8::String>,
  filename: v8::Local<'a, v8::Value>,
  line_offset: i32,
  column_offset: i32,
  #[buffer] cached_data: Option<JsBuffer>,
  produce_cached_data: bool,
  parsing_context: Option<v8::Local<'a, v8::Object>>,
) -> Option<CompileResult<'a>> {
  ContextifyScript::create(
    scope,
    source,
    filename,
    line_offset,
    column_offset,
    cached_data,
    produce_cached_data,
    parsing_context,
  )
}

#[op2(reentrant)]
pub fn op_vm_script_run_in_context<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[cppgc] script: &ContextifyScript,
  sandbox: Option<v8::Local<'a, v8::Object>>,
  #[serde] timeout: i64,
  display_errors: bool,
  break_on_sigint: bool,
) -> Option<v8::Local<'a, v8::Value>> {
  script.run_in_context(
    scope,
    sandbox,
    timeout,
    display_errors,
    break_on_sigint,
  )
}

#[op2(fast)]
pub fn op_vm_create_context(
  scope: &mut v8::PinScope<'_, '_>,
  sandbox_obj: v8::Local<v8::Object>,
  #[string] name: String,
  #[string] origin: String,
  allow_code_gen_strings: bool,
  allow_code_gen_wasm: bool,
  own_microtask_queue: bool,
) {
  // Don't allow contextifying a sandbox multiple times.
  assert!(!ContextifyContext::is_contextify_context(
    scope,
    sandbox_obj
  ));

  ContextifyContext::attach(
    scope,
    sandbox_obj,
    name,
    origin,
    allow_code_gen_strings,
    allow_code_gen_wasm,
    own_microtask_queue,
  );
}

#[op2]
pub fn op_vm_create_context_without_contextify<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  allow_code_gen_strings: bool,
  allow_code_gen_wasm: bool,
  own_microtask_queue: bool,
) -> v8::Local<'s, v8::Value> {
  ContextifyContext::attach_vanilla(
    scope,
    allow_code_gen_strings,
    allow_code_gen_wasm,
    own_microtask_queue,
  )
  .into()
}

#[op2(fast)]
pub fn op_vm_is_context(
  scope: &mut v8::PinScope<'_, '_>,
  sandbox_obj: v8::Local<v8::Value>,
) -> bool {
  sandbox_obj
    .try_into()
    .map(|sandbox_obj| {
      ContextifyContext::is_contextify_context(scope, sandbox_obj)
    })
    .unwrap_or(false)
}

#[derive(serde::Serialize)]
struct CompileResult<'s> {
  value: serde_v8::Value<'s>,
  cached_data: Option<serde_v8::Value<'s>>,
  cached_data_rejected: bool,
  cached_data_produced: bool,
}

#[allow(clippy::too_many_arguments, reason = "op")]
#[op2]
#[serde]
pub fn op_vm_compile_function<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  source: v8::Local<'s, v8::String>,
  filename: v8::Local<'s, v8::Value>,
  line_offset: i32,
  column_offset: i32,
  #[buffer] cached_data: Option<JsBuffer>,
  produce_cached_data: bool,
  parsing_context: Option<v8::Local<'s, v8::Object>>,
  context_extensions: Option<v8::Local<'s, v8::Array>>,
  params: Option<v8::Local<'s, v8::Array>>,
) -> Option<CompileResult<'s>> {
  let context = if let Some(parsing_context) = parsing_context {
    let Some(context) =
      ContextifyContext::from_sandbox_obj(scope, parsing_context)
    else {
      let message = v8::String::new(scope, "Invalid sandbox object").unwrap();
      let exception = v8::Exception::type_error(scope, message);
      scope.throw_exception(exception);
      return None;
    };
    context.context(scope)
  } else {
    scope.get_current_context()
  };

  let scope = &mut v8::ContextScope::new(scope, context);
  let host_defined_options = create_host_defined_options(scope);
  let origin = v8::ScriptOrigin::new(
    scope,
    filename,
    line_offset,
    column_offset,
    true,
    -1,
    None,
    false,
    false,
    false,
    Some(host_defined_options),
  );

  let mut source = if let Some(cached_data) = cached_data {
    let cached_data = v8::script_compiler::CachedData::new(&cached_data);
    v8::script_compiler::Source::new_with_cached_data(
      source,
      Some(&origin),
      cached_data,
    )
  } else {
    v8::script_compiler::Source::new(source, Some(&origin))
  };

  let context_extensions = if let Some(context_extensions) = context_extensions
  {
    let mut exts = Vec::with_capacity(context_extensions.length() as _);
    for i in 0..context_extensions.length() {
      let ext = context_extensions.get_index(scope, i)?.try_into().ok()?;
      exts.push(ext);
    }
    exts
  } else {
    vec![]
  };

  let params = if let Some(params) = params {
    let mut exts = Vec::with_capacity(params.length() as _);
    for i in 0..params.length() {
      let ext = params.get_index(scope, i)?.try_into().ok()?;
      exts.push(ext);
    }
    exts
  } else {
    vec![]
  };

  let options = if source.get_cached_data().is_some() {
    v8::script_compiler::CompileOptions::ConsumeCodeCache
  } else {
    v8::script_compiler::CompileOptions::NoCompileOptions
  };

  v8::tc_scope!(scope, scope);
  let Some(function) = v8::script_compiler::compile_function(
    scope,
    &mut source,
    &params,
    &context_extensions,
    options,
    v8::script_compiler::NoCacheReason::NoReason,
  ) else {
    if scope.has_caught() && !scope.has_terminated() {
      scope.rethrow();
    }
    return None;
  };

  let cached_data = if produce_cached_data {
    function.create_code_cache()
  } else {
    None
  };

  Some(CompileResult {
    value: serde_v8::Value {
      v8_value: function.into(),
    },
    cached_data: cached_data.as_ref().map(|c| {
      let backing_store =
        v8::ArrayBuffer::new_backing_store_from_vec(c.to_vec());
      v8::ArrayBuffer::with_backing_store(scope, &backing_store.make_shared())
        .into()
    }),
    cached_data_rejected: source
      .get_cached_data()
      .map(|c| c.rejected())
      .unwrap_or(false),
    cached_data_produced: cached_data.is_some(),
  })
}

#[op2]
pub fn op_vm_script_get_source_map_url<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  #[cppgc] script: &ContextifyScript,
) -> v8::Local<'s, v8::Value> {
  let unbound_script = script.script.get(scope).unwrap();
  unbound_script.get_source_mapping_url(scope)
}

#[op2]
pub fn op_vm_script_create_cached_data<'s>(
  scope: &mut v8::PinScope<'s, '_>,
  #[cppgc] script: &ContextifyScript,
) -> v8::Local<'s, v8::Value> {
  let unbound_script = script.script.get(scope).unwrap();
  let data = match unbound_script.create_code_cache() {
    Some(c) => c.to_vec(),
    None => vec![],
  };
  let backing_store = v8::ArrayBuffer::new_backing_store_from_vec(data);
  v8::ArrayBuffer::with_backing_store(scope, &backing_store.make_shared())
    .into()
}

/// Wraps a v8::Module compiled in a contextified context.
pub struct ContextifyModule {
  module: v8::TracedReference<v8::Module>,
  context: v8::TracedReference<v8::Context>,
  microtask_queue: *mut v8::MicrotaskQueue,
  identifier: String,
  /// Map of import specifier -> resolved module wrapper.
  /// Populated by `op_vm_module_link` before instantiation, and consumed by
  /// the resolve callback during `instantiate_module`.
  resolutions: RefCell<HashMap<String, v8::TracedReference<v8::Module>>>,
  /// Top-level evaluation promise, kept alive across calls.
  evaluation_promise: RefCell<Option<v8::TracedReference<v8::Promise>>>,
}

// SAFETY: all v8 references are visited during cppgc trace.
unsafe impl v8::cppgc::GarbageCollected for ContextifyModule {
  fn trace(&self, visitor: &mut v8::cppgc::Visitor) {
    visitor.trace(&self.module);
    visitor.trace(&self.context);
    for r in self.resolutions.borrow().values() {
      visitor.trace(r);
    }
    if let Some(p) = self.evaluation_promise.borrow().as_ref() {
      visitor.trace(p);
    }
  }

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"ContextifyModule"
  }
}

#[op2]
#[cppgc]
pub fn op_vm_module_create_source_text_module<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  source: v8::Local<'a, v8::String>,
  #[string] identifier: String,
  line_offset: i32,
  column_offset: i32,
  context_object: v8::Local<'a, v8::Object>,
) -> Option<ContextifyModule> {
  let contextify = ContextifyContext::from_sandbox_obj(scope, context_object)?;
  let context = contextify.context(scope);
  let microtask_queue = contextify.microtask_queue;

  let scope = &mut v8::ContextScope::new(scope, context);
  let host_defined_options = create_host_defined_options(scope);
  let filename = v8::String::new(scope, &identifier)?;
  let origin = v8::ScriptOrigin::new(
    scope,
    filename.into(),
    line_offset,
    column_offset,
    true,
    -1,
    None,
    false,
    false,
    true, // is_module
    Some(host_defined_options),
  );

  let mut compile_source =
    v8::script_compiler::Source::new(source, Some(&origin));

  v8::tc_scope!(scope, scope);
  let module = v8::script_compiler::compile_module(scope, &mut compile_source);
  if scope.has_caught() {
    scope.rethrow();
    return None;
  }
  let module = module?;

  Some(ContextifyModule {
    module: v8::TracedReference::new(scope, module),
    context: v8::TracedReference::new(scope, context),
    microtask_queue,
    identifier,
    resolutions: RefCell::new(HashMap::new()),
    evaluation_promise: RefCell::new(None),
  })
}

#[op2(fast)]
pub fn op_vm_module_link<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[cppgc] this: &ContextifyModule,
  specifiers: v8::Local<'a, v8::Array>,
  modules: v8::Local<'a, v8::Array>,
) -> bool {
  let len = specifiers.length();
  if modules.length() != len {
    let message = v8::String::new(
      scope,
      "specifiers and modules arrays must have the same length",
    )
    .unwrap();
    let exception = v8::Exception::error(scope, message);
    scope.throw_exception(exception);
    return false;
  }

  let mut resolutions = this.resolutions.borrow_mut();
  resolutions.clear();
  for i in 0..len {
    let Some(specifier) = specifiers.get_index(scope, i) else {
      return false;
    };
    let Some(module_obj) = modules.get_index(scope, i) else {
      return false;
    };
    let specifier_str = specifier.to_rust_string_lossy(scope);
    let module_obj: v8::Local<v8::Object> = module_obj.try_into().unwrap();
    let other = deno_core::cppgc::try_unwrap_cppgc_object::<ContextifyModule>(
      scope,
      module_obj.into(),
    );
    let Some(other) = other else {
      let message =
        v8::String::new(scope, "expected ContextifyModule").unwrap();
      let exception = v8::Exception::error(scope, message);
      scope.throw_exception(exception);
      return false;
    };
    resolutions.insert(
      specifier_str,
      v8::TracedReference::new(scope, other.module.get(scope).unwrap()),
    );
  }
  true
}

#[op2(fast, reentrant)]
pub fn op_vm_module_instantiate(
  scope: &mut v8::PinScope<'_, '_>,
  #[cppgc] this: &ContextifyModule,
) -> bool {
  let context = this.context.get(scope).unwrap();
  let module = this.module.get(scope).unwrap();
  let referrer_hash = module.get_identity_hash();

  // Build a map of specifier -> v8::Global<Module> usable from the resolve
  // callback (which runs inside instantiate_module).
  let mut specifier_to_module: HashMap<String, v8::Global<v8::Module>> =
    HashMap::new();
  {
    let resolutions = this.resolutions.borrow();
    for (sp, tref) in resolutions.iter() {
      let Some(m) = tref.get(scope) else { continue };
      specifier_to_module.insert(sp.clone(), v8::Global::new(scope, m));
    }
  }

  MODULE_RESOLUTIONS.with(|r| {
    r.borrow_mut().insert(referrer_hash, specifier_to_module);
  });

  let scope = &mut v8::ContextScope::new(scope, context);
  v8::tc_scope!(scope, scope);

  let result = module.instantiate_module(scope, module_resolve_callback);

  MODULE_RESOLUTIONS.with(|r| {
    r.borrow_mut().remove(&referrer_hash);
  });

  if scope.has_caught() {
    scope.rethrow();
    return false;
  }

  result.unwrap_or(false)
}

thread_local! {
  /// Map from referrer module identity hash -> (specifier -> resolved module global).
  /// Populated only for the duration of an instantiate_module call.
  static MODULE_RESOLUTIONS: RefCell<HashMap<NonZeroI32, HashMap<String, v8::Global<v8::Module>>>> =
    RefCell::new(HashMap::new());
}

fn module_resolve_callback<'s>(
  context: v8::Local<'s, v8::Context>,
  specifier: v8::Local<'s, v8::String>,
  _import_attributes: v8::Local<'s, v8::FixedArray>,
  referrer: v8::Local<'s, v8::Module>,
) -> Option<v8::Local<'s, v8::Module>> {
  // SAFETY: callback runs inside an active V8 callback context.
  let mut scope_storage = unsafe { v8::CallbackScope::new(context) };
  // SAFETY: scope_storage stays in scope for the rest of this function.
  let mut scope_pin =
    unsafe { std::pin::Pin::new_unchecked(&mut scope_storage) };
  let scope = &mut scope_pin.as_mut().init();

  let specifier_str = specifier.to_rust_string_lossy(scope);
  let referrer_hash = referrer.get_identity_hash();

  let resolved_local = MODULE_RESOLUTIONS.with(|r| {
    let map = r.borrow();
    let resolved_global = map
      .get(&referrer_hash)
      .and_then(|m| m.get(&specifier_str))?;
    Some(v8::Local::new(scope, resolved_global))
  });

  if let Some(local) = resolved_local {
    return Some(local);
  }

  let message = v8::String::new(
    scope,
    &format!(
      "Cannot find module '{specifier_str}' (linker did not provide it)"
    ),
  )?;
  let exception = v8::Exception::error(scope, message);
  scope.throw_exception(exception);
  None
}

#[op2(reentrant)]
pub fn op_vm_module_evaluate<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[cppgc] this: &ContextifyModule,
) -> Option<v8::Local<'a, v8::Value>> {
  let inner_context = this.context.get(scope).unwrap();
  let module = this.module.get(scope).unwrap();
  let outer_context = scope.get_current_context();

  // Enter the module's context, evaluate the module, then come back out.
  let inner_result = {
    let scope = &mut v8::ContextScope::new(scope, inner_context);
    v8::tc_scope!(scope, scope);
    let r = module.evaluate(scope);
    if scope.has_caught() {
      scope.rethrow();
      return None;
    }
    r
  };
  let inner_result = inner_result?;

  // If the module's context has its own microtask queue (microtaskMode:
  // "afterEvaluate"), wrap the inner promise in an outer-context promise so
  // that `await` from the outer context isn't queued on the inner queue and
  // silently dropped (https://github.com/nodejs/node/issues/59541).
  let returned: v8::Local<v8::Value> = if !this.microtask_queue.is_null()
    && let Ok(inner_promise) = v8::Local::<v8::Promise>::try_from(inner_result)
  {
    // We're currently in `outer_context` (we exited the inner ContextScope).
    let resolver = v8::PromiseResolver::new(scope)?;
    let outer_promise = resolver.get_promise(scope);
    resolver.resolve(scope, inner_promise.into())?;

    // Drain the inner microtask queue so the resolution chain can progress.
    // SAFETY: pointer is validated as non-null above.
    let mtask_queue = unsafe { &*this.microtask_queue };
    mtask_queue.perform_checkpoint(scope);

    *this.evaluation_promise.borrow_mut() =
      Some(v8::TracedReference::new(scope, outer_promise));
    outer_promise.into()
  } else {
    if let Ok(p) = v8::Local::<v8::Promise>::try_from(inner_result) {
      *this.evaluation_promise.borrow_mut() =
        Some(v8::TracedReference::new(scope, p));
    }
    inner_result
  };

  // Suppress unused warning when both contexts are the same.
  let _ = outer_context;
  Some(returned)
}

#[op2(fast)]
pub fn op_vm_module_get_status(
  #[cppgc] this: &ContextifyModule,
  scope: &mut v8::PinScope<'_, '_>,
) -> u32 {
  let module = this.module.get(scope).unwrap();
  module.get_status() as u32
}

#[op2]
pub fn op_vm_module_get_namespace<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[cppgc] this: &ContextifyModule,
) -> v8::Local<'a, v8::Value> {
  let module = this.module.get(scope).unwrap();
  module.get_module_namespace()
}

#[op2]
pub fn op_vm_module_get_exception<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[cppgc] this: &ContextifyModule,
) -> v8::Local<'a, v8::Value> {
  let module = this.module.get(scope).unwrap();
  if module.get_status() != v8::ModuleStatus::Errored {
    return v8::undefined(scope).into();
  }
  module.get_exception()
}

#[op2]
#[serde]
pub fn op_vm_module_get_module_requests(
  scope: &mut v8::PinScope<'_, '_>,
  #[cppgc] this: &ContextifyModule,
) -> Vec<String> {
  let module = this.module.get(scope).unwrap();
  let requests = module.get_module_requests();
  let len = requests.length();
  let mut out = Vec::with_capacity(len);
  for i in 0..len {
    let Some(req) = requests.get(scope, i) else {
      continue;
    };
    let req: v8::Local<v8::ModuleRequest> = req.cast();
    let specifier = req.get_specifier();
    out.push(specifier.to_rust_string_lossy(scope));
  }
  out
}

#[op2]
pub fn op_vm_module_get_identifier<'a>(
  scope: &mut v8::PinScope<'a, '_>,
  #[cppgc] this: &ContextifyModule,
) -> v8::Local<'a, v8::String> {
  v8::String::new(scope, &this.identifier).unwrap()
}
