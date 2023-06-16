// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::rc::Rc;

use deno_core::v8;
use deno_core::v8::GetPropertyNamesArgs;

use crate::NodeResolver;

/// Convert an ASCII string to a UTF-16 byte encoding of the string.
const fn str_to_utf16<const N: usize>(s: &str) -> [u16; N] {
  let mut out = [0_u16; N];
  let mut i = 0;
  let bytes = s.as_bytes();
  assert!(N == bytes.len());
  while i < bytes.len() {
    assert!(bytes[i] < 128, "only works for ASCII strings");
    out[i] = bytes[i] as u16;
    i += 1;
  }
  out
}

// ext/node changes the global object to be a proxy object that intercepts all
// property accesses for globals that are different between Node and Deno and
// dynamically returns a different value depending on if the accessing code is
// in node_modules/ or not.
//
// To make this performant, a v8 named property handler is used, that only
// intercepts property accesses for properties that are not already present on
// the global object (it is non-masking). This means that in the common case,
// when a user accesses a global that is the same between Node and Deno (like
// Uint8Array or fetch), the proxy overhead is avoided.
//
// The Deno and Node specific globals are stored in objects in the internal
// fields of the proxy object. The first internal field is the object storing
// the Deno specific globals, the second internal field is the object storing
// the Node specific globals.
//
// These are the globals that are handled:
// - Buffer (node only)
// - clearImmediate (node only)
// - clearInterval (both, but different implementation)
// - clearTimeout (both, but different implementation)
// - console (both, but different implementation)
// - global (node only)
// - performance (both, but different implementation)
// - process (node only)
// - setImmediate (node only)
// - setInterval (both, but different implementation)
// - setTimeout (both, but different implementation)
// - window (deno only)

// UTF-16 encodings of the managed globals. THIS LIST MUST BE SORTED.
#[rustfmt::skip]
const MANAGED_GLOBALS: [&[u16]; 12] = [
  &str_to_utf16::<6>("Buffer"),
  &str_to_utf16::<14>("clearImmediate"),
  &str_to_utf16::<13>("clearInterval"),
  &str_to_utf16::<12>("clearTimeout"),
  &str_to_utf16::<7>("console"),
  &str_to_utf16::<6>("global"),
  &str_to_utf16::<11>("performance"),
  &str_to_utf16::<7>("process"),
  &str_to_utf16::<12>("setImmediate"),
  &str_to_utf16::<11>("setInterval"),
  &str_to_utf16::<10>("setTimeout"),
  &str_to_utf16::<6>("window"),
];

const SHORTEST_MANAGED_GLOBAL: usize = 6;
const LONGEST_MANAGED_GLOBAL: usize = 14;

#[derive(Debug, Clone, Copy)]
enum Mode {
  Deno,
  Node,
}

pub fn global_template_middleware<'s>(
  _scope: &mut v8::HandleScope<'s, ()>,
  template: v8::Local<'s, v8::ObjectTemplate>,
) -> v8::Local<'s, v8::ObjectTemplate> {
  // The internal field layout is as follows:
  // 0: Reflect.get
  // 1: Reflect.set
  // 2: An object containing the Deno specific globals
  // 3: An object containing the Node specific globals
  assert_eq!(template.internal_field_count(), 0);
  template.set_internal_field_count(4);

  let config = v8::NamedPropertyHandlerConfiguration::new()
    .flags(
      v8::PropertyHandlerFlags::NON_MASKING
        | v8::PropertyHandlerFlags::HAS_NO_SIDE_EFFECT,
    )
    .getter(getter)
    .setter(setter)
    .query(query)
    .deleter(deleter)
    .enumerator(enumerator)
    .definer(definer)
    .descriptor(descriptor);

  template.set_named_property_handler(config);

  template
}

