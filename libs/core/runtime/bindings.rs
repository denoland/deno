// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::os::raw::c_void;
use std::path::PathBuf;
use url::Url;
use v8::MapFnTo;

use super::jsruntime::BUILTIN_SOURCES;
use super::jsruntime::CONTEXT_SETUP_SOURCES;
use super::v8_static_strings::*;
use crate::_ops::OpMethodDecl;
use crate::AccessorType;
use crate::FastStaticString;
use crate::FastString;
use crate::JsRuntime;
use crate::ModuleType;
use crate::OpDecl;
use crate::cppgc::FunctionTemplateData;
use crate::cppgc::cppgc_template_constructor;
use crate::error::CoreError;
use crate::error::CoreModuleExecuteError;
use crate::error::CoreModuleParseError;
use crate::error::JsStackFrame;
use crate::error::callsite_fns;
use crate::error::has_call_site;
use crate::error::is_instance_of_error;
use crate::extension_set::LoadedSources;
use crate::modules::ImportAttributesKind;
use crate::modules::ModuleImportPhase;
use crate::modules::ModuleMap;
use crate::modules::get_requested_module_type_from_attributes;
use crate::modules::parse_import_attributes;
use crate::modules::synthetic_module_evaluation_steps;
use crate::ops::OpCtx;
use crate::runtime::InitMode;
use crate::runtime::JsRealm;
use deno_error::JsErrorBox;

pub(crate) fn create_external_references(
  ops: &[OpCtx],
  additional_references: &[v8::ExternalReference],
  sources: &[v8::OneByteConst],
  ops_in_snapshot: usize,
  sources_in_snapshot: usize,
) -> Vec<v8::ExternalReference> {
  // Overallocate a bit, it's better than having to resize the vector.
  let mut references = Vec::with_capacity(
    6 + CONTEXT_SETUP_SOURCES.len()
      + BUILTIN_SOURCES.len()
      + (ops.len() * 4)
      + additional_references.len()
      + sources.len()
      + 18 // for callsite_fns
      + 1, // nullptr
  );

  references.push(v8::ExternalReference {
    function: call_console.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: import_meta_resolve.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: catch_dynamic_import_promise_error.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: op_disabled_fn.map_fn_to(),
  });
  let syn_module_eval_fn: v8::SyntheticModuleEvaluationSteps =
    synthetic_module_evaluation_steps.map_fn_to();
  references.push(v8::ExternalReference {
    pointer: syn_module_eval_fn as *mut c_void,
  });

  references.push(v8::ExternalReference {
    function: cppgc_template_constructor.map_fn_to(),
  });

  // Using v8::OneByteConst and passing external references to it
  // allows V8 to take an optimized path when deserializing the snapshot.
  for source_file in &CONTEXT_SETUP_SOURCES {
    references.push(v8::ExternalReference {
      pointer: source_file.source.into_v8_const_ptr() as _,
    });
  }

  for source_file in &BUILTIN_SOURCES {
    references.push(v8::ExternalReference {
      pointer: source_file.source.into_v8_const_ptr() as _,
    });
  }

  references.extend_from_slice(additional_references);

  for ctx in &ops[..ops_in_snapshot] {
    references.extend_from_slice(&ctx.external_references());
  }

  for source in &sources[..sources_in_snapshot] {
    references.push(v8::ExternalReference {
      pointer: source as *const _ as _,
    })
  }

  for ctx in &ops[ops_in_snapshot..] {
    references.extend_from_slice(&ctx.external_references());
  }

  for source in &sources[sources_in_snapshot..] {
    references.push(v8::ExternalReference {
      pointer: source as *const _ as _,
    })
  }

  references.push(v8::ExternalReference {
    function: callsite_fns::get_this.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_type_name.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_function.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_function_name.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_method_name.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_file_name.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_script_name_or_source_url.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_line_number.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_column_number.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_eval_origin.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::is_toplevel.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::is_eval.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::is_native.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::is_constructor.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::is_async.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::is_promise_all.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::get_promise_index.map_fn_to(),
  });
  references.push(v8::ExternalReference {
    function: callsite_fns::to_string.map_fn_to(),
  });

  // null terminate so rusty_v8 doesn't have to make a copy.
  references.push(v8::ExternalReference {
    pointer: std::ptr::null_mut(),
  });

  references
}

