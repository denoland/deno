// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#![allow(non_upper_case_globals)]
#![deny(unsafe_op_in_unsafe_fn)]

const NAPI_VERSION: u32 = 9;

use deno_runtime::deno_napi::*;
use libc::INT_MAX;

use super::util::check_new_from_utf8;
use super::util::check_new_from_utf8_len;
use super::util::get_array_buffer_ptr;
use super::util::make_external_backing_store;
use super::util::napi_clear_last_error;
use super::util::napi_set_last_error;
use super::util::v8_name_from_property_descriptor;
use crate::check_arg;
use crate::check_env;
use deno_runtime::deno_napi::function::create_function;
use deno_runtime::deno_napi::function::create_function_template;
use deno_runtime::deno_napi::function::CallbackInfo;
use napi_sym::napi_sym;
use std::ptr::NonNull;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ReferenceOwnership {
  Runtime,
  Userland,
}

enum ReferenceState {
  Strong(v8::Global<v8::Value>),
  Weak(v8::Weak<v8::Value>),
}

struct Reference {
  env: *mut Env,
  state: ReferenceState,
  ref_count: u32,
  ownership: ReferenceOwnership,
  finalize_cb: Option<napi_finalize>,
  finalize_data: *mut c_void,
  finalize_hint: *mut c_void,
}

impl Reference {
  fn new(
    env: *mut Env,
    value: v8::Local<v8::Value>,
    initial_ref_count: u32,
    ownership: ReferenceOwnership,
    finalize_cb: Option<napi_finalize>,
    finalize_data: *mut c_void,
    finalize_hint: *mut c_void,
  ) -> Box<Self> {
    let isolate = unsafe { (*env).isolate() };

    let mut reference = Box::new(Reference {
      env,
      state: ReferenceState::Strong(v8::Global::new(isolate, value)),
      ref_count: initial_ref_count,
      ownership,
      finalize_cb,
      finalize_data,
      finalize_hint,
    });

    if initial_ref_count == 0 {
      reference.set_weak();
    }

    reference
  }

  fn ref_(&mut self) -> u32 {
    self.ref_count += 1;
    if self.ref_count == 1 {
      self.set_strong();
    }
    self.ref_count
  }

  fn unref(&mut self) -> u32 {
    let old_ref_count = self.ref_count;
    if self.ref_count > 0 {
      self.ref_count -= 1;
    }
    if old_ref_count == 1 && self.ref_count == 0 {
      self.set_weak();
    }
    self.ref_count
  }

  fn reset(&mut self) {
    self.finalize_cb = None;
    self.finalize_data = std::ptr::null_mut();
    self.finalize_hint = std::ptr::null_mut();
  }

  fn set_strong(&mut self) {
    if let ReferenceState::Weak(w) = &self.state {
      let isolate = unsafe { (*self.env).isolate() };
      if let Some(g) = w.to_global(isolate) {
        self.state = ReferenceState::Strong(g);
      }
    }
  }

  fn set_weak(&mut self) {
    let reference = self as *mut Reference;
    if let ReferenceState::Strong(g) = &self.state {
      let cb = Box::new(move |_: &mut v8::Isolate| {
        Reference::weak_callback(reference)
      });
      let isolate = unsafe { (*self.env).isolate() };
      self.state =
        ReferenceState::Weak(v8::Weak::with_finalizer(isolate, g, cb));
    }
  }

  fn weak_callback(reference: *mut Reference) {
    let reference = unsafe { &mut *reference };

    let finalize_cb = reference.finalize_cb;
    let finalize_data = reference.finalize_data;
    let finalize_hint = reference.finalize_hint;
    reference.reset();

    // copy this value before the finalize callback, since
    // it might free the reference (which would be a UAF)
    let ownership = reference.ownership;
    if let Some(finalize_cb) = finalize_cb {
      unsafe {
        finalize_cb(reference.env as _, finalize_data, finalize_hint);
      }
    }

    if ownership == ReferenceOwnership::Runtime {
      unsafe { drop(Reference::from_raw(reference)) }
    }
  }

  fn into_raw(r: Box<Reference>) -> *mut Reference {
    Box::into_raw(r)
  }

  unsafe fn from_raw(r: *mut Reference) -> Box<Reference> {
    unsafe { Box::from_raw(r) }
  }

  unsafe fn remove(r: *mut Reference) {
    let r = unsafe { &mut *r };
    if r.ownership == ReferenceOwnership::Userland {
      r.reset();
    } else {
      unsafe { drop(Reference::from_raw(r)) }
    }
  }
}

#[napi_sym]
fn napi_get_last_error_info(
  env: *mut Env,
  result: *mut *const napi_extended_error_info,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, result);

  if env.last_error.error_code == napi_ok {
    napi_clear_last_error(env);
  } else {
    env.last_error.error_message =
      ERROR_MESSAGES[env.last_error.error_code as usize].as_ptr();
  }

  unsafe {
    *result = &env.last_error;
  }

  napi_ok
}

#[napi_sym]
fn napi_create_function<'s>(
  env: &'s mut Env,
  name: *const c_char,
  length: usize,
  cb: Option<napi_callback>,
  cb_info: napi_callback_info,
  result: *mut napi_value<'s>,
) -> napi_status {
  let env_ptr = env as *mut Env;
  check_arg!(env, result);
  check_arg!(env, cb);

  let name = if !name.is_null() {
    match unsafe { check_new_from_utf8_len(env, name, length) } {
      Ok(s) => Some(s),
      Err(status) => return status,
    }
  } else {
    None
  };

  unsafe {
    *result =
      create_function(&mut env.scope(), env_ptr, name, cb.unwrap(), cb_info)
        .into();
  }

  napi_ok
}

#[napi_sym]
#[allow(clippy::too_many_arguments)]
fn napi_define_class<'s>(
  env: &'s mut Env,
  utf8name: *const c_char,
  length: usize,
  constructor: Option<napi_callback>,
  callback_data: *mut c_void,
  property_count: usize,
  properties: *const napi_property_descriptor,
  result: *mut napi_value<'s>,
) -> napi_status {
  let env_ptr = env as *mut Env;
  check_arg!(env, result);
  check_arg!(env, constructor);

  if property_count > 0 {
    check_arg!(env, properties);
  }

  let name = match unsafe { check_new_from_utf8_len(env, utf8name, length) } {
    Ok(string) => string,
    Err(status) => return status,
  };

  let tpl = create_function_template(
    &mut env.scope(),
    env_ptr,
    Some(name),
    constructor.unwrap(),
    callback_data,
  );

  let napi_properties: &[napi_property_descriptor] = if property_count > 0 {
    unsafe { std::slice::from_raw_parts(properties, property_count) }
  } else {
    &[]
  };
  let mut static_property_count = 0;

  for p in napi_properties {
    if p.attributes & napi_static != 0 {
      // Will be handled below
      static_property_count += 1;
      continue;
    }

    let name = match unsafe { v8_name_from_property_descriptor(env_ptr, p) } {
      Ok(name) => name,
      Err(status) => return status,
    };

    if p.getter.is_some() || p.setter.is_some() {
      let getter = p.getter.map(|g| {
        create_function_template(&mut env.scope(), env_ptr, None, g, p.data)
      });
      let setter = p.setter.map(|s| {
        create_function_template(&mut env.scope(), env_ptr, None, s, p.data)
      });

      let mut accessor_property = v8::PropertyAttribute::NONE;
      if getter.is_some()
        && setter.is_some()
        && (p.attributes & napi_writable) == 0
      {
        accessor_property =
          accessor_property | v8::PropertyAttribute::READ_ONLY;
      }
      if p.attributes & napi_enumerable == 0 {
        accessor_property =
          accessor_property | v8::PropertyAttribute::DONT_ENUM;
      }
      if p.attributes & napi_configurable == 0 {
        accessor_property =
          accessor_property | v8::PropertyAttribute::DONT_DELETE;
      }

      let proto = tpl.prototype_template(&mut env.scope());
      proto.set_accessor_property(name, getter, setter, accessor_property);
    } else if let Some(method) = p.method {
      let function = create_function_template(
        &mut env.scope(),
        env_ptr,
        None,
        method,
        p.data,
      );
      let proto = tpl.prototype_template(&mut env.scope());
      proto.set(name, function.into());
    } else {
      let proto = tpl.prototype_template(&mut env.scope());
      proto.set(name, p.value.unwrap().into());
    }
  }

  let value: v8::Local<v8::Value> =
    tpl.get_function(&mut env.scope()).unwrap().into();

  unsafe {
    *result = value.into();
  }

  if static_property_count > 0 {
    let mut static_descriptors = Vec::with_capacity(static_property_count);

    for p in napi_properties {
      if p.attributes & napi_static != 0 {
        static_descriptors.push(*p);
      }
    }

    crate::status_call!(unsafe {
      napi_define_properties(
        env_ptr,
        *result,
        static_descriptors.len(),
        static_descriptors.as_ptr(),
      )
    });
  }

  napi_ok
}

