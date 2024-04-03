// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::error::type_error;
use deno_core::v8;
use deno_core::v8::MapFnTo;

pub const PRIVATE_SYMBOL_NAME: v8::OneByteConst = v8::String::create_external_onebyte_const(b"node:contextify:context");

/// An unbounded script that can be run in a context.
#[derive(Debug)]
pub struct ContextifyScript {
  script: v8::Global<v8::UnboundScript>,
}

impl ContextifyScript {
  pub fn new(
    scope: &mut v8::HandleScope,
    source_str: v8::Local<v8::String>,
  ) -> Result<Self, AnyError> {
    let source = v8::script_compiler::Source::new(source_str, None);

    let unbound_script = v8::script_compiler::compile_unbound_script(
      scope,
      source,
      v8::script_compiler::CompileOptions::NoCompileOptions,
      v8::script_compiler::NoCacheReason::NoReason,
    ).ok_or_else(|| type_error("Failed to compile script"))?;
    let script = v8::Global::new(scope, unbound_script);
    Ok(Self { script })
  }

  pub fn eval_machine<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    context: v8::Local<v8::Context>,
  ) -> Option<v8::Local<'s, v8::Value>> {
    let tc_scope = &mut v8::TryCatch::new(scope);

    let unbound_script = v8::Local::new(tc_scope, self.script.clone());
    let script = unbound_script.bind_to_current_context(tc_scope);

    // TODO: support `break_on_first_line` arg
    // TODO: support `break_on_sigint` and `timeout` args
    let result = script.run(tc_scope);
    // TODO: support `microtask_queue` arg

    if tc_scope.has_caught() {
      // TODO:
      // if display_errors {
      //
      // }

      if !tc_scope.has_terminated() {
        tc_scope.rethrow();
      }

      return None;
    }

    Some(result.unwrap())
  }
}

#[derive(Debug, Clone)]
pub struct ContextifyContext {
  context: v8::Global<v8::Context>,
  sandbox: v8::Global<v8::Object>,
}

impl ContextifyContext {
  pub fn attach(
    scope: &mut v8::HandleScope,
    sandbox_obj: v8::Local<v8::Object>,
  ) {
    let tmp = init_global_template(scope);

    let context = create_v8_context(scope, tmp, None);
    Self::from_context(scope, context, sandbox_obj);
  }

  pub fn from_context(
    scope: &mut v8::HandleScope,
    v8_context: v8::Local<v8::Context>,
    sandbox_obj: v8::Local<v8::Object>,
  ) {
    let main_context = scope.get_current_context();
    let new_context_global = v8_context.global(scope);
    v8_context.set_security_token(main_context.get_security_token(scope));

    let context = v8::Global::new(scope, v8_context);
    let sandbox = v8::Global::new(scope, sandbox_obj);
    let wrapper = deno_core::cppgc::make_cppgc_object(scope, Self {
      context,
      sandbox,
    });

    let private_str = v8::String::new_from_onebyte_const(scope, &PRIVATE_SYMBOL_NAME);
    let private_symbol = v8::Private::for_api(scope, private_str);

    sandbox_obj
      .set_private(scope, private_symbol, wrapper.into());
  }

  pub fn from_sandbox_obj<'a>(
    scope: &mut v8::HandleScope,
    sandbox_obj: v8::Local<v8::Object>,
  ) -> Option<&'a Self> {
    let private_str = v8::String::new_from_onebyte_const(scope, &PRIVATE_SYMBOL_NAME);
    let private_symbol = v8::Private::for_api(scope, private_str);

    sandbox_obj.get_private(scope, private_symbol)
      .and_then(|wrapper| deno_core::cppgc::try_unwrap_cppgc_object::<Self>(wrapper))
  }

  pub fn context<'a>(&self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Context> {
    v8::Local::new(scope, &self.context)
  }

  fn global_proxy<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
  ) -> v8::Local<'s, v8::Object> {
    let ctx = self.context(scope);
    ctx.global(scope)
  }

  fn sandbox<'a>(&self, scope: &mut v8::HandleScope<'a>) -> v8::Local<'a, v8::Object> {
    v8::Local::new(scope, &self.sandbox)
  }

  fn get(
    scope: &mut v8::HandleScope,
    object: v8::Local<v8::Object>,
  ) -> Option<ContextifyContext> {
    let Some(context) = object.get_creation_context(scope) else {
      return None;
    };

    context.get_slot::<ContextifyContext>(scope).cloned()
  }
}