/// Combine the snapshotted sources (which may be empty) with the loaded sources, and ensure that
/// each of the loaded source files passed to this function has a correct `v8::OneByteConst` backing
/// that can be used for compilation.
pub(crate) fn externalize_sources(
  sources: &mut LoadedSources,
  snapshot_sources: Vec<&'static [u8]>,
) -> (Box<[v8::OneByteConst]>, Box<[FastString]>) {
  // This is a complex method partly because we're still waiting on the `Copy` trait on v8::OneByteConst
  // to land.

  // Create an uninitialized Box<[v8::OneByteConst]. This will be simplified when we get Copy on OneByteConst.
  // Because we don't have that trait, we work around it with [usize; 3].
  const INIT_VALUE: MaybeUninit<[usize; 3]> =
    MaybeUninit::<[usize; 3]>::uninit();
  let externals =
    vec![INIT_VALUE; sources.len() + snapshot_sources.len()].into_boxed_slice();

  // Keep the original sources around, since we're borrowing from them.
  let mut original_sources = Vec::with_capacity(sources.len());

  // SAFETY: We are creating `v8::OneByteConst`s here for each of the input sources. Because
  // we keep the original source alive, we can safely make a `v8::OneByteConst` from _any_
  // source type. We'll make this lifetime static elsewhere in the code so we can safely
  // use it with v8 strings.
  unsafe {
    let mut externals: Box<[v8::OneByteConst]> = std::mem::transmute(externals);

    // First, add all the snapshot sources. These must be done first because we need
    // to ensure that snapshotted sources are added to the externalrefs before non-snapshotted
    // sources so that they line up correct.
    let offset = 0;
    for (index, source) in snapshot_sources.iter().enumerate() {
      externals[index + offset] =
        FastStaticString::create_external_onebyte_const(source);
    }

    // Next, add the non-snapshot sources. For each source file, we swap its `code`
    // member to use this new external string. Note that this is only safe because
    // we keep the original source alive.
    let offset = snapshot_sources.len();
    for (index, source) in sources.into_iter().enumerate() {
      externals[index + offset] =
        FastStaticString::create_external_onebyte_const(std::mem::transmute::<
          &[u8],
          &[u8],
        >(
          source.code.as_bytes(),
        ));
      let ptr = &externals[index + offset] as *const v8::OneByteConst;
      let original_source = std::mem::replace(
        &mut source.code,
        FastStaticString::from(&*ptr).into(),
      );
      original_sources.push(original_source)
    }

    (externals, original_sources.into_boxed_slice())
  }
}

pub(crate) fn get<'s, 'i, T>(
  scope: &mut v8::PinScope<'s, 'i>,
  from: v8::Local<'s, v8::Object>,
  key: FastStaticString,
  path: &'static str,
) -> T
where
  v8::Local<'s, v8::Value>: TryInto<T>,
{
  let key = key.v8_string(scope).unwrap();
  from
    .get(scope, key.into())
    .unwrap_or_else(|| panic!("{path} exists"))
    .try_into()
    .unwrap_or_else(|_| panic!("unable to convert"))
}

/// Create an object on the `globalThis` that looks like this:
/// ```ignore
/// globalThis.Deno = {
///   core: {
///     ops: {},
///   },
///   // console from V8
///   console,
///   // wrapper fn to forward message to V8 console
///   callConsole,
/// };
/// ```
pub(crate) fn initialize_deno_core_namespace<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  context: v8::Local<'s, v8::Context>,
  init_mode: InitMode,
) {
  let global = context.global(scope);
  let deno_str = DENO.v8_string(scope).unwrap();

  let maybe_deno_obj_val = global.get(scope, deno_str.into());

  // If `Deno.core` is already set up, let's exit early.
  if let Some(deno_obj_val) = maybe_deno_obj_val
    && !deno_obj_val.is_undefined()
  {
    return;
  }

  let deno_obj = v8::Object::new(scope);
  let deno_core_key = CORE.v8_string(scope).unwrap();
  // Set up `Deno.core.ops` object
  let deno_core_ops_obj = v8::Object::new(scope);
  let deno_core_ops_key = OPS.v8_string(scope).unwrap();

  let deno_core_obj = v8::Object::new(scope);
  deno_core_obj
    .set(scope, deno_core_ops_key.into(), deno_core_ops_obj.into())
    .unwrap();

  // If we're initializing fresh context set up the console
  if init_mode == InitMode::New {
    // Bind `call_console` to Deno.core.callConsole
    let call_console_fn = v8::Function::new(scope, call_console).unwrap();
    let call_console_key = CALL_CONSOLE.v8_string(scope).unwrap();
    deno_core_obj.set(scope, call_console_key.into(), call_console_fn.into());

    // Bind v8 console object to Deno.core.console
    let extra_binding_obj = context.get_extras_binding_object(scope);
    let console_obj: v8::Local<v8::Object> = get(
      scope,
      extra_binding_obj,
      CONSOLE,
      "ExtrasBindingObject.console",
    );
    let console_key = CONSOLE.v8_string(scope).unwrap();
    deno_core_obj.set(scope, console_key.into(), console_obj.into());
  }

  deno_obj.set(scope, deno_core_key.into(), deno_core_obj.into());
  global.set(scope, deno_str.into(), deno_obj.into());
}