#[napi_sym]
fn napi_get_property_names(
  env: *mut Env,
  object: napi_value,
  result: *mut napi_value,
) -> napi_status {
  unsafe {
    napi_get_all_property_names(
      env,
      object,
      napi_key_include_prototypes,
      napi_key_enumerable | napi_key_skip_symbols,
      napi_key_numbers_to_strings,
      result,
    )
  }
}

#[napi_sym]
fn napi_get_all_property_names<'s>(
  env: &'s mut Env,
  object: napi_value,
  key_mode: napi_key_collection_mode,
  key_filter: napi_key_filter,
  key_conversion: napi_key_conversion,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let scope = &mut env.scope();

  let Some(obj) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let mut filter = v8::PropertyFilter::ALL_PROPERTIES;

  if key_filter & napi_key_writable != 0 {
    filter = filter | v8::PropertyFilter::ONLY_WRITABLE;
  }
  if key_filter & napi_key_enumerable != 0 {
    filter = filter | v8::PropertyFilter::ONLY_ENUMERABLE;
  }
  if key_filter & napi_key_configurable != 0 {
    filter = filter | v8::PropertyFilter::ONLY_CONFIGURABLE;
  }
  if key_filter & napi_key_skip_strings != 0 {
    filter = filter | v8::PropertyFilter::SKIP_STRINGS;
  }
  if key_filter & napi_key_skip_symbols != 0 {
    filter = filter | v8::PropertyFilter::SKIP_SYMBOLS;
  }

  let key_mode = match key_mode {
    napi_key_include_prototypes => v8::KeyCollectionMode::IncludePrototypes,
    napi_key_own_only => v8::KeyCollectionMode::OwnOnly,
    _ => return napi_invalid_arg,
  };

  let key_conversion = match key_conversion {
    napi_key_keep_numbers => v8::KeyConversionMode::KeepNumbers,
    napi_key_numbers_to_strings => v8::KeyConversionMode::ConvertToString,
    _ => return napi_invalid_arg,
  };

  let filter = v8::GetPropertyNamesArgsBuilder::new()
    .mode(key_mode)
    .property_filter(filter)
    .index_filter(v8::IndexFilter::IncludeIndices)
    .key_conversion(key_conversion)
    .build();

  let property_names = match obj.get_property_names(scope, filter) {
    Some(n) => n,
    None => return napi_generic_failure,
  };

  unsafe {
    *result = property_names.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_set_property(
  env: &mut Env,
  object: napi_value,
  key: napi_value,
  value: napi_value,
) -> napi_status {
  check_arg!(env, key);
  check_arg!(env, value);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  if object.set(scope, key.unwrap(), value.unwrap()).is_none() {
    return napi_generic_failure;
  };

  napi_ok
}

#[napi_sym]
fn napi_has_property(
  env: &mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut bool,
) -> napi_status {
  check_arg!(env, key);
  check_arg!(env, result);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let Some(has) = object.has(scope, key.unwrap()) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = has;
  }

  napi_ok
}

#[napi_sym]
fn napi_get_property<'s>(
  env: &'s mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, key);
  check_arg!(env, result);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let Some(value) = object.get(scope, key.unwrap()) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = value.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_delete_property(
  env: &mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut bool,
) -> napi_status {
  check_arg!(env, key);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let Some(deleted) = object.delete(scope, key.unwrap()) else {
    return napi_generic_failure;
  };

  if !result.is_null() {
    unsafe {
      *result = deleted;
    }
  }

  napi_ok
}

#[napi_sym]
fn napi_has_own_property(
  env: &mut Env,
  object: napi_value,
  key: napi_value,
  result: *mut bool,
) -> napi_status {
  check_arg!(env, key);
  check_arg!(env, result);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let Ok(key) = v8::Local::<v8::Name>::try_from(key.unwrap()) else {
    return napi_name_expected;
  };

  let Some(has_own) = object.has_own_property(scope, key) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = has_own;
  }

  napi_ok
}

#[napi_sym]
fn napi_has_named_property<'s>(
  env: &'s mut Env,
  object: napi_value<'s>,
  utf8name: *const c_char,
  result: *mut bool,
) -> napi_status {
  let env_ptr = env as *mut Env;
  check_arg!(env, result);

  let Some(object) = object.and_then(|o| o.to_object(&mut env.scope())) else {
    return napi_object_expected;
  };

  let key = match unsafe { check_new_from_utf8(env_ptr, utf8name) } {
    Ok(key) => key,
    Err(status) => return status,
  };

  let Some(has_property) = object.has(&mut env.scope(), key.into()) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = has_property;
  }

  napi_ok
}

#[napi_sym]
fn napi_set_named_property<'s>(
  env: &'s mut Env,
  object: napi_value<'s>,
  utf8name: *const c_char,
  value: napi_value<'s>,
) -> napi_status {
  check_arg!(env, value);
  let env_ptr = env as *mut Env;

  let Some(object) = object.and_then(|o| o.to_object(&mut env.scope())) else {
    return napi_object_expected;
  };

  let key = match unsafe { check_new_from_utf8(env_ptr, utf8name) } {
    Ok(key) => key,
    Err(status) => return status,
  };

  let value = value.unwrap();

  if !object
    .set(&mut env.scope(), key.into(), value)
    .unwrap_or(false)
  {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym]
fn napi_get_named_property<'s>(
  env: &'s mut Env,
  object: napi_value<'s>,
  utf8name: *const c_char,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);
  let env_ptr = env as *mut Env;

  let Some(object) = object.and_then(|o| o.to_object(&mut env.scope())) else {
    return napi_object_expected;
  };

  let key = match unsafe { check_new_from_utf8(env_ptr, utf8name) } {
    Ok(key) => key,
    Err(status) => return status,
  };

  let Some(value) = object.get(&mut env.scope(), key.into()) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = value.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_set_element<'s>(
  env: &'s mut Env,
  object: napi_value<'s>,
  index: u32,
  value: napi_value<'s>,
) -> napi_status {
  check_arg!(env, value);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  if !object
    .set_index(scope, index, value.unwrap())
    .unwrap_or(false)
  {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym]
fn napi_has_element(
  env: &mut Env,
  object: napi_value,
  index: u32,
  result: *mut bool,
) -> napi_status {
  check_arg!(env, result);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let Some(has) = object.has_index(scope, index) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = has;
  }

  napi_ok
}

#[napi_sym]
fn napi_get_element<'s>(
  env: &'s mut Env,
  object: napi_value,
  index: u32,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let Some(value) = object.get_index(scope, index) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = value.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_delete_element(
  env: &mut Env,
  object: napi_value,
  index: u32,
  result: *mut bool,
) -> napi_status {
  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let Some(deleted) = object.delete_index(scope, index) else {
    return napi_generic_failure;
  };

  if !result.is_null() {
    unsafe {
      *result = deleted;
    }
  }

  napi_ok
}

#[napi_sym]
fn napi_define_properties(
  env: &mut Env,
  object: napi_value,
  property_count: usize,
  properties: *const napi_property_descriptor,
) -> napi_status {
  let env_ptr = env as *mut Env;

  if property_count > 0 {
    check_arg!(env, properties);
  }

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let properties = if property_count == 0 {
    &[]
  } else {
    unsafe { std::slice::from_raw_parts(properties, property_count) }
  };
  for property in properties {
    let property_name =
      match unsafe { v8_name_from_property_descriptor(env_ptr, property) } {
        Ok(name) => name,
        Err(status) => return status,
      };

    let writable = property.attributes & napi_writable != 0;
    let enumerable = property.attributes & napi_enumerable != 0;
    let configurable = property.attributes & napi_configurable != 0;

    if property.getter.is_some() || property.setter.is_some() {
      let local_getter: v8::Local<v8::Value> = if let Some(getter) =
        property.getter
      {
        create_function(&mut env.scope(), env_ptr, None, getter, property.data)
          .into()
      } else {
        v8::undefined(scope).into()
      };
      let local_setter: v8::Local<v8::Value> = if let Some(setter) =
        property.setter
      {
        create_function(&mut env.scope(), env_ptr, None, setter, property.data)
          .into()
      } else {
        v8::undefined(scope).into()
      };

      let mut desc =
        v8::PropertyDescriptor::new_from_get_set(local_getter, local_setter);
      desc.set_enumerable(enumerable);
      desc.set_configurable(configurable);

      if !object
        .define_property(scope, property_name, &desc)
        .unwrap_or(false)
      {
        return napi_invalid_arg;
      }
    } else if let Some(method) = property.method {
      let method: v8::Local<v8::Value> = {
        let function = create_function(
          &mut env.scope(),
          env_ptr,
          None,
          method,
          property.data,
        );
        function.into()
      };

      let mut desc =
        v8::PropertyDescriptor::new_from_value_writable(method, writable);
      desc.set_enumerable(enumerable);
      desc.set_configurable(configurable);

      if !object
        .define_property(scope, property_name, &desc)
        .unwrap_or(false)
      {
        return napi_generic_failure;
      }
    } else {
      let value = property.value.unwrap();

      if enumerable & writable & configurable {
        if !object
          .create_data_property(scope, property_name, value)
          .unwrap_or(false)
        {
          return napi_invalid_arg;
        }
      } else {
        let mut desc =
          v8::PropertyDescriptor::new_from_value_writable(value, writable);
        desc.set_enumerable(enumerable);
        desc.set_configurable(configurable);

        if !object
          .define_property(scope, property_name, &desc)
          .unwrap_or(false)
        {
          return napi_invalid_arg;
        }
      }
    }
  }

  napi_ok
}