pub const VM_CONTEXT_INDEX: usize = 0;

fn create_v8_context<'a>(
  scope: &mut v8::HandleScope<'a>,
  object_template: v8::Local<v8::ObjectTemplate>,
  snapshot_data: Option<&'static [u8]>,
) -> v8::Local<'a, v8::Context> {
  let scope = &mut v8::EscapableHandleScope::new(scope);

  let context = if let Some(_snapshot_data) = snapshot_data {
    v8::Context::from_snapshot(scope, VM_CONTEXT_INDEX).unwrap()
  } else {
    v8::Context::new_from_template(scope, object_template)
  };

  scope.escape(context)
}

#[derive(Debug, Clone)]
struct SlotContextifyGlobalTemplate(v8::Global<v8::ObjectTemplate>);

fn init_global_template<'a>(
  scope: &mut v8::HandleScope<'a>,
) -> v8::Local<'a, v8::ObjectTemplate> {
  let mut maybe_object_template_slot =
    scope.get_slot::<SlotContextifyGlobalTemplate>();

  if maybe_object_template_slot.is_none() {
    init_global_template_inner(scope);
    maybe_object_template_slot =
      scope.get_slot::<SlotContextifyGlobalTemplate>();
  }
  let object_template_slot = maybe_object_template_slot
    .expect("ContextifyGlobalTemplate slot should be already populated.")
    .clone();
  v8::Local::new(scope, object_template_slot.0)
}


extern "C" fn c_noop(info: *const v8::FunctionCallbackInfo) {}

thread_local! {
  pub static GETTER_MAP_FN: v8::GenericNamedPropertyGetterCallback<'static> = property_getter.map_fn_to();
  pub static SETTER_MAP_FN: v8::GenericNamedPropertySetterCallback<'static> = property_setter.map_fn_to();
  pub static DELETER_MAP_FN: v8::GenericNamedPropertyGetterCallback<'static> = property_deleter.map_fn_to();
  pub static ENUMERATOR_MAP_FN: v8::GenericNamedPropertyEnumeratorCallback<'static> = property_enumerator.map_fn_to();
  pub static DEFINER_MAP_FN: v8::GenericNamedPropertyDefinerCallback<'static> = property_definer.map_fn_to();
  pub static DESCRIPTOR_MAP_FN: v8::GenericNamedPropertyGetterCallback<'static> = property_descriptor.map_fn_to();
}

fn init_global_template_inner(scope: &mut v8::HandleScope) {
  let global_func_template =
    v8::FunctionTemplate::builder_raw(c_noop).build(scope);
  let global_object_template = global_func_template.instance_template(scope);

  let named_property_handler_config = {
    let mut config = v8::NamedPropertyHandlerConfiguration::new()
      .flags(v8::PropertyHandlerFlags::HAS_NO_SIDE_EFFECT);

    config = GETTER_MAP_FN.with(|getter| config.getter_raw(*getter));
    config = SETTER_MAP_FN.with(|setter| config.setter_raw(*setter));
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
    config
  };

  global_object_template
    .set_named_property_handler(named_property_handler_config);
  global_object_template
    .set_indexed_property_handler(indexed_property_handler_config);
  let contextify_global_template_slot = SlotContextifyGlobalTemplate(
    v8::Global::new(scope, global_object_template),
  );
  scope.set_slot(contextify_global_template_slot);
}


fn property_getter<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  let ctx = ContextifyContext::get(scope, args.this());

  let context = ctx.context(scope);
  let sandbox = ctx.sandbox(scope);
}

fn property_setter<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  value: v8::Local<'s, v8::Value>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
}

fn property_deleter<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {

}

fn property_enumerator<'s>(
  scope: &mut v8::HandleScope<'s>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
}

fn property_definer<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  descriptor: &v8::PropertyDescriptor,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
}

fn property_descriptor<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
}