/// Execute `00_primordials.js` and `00_infra.js` that are required for ops
/// to function properly
pub(crate) fn initialize_primordials_and_infra(
  scope: &mut v8::PinScope,
) -> Result<(), CoreError> {
  for source_file in &CONTEXT_SETUP_SOURCES {
    let name = source_file.specifier.v8_string(scope).unwrap();
    let source = source_file.source.v8_string(scope).unwrap();

    let origin = crate::modules::script_origin(scope, name, false, None);
    // TODO(bartlomieju): these two calls will panic if there's any problem in the JS code
    let script = v8::Script::compile(scope, source, Some(&origin))
      .ok_or(CoreModuleParseError(source_file.specifier))?;
    script
      .run(scope)
      .ok_or(CoreModuleExecuteError(source_file.specifier))?;
  }

  Ok(())
}

fn name_key<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  decl: &OpDecl,
) -> v8::Local<'s, v8::Name> {
  let key_str = decl.name_fast.v8_string(scope).unwrap();
  if decl.symbol_for {
    v8::Symbol::for_key(scope, key_str).into()
  } else {
    key_str.into()
  }
}

/// Set up JavaScript bindings for ops.
pub(crate) fn initialize_deno_core_ops_bindings<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  context: v8::Local<'s, v8::Context>,
  op_ctxs: &[OpCtx],
  op_method_decls: &[OpMethodDecl],
  methods_ctx_offset: usize,
  fn_template_store: &mut FunctionTemplateData,
) {
  let global = context.global(scope);

  // Set up JavaScript bindings for the defined op - this will insert proper
  // `v8::Function` into `Deno.core.ops` object. For async ops, there a bit
  // more machinery involved, see comment below.
  let deno_obj = get(scope, global, DENO, "Deno");
  let deno_core_obj = get(scope, deno_obj, CORE, "Deno.core");
  let deno_core_ops_obj: v8::Local<v8::Object> =
    get(scope, deno_core_obj, OPS, "Deno.core.ops");

  let set_up_async_stub_fn: v8::Local<v8::Function> = get(
    scope,
    deno_core_obj,
    SET_UP_ASYNC_STUB,
    "Deno.core.setUpAsyncStub",
  );

  let undefined = v8::undefined(scope);
  let mut index = 0;

  for decl in op_method_decls {
    if index == methods_ctx_offset {
      break;
    }

    let tmpl = if decl.constructor.is_some() {
      let constructor_ctx = &op_ctxs[index];

      let tmpl =
        op_ctx_template(scope, constructor_ctx, v8::ConstructorBehavior::Allow);

      index += 1;

      tmpl
    } else {
      crate::cppgc::make_cppgc_template(scope)
    };

    let key = decl.name.1.v8_string(scope).unwrap();

    let prototype = tmpl.prototype_template(scope);
    let method_ctxs = &op_ctxs[index..index + decl.methods.len()];

    let accessor_store = create_accessor_store(method_ctxs);

    for method in method_ctxs.iter() {
      // Skip async methods, we are going to register them later.
      if method.decl.is_async {
        continue;
      }

      op_ctx_template_or_accessor(
        &accessor_store,
        set_up_async_stub_fn,
        scope,
        prototype,
        tmpl,
        method,
      );
    }

    index += decl.methods.len();

    let static_method_ctxs = &op_ctxs[index..index + decl.static_methods.len()];
    for method in static_method_ctxs.iter() {
      let op_fn =
        op_ctx_template(scope, method, v8::ConstructorBehavior::Throw);
      let method_key = name_key(scope, &method.decl);

      tmpl.set(method_key, op_fn.into());
    }

    index += decl.static_methods.len();

    // Register async methods at the end since we need to create the template instance.
    for method in method_ctxs.iter() {
      if method.decl.is_async {
        op_ctx_template_or_accessor(
          &accessor_store,
          set_up_async_stub_fn,
          scope,
          prototype,
          tmpl,
          method,
        );
      }
    }

    if let Some(e) = (decl.inherits_type_name)() {
      let parent = fn_template_store.get_raw(e).unwrap();
      tmpl.inherit(v8::Local::new(scope, parent));
    }

    let op_fn = tmpl.get_function(scope).unwrap();
    op_fn.set_name(key);
    deno_core_ops_obj.set(scope, key.into(), op_fn.into());

    let id = (decl.type_name)().to_string();
    fn_template_store.insert(id, v8::Global::new(scope, tmpl));
  }

  let op_ctxs = &op_ctxs[index..];
  for op_ctx in op_ctxs {
    let mut op_fn =
      op_ctx_function(scope, op_ctx, v8::ConstructorBehavior::Allow);
    let key = op_ctx.decl.name_fast.v8_string(scope).unwrap();

    // For async ops we need to set them up, by calling `Deno.core.setUpAsyncStub` -
    // this call will generate an optimized function that binds to the provided
    // op, while keeping track of promises and error remapping.
    if op_ctx.decl.is_async {
      let result = set_up_async_stub_fn
        .call(scope, undefined.into(), &[key.into(), op_fn.into()])
        .unwrap();
      op_fn = result.try_into().unwrap()
    }

    deno_core_ops_obj.set(scope, key.into(), op_fn.into());
  }
}

