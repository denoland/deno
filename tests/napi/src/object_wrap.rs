// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;
use napi_sys::ValueType::napi_number;
use napi_sys::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::ptr;

pub struct NapiObject {
  counter: i32,
}

thread_local! {
  // map from native object ptr to napi reference (this is similar to what napi-rs does)
  static REFS: RefCell<HashMap<*mut c_void, napi_ref>> = RefCell::new(HashMap::new());
}

pub extern "C" fn finalize_napi_object(
  env: napi_env,
  finalize_data: *mut c_void,
  _finalize_hint: *mut c_void,
) {
  let obj = unsafe { Box::from_raw(finalize_data as *mut NapiObject) };
  drop(obj);
  if let Some(reference) =
    REFS.with_borrow_mut(|map| map.remove(&finalize_data))
  {
    unsafe { napi_delete_reference(env, reference) };
  }
}

impl NapiObject {
  fn new_inner(
    env: napi_env,
    info: napi_callback_info,
    finalizer: napi_finalize,
    out_ptr: Option<*mut napi_ref>,
  ) -> napi_value {
    assert!(matches!(
      (finalizer, out_ptr),
      (None, None) | (Some(_), Some(_))
    ));
    let mut new_target: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_get_new_target(env, info, &mut new_target));
    let is_constructor = !new_target.is_null();

    let (args, argc, this) = napi_get_callback_info!(env, info, 1);
    assert_eq!(argc, 1);

    if is_constructor {
      let mut value = 0;

      let mut ty = -1;
      assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
      assert_eq!(ty, napi_number);

      assert_napi_ok!(napi_get_value_int32(env, args[0], &mut value));

      let obj = Box::new(Self { counter: value });
      let obj_raw = Box::into_raw(obj) as *mut c_void;
      assert_napi_ok!(napi_wrap(
        env,
        this,
        obj_raw,
        finalizer,
        ptr::null_mut(),
        out_ptr.unwrap_or(ptr::null_mut())
      ));

      if let Some(p) = out_ptr {
        if finalizer.is_some() {
          REFS.with_borrow_mut(|map| map.insert(obj_raw, unsafe { p.read() }));
        }
      }

      return this;
    }

    unreachable!();
  }

  #[allow(clippy::new_ret_no_self)]
  pub extern "C" fn new(env: napi_env, info: napi_callback_info) -> napi_value {
    Self::new_inner(env, info, None, None)
  }

  #[allow(clippy::new_ret_no_self)]
  pub extern "C" fn new_with_finalizer(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value {
    let mut out = ptr::null_mut();
    Self::new_inner(env, info, Some(finalize_napi_object), Some(&mut out))
  }

  pub extern "C" fn set_value(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value {
    let (args, argc, this) = napi_get_callback_info!(env, info, 1);
    assert_eq!(argc, 1);
    let mut obj: *mut Self = ptr::null_mut();
    assert_napi_ok!(napi_unwrap(
      env,
      this,
      &mut obj as *mut _ as *mut *mut c_void
    ));

    assert_napi_ok!(napi_get_value_int32(env, args[0], &mut (*obj).counter));

    ptr::null_mut()
  }

  pub extern "C" fn get_value(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value {
    let (_args, argc, this) = napi_get_callback_info!(env, info, 0);
    assert_eq!(argc, 0);
    let mut obj: *mut Self = ptr::null_mut();
    assert_napi_ok!(napi_unwrap(
      env,
      this,
      &mut obj as *mut _ as *mut *mut c_void
    ));

    let mut num: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_create_int32(env, (*obj).counter, &mut num));

    num
  }

  pub extern "C" fn increment(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value {
    let (_args, argc, this) = napi_get_callback_info!(env, info, 0);
    assert_eq!(argc, 0);
    let mut obj: *mut Self = ptr::null_mut();
    assert_napi_ok!(napi_unwrap(
      env,
      this,
      &mut obj as *mut _ as *mut *mut c_void
    ));

    unsafe {
      (*obj).counter += 1;
    }

    ptr::null_mut()
  }

  pub extern "C" fn factory(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value {
    let (_args, argc, _this) = napi_get_callback_info!(env, info, 0);
    assert_eq!(argc, 0);

    let int64 = 64;
    let mut value: napi_value = ptr::null_mut();
    assert_napi_ok!(napi_create_int64(env, int64, &mut value));
    value
  }
}

pub fn init(env: napi_env, exports: napi_value) {
  let mut static_prop = napi_new_property!(env, "factory", NapiObject::factory);
  static_prop.attributes = PropertyAttributes::static_;

  let properties = &[
    napi_new_property!(env, "set_value", NapiObject::set_value),
    napi_new_property!(env, "get_value", NapiObject::get_value),
    napi_new_property!(env, "increment", NapiObject::increment),
    static_prop,
  ];

  let mut cons: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_define_class(
    env,
    "NapiObject\0".as_ptr() as *mut c_char,
    usize::MAX,
    Some(NapiObject::new),
    ptr::null_mut(),
    properties.len(),
    properties.as_ptr(),
    &mut cons,
  ));

  assert_napi_ok!(napi_set_named_property(
    env,
    exports,
    "NapiObject\0".as_ptr() as *const c_char,
    cons,
  ));

  let mut cons: napi_value = ptr::null_mut();
  assert_napi_ok!(napi_define_class(
    env,
    c"NapiObjectOwned".as_ptr(),
    usize::MAX,
    Some(NapiObject::new_with_finalizer),
    ptr::null_mut(),
    properties.len(),
    properties.as_ptr(),
    &mut cons,
  ));

  assert_napi_ok!(napi_set_named_property(
    env,
    exports,
    "NapiObjectOwned\0".as_ptr() as *const c_char,
    cons,
  ));
}