pub fn global_object_middleware<'s>(
  scope: &mut v8::HandleScope<'s>,
  global: v8::Local<'s, v8::Object>,
) {
  // ensure the global object is not Object.prototype
  let object_key =
    v8::String::new_external_onebyte_static(scope, b"Object").unwrap();
  let object = global
    .get(scope, object_key.into())
    .unwrap()
    .to_object(scope)
    .unwrap();
  let prototype_key =
    v8::String::new_external_onebyte_static(scope, b"prototype").unwrap();
  let object_prototype = object
    .get(scope, prototype_key.into())
    .unwrap()
    .to_object(scope)
    .unwrap();
  assert_ne!(global, object_prototype);

  // get the Reflect.get and Reflect.set functions
  let reflect_key =
    v8::String::new_external_onebyte_static(scope, b"Reflect").unwrap();
  let reflect = global
    .get(scope, reflect_key.into())
    .unwrap()
    .to_object(scope)
    .unwrap();
  let get_key = v8::String::new_external_onebyte_static(scope, b"get").unwrap();
  let reflect_get = reflect.get(scope, get_key.into()).unwrap();
  assert_eq!(reflect_get.is_function(), true);
  let set_key = v8::String::new_external_onebyte_static(scope, b"set").unwrap();
  let reflect_set = reflect.get(scope, set_key.into()).unwrap();
  assert_eq!(reflect_set.is_function(), true);

  // globalThis.__bootstrap.ext_node_denoGlobals and
  // globalThis.__bootstrap.ext_node_nodeGlobals are the objects that contain
  // the Deno and Node specific globals respectively. If they do not yet exist
  // on the global object, create them as null prototype objects.
  let bootstrap_key =
    v8::String::new_external_onebyte_static(scope, b"__bootstrap").unwrap();
  let bootstrap = match global.get(scope, bootstrap_key.into()) {
    Some(value) if value.is_object() => value.to_object(scope).unwrap(),
    Some(value) if value.is_undefined() => {
      let null = v8::null(scope);
      let obj =
        v8::Object::with_prototype_and_properties(scope, null.into(), &[], &[]);
      global.set(scope, bootstrap_key.into(), obj.into());
      obj
    }
    _ => panic!("__bootstrap should not be tampered with"),
  };
  let deno_globals_key =
    v8::String::new_external_onebyte_static(scope, b"ext_node_denoGlobals")
      .unwrap();
  let deno_globals = match bootstrap.get(scope, deno_globals_key.into()) {
    Some(value) if value.is_object() => value,
    Some(value) if value.is_undefined() => {
      let null = v8::null(scope);
      let obj =
        v8::Object::with_prototype_and_properties(scope, null.into(), &[], &[])
          .into();
      bootstrap.set(scope, deno_globals_key.into(), obj);
      obj
    }
    _ => panic!("__bootstrap.ext_node_denoGlobals should not be tampered with"),
  };
  let node_globals_key =
    v8::String::new_external_onebyte_static(scope, b"ext_node_nodeGlobals")
      .unwrap();
  let node_globals = match bootstrap.get(scope, node_globals_key.into()) {
    Some(value) if value.is_object() => value,
    Some(value) if value.is_undefined() => {
      let null = v8::null(scope);
      let obj =
        v8::Object::with_prototype_and_properties(scope, null.into(), &[], &[])
          .into();
      bootstrap.set(scope, node_globals_key.into(), obj);
      obj
    }
    _ => panic!("__bootstrap.ext_node_nodeGlobals should not be tampered with"),
  };

  // set the internal fields
  assert!(global.set_internal_field(0, reflect_get));
  assert!(global.set_internal_field(1, reflect_set));
  assert!(global.set_internal_field(2, deno_globals));
  assert!(global.set_internal_field(3, node_globals));
}

fn is_managed_key(
  scope: &mut v8::HandleScope,
  key: v8::Local<v8::Name>,
) -> bool {
  let Ok(str): Result<v8::Local<v8::String>, _> = key.try_into() else {
    return false;
  };
  let len = str.length();
  if len < SHORTEST_MANAGED_GLOBAL || len > LONGEST_MANAGED_GLOBAL {
    return false;
  }
  let buf = &mut [0u16; LONGEST_MANAGED_GLOBAL];
  let written = str.write(
    scope,
    buf.as_mut_slice(),
    0,
    v8::WriteOptions::NO_NULL_TERMINATION,
  );
  assert_eq!(written, len);
  return MANAGED_GLOBALS.binary_search(&&buf[..len]).is_ok();
}

fn current_mode(scope: &mut v8::HandleScope) -> Mode {
  let Some(v8_string) = v8::StackTrace::current_script_name_or_source_url(scope) else {
    return Mode::Deno;
  };
  let string = v8_string.to_rust_string_lossy(scope);
  // TODO: don't require parsing the specifier
  let Ok(specifier) = deno_core::ModuleSpecifier::parse(&string) else {
    return Mode::Deno;
  };
  let op_state = deno_core::JsRuntime::op_state_from(scope);
  let op_state = op_state.borrow();
  let Some(node_resolver) = op_state.try_borrow::<Rc<NodeResolver>>() else {
    return Mode::Deno;
  };
  if node_resolver.in_npm_package(&specifier) {
    Mode::Node
  } else {
    Mode::Deno
  }
}