fn op_ctx_template_or_accessor<'s, 'i>(
  accessor_store: &AccessorStore,
  set_up_async_stub_fn: v8::Local<'s, v8::Function>,
  scope: &mut v8::PinScope<'s, 'i>,
  tmpl: v8::Local<'s, v8::ObjectTemplate>,
  constructor: v8::Local<'s, v8::FunctionTemplate>,
  op_ctx: &OpCtx,
) {
  if !op_ctx.decl.is_accessor() {
    let op_fn = op_ctx_template(scope, op_ctx, v8::ConstructorBehavior::Throw);
    let method_key = name_key(scope, &op_ctx.decl);
    if op_ctx.decl.is_async {
      let undefined = v8::undefined(scope);
      let op_fn = op_fn.get_function(scope).unwrap();

      let tmpl_fn = constructor.get_function(scope).unwrap();

      let _result = set_up_async_stub_fn
        .call(
          scope,
          undefined.into(),
          &[method_key.into(), op_fn.into(), tmpl_fn.into()],
        )
        .unwrap();

      return;
    }

    tmpl.set(method_key, op_fn.into());

    return;
  }

  let op_ctx_ptr = op_ctx as *const OpCtx as *const c_void;
  let external = v8::External::new(scope, op_ctx_ptr as *mut c_void);

  if let Some((named_getter, named_setter)) =
    accessor_store.get(op_ctx.decl.name)
  {
    let getter_fn = if let Some(getter) = named_getter {
      let getter_raw = if getter.metrics_enabled() {
        getter.decl.slow_fn_with_metrics
      } else {
        getter.decl.slow_fn
      };

      let tmpl = v8::FunctionTemplate::builder_raw(getter_raw)
        .data(external.into())
        .build(scope);
      let op_fn = tmpl.get_function(scope).unwrap();
      let method_name = format!("get {}", op_ctx.decl.name_fast);
      let method_name = v8::String::new(scope, method_name.as_str()).unwrap();
      op_fn.set_name(method_name);

      Some(tmpl)
    } else {
      None
    };

    let setter_fn = if let Some(setter) = named_setter {
      let setter_raw = if setter.metrics_enabled() {
        setter.decl.slow_fn_with_metrics
      } else {
        setter.decl.slow_fn
      };

      let tmpl = v8::FunctionTemplate::builder_raw(setter_raw)
        .data(external.into())
        .length(1)
        .build(scope);
      let op_fn = tmpl.get_function(scope).unwrap();
      let method_name = format!("set {}", op_ctx.decl.name_fast);
      let method_name = v8::String::new(scope, method_name.as_str()).unwrap();
      op_fn.set_name(method_name);

      Some(tmpl)
    } else {
      None
    };

    let key = op_ctx.decl.name_fast.v8_string(scope).unwrap();
    tmpl.set_accessor_property(
      key.into(),
      getter_fn,
      setter_fn,
      v8::PropertyAttribute::default(),
    );
  }
}

pub(crate) fn op_ctx_template<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  op_ctx: &OpCtx,
  constructor_behaviour: v8::ConstructorBehavior,
) -> v8::Local<'s, v8::FunctionTemplate> {
  let op_ctx_ptr = op_ctx as *const OpCtx as *const c_void;
  let external = v8::External::new(scope, op_ctx_ptr as *mut c_void);

  let (slow_fn, fast_fn) = if op_ctx.metrics_enabled() {
    (
      op_ctx.decl.slow_fn_with_metrics,
      op_ctx.decl.fast_fn_with_metrics,
    )
  } else {
    (op_ctx.decl.slow_fn, op_ctx.decl.fast_fn)
  };

  let builder: v8::FunctionBuilder<v8::FunctionTemplate> =
    v8::FunctionTemplate::builder_raw(slow_fn)
      .data(external.into())
      .constructor_behavior(constructor_behaviour)
      .side_effect_type(if op_ctx.decl.no_side_effects {
        v8::SideEffectType::HasNoSideEffect
      } else {
        v8::SideEffectType::HasSideEffect
      })
      .length(op_ctx.decl.arg_count as i32);

  let template = if let Some(fast_function) = fast_fn {
    builder.build_fast(scope, &[fast_function])
  } else {
    builder.build(scope)
  };
  template.set_class_name(op_ctx.decl.name_fast.v8_string(scope).unwrap());

  template
}

