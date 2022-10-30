// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use napi_sys::Status::napi_ok;
use napi_sys::ValueType::napi_number;
use napi_sys::*;
use std::os::raw::{c_char, c_void};
use std::ptr;

pub struct NapiObject {
  counter: i32,
  _wrapper: napi_ref,
}

impl NapiObject {
  #[allow(clippy::new_ret_no_self)]
  pub extern "C" fn new(env: napi_env, info: napi_callback_info) -> napi_value {
    let mut new_target: napi_value = ptr::null_mut();
    assert!(
      unsafe { napi_get_new_target(env, info, &mut new_target) } == napi_ok
    );
    let is_constructor = !new_target.is_null();

    let (args, argc, this) = crate::get_callback_info!(env, info, 1);
    assert_eq!(argc, 1);

    if is_constructor {
      let mut value = 0;

      let mut ty = -1;
      assert!(unsafe { napi_typeof(env, args[0], &mut ty) } == napi_ok);
      assert_eq!(ty, napi_number);

      assert!(
        unsafe { napi_get_value_int32(env, args[0], &mut value) } == napi_ok
      );

      let mut wrapper: napi_ref = ptr::null_mut();
      let obj = Box::new(Self {
        counter: value,
        _wrapper: wrapper,
      });
      assert!(
        unsafe {
          napi_wrap(
            env,
            this,
            Box::into_raw(obj) as *mut c_void,
            None,
            ptr::null_mut(),
            &mut wrapper,
          )
        } == napi_ok
      );

      return this;
    }

    unreachable!();
  }

  pub extern "C" fn set_value(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value {
    let (args, argc, this) = crate::get_callback_info!(env, info, 1);
    assert_eq!(argc, 1);
    let mut obj: *mut Self = ptr::null_mut();
    assert!(
      unsafe { napi_unwrap(env, this, &mut obj as *mut _ as *mut *mut c_void) }
        == napi_ok
    );

    assert!(
      unsafe { napi_get_value_int32(env, args[0], &mut (*obj).counter) }
        == napi_ok
    );

    ptr::null_mut()
  }

  pub extern "C" fn get_value(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value {
    let (_args, argc, this) = crate::get_callback_info!(env, info, 0);
    assert_eq!(argc, 0);
    let mut obj: *mut Self = ptr::null_mut();
    assert!(
      unsafe { napi_unwrap(env, this, &mut obj as *mut _ as *mut *mut c_void) }
        == napi_ok
    );

    let mut num: napi_value = ptr::null_mut();
    assert!(
      unsafe { napi_create_int32(env, (*obj).counter, &mut num) } == napi_ok
    );

    num
  }

  pub extern "C" fn increment(
    env: napi_env,
    info: napi_callback_info,
  ) -> napi_value {
    let (_args, argc, this) = crate::get_callback_info!(env, info, 0);
    assert_eq!(argc, 0);
    let mut obj: *mut Self = ptr::null_mut();
    assert!(
      unsafe { napi_unwrap(env, this, &mut obj as *mut _ as *mut *mut c_void) }
        == napi_ok
    );

    unsafe {
      (*obj).counter += 1;
    }

    ptr::null_mut()
  }
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    crate::new_property!(env, "set_value\0", NapiObject::set_value),
    crate::new_property!(env, "get_value\0", NapiObject::get_value),
    crate::new_property!(env, "increment\0", NapiObject::increment),
  ];

  let mut cons: napi_value = ptr::null_mut();
  assert!(
    unsafe {
      napi_define_class(
        env,
        "NapiObject\0".as_ptr() as *mut c_char,
        usize::MAX,
        Some(NapiObject::new),
        ptr::null_mut(),
        properties.len(),
        properties.as_ptr(),
        &mut cons,
      )
    } == napi_ok
  );

  assert!(
    unsafe {
      napi_set_named_property(
        env,
        exports,
        "NapiObject\0".as_ptr() as *const c_char,
        cons,
      )
    } == napi_ok
  );
}