#[napi_sym]
fn napi_object_freeze(env: &mut Env, object: napi_value) -> napi_status {
  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  if !object
    .set_integrity_level(scope, v8::IntegrityLevel::Frozen)
    .unwrap_or(false)
  {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym]
fn napi_object_seal(env: &mut Env, object: napi_value) -> napi_status {
  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  if !object
    .set_integrity_level(scope, v8::IntegrityLevel::Sealed)
    .unwrap_or(false)
  {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym]
fn napi_is_array(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  let value = value.unwrap();

  unsafe {
    *result = value.is_array();
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_array_length(
  env: &mut Env,
  value: napi_value,
  result: *mut u32,
) -> napi_status {
  check_arg!(env, value);
  check_arg!(env, result);

  let value = value.unwrap();

  match v8::Local::<v8::Array>::try_from(value) {
    Ok(array) => {
      unsafe {
        *result = array.length();
      }
      napi_ok
    }
    Err(_) => napi_array_expected,
  }
}

#[napi_sym]
fn napi_strict_equals(
  env: &mut Env,
  lhs: napi_value,
  rhs: napi_value,
  result: *mut bool,
) -> napi_status {
  check_arg!(env, lhs);
  check_arg!(env, rhs);
  check_arg!(env, result);

  unsafe {
    *result = lhs.unwrap().strict_equals(rhs.unwrap());
  }

  napi_ok
}

#[napi_sym]
fn napi_get_prototype<'s>(
  env: &'s mut Env,
  object: napi_value,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let scope = &mut env.scope();

  let Some(object) = object.and_then(|o| o.to_object(scope)) else {
    return napi_object_expected;
  };

  let Some(proto) = object.get_prototype(scope) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = proto.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_create_object(
  env_ptr: *mut Env,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::Object::new(&mut env.scope()).into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_create_array(
  env_ptr: *mut Env,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::Array::new(&mut env.scope(), 0).into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_create_array_with_length(
  env_ptr: *mut Env,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::Array::new(&mut env.scope(), length as _).into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_create_string_latin1(
  env_ptr: *mut Env,
  string: *const c_char,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  if length > 0 {
    check_arg!(env, string);
  }
  crate::return_status_if_false!(
    env,
    (length == NAPI_AUTO_LENGTH) || length <= INT_MAX as _,
    napi_invalid_arg
  );

  let buffer = if length > 0 {
    unsafe {
      std::slice::from_raw_parts(
        string as _,
        if length == NAPI_AUTO_LENGTH {
          std::ffi::CStr::from_ptr(string).to_bytes().len()
        } else {
          length
        },
      )
    }
  } else {
    &[]
  };

  let Some(string) = v8::String::new_from_one_byte(
    &mut env.scope(),
    buffer,
    v8::NewStringType::Normal,
  ) else {
    return napi_set_last_error(env_ptr, napi_generic_failure);
  };

  unsafe {
    *result = string.into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_create_string_utf8(
  env_ptr: *mut Env,
  string: *const c_char,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  if length > 0 {
    check_arg!(env, string);
  }
  crate::return_status_if_false!(
    env,
    (length == NAPI_AUTO_LENGTH) || length <= INT_MAX as _,
    napi_invalid_arg
  );

  let buffer = if length > 0 {
    unsafe {
      std::slice::from_raw_parts(
        string as _,
        if length == NAPI_AUTO_LENGTH {
          std::ffi::CStr::from_ptr(string).to_bytes().len()
        } else {
          length
        },
      )
    }
  } else {
    &[]
  };

  let Some(string) = v8::String::new_from_utf8(
    &mut env.scope(),
    buffer,
    v8::NewStringType::Normal,
  ) else {
    return napi_set_last_error(env_ptr, napi_generic_failure);
  };

  unsafe {
    *result = string.into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_create_string_utf16(
  env_ptr: *mut Env,
  string: *const u16,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  if length > 0 {
    check_arg!(env, string);
  }
  crate::return_status_if_false!(
    env,
    (length == NAPI_AUTO_LENGTH) || length <= INT_MAX as _,
    napi_invalid_arg
  );

  let buffer = if length > 0 {
    unsafe {
      std::slice::from_raw_parts(
        string,
        if length == NAPI_AUTO_LENGTH {
          let mut length = 0;
          while *(string.add(length)) != 0 {
            length += 1;
          }
          length
        } else {
          length
        },
      )
    }
  } else {
    &[]
  };

  let Some(string) = v8::String::new_from_two_byte(
    &mut env.scope(),
    buffer,
    v8::NewStringType::Normal,
  ) else {
    return napi_set_last_error(env_ptr, napi_generic_failure);
  };

  unsafe {
    *result = string.into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn node_api_create_external_string_latin1(
  env_ptr: *mut Env,
  string: *const c_char,
  length: usize,
  nogc_finalize_callback: Option<napi_finalize>,
  finalize_hint: *mut c_void,
  result: *mut napi_value,
  copied: *mut bool,
) -> napi_status {
  let status =
    unsafe { napi_create_string_latin1(env_ptr, string, length, result) };

  if status == napi_ok {
    unsafe {
      *copied = true;
    }

    if let Some(finalize) = nogc_finalize_callback {
      unsafe {
        finalize(env_ptr as napi_env, string as *mut c_void, finalize_hint);
      }
    }
  }

  status
}

#[napi_sym]
fn node_api_create_external_string_utf16(
  env_ptr: *mut Env,
  string: *const u16,
  length: usize,
  nogc_finalize_callback: Option<napi_finalize>,
  finalize_hint: *mut c_void,
  result: *mut napi_value,
  copied: *mut bool,
) -> napi_status {
  let status =
    unsafe { napi_create_string_utf16(env_ptr, string, length, result) };

  if status == napi_ok {
    unsafe {
      *copied = true;
    }

    if let Some(finalize) = nogc_finalize_callback {
      unsafe {
        finalize(env_ptr as napi_env, string as *mut c_void, finalize_hint);
      }
    }
  }

  status
}

#[napi_sym]
fn node_api_create_property_key_utf16(
  env_ptr: *mut Env,
  string: *const u16,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  if length > 0 {
    check_arg!(env, string);
  }
  crate::return_status_if_false!(
    env,
    (length == NAPI_AUTO_LENGTH) || length <= INT_MAX as _,
    napi_invalid_arg
  );

  let buffer = if length > 0 {
    unsafe {
      std::slice::from_raw_parts(
        string,
        if length == NAPI_AUTO_LENGTH {
          let mut length = 0;
          while *(string.add(length)) != 0 {
            length += 1;
          }
          length
        } else {
          length
        },
      )
    }
  } else {
    &[]
  };

  let Some(string) = v8::String::new_from_two_byte(
    &mut env.scope(),
    buffer,
    v8::NewStringType::Internalized,
  ) else {
    return napi_set_last_error(env_ptr, napi_generic_failure);
  };

  unsafe {
    *result = string.into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_create_double(
  env_ptr: *mut Env,
  value: f64,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::Number::new(&mut env.scope(), value).into();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_create_int32(
  env_ptr: *mut Env,
  value: i32,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::Integer::new(&mut env.scope(), value).into();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_create_uint32(
  env_ptr: *mut Env,
  value: u32,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::Integer::new_from_unsigned(&mut env.scope(), value).into();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_create_int64(
  env_ptr: *mut Env,
  value: i64,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::Number::new(&mut env.scope(), value as _).into();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_create_bigint_int64(
  env_ptr: *mut Env,
  value: i64,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::BigInt::new_from_i64(&mut env.scope(), value).into();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_create_bigint_uint64(
  env_ptr: *mut Env,
  value: u64,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = v8::BigInt::new_from_u64(&mut env.scope(), value).into();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_create_bigint_words<'s>(
  env: &'s mut Env,
  sign_bit: bool,
  word_count: usize,
  words: *const u64,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, words);
  check_arg!(env, result);

  if word_count > INT_MAX as _ {
    return napi_invalid_arg;
  }

  match v8::BigInt::new_from_words(&mut env.scope(), sign_bit, unsafe {
    std::slice::from_raw_parts(words, word_count)
  }) {
    Some(value) => unsafe {
      *result = value.into();
    },
    None => {
      return napi_generic_failure;
    }
  }

  napi_ok
}

#[napi_sym]
fn napi_get_boolean(
  env: *mut Env,
  value: bool,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, result);

  unsafe {
    *result = v8::Boolean::new(env.isolate(), value).into();
  }

  return napi_clear_last_error(env);
}

#[napi_sym]
fn napi_create_symbol(
  env_ptr: *mut Env,
  description: napi_value,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  let description = if let Some(d) = *description {
    let Some(d) = d.to_string(&mut env.scope()) else {
      return napi_set_last_error(env, napi_string_expected);
    };
    Some(d)
  } else {
    None
  };

  unsafe {
    *result = v8::Symbol::new(&mut env.scope(), description).into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn node_api_symbol_for(
  env: *mut Env,
  utf8description: *const c_char,
  length: usize,
  result: *mut napi_value,
) -> napi_status {
  {
    let env = check_env!(env);
    check_arg!(env, result);

    let description_string =
      match unsafe { check_new_from_utf8_len(env, utf8description, length) } {
        Ok(s) => s,
        Err(status) => return napi_set_last_error(env, status),
      };

    unsafe {
      *result =
        v8::Symbol::for_key(&mut env.scope(), description_string).into();
    }
  }

  napi_clear_last_error(env)
}

macro_rules! napi_create_error_impl {
  ($env_ptr:ident, $code:ident, $msg:ident, $result:ident, $error:ident) => {{
    let env_ptr = $env_ptr;
    let code = $code;
    let msg = $msg;
    let result = $result;

    let env = check_env!(env_ptr);
    check_arg!(env, msg);
    check_arg!(env, result);

    let Some(message) =
      msg.and_then(|v| v8::Local::<v8::String>::try_from(v).ok())
    else {
      return napi_set_last_error(env_ptr, napi_string_expected);
    };

    let error = v8::Exception::$error(&mut env.scope(), message);

    if let Some(code) = *code {
      let error_obj: v8::Local<v8::Object> = error.try_into().unwrap();
      let code_key = v8::String::new(&mut env.scope(), "code").unwrap();
      if !error_obj
        .set(&mut env.scope(), code_key.into(), code)
        .unwrap_or(false)
      {
        return napi_set_last_error(env_ptr, napi_generic_failure);
      }
    }

    unsafe {
      *result = error.into();
    }

    return napi_clear_last_error(env_ptr);
  }};
}

#[napi_sym]
fn napi_create_error(
  env_ptr: *mut Env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> napi_status {
  napi_create_error_impl!(env_ptr, code, msg, result, error)
}

#[napi_sym]
fn napi_create_type_error(
  env_ptr: *mut Env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> napi_status {
  napi_create_error_impl!(env_ptr, code, msg, result, type_error)
}

#[napi_sym]
fn napi_create_range_error(
  env_ptr: *mut Env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> napi_status {
  napi_create_error_impl!(env_ptr, code, msg, result, range_error)
}

#[napi_sym]
fn node_api_create_syntax_error(
  env_ptr: *mut Env,
  code: napi_value,
  msg: napi_value,
  result: *mut napi_value,
) -> napi_status {
  napi_create_error_impl!(env_ptr, code, msg, result, syntax_error)
}

pub fn get_value_type(value: v8::Local<v8::Value>) -> Option<napi_valuetype> {
  if value.is_undefined() {
    Some(napi_undefined)
  } else if value.is_null() {
    Some(napi_null)
  } else if value.is_external() {
    Some(napi_external)
  } else if value.is_boolean() {
    Some(napi_boolean)
  } else if value.is_number() {
    Some(napi_number)
  } else if value.is_big_int() {
    Some(napi_bigint)
  } else if value.is_string() {
    Some(napi_string)
  } else if value.is_symbol() {
    Some(napi_symbol)
  } else if value.is_function() {
    Some(napi_function)
  } else if value.is_object() {
    Some(napi_object)
  } else {
    None
  }
}

#[napi_sym]
fn napi_typeof(
  env: *mut Env,
  value: napi_value,
  result: *mut napi_valuetype,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(ty) = get_value_type(value.unwrap()) else {
    return napi_set_last_error(env, napi_invalid_arg);
  };

  unsafe {
    *result = ty;
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_undefined(env: *mut Env, result: *mut napi_value) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, result);

  unsafe {
    *result = v8::undefined(&mut env.scope()).into();
  }

  return napi_clear_last_error(env);
}

#[napi_sym]
fn napi_get_null(env: *mut Env, result: *mut napi_value) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, result);

  unsafe {
    *result = v8::null(&mut env.scope()).into();
  }

  return napi_clear_last_error(env);
}

#[napi_sym]
fn napi_get_cb_info(
  env: *mut Env,
  cbinfo: napi_callback_info,
  argc: *mut i32,
  argv: *mut napi_value,
  this_arg: *mut napi_value,
  data: *mut *mut c_void,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, cbinfo);

  let cbinfo: &CallbackInfo = unsafe { &*(cbinfo as *const CallbackInfo) };
  let args = unsafe { &*(cbinfo.args as *const v8::FunctionCallbackArguments) };

  if !argv.is_null() {
    check_arg!(env, argc);
    let argc = unsafe { *argc as usize };
    for i in 0..argc {
      let mut arg = args.get(i as _);
      unsafe {
        *argv.add(i) = arg.into();
      }
    }
  }

  if !argc.is_null() {
    unsafe {
      *argc = args.length();
    }
  }

  if !this_arg.is_null() {
    unsafe {
      *this_arg = args.this().into();
    }
  }

  if !data.is_null() {
    unsafe {
      *data = cbinfo.cb_info;
    }
  }

  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym]
fn napi_get_new_target(
  env: *mut Env,
  cbinfo: napi_callback_info,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, cbinfo);
  check_arg!(env, result);

  let cbinfo: &CallbackInfo = unsafe { &*(cbinfo as *const CallbackInfo) };
  let args = unsafe { &*(cbinfo.args as *const v8::FunctionCallbackArguments) };

  unsafe {
    *result = args.new_target().into();
  }

  return napi_clear_last_error(env);
}

#[napi_sym]
fn napi_call_function(
  env_ptr: *mut Env,
  recv: napi_value,
  func: napi_value,
  argc: usize,
  argv: *const napi_value,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, recv);
  let args = if argc > 0 {
    check_arg!(env, argv);
    unsafe {
      std::slice::from_raw_parts(argv as *mut v8::Local<v8::Value>, argc)
    }
  } else {
    &[]
  };

  let Some(func) =
    func.and_then(|f| v8::Local::<v8::Function>::try_from(f).ok())
  else {
    return napi_set_last_error(env, napi_function_expected);
  };

  let Some(v) = func.call(&mut env.scope(), recv.unwrap(), args) else {
    return napi_set_last_error(env_ptr, napi_generic_failure);
  };

  if !result.is_null() {
    unsafe {
      *result = v.into();
    }
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_global(env_ptr: *mut Env, result: *mut napi_value) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  let global = v8::Local::new(&mut env.scope(), &env.global);
  unsafe {
    *result = global.into();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_throw(env: *mut Env, error: napi_value) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, error);

  if env.last_exception.is_some() {
    return napi_pending_exception;
  }

  let error = error.unwrap();
  env.scope().throw_exception(error);
  let error = v8::Global::new(&mut env.scope(), error);
  env.last_exception = Some(error);

  napi_clear_last_error(env)
}

macro_rules! napi_throw_error_impl {
  ($env:ident, $code:ident, $msg:ident, $error:ident) => {{
    let env = check_env!($env);
    let env_ptr = env as *mut Env;
    let code = $code;
    let msg = $msg;

    if env.last_exception.is_some() {
      return napi_pending_exception;
    }

    let str_ = match unsafe { check_new_from_utf8(env, msg) } {
      Ok(s) => s,
      Err(status) => return status,
    };

    let error = v8::Exception::$error(&mut env.scope(), str_);

    if !code.is_null() {
      let error_obj: v8::Local<v8::Object> = error.try_into().unwrap();
      let code = match unsafe { check_new_from_utf8(env_ptr, code) } {
        Ok(s) => s,
        Err(status) => return napi_set_last_error(env, status),
      };
      let code_key = v8::String::new(&mut env.scope(), "code").unwrap();
      if !error_obj
        .set(&mut env.scope(), code_key.into(), code.into())
        .unwrap_or(false)
      {
        return napi_set_last_error(env, napi_generic_failure);
      }
    }

    env.scope().throw_exception(error);
    let error = v8::Global::new(&mut env.scope(), error);
    env.last_exception = Some(error);

    napi_clear_last_error(env)
  }};
}

#[napi_sym]
fn napi_throw_error(
  env: *mut Env,
  code: *const c_char,
  msg: *const c_char,
) -> napi_status {
  napi_throw_error_impl!(env, code, msg, error)
}

#[napi_sym]
fn napi_throw_type_error(
  env: *mut Env,
  code: *const c_char,
  msg: *const c_char,
) -> napi_status {
  napi_throw_error_impl!(env, code, msg, type_error)
}

#[napi_sym]
fn napi_throw_range_error(
  env: *mut Env,
  code: *const c_char,
  msg: *const c_char,
) -> napi_status {
  napi_throw_error_impl!(env, code, msg, range_error)
}

#[napi_sym]
fn node_api_throw_syntax_error(
  env: *mut Env,
  code: *const c_char,
  msg: *const c_char,
) -> napi_status {
  napi_throw_error_impl!(env, code, msg, syntax_error)
}

#[napi_sym]
fn napi_is_error(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  unsafe {
    *result = value.unwrap().is_native_error();
  }

  return napi_clear_last_error(env);
}

#[napi_sym]
fn napi_get_value_double(
  env_ptr: *mut Env,
  value: napi_value,
  result: *mut f64,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(number) =
    value.and_then(|v| v8::Local::<v8::Number>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_number_expected);
  };

  unsafe {
    *result = number.value();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_value_int32(
  env_ptr: *mut Env,
  value: napi_value,
  result: *mut i32,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(value) = value.unwrap().int32_value(&mut env.scope()) else {
    return napi_set_last_error(env, napi_number_expected);
  };

  unsafe {
    *result = value;
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_value_uint32(
  env_ptr: *mut Env,
  value: napi_value,
  result: *mut u32,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(value) = value.unwrap().uint32_value(&mut env.scope()) else {
    return napi_set_last_error(env, napi_number_expected);
  };

  unsafe {
    *result = value;
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_value_int64(
  env_ptr: *mut Env,
  value: napi_value,
  result: *mut i64,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(number) =
    value.and_then(|v| v8::Local::<v8::Number>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_number_expected);
  };

  let value = number.value();

  unsafe {
    if value.is_finite() {
      *result = value as _;
    } else {
      *result = 0;
    }
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_value_bigint_int64(
  env_ptr: *mut Env,
  value: napi_value,
  result: *mut i64,
  lossless: *mut bool,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, result);
  check_arg!(env, lossless);

  let Some(bigint) =
    value.and_then(|v| v8::Local::<v8::BigInt>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_bigint_expected);
  };

  let (result_, lossless_) = bigint.i64_value();

  unsafe {
    *result = result_;
    *lossless = lossless_;
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_value_bigint_uint64(
  env_ptr: *mut Env,
  value: napi_value,
  result: *mut u64,
  lossless: *mut bool,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, result);
  check_arg!(env, lossless);

  let Some(bigint) =
    value.and_then(|v| v8::Local::<v8::BigInt>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_bigint_expected);
  };

  let (result_, lossless_) = bigint.u64_value();

  unsafe {
    *result = result_;
    *lossless = lossless_;
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_value_bigint_words(
  env_ptr: *mut Env,
  value: napi_value,
  sign_bit: *mut i32,
  word_count: *mut usize,
  words: *mut u64,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, word_count);

  let Some(bigint) =
    value.and_then(|v| v8::Local::<v8::BigInt>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_bigint_expected);
  };

  let word_count_int;

  if sign_bit.is_null() && words.is_null() {
    word_count_int = bigint.word_count();
  } else {
    check_arg!(env, sign_bit);
    check_arg!(env, words);
    let out_words =
      unsafe { std::slice::from_raw_parts_mut(words, *word_count) };
    let (sign, slice_) = bigint.to_words_array(out_words);
    word_count_int = slice_.len();
    unsafe {
      *sign_bit = sign as i32;
    }
  }

  unsafe {
    *word_count = word_count_int;
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_value_bool(
  env_ptr: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(boolean) =
    value.and_then(|v| v8::Local::<v8::Boolean>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_boolean_expected);
  };

  unsafe {
    *result = boolean.is_true();
  }

  return napi_clear_last_error(env_ptr);
}

#[napi_sym]
fn napi_get_value_string_latin1(
  env_ptr: *mut Env,
  value: napi_value,
  buf: *mut c_char,
  bufsize: usize,
  result: *mut usize,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);

  let Some(value) =
    value.and_then(|v| v8::Local::<v8::String>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_string_expected);
  };

  if buf.is_null() {
    check_arg!(env, result);
    unsafe {
      *result = value.length();
    }
  } else if bufsize != 0 {
    let buffer =
      unsafe { std::slice::from_raw_parts_mut(buf as _, bufsize - 1) };
    let copied = value.write_one_byte(
      &mut env.scope(),
      buffer,
      0,
      v8::WriteOptions::NO_NULL_TERMINATION,
    );
    unsafe {
      buf.add(copied).write(0);
    }
    if !result.is_null() {
      unsafe {
        *result = copied;
      }
    }
  } else if !result.is_null() {
    unsafe {
      *result = 0;
    }
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_get_value_string_utf8(
  env_ptr: *mut Env,
  value: napi_value,
  buf: *mut u8,
  bufsize: usize,
  result: *mut usize,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);

  let Some(value) =
    value.and_then(|v| v8::Local::<v8::String>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_string_expected);
  };

  if buf.is_null() {
    check_arg!(env, result);
    unsafe {
      *result = value.utf8_length(env.isolate());
    }
  } else if bufsize != 0 {
    let buffer =
      unsafe { std::slice::from_raw_parts_mut(buf as _, bufsize - 1) };
    let copied = value.write_utf8(
      &mut env.scope(),
      buffer,
      None,
      v8::WriteOptions::REPLACE_INVALID_UTF8
        | v8::WriteOptions::NO_NULL_TERMINATION,
    );
    unsafe {
      buf.add(copied).write(0);
    }
    if !result.is_null() {
      unsafe {
        *result = copied;
      }
    }
  } else if !result.is_null() {
    unsafe {
      *result = 0;
    }
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_get_value_string_utf16(
  env_ptr: *mut Env,
  value: napi_value,
  buf: *mut u16,
  bufsize: usize,
  result: *mut usize,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);

  let Some(value) =
    value.and_then(|v| v8::Local::<v8::String>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_string_expected);
  };

  if buf.is_null() {
    check_arg!(env, result);
    unsafe {
      *result = value.length();
    }
  } else if bufsize != 0 {
    let buffer =
      unsafe { std::slice::from_raw_parts_mut(buf as _, bufsize - 1) };
    let copied = value.write(
      &mut env.scope(),
      buffer,
      0,
      v8::WriteOptions::NO_NULL_TERMINATION,
    );
    unsafe {
      buf.add(copied).write(0);
    }
    if !result.is_null() {
      unsafe {
        *result = copied;
      }
    }
  } else if !result.is_null() {
    unsafe {
      *result = 0;
    }
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_coerce_to_bool<'s>(
  env: &'s mut Env,
  value: napi_value,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, value);
  check_arg!(env, result);

  let coerced = value.unwrap().to_boolean(&mut env.scope());

  unsafe {
    *result = coerced.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_coerce_to_number<'s>(
  env: &'s mut Env,
  value: napi_value,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(coerced) = value.unwrap().to_number(&mut env.scope()) else {
    return napi_number_expected;
  };

  unsafe {
    *result = coerced.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_coerce_to_object<'s>(
  env: &'s mut Env,
  value: napi_value,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(coerced) = value.unwrap().to_object(&mut env.scope()) else {
    return napi_object_expected;
  };

  unsafe {
    *result = coerced.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_coerce_to_string<'s>(
  env: &'s mut Env,
  value: napi_value,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(coerced) = value.unwrap().to_string(&mut env.scope()) else {
    return napi_string_expected;
  };

  unsafe {
    *result = coerced.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_wrap(
  env: &mut Env,
  js_object: napi_value,
  native_object: *mut c_void,
  finalize_cb: Option<napi_finalize>,
  finalize_hint: *mut c_void,
  result: *mut napi_ref,
) -> napi_status {
  check_arg!(env, js_object);
  let env_ptr = env as *mut Env;

  let Some(obj) =
    js_object.and_then(|v| v8::Local::<v8::Object>::try_from(v).ok())
  else {
    return napi_invalid_arg;
  };

  let napi_wrap = v8::Local::new(&mut env.scope(), &env.shared().napi_wrap);

  if obj
    .has_private(&mut env.scope(), napi_wrap)
    .unwrap_or(false)
  {
    return napi_invalid_arg;
  }

  if !result.is_null() {
    check_arg!(env, finalize_cb);
  }

  let ownership = if result.is_null() {
    ReferenceOwnership::Runtime
  } else {
    ReferenceOwnership::Userland
  };
  let reference = Reference::new(
    env_ptr,
    obj.into(),
    0,
    ownership,
    finalize_cb,
    native_object,
    finalize_hint,
  );

  let reference = Reference::into_raw(reference) as *mut c_void;

  if !result.is_null() {
    check_arg!(env, finalize_cb);
    unsafe {
      *result = reference;
    }
  }

  let external = v8::External::new(&mut env.scope(), reference);
  assert!(obj
    .set_private(&mut env.scope(), napi_wrap, external.into())
    .unwrap());

  napi_ok
}

fn unwrap(
  env: &mut Env,
  obj: napi_value,
  result: *mut *mut c_void,
  keep: bool,
) -> napi_status {
  check_arg!(env, obj);
  if keep {
    check_arg!(env, result);
  }

  let Some(obj) = obj.and_then(|v| v8::Local::<v8::Object>::try_from(v).ok())
  else {
    return napi_invalid_arg;
  };

  let napi_wrap = v8::Local::new(&mut env.scope(), &env.shared().napi_wrap);
  let Some(val) = obj.get_private(&mut env.scope(), napi_wrap) else {
    return napi_invalid_arg;
  };

  let Ok(external) = v8::Local::<v8::External>::try_from(val) else {
    return napi_invalid_arg;
  };

  let reference = external.value() as *mut Reference;
  let reference = unsafe { &mut *reference };

  if !result.is_null() {
    unsafe {
      *result = reference.finalize_data;
    }
  }

  if !keep {
    assert!(obj
      .delete_private(&mut env.scope(), napi_wrap)
      .unwrap_or(false));
    unsafe { Reference::remove(reference) };
  }

  napi_ok
}

#[napi_sym]
fn napi_unwrap(
  env: &mut Env,
  obj: napi_value,
  result: *mut *mut c_void,
) -> napi_status {
  unwrap(env, obj, result, true)
}

#[napi_sym]
fn napi_remove_wrap(
  env: &mut Env,
  obj: napi_value,
  result: *mut *mut c_void,
) -> napi_status {
  unwrap(env, obj, result, false)
}

struct ExternalWrapper {
  data: *mut c_void,
  type_tag: Option<napi_type_tag>,
}

#[napi_sym]
fn napi_create_external<'s>(
  env: &'s mut Env,
  data: *mut c_void,
  finalize_cb: Option<napi_finalize>,
  finalize_hint: *mut c_void,
  result: *mut napi_value<'s>,
) -> napi_status {
  let env_ptr = env as *mut Env;
  check_arg!(env, result);

  let wrapper = Box::new(ExternalWrapper {
    data,
    type_tag: None,
  });

  let wrapper = Box::into_raw(wrapper);
  let external = v8::External::new(&mut env.scope(), wrapper as _);

  if let Some(finalize_cb) = finalize_cb {
    Reference::into_raw(Reference::new(
      env_ptr,
      external.into(),
      0,
      ReferenceOwnership::Runtime,
      Some(finalize_cb),
      data,
      finalize_hint,
    ));
  }

  unsafe {
    *result = external.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_type_tag_object(
  env: &mut Env,
  object_or_external: napi_value,
  type_tag: *const napi_type_tag,
) -> napi_status {
  check_arg!(env, object_or_external);
  check_arg!(env, type_tag);

  let val = object_or_external.unwrap();

  if let Ok(external) = v8::Local::<v8::External>::try_from(val) {
    let wrapper_ptr = external.value() as *mut ExternalWrapper;
    let wrapper = unsafe { &mut *wrapper_ptr };
    if wrapper.type_tag.is_some() {
      return napi_invalid_arg;
    }
    wrapper.type_tag = Some(unsafe { *type_tag });
    return napi_ok;
  }

  let Some(object) = val.to_object(&mut env.scope()) else {
    return napi_object_expected;
  };

  let key = v8::Local::new(&mut env.scope(), &env.shared().type_tag);

  if object.has_private(&mut env.scope(), key).unwrap_or(false) {
    return napi_invalid_arg;
  }

  let slice = unsafe { std::slice::from_raw_parts(type_tag as *const u64, 2) };
  let Some(tag) = v8::BigInt::new_from_words(&mut env.scope(), false, slice)
  else {
    return napi_generic_failure;
  };

  if !object
    .set_private(&mut env.scope(), key, tag.into())
    .unwrap_or(false)
  {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym]
fn napi_check_object_type_tag(
  env: &mut Env,
  object_or_external: napi_value,
  type_tag: *const napi_type_tag,
  result: *mut bool,
) -> napi_status {
  check_arg!(env, object_or_external);
  check_arg!(env, type_tag);
  check_arg!(env, result);

  let type_tag = unsafe { *type_tag };

  let val = object_or_external.unwrap();

  if let Ok(external) = v8::Local::<v8::External>::try_from(val) {
    let wrapper_ptr = external.value() as *mut ExternalWrapper;
    let wrapper = unsafe { &mut *wrapper_ptr };
    unsafe {
      *result = match wrapper.type_tag {
        Some(t) => t == type_tag,
        None => false,
      };
    };
    return napi_ok;
  }

  let Some(object) = val.to_object(&mut env.scope()) else {
    return napi_object_expected;
  };

  let key = v8::Local::new(&mut env.scope(), &env.shared().type_tag);

  let Some(val) = object.get_private(&mut env.scope(), key) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = false;
  }

  if let Ok(bigint) = v8::Local::<v8::BigInt>::try_from(val) {
    let mut words = [0u64; 2];
    let (sign, words) = bigint.to_words_array(&mut words);
    if !sign {
      let pass = if words.len() == 2 {
        type_tag.lower == words[0] && type_tag.upper == words[1]
      } else if words.len() == 1 {
        type_tag.lower == words[0] && type_tag.upper == 0
      } else if words.is_empty() {
        type_tag.lower == 0 && type_tag.upper == 0
      } else {
        false
      };
      unsafe {
        *result = pass;
      }
    }
  }

  napi_ok
}

#[napi_sym]
fn napi_get_value_external(
  env: *mut Env,
  value: napi_value,
  result: *mut *mut c_void,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  let Some(external) =
    value.and_then(|v| v8::Local::<v8::External>::try_from(v).ok())
  else {
    return napi_set_last_error(env, napi_invalid_arg);
  };

  let wrapper_ptr = external.value() as *const ExternalWrapper;
  let wrapper = unsafe { &*wrapper_ptr };

  unsafe {
    *result = wrapper.data;
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_create_reference(
  env: *mut Env,
  value: napi_value,
  initial_refcount: u32,
  result: *mut napi_ref,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  let value = value.unwrap();

  let reference = Reference::new(
    env,
    value,
    initial_refcount,
    ReferenceOwnership::Userland,
    None,
    std::ptr::null_mut(),
    std::ptr::null_mut(),
  );

  let ptr = Reference::into_raw(reference);

  unsafe {
    *result = ptr as _;
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_delete_reference(env: *mut Env, ref_: napi_ref) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, ref_);

  let reference = unsafe { Reference::from_raw(ref_ as _) };

  drop(reference);

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_reference_ref(
  env: *mut Env,
  ref_: napi_ref,
  result: *mut u32,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, ref_);

  let reference = unsafe { &mut *(ref_ as *mut Reference) };

  let count = reference.ref_();

  if !result.is_null() {
    unsafe {
      *result = count;
    }
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_reference_unref(
  env: *mut Env,
  ref_: napi_ref,
  result: *mut u32,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, ref_);

  let reference = unsafe { &mut *(ref_ as *mut Reference) };

  if reference.ref_count == 0 {
    return napi_set_last_error(env, napi_generic_failure);
  }

  let count = reference.unref();

  if !result.is_null() {
    unsafe {
      *result = count;
    }
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_reference_value(
  env_ptr: *mut Env,
  ref_: napi_ref,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, ref_);
  check_arg!(env, result);

  let reference = unsafe { &mut *(ref_ as *mut Reference) };

  let value = match &reference.state {
    ReferenceState::Strong(g) => Some(v8::Local::new(&mut env.scope(), g)),
    ReferenceState::Weak(w) => w.to_local(&mut env.scope()),
  };

  unsafe {
    *result = value.into();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_open_handle_scope(
  env: *mut Env,
  _result: *mut napi_handle_scope,
) -> napi_status {
  let env = check_env!(env);
  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_close_handle_scope(
  env: *mut Env,
  _scope: napi_handle_scope,
) -> napi_status {
  let env = check_env!(env);
  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_open_escapable_handle_scope(
  env: *mut Env,
  _result: *mut napi_escapable_handle_scope,
) -> napi_status {
  let env = check_env!(env);
  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_close_escapable_handle_scope(
  env: *mut Env,
  _scope: napi_escapable_handle_scope,
) -> napi_status {
  let env = check_env!(env);
  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_escape_handle<'s>(
  env: *mut Env,
  _scope: napi_escapable_handle_scope,
  escapee: napi_value<'s>,
  result: *mut napi_value<'s>,
) -> napi_status {
  let env = check_env!(env);

  unsafe {
    *result = escapee;
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_new_instance<'s>(
  env: &'s mut Env,
  constructor: napi_value,
  argc: usize,
  argv: *const napi_value,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, constructor);
  if argc > 0 {
    check_arg!(env, argv);
  }
  check_arg!(env, result);

  let Some(func) =
    constructor.and_then(|v| v8::Local::<v8::Function>::try_from(v).ok())
  else {
    return napi_invalid_arg;
  };

  let args = if argc > 0 {
    unsafe {
      std::slice::from_raw_parts(argv as *mut v8::Local<v8::Value>, argc)
    }
  } else {
    &[]
  };

  let Some(value) = func.new_instance(&mut env.scope(), args) else {
    return napi_pending_exception;
  };

  unsafe {
    *result = value.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_instanceof(
  env: &mut Env,
  object: napi_value,
  constructor: napi_value,
  result: *mut bool,
) -> napi_status {
  check_arg!(env, object);
  check_arg!(env, result);

  let Some(ctor) = constructor.and_then(|v| v.to_object(&mut env.scope()))
  else {
    return napi_object_expected;
  };

  if !ctor.is_function() {
    unsafe {
      napi_throw_type_error(
        env,
        c"ERR_NAPI_CONS_FUNCTION".as_ptr(),
        c"Constructor must be a function".as_ptr(),
      );
    }
    return napi_function_expected;
  }

  let Some(res) = object.unwrap().instance_of(&mut env.scope(), ctor) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = res;
  }

  napi_ok
}

#[napi_sym]
fn napi_is_exception_pending(
  env_ptr: *mut Env,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  unsafe {
    *result = env.last_exception.is_some();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_get_and_clear_last_exception(
  env_ptr: *mut Env,
  result: *mut napi_value,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, result);

  let ex: v8::Local<v8::Value> =
    if let Some(last_exception) = env.last_exception.take() {
      v8::Local::new(&mut env.scope(), last_exception)
    } else {
      v8::undefined(&mut env.scope()).into()
    };

  unsafe {
    *result = ex.into();
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_is_arraybuffer(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  unsafe {
    *result = value.unwrap().is_array_buffer();
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_create_arraybuffer<'s>(
  env: &'s mut Env,
  len: usize,
  data: *mut *mut c_void,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let buffer = v8::ArrayBuffer::new(&mut env.scope(), len);

  if !data.is_null() {
    unsafe {
      *data = get_array_buffer_ptr(buffer);
    }
  }

  unsafe {
    *result = buffer.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_create_external_arraybuffer<'s>(
  env: &'s mut Env,
  data: *mut c_void,
  byte_length: usize,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let store = make_external_backing_store(
    env,
    data,
    byte_length,
    std::ptr::null_mut(),
    finalize_cb,
    finalize_hint,
  );

  let ab =
    v8::ArrayBuffer::with_backing_store(&mut env.scope(), &store.make_shared());
  let value: v8::Local<v8::Value> = ab.into();

  unsafe {
    *result = value.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_get_arraybuffer_info(
  env: *mut Env,
  value: napi_value,
  data: *mut *mut c_void,
  length: *mut usize,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);

  let Some(buf) =
    value.and_then(|v| v8::Local::<v8::ArrayBuffer>::try_from(v).ok())
  else {
    return napi_set_last_error(env, napi_invalid_arg);
  };

  if !data.is_null() {
    unsafe {
      *data = get_array_buffer_ptr(buf);
    }
  }

  if !length.is_null() {
    unsafe {
      *length = buf.byte_length();
    }
  }

  napi_ok
}

#[napi_sym]
fn napi_is_typedarray(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  unsafe {
    *result = value.unwrap().is_typed_array();
  }

  napi_ok
}

#[napi_sym]
fn napi_create_typedarray<'s>(
  env: &'s mut Env,
  ty: napi_typedarray_type,
  length: usize,
  arraybuffer: napi_value,
  byte_offset: usize,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, arraybuffer);
  check_arg!(env, result);

  let Some(ab) =
    arraybuffer.and_then(|v| v8::Local::<v8::ArrayBuffer>::try_from(v).ok())
  else {
    return napi_arraybuffer_expected;
  };

  macro_rules! create {
    ($TypedArray:ident, $size_of_element:expr) => {{
      let soe = $size_of_element;
      if soe > 1 && byte_offset % soe != 0 {
        let message = v8::String::new(
          &mut env.scope(),
          format!(
            "start offset of {} should be multiple of {}",
            stringify!($TypedArray),
            soe
          )
          .as_str(),
        )
        .unwrap();
        let exc = v8::Exception::range_error(&mut env.scope(), message);
        env.scope().throw_exception(exc);
        return napi_pending_exception;
      }

      if length * soe + byte_offset > ab.byte_length() {
        let message =
          v8::String::new(&mut env.scope(), "Invalid typed array length")
            .unwrap();
        let exc = v8::Exception::range_error(&mut env.scope(), message);
        env.scope().throw_exception(exc);
        return napi_pending_exception;
      }

      let Some(ta) =
        v8::$TypedArray::new(&mut env.scope(), ab, byte_offset, length)
      else {
        return napi_generic_failure;
      };
      ta.into()
    }};
  }

  let typedarray: v8::Local<v8::Value> = match ty {
    napi_uint8_array => create!(Uint8Array, 1),
    napi_uint8_clamped_array => create!(Uint8ClampedArray, 1),
    napi_int8_array => create!(Int8Array, 1),
    napi_uint16_array => create!(Uint16Array, 2),
    napi_int16_array => create!(Int16Array, 2),
    napi_uint32_array => create!(Uint32Array, 4),
    napi_int32_array => create!(Int32Array, 4),
    napi_float32_array => create!(Float32Array, 4),
    napi_float64_array => create!(Float64Array, 8),
    napi_bigint64_array => create!(BigInt64Array, 8),
    napi_biguint64_array => create!(BigUint64Array, 8),
    _ => {
      return napi_invalid_arg;
    }
  };

  unsafe {
    *result = typedarray.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_get_typedarray_info(
  env_ptr: *mut Env,
  typedarray: napi_value,
  type_: *mut napi_typedarray_type,
  length: *mut usize,
  data: *mut *mut c_void,
  arraybuffer: *mut napi_value,
  byte_offset: *mut usize,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, typedarray);

  let Some(array) =
    typedarray.and_then(|v| v8::Local::<v8::TypedArray>::try_from(v).ok())
  else {
    return napi_set_last_error(env_ptr, napi_invalid_arg);
  };

  if !type_.is_null() {
    let tatype = if array.is_int8_array() {
      napi_int8_array
    } else if array.is_uint8_array() {
      napi_uint8_array
    } else if array.is_uint8_clamped_array() {
      napi_uint8_clamped_array
    } else if array.is_int16_array() {
      napi_int16_array
    } else if array.is_uint16_array() {
      napi_uint16_array
    } else if array.is_int32_array() {
      napi_int32_array
    } else if array.is_uint32_array() {
      napi_uint32_array
    } else if array.is_float32_array() {
      napi_float32_array
    } else if array.is_float64_array() {
      napi_float64_array
    } else if array.is_big_int64_array() {
      napi_bigint64_array
    } else if array.is_big_uint64_array() {
      napi_biguint64_array
    } else {
      unreachable!();
    };

    unsafe {
      *type_ = tatype;
    }
  }

  if !length.is_null() {
    unsafe {
      *length = array.length();
    }
  }

  if !data.is_null() {
    unsafe {
      *data = array.data();
    }
  }

  if !arraybuffer.is_null() {
    let buf = array.buffer(&mut env.scope()).unwrap();
    unsafe {
      *arraybuffer = buf.into();
    }
  }

  if !byte_offset.is_null() {
    unsafe {
      *byte_offset = array.byte_offset();
    }
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_create_dataview<'s>(
  env: &'s mut Env,
  byte_length: usize,
  arraybuffer: napi_value<'s>,
  byte_offset: usize,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, arraybuffer);
  check_arg!(env, result);

  let Some(buffer) =
    arraybuffer.and_then(|v| v8::Local::<v8::ArrayBuffer>::try_from(v).ok())
  else {
    return napi_invalid_arg;
  };

  if byte_length + byte_offset > buffer.byte_length() {
    unsafe {
      return napi_throw_range_error(
          env,
          c"ERR_NAPI_INVALID_DATAVIEW_ARGS".as_ptr(),
          c"byte_offset + byte_length should be less than or equal to the size in bytes of the array passed in".as_ptr(),
        );
    }
  }

  let dataview =
    v8::DataView::new(&mut env.scope(), buffer, byte_offset, byte_length);

  unsafe {
    *result = dataview.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_is_dataview(
  env: *mut Env,
  value: napi_value,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, result);

  unsafe {
    *result = value.unwrap().is_data_view();
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_dataview_info(
  env_ptr: *mut Env,
  dataview: napi_value,
  byte_length: *mut usize,
  data: *mut *mut c_void,
  arraybuffer: *mut napi_value,
  byte_offset: *mut usize,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, dataview);

  let Some(array) =
    dataview.and_then(|v| v8::Local::<v8::DataView>::try_from(v).ok())
  else {
    return napi_invalid_arg;
  };

  if !byte_length.is_null() {
    unsafe {
      *byte_length = array.byte_length();
    }
  }

  if !arraybuffer.is_null() {
    let Some(buffer) = array.buffer(&mut env.scope()) else {
      return napi_generic_failure;
    };

    unsafe {
      *arraybuffer = buffer.into();
    }
  }

  if !data.is_null() {
    unsafe {
      *data = array.data();
    }
  }

  if !byte_offset.is_null() {
    unsafe {
      *byte_offset = array.byte_offset();
    }
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn napi_get_version(env: *mut Env, result: *mut u32) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, result);

  unsafe {
    *result = NAPI_VERSION;
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_create_promise<'s>(
  env: &'s mut Env,
  deferred: *mut napi_deferred,
  promise: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, deferred);
  check_arg!(env, promise);

  let resolver = v8::PromiseResolver::new(&mut env.scope()).unwrap();

  let global = v8::Global::new(&mut env.scope(), resolver);
  let global_ptr = global.into_raw().as_ptr() as napi_deferred;

  let p = resolver.get_promise(&mut env.scope());

  unsafe {
    *deferred = global_ptr;
  }

  unsafe {
    *promise = p.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_resolve_deferred(
  env: &mut Env,
  deferred: napi_deferred,
  result: napi_value,
) -> napi_status {
  check_arg!(env, result);
  check_arg!(env, deferred);

  // Make sure microtasks don't run and call back into JS
  env
    .scope()
    .set_microtasks_policy(v8::MicrotasksPolicy::Explicit);

  let deferred_ptr =
    unsafe { NonNull::new_unchecked(deferred as *mut v8::PromiseResolver) };
  let global = unsafe { v8::Global::from_raw(env.isolate(), deferred_ptr) };
  let resolver = v8::Local::new(&mut env.scope(), global);

  let success = resolver
    .resolve(&mut env.scope(), result.unwrap())
    .unwrap_or(false);

  // Restore policy
  env
    .scope()
    .set_microtasks_policy(v8::MicrotasksPolicy::Auto);

  if success {
    napi_ok
  } else {
    napi_generic_failure
  }
}

#[napi_sym]
fn napi_reject_deferred(
  env: &mut Env,
  deferred: napi_deferred,
  result: napi_value,
) -> napi_status {
  check_arg!(env, result);
  check_arg!(env, deferred);

  let deferred_ptr =
    unsafe { NonNull::new_unchecked(deferred as *mut v8::PromiseResolver) };
  let global = unsafe { v8::Global::from_raw(env.isolate(), deferred_ptr) };
  let resolver = v8::Local::new(&mut env.scope(), global);

  if !resolver
    .reject(&mut env.scope(), result.unwrap())
    .unwrap_or(false)
  {
    return napi_generic_failure;
  }

  napi_ok
}

#[napi_sym]
fn napi_is_promise(
  env: *mut Env,
  value: napi_value,
  is_promise: *mut bool,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, is_promise);

  unsafe {
    *is_promise = value.unwrap().is_promise();
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_create_date<'s>(
  env: &'s mut Env,
  time: f64,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, result);

  let Some(date) = v8::Date::new(&mut env.scope(), time) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = date.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_is_date(
  env: *mut Env,
  value: napi_value,
  is_date: *mut bool,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);
  check_arg!(env, is_date);

  unsafe {
    *is_date = value.unwrap().is_date();
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_date_value(
  env: &mut Env,
  value: napi_value,
  result: *mut f64,
) -> napi_status {
  check_arg!(env, result);

  let Some(date) = value.and_then(|v| v8::Local::<v8::Date>::try_from(v).ok())
  else {
    return napi_date_expected;
  };

  unsafe {
    *result = date.value_of();
  }

  napi_ok
}

#[napi_sym]
fn napi_run_script<'s>(
  env: &'s mut Env,
  script: napi_value,
  result: *mut napi_value<'s>,
) -> napi_status {
  check_arg!(env, script);
  check_arg!(env, result);

  let Some(script) =
    script.and_then(|v| v8::Local::<v8::String>::try_from(v).ok())
  else {
    return napi_string_expected;
  };

  let Some(script) = v8::Script::compile(&mut env.scope(), script, None) else {
    return napi_generic_failure;
  };

  let Some(rv) = script.run(&mut env.scope()) else {
    return napi_generic_failure;
  };

  unsafe {
    *result = rv.into();
  }

  napi_ok
}

#[napi_sym]
fn napi_add_finalizer(
  env_ptr: *mut Env,
  value: napi_value,
  finalize_data: *mut c_void,
  finalize_cb: Option<napi_finalize>,
  finalize_hint: *mut c_void,
  result: *mut napi_ref,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, value);
  check_arg!(env, finalize_cb);

  let Some(value) =
    value.and_then(|v| v8::Local::<v8::Object>::try_from(v).ok())
  else {
    return napi_set_last_error(env, napi_invalid_arg);
  };

  let ownership = if result.is_null() {
    ReferenceOwnership::Runtime
  } else {
    ReferenceOwnership::Userland
  };
  let reference = Reference::new(
    env,
    value.into(),
    0,
    ownership,
    finalize_cb,
    finalize_data,
    finalize_hint,
  );

  if !result.is_null() {
    unsafe {
      *result = Reference::into_raw(reference) as _;
    }
  }

  napi_clear_last_error(env_ptr)
}

#[napi_sym]
fn node_api_post_finalizer(
  env: *mut Env,
  _finalize_cb: napi_finalize,
  _finalize_data: *mut c_void,
  _finalize_hint: *mut c_void,
) -> napi_status {
  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_adjust_external_memory(
  env: *mut Env,
  change_in_bytes: i64,
  adjusted_value: *mut i64,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, adjusted_value);

  unsafe {
    *adjusted_value = env
      .isolate()
      .adjust_amount_of_external_allocated_memory(change_in_bytes);
  }

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_set_instance_data(
  env: *mut Env,
  data: *mut c_void,
  finalize_cb: Option<napi_finalize>,
  finalize_hint: *mut c_void,
) -> napi_status {
  let env = check_env!(env);

  env.shared_mut().instance_data = Some(InstanceData {
    data,
    finalize_cb,
    finalize_hint,
  });

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_get_instance_data(
  env: *mut Env,
  data: *mut *mut c_void,
) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, data);

  let instance_data = match &env.shared().instance_data {
    Some(v) => v.data,
    None => std::ptr::null_mut(),
  };

  unsafe { *data = instance_data };

  napi_clear_last_error(env)
}

#[napi_sym]
fn napi_detach_arraybuffer(env: *mut Env, value: napi_value) -> napi_status {
  let env = check_env!(env);
  check_arg!(env, value);

  let Some(ab) =
    value.and_then(|v| v8::Local::<v8::ArrayBuffer>::try_from(v).ok())
  else {
    return napi_set_last_error(env, napi_arraybuffer_expected);
  };

  if !ab.is_detachable() {
    return napi_set_last_error(env, napi_detachable_arraybuffer_expected);
  }

  // Expected to crash for None.
  ab.detach(None).unwrap();

  napi_clear_last_error(env);
  napi_ok
}

#[napi_sym]
fn napi_is_detached_arraybuffer(
  env_ptr: *mut Env,
  arraybuffer: napi_value,
  result: *mut bool,
) -> napi_status {
  let env = check_env!(env_ptr);
  check_arg!(env, arraybuffer);
  check_arg!(env, result);

  let is_detached = match arraybuffer
    .and_then(|v| v8::Local::<v8::ArrayBuffer>::try_from(v).ok())
  {
    Some(ab) => ab.was_detached(),
    None => false,
  };

  unsafe {
    *result = is_detached;
  }

  napi_clear_last_error(env)
}