fn op_ctx_function<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  op_ctx: &OpCtx,
  constructor_behaviour: v8::ConstructorBehavior,
) -> v8::Local<'s, v8::Function> {
  let v8name = op_ctx.decl.name_fast.v8_string(scope).unwrap();
  let template = op_ctx_template(scope, op_ctx, constructor_behaviour);
  let v8fn = template.get_function(scope).unwrap();
  v8fn.set_name(v8name);
  v8fn
}

type AccessorStore<'a> =
  HashMap<String, (Option<&'a OpCtx>, Option<&'a OpCtx>)>;

fn create_accessor_store(method_ctxs: &[OpCtx]) -> AccessorStore<'_> {
  let mut store = AccessorStore::new();

  for method in method_ctxs.iter() {
    // Populate all setters first.
    if method.decl.accessor_type == AccessorType::Setter {
      let key = method.decl.name_fast;

      let key_str = key.to_string();
      // There must be a getter for each setter.
      let getter = method_ctxs.iter().find(|m| {
        m.decl.name == key_str && m.decl.accessor_type == AccessorType::Getter
      });

      store.insert(key.to_string(), (getter, Some(method)));
    }
  }

  // Populate getters without setters.
  for method in method_ctxs.iter() {
    if method.decl.accessor_type == AccessorType::Getter {
      let key = method.decl.name_fast.to_string();

      store.entry(key).or_insert((Some(method), None));
    }
  }

  store
}

pub extern "C" fn wasm_async_resolve_promise_callback(
  _isolate: v8::UnsafeRawIsolatePtr,
  context: v8::Local<v8::Context>,
  resolver: v8::Local<v8::PromiseResolver>,
  compilation_result: v8::Local<v8::Value>,
  success: v8::WasmAsyncSuccess,
) {
  // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
  v8::callback_scope!(unsafe scope, context);
  if success == v8::WasmAsyncSuccess::Success {
    resolver.resolve(scope, compilation_result).unwrap();
  } else {
    resolver.reject(scope, compilation_result).unwrap();
  }
}

#[allow(clippy::unnecessary_wraps)]
pub fn host_import_module_dynamically_callback<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  host_defined_options: v8::Local<'s, v8::Data>,
  resource_name: v8::Local<'s, v8::Value>,
  specifier: v8::Local<'s, v8::String>,
  import_attributes: v8::Local<'s, v8::FixedArray>,
) -> Option<v8::Local<'s, v8::Promise>> {
  host_import_module_with_phase_dynamically_callback(
    scope,
    host_defined_options,
    resource_name,
    specifier,
    v8::ModuleImportPhase::kEvaluation,
    import_attributes,
  )
}

#[allow(clippy::unnecessary_wraps)]
pub fn host_import_module_with_phase_dynamically_callback<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  _host_defined_options: v8::Local<'s, v8::Data>,
  resource_name: v8::Local<'s, v8::Value>,
  specifier: v8::Local<'s, v8::String>,
  phase: v8::ModuleImportPhase,
  import_attributes: v8::Local<'s, v8::FixedArray>,
) -> Option<v8::Local<'s, v8::Promise>> {
  // NOTE(bartlomieju): will crash for non-UTF-8 specifier
  let specifier_str = specifier
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);

  let phase = match phase {
    v8::ModuleImportPhase::kEvaluation => ModuleImportPhase::Evaluation,
    v8::ModuleImportPhase::kSource => ModuleImportPhase::Source,
    v8::ModuleImportPhase::kDefer => ModuleImportPhase::Defer,
  };

  let cped = scope.get_continuation_preserved_embedder_data();
  let referrer_name_str = resource_name
    .to_string(scope)
    .unwrap()
    .to_rust_string_lossy(scope);

  let resolver = v8::PromiseResolver::new(scope).unwrap();
  let promise = resolver.get_promise(scope);

  let assertions = parse_import_attributes(
    scope,
    import_attributes,
    ImportAttributesKind::DynamicImport,
  );

  v8::tc_scope!(let tc_scope, scope);

  {
    {
      let state = JsRuntime::state_from(tc_scope);
      if let Some(validate_import_attributes_cb) =
        &state.validate_import_attributes_cb
      {
        (validate_import_attributes_cb)(tc_scope, &assertions);
      }
    }

    if tc_scope.has_caught() {
      let e = tc_scope.exception().unwrap();
      resolver.reject(tc_scope, e);
    }
  }
  let requested_module_type =
    get_requested_module_type_from_attributes(&assertions);

  let resolver_handle = v8::Global::new(tc_scope, resolver);
  let cped_handle = v8::Global::new(tc_scope, cped);
  {
    let state = JsRuntime::state_from(tc_scope);
    let module_map_rc = JsRealm::module_map_from(tc_scope);

    if !ModuleMap::load_dynamic_import(
      module_map_rc,
      tc_scope,
      specifier_str,
      referrer_name_str,
      requested_module_type,
      phase,
      resolver_handle,
      cped_handle,
    ) {
      // Short-circuit if the module is already cached and we know it won't error.
      return Some(promise);
    }

    state.notify_new_dynamic_import();
  }
  // Map errors from module resolution (not JS errors from module execution) to
  // ones rethrown from this scope, so they include the call stack of the
  // dynamic import site. Error objects without any stack frames are assumed to
  // be module resolution errors, other exception values are left as they are.
  let builder = v8::FunctionBuilder::new(catch_dynamic_import_promise_error);

  let map_err =
    v8::FunctionBuilder::<v8::Function>::build(builder, tc_scope).unwrap();

  let Some(promise_new) = promise.catch(tc_scope, map_err) else {
    if tc_scope.has_caught() {
      let e = tc_scope.exception().unwrap();
      resolver.reject(tc_scope, e);
    }

    return Some(promise);
  };

  Some(promise_new)
}