fn inner_object<'s>(
  scope: &mut v8::HandleScope<'s>,
  real_global_object: v8::Local<'s, v8::Object>,
  mode: Mode,
) -> v8::Local<'s, v8::Object> {
  let value = match mode {
    Mode::Deno => real_global_object.get_internal_field(scope, 2).unwrap(),
    Mode::Node => real_global_object.get_internal_field(scope, 3).unwrap(),
  };
  v8::Local::<v8::Object>::try_from(value).unwrap()
}

fn real_global_object<'s>(
  scope: &mut v8::HandleScope<'s>,
) -> v8::Local<'s, v8::Object> {
  let context = scope.get_current_context();
  let global = context.global(scope);
  let global = global
    .get_prototype(scope)
    .unwrap()
    .to_object(scope)
    .unwrap();
  global
}

pub fn getter<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  if !is_managed_key(scope, key) {
    return;
  };

  let this = args.this();
  let real_global_object = real_global_object(scope);
  let mode = current_mode(scope);

  let reflect_get: v8::Local<v8::Function> = real_global_object
    .get_internal_field(scope, 0)
    .unwrap()
    .try_into()
    .unwrap();

  let inner = inner_object(scope, real_global_object, mode);
  let undefined = v8::undefined(scope);
  let Some(value) = reflect_get.call(
    scope,
    undefined.into(),
    &[inner.into(), key.into(), this.into()],
  ) else {
    return;
  };

  rv.set(value);
}

pub fn setter<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  value: v8::Local<'s, v8::Value>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  if !is_managed_key(scope, key) {
    return;
  };

  let this = args.this();
  let real_global_object = real_global_object(scope);
  let mode = current_mode(scope);

  let reflect_set: v8::Local<v8::Function> = real_global_object
    .get_internal_field(scope, 1)
    .unwrap()
    .try_into()
    .unwrap();
  let inner = inner_object(scope, real_global_object, mode);
  let undefined = v8::undefined(scope);

  let Some(success) = reflect_set.call(
    scope,
    undefined.into(),
    &[inner.into(), key.into(), value, this.into()],
  ) else {
    return;
  };

  rv.set(success);
}

pub fn query<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  _args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  if !is_managed_key(scope, key) {
    return;
  };
  let real_global_object = real_global_object(scope);
  let mode = current_mode(scope);

  let inner = inner_object(scope, real_global_object, mode);

  let Some(true) = inner.has_own_property(scope, key) else {
    return;
  };

  let Some(attributes) = inner.get_property_attributes(scope, key.into()) else {
    return;
  };

  rv.set_uint32(attributes.as_u32());
}

pub fn deleter<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  if !is_managed_key(scope, key) {
    return;
  };

  let real_global_object = real_global_object(scope);
  let mode = current_mode(scope);

  let inner = inner_object(scope, real_global_object, mode);

  let Some(success) = inner.delete(scope, key.into()) else {
    return;
  };

  if args.should_throw_on_error() && !success {
    let message = v8::String::new(scope, "Cannot delete property").unwrap();
    let exception = v8::Exception::type_error(scope, message);
    scope.throw_exception(exception);
    return;
  }

  rv.set_bool(success);
}

pub fn enumerator<'s>(
  scope: &mut v8::HandleScope<'s>,
  _args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  let real_global_object = real_global_object(scope);
  let mode = current_mode(scope);

  let inner = inner_object(scope, real_global_object, mode);

  let Some(array) = inner.get_property_names(scope, GetPropertyNamesArgs::default()) else {
    return;
  };

  rv.set(array.into());
}

pub fn definer<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  descriptor: &v8::PropertyDescriptor,
  args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  if !is_managed_key(scope, key) {
    return;
  };

  let real_global_object = real_global_object(scope);
  let mode = current_mode(scope);

  let inner = inner_object(scope, real_global_object, mode);
  let Some(success) = inner.define_property(scope, key, descriptor) else {
    return;
  };

  if args.should_throw_on_error() && !success {
    let message = v8::String::new(scope, "Cannot define property").unwrap();
    let exception = v8::Exception::type_error(scope, message);
    scope.throw_exception(exception);
    return;
  }

  rv.set_bool(success);
}

pub fn descriptor<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: v8::Local<'s, v8::Name>,
  _args: v8::PropertyCallbackArguments<'s>,
  mut rv: v8::ReturnValue,
) {
  if !is_managed_key(scope, key) {
    return;
  };

  let real_global_object = real_global_object(scope);
  let mode = current_mode(scope);

  let scope = &mut v8::TryCatch::new(scope);

  let inner = inner_object(scope, real_global_object, mode);
  let Some(descriptor) = inner.get_own_property_descriptor(scope, key.into()) else {
    scope.rethrow().expect("to have caught an exception");
    return;
  };

  if descriptor.is_undefined() {
    return;
  }

  rv.set(descriptor);
}