pub extern "C" fn host_initialize_import_meta_object_callback(
  context: v8::Local<v8::Context>,
  module: v8::Local<v8::Module>,
  meta: v8::Local<v8::Object>,
) {
  // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
  v8::callback_scope!(unsafe scope, context);
  let module_map = JsRealm::module_map_from(scope);
  let state = JsRealm::state_from_scope(scope);

  let module_global = v8::Global::new(scope, module);
  let name = module_map
    .get_name_by_module(&module_global)
    .expect("Module not found");
  let module_type = module_map
    .get_type_by_module(&module_global)
    .expect("Module not found");

  let url_key = URL.v8_string(scope).unwrap();
  let url_val = v8::String::new(scope, &name).unwrap();
  meta.create_data_property(scope, url_key.into(), url_val.into());

  let main_key = MAIN.v8_string(scope).unwrap();
  let main = module_map.is_main_module(&module_global);
  let main_val = v8::Boolean::new(scope, main);
  meta.create_data_property(scope, main_key.into(), main_val.into());

  // Add special method that allows Wasm module to instantiate themselves.
  if module_type == ModuleType::Wasm {
    let wasm_instance_key = WASM_INSTANCE.v8_string(scope).unwrap();
    match state.wasm_instance_fn.borrow().as_ref() {
      Some(f) => {
        let wasm_instance_val = v8::Local::new(scope, f.clone());
        meta.create_data_property(
          scope,
          wasm_instance_key.into(),
          wasm_instance_val.into(),
        );
      }
      _ => {
        let message = v8::String::new(
          scope,
          "WebAssembly is not available in this environment",
        )
        .unwrap();
        let exception = v8::Exception::error(scope, message);
        scope.throw_exception(exception);
        return;
      }
    }
  }

  let builder =
    v8::FunctionBuilder::new(import_meta_resolve).data(url_val.into());
  let val = v8::FunctionBuilder::<v8::Function>::build(builder, scope).unwrap();
  let resolve_key = RESOLVE.v8_string(scope).unwrap();
  meta.set(scope, resolve_key.into(), val.into());

  maybe_add_import_meta_filename_dirname(scope, meta, &name);

  if name.starts_with("ext:")
    && let Some(proto) = state.ext_import_meta_proto.borrow().clone()
  {
    let prototype = v8::Local::new(scope, proto);
    meta.set_prototype(scope, prototype.into());
  }
}

fn maybe_add_import_meta_filename_dirname<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  meta: v8::Local<'s, v8::Object>,
  name: &str,
) {
  // For `file:` URL we provide additional `filename` and `dirname` values
  let Ok(name_url) = Url::parse(name) else {
    return;
  };

  if name_url.scheme() != "file" {
    return;
  }

  // If something goes wrong acquiring a filepath, let skip instead of crashing
  // (mostly concerned about file paths on Windows).
  let Ok(file_path) = name_url.to_file_path() else {
    return;
  };

  // Use display() here so that Rust takes care of proper forward/backward slash
  // formatting depending on the OS.
  let escaped_filename = file_path.display().to_string();
  let Some(filename_val) = v8::String::new(scope, &escaped_filename) else {
    return;
  };
  let filename_key = FILENAME.v8_string(scope).unwrap();
  meta.create_data_property(scope, filename_key.into(), filename_val.into());

  let dir_path = file_path
    .parent()
    .map(|p| p.to_owned())
    .unwrap_or_else(|| PathBuf::from("/"));
  let escaped_dirname = dir_path.display().to_string();
  let Some(dirname_val) = v8::String::new(scope, &escaped_dirname) else {
    return;
  };
  let dirname_key = DIRNAME.v8_string(scope).unwrap();
  meta.create_data_property(scope, dirname_key.into(), dirname_val.into());
}

fn import_meta_resolve(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  mut rv: v8::ReturnValue,
) {
  if args.length() > 1 {
    return crate::error::throw_js_error_class(
      scope,
      &JsErrorBox::type_error("Invalid arguments"),
    );
  }

  let maybe_arg_str = args.get(0).to_string(scope);
  if maybe_arg_str.is_none() {
    return crate::error::throw_js_error_class(
      scope,
      &JsErrorBox::type_error("Invalid arguments"),
    );
  }
  let specifier = maybe_arg_str.unwrap();
  let referrer = {
    let url_prop = args.data();
    url_prop.to_rust_string_lossy(scope)
  };
  let module_map_rc = JsRealm::module_map_from(scope);
  let loader = module_map_rc.loader.clone();
  let specifier_str = specifier.to_rust_string_lossy(scope);

  let import_meta_resolve_result = {
    let loader = loader.borrow();
    let loader = loader.as_ref();
    loader.import_meta_resolve(&specifier_str, &referrer)
  };

  match import_meta_resolve_result {
    Ok(resolved) => {
      let resolved_val = serde_v8::to_v8(scope, resolved.as_str()).unwrap();
      rv.set(resolved_val);
    }
    Err(err) => {
      crate::error::throw_js_error_class(scope, &err);
    }
  };
}

pub(crate) fn op_disabled_fn(
  scope: &mut v8::PinScope,
  _args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  let message = v8::String::new(scope, "op is disabled").unwrap();
  let exception = v8::Exception::error(scope, message);
  scope.throw_exception(exception);
}

fn catch_dynamic_import_promise_error<'s, 'i>(
  scope: &mut v8::PinScope<'s, 'i>,
  args: v8::FunctionCallbackArguments<'s>,
  _rv: v8::ReturnValue,
) {
  let arg = args.get(0);
  if is_instance_of_error(scope, arg) {
    let e: crate::error::NativeJsError = serde_v8::from_v8(scope, arg).unwrap();
    let name = e.name.unwrap_or_else(|| {
      deno_error::builtin_classes::GENERIC_ERROR.to_string()
    });
    if !has_call_site(scope, arg) {
      let msg = v8::Exception::create_message(scope, arg);
      let arg: v8::Local<v8::Object> = arg.try_into().unwrap();
      let message_key = MESSAGE.v8_string(scope).unwrap();
      let message = arg.get(scope, message_key.into()).unwrap();
      let mut message: v8::Local<v8::String> = message.try_into().unwrap();
      if let Some(stack_frame) = JsStackFrame::from_v8_message(scope, msg)
        && let Some(location) = stack_frame.maybe_format_location()
      {
        let str =
          format!("{} at {location}", message.to_rust_string_lossy(scope));
        message = v8::String::new(scope, &str).unwrap();
      }
      let exception = match name.as_str() {
        deno_error::builtin_classes::RANGE_ERROR => {
          v8::Exception::range_error(scope, message)
        }
        deno_error::builtin_classes::TYPE_ERROR => {
          v8::Exception::type_error(scope, message)
        }
        deno_error::builtin_classes::SYNTAX_ERROR => {
          v8::Exception::syntax_error(scope, message)
        }
        deno_error::builtin_classes::REFERENCE_ERROR => {
          v8::Exception::reference_error(scope, message)
        }
        _ => v8::Exception::error(scope, message),
      };
      let code_key = CODE.v8_string(scope).unwrap();
      let code_value = ERR_MODULE_NOT_FOUND.v8_string(scope).unwrap();
      let exception_obj = exception.to_object(scope).unwrap();
      exception_obj.set(scope, code_key.into(), code_value.into());
      scope.throw_exception(exception);
      return;
    }
  }
  scope.throw_exception(arg);
}

pub extern "C" fn promise_reject_callback(message: v8::PromiseRejectMessage) {
  // SAFETY: `CallbackScope` can be safely constructed from `&PromiseRejectMessage`
  v8::callback_scope!(unsafe scope, &message);

  let exception_state = JsRealm::exception_state_from_scope(scope);
  exception_state.track_promise_rejection(
    scope,
    message.get_promise(),
    message.get_event(),
    message.get_value(),
  );
}

/// This binding should be used if there's a custom console implementation
/// available. Using it will make sure that proper stack frames are displayed
/// in the inspector console.
///
/// Each method on console object should be bound to this function, eg:
/// ```ignore
/// function wrapConsole(consoleFromDeno, consoleFromV8) {
///   const callConsole = core.callConsole;
///
///   for (const key of Object.keys(consoleFromV8)) {
///     if (consoleFromDeno.hasOwnProperty(key)) {
///       consoleFromDeno[key] = callConsole.bind(
///         consoleFromDeno,
///         consoleFromV8[key],
///         consoleFromDeno[key],
///       );
///     }
///   }
/// }
/// ```
///
/// Inspired by:
/// https://github.com/nodejs/node/blob/1317252dfe8824fd9cfee125d2aaa94004db2f3b/src/inspector_js_api.cc#L194-L222
fn call_console(
  scope: &mut v8::PinScope,
  args: v8::FunctionCallbackArguments,
  _rv: v8::ReturnValue,
) {
  if args.length() < 2
    || !args.get(0).is_function()
    || !args.get(1).is_function()
  {
    return crate::error::throw_js_error_class(
      scope,
      &JsErrorBox::type_error("Invalid arguments"),
    );
  }

  let mut call_args = vec![];
  for i in 2..args.length() {
    call_args.push(args.get(i));
  }

  let receiver = args.this();
  let inspector_console_method =
    v8::Local::<v8::Function>::try_from(args.get(0)).unwrap();
  let deno_console_method =
    v8::Local::<v8::Function>::try_from(args.get(1)).unwrap();

  inspector_console_method.call(scope, receiver.into(), &call_args);
  deno_console_method.call(scope, receiver.into(), &call_args);
}

fn cast_closure<F>(f: F) -> F
where
  F: for<'a, 'b> Fn(
      &mut v8::PinScope<'a, 'b>,
      v8::FunctionCallbackArguments<'a>,
      v8::ReturnValue<'a>,
    ) + 'static,
{
  f
}

/// Wrap a promise with `then` handlers allowing us to watch the resolution progress from a Rust closure.
/// This has a side-effect of preventing unhandled rejection handlers from triggering. If that is
/// desired, the final handler may choose to rethrow the exception.
pub(crate) fn watch_promise<'s, 'i, F>(
  scope: &mut v8::PinScope<'s, 'i>,
  promise: v8::Local<'s, v8::Promise>,
  f: F,
) -> Option<v8::Local<'s, v8::Promise>>
where
  F: for<'a, 'b> FnOnce(
      &mut v8::PinScope<'a, 'b>,
      v8::ReturnValue<'a>,
      Result<v8::Local<'a, v8::Value>, v8::Local<'a, v8::Value>>,
    ) + 'static,
{
  let external =
    v8::External::new(scope, Box::into_raw(Box::new(Some(f))) as _);

  fn get_handler<F>(external: v8::Local<v8::External>) -> F {
    unsafe { Box::<Option<F>>::from_raw(external.value() as _) }
      .take()
      .unwrap()
  }

  let on_fulfilled = v8::Function::builder(cast_closure(
    |scope: &mut v8::PinScope,
     args: v8::FunctionCallbackArguments,
     rv: v8::ReturnValue| {
      let data = v8::Local::<v8::External>::try_from(args.data()).unwrap();
      let f = get_handler::<F>(data);
      f(scope, rv, Ok(args.get(0)));
    },
  ))
  .data(external.into())
  .build(scope);

  let on_rejected = v8::Function::builder(cast_closure(
    |scope: &mut v8::PinScope,
     args: v8::FunctionCallbackArguments,
     rv: v8::ReturnValue| {
      let data = v8::Local::<v8::External>::try_from(args.data()).unwrap();
      let f = get_handler::<F>(data);
      f(scope, rv, Err(args.get(0)));
    },
  ))
  .data(external.into())
  .build(scope);

  // function builders will return None if the runtime is shutting down
  let (Some(on_fulfilled), Some(on_rejected)) = (on_fulfilled, on_rejected)
  else {
    _ = get_handler::<F>(external);
    return None;
  };

  // then2 will return None if the runtime is shutting down
  let Some(promise) = promise.then2(scope, on_fulfilled, on_rejected) else {
    _ = get_handler::<F>(external);
    return None;
  };

  Some(promise)
}

/// This function generates a list of tuples, that are a mapping of `<op_name>`
/// to a JavaScript function that executes and op.
pub fn create_exports_for_ops_virtual_module<'s, 'i>(
  op_ctxs: &[OpCtx],
  op_method_decls: &[OpMethodDecl],
  methods_ctx_offset: usize,
  scope: &mut v8::PinScope<'s, 'i>,
  global: v8::Local<'s, v8::Object>,
) -> Vec<(FastStaticString, v8::Local<'s, v8::Value>)> {
  let mut exports = Vec::with_capacity(op_ctxs.len());

  let deno_obj = get(scope, global, DENO, "Deno");
  let deno_core_obj = get(scope, deno_obj, CORE, "Deno.core");
  let ops_obj = get(scope, deno_core_obj, OPS, "Deno.core.ops");

  let mut index = 0;

  for decl in op_method_decls {
    if index == methods_ctx_offset {
      break;
    }

    if decl.constructor.is_some() {
      index += 1;
    }

    let name = decl.name.1;
    let op_fn = get(scope, ops_obj, name, "op");
    exports.push((name, op_fn));

    index += decl.methods.len() + decl.static_methods.len();
  }

  let op_ctxs = &op_ctxs[index..];
  for op_ctx in op_ctxs {
    let op_fn = get(scope, ops_obj, op_ctx.decl.name_fast, "op");
    exports.push((op_ctx.decl.name_fast, op_fn));
  }

  exports
}
