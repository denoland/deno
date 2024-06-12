// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_runtime::deno_napi::*;
use libc::INT_MAX;

#[repr(transparent)]
pub struct SendPtr<T>(pub *const T);

impl<T> SendPtr<T> {
  // silly function to get around `clippy::redundant_locals`
  pub fn take(self) -> *const T {
    self.0
  }
}

unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

pub fn get_array_buffer_ptr(ab: v8::Local<v8::ArrayBuffer>) -> *mut c_void {
  match ab.data() {
    Some(p) => p.as_ptr(),
    None => std::ptr::null_mut(),
  }
}

struct BufferFinalizer {
  env: *mut Env,
  finalize_cb: napi_finalize,
  finalize_data: *mut c_void,
  finalize_hint: *mut c_void,
}

impl Drop for BufferFinalizer {
  fn drop(&mut self) {
    unsafe {
      (self.finalize_cb)(self.env as _, self.finalize_data, self.finalize_hint);
    }
  }
}

pub extern "C" fn backing_store_deleter_callback(
  data: *mut c_void,
  _byte_length: usize,
  deleter_data: *mut c_void,
) {
  let mut finalizer =
    unsafe { Box::<BufferFinalizer>::from_raw(deleter_data as _) };

  finalizer.finalize_data = data;

  drop(finalizer);
}

pub fn make_external_backing_store(
  env: *mut Env,
  data: *mut c_void,
  byte_length: usize,
  finalize_data: *mut c_void,
  finalize_cb: napi_finalize,
  finalize_hint: *mut c_void,
) -> v8::UniqueRef<v8::BackingStore> {
  let finalizer = Box::new(BufferFinalizer {
    env,
    finalize_data,
    finalize_cb,
    finalize_hint,
  });

  unsafe {
    v8::ArrayBuffer::new_backing_store_from_ptr(
      data,
      byte_length,
      backing_store_deleter_callback,
      Box::into_raw(finalizer) as _,
    )
  }
}

#[macro_export]
macro_rules! check_env {
  ($env: expr) => {{
    let env = $env;
    if env.is_null() {
      return napi_invalid_arg;
    }
    unsafe { &mut *env }
  }};
}

#[macro_export]
macro_rules! return_error_status_if_false {
  ($env: expr, $condition: expr, $status: ident) => {
    if !$condition {
      return Err(
        $crate::napi::util::napi_set_last_error($env, $status).into(),
      );
    }
  };
}

#[macro_export]
macro_rules! return_status_if_false {
  ($env: expr, $condition: expr, $status: ident) => {
    if !$condition {
      return $crate::napi::util::napi_set_last_error($env, $status);
    }
  };
}

pub(crate) unsafe fn check_new_from_utf8_len<'s>(
  env: *mut Env,
  str_: *const c_char,
  len: usize,
) -> Result<v8::Local<'s, v8::String>, napi_status> {
  let env = unsafe { &mut *env };
  return_error_status_if_false!(
    env,
    (len == NAPI_AUTO_LENGTH) || len <= INT_MAX as _,
    napi_invalid_arg
  );
  return_error_status_if_false!(env, !str_.is_null(), napi_invalid_arg);
  let string = if len == NAPI_AUTO_LENGTH {
    let result = unsafe { std::ffi::CStr::from_ptr(str_ as *const _) }.to_str();
    return_error_status_if_false!(env, result.is_ok(), napi_generic_failure);
    result.unwrap()
  } else {
    let string = unsafe { std::slice::from_raw_parts(str_ as *const u8, len) };
    let result = std::str::from_utf8(string);
    return_error_status_if_false!(env, result.is_ok(), napi_generic_failure);
    result.unwrap()
  };
  let result = {
    let env = unsafe { &mut *(env as *mut Env) };
    v8::String::new(&mut env.scope(), string)
  };
  return_error_status_if_false!(env, result.is_some(), napi_generic_failure);
  Ok(result.unwrap())
}

#[inline]
pub(crate) unsafe fn check_new_from_utf8<'s>(
  env: *mut Env,
  str_: *const c_char,
) -> Result<v8::Local<'s, v8::String>, napi_status> {
  unsafe { check_new_from_utf8_len(env, str_, NAPI_AUTO_LENGTH) }
}

pub(crate) unsafe fn v8_name_from_property_descriptor<'s>(
  env: *mut Env,
  p: &'s napi_property_descriptor,
) -> Result<v8::Local<'s, v8::Name>, napi_status> {
  if !p.utf8name.is_null() {
    unsafe { check_new_from_utf8(env, p.utf8name).map(|v| v.into()) }
  } else {
    match *p.name {
      Some(v) => match v.try_into() {
        Ok(name) => Ok(name),
        Err(_) => Err(napi_name_expected),
      },
      None => Err(napi_name_expected),
    }
  }
}

pub(crate) fn napi_clear_last_error(env: *mut Env) -> napi_status {
  let env = unsafe { &mut *env };
  env.last_error.error_code = napi_ok;
  env.last_error.engine_error_code = 0;
  env.last_error.engine_reserved = std::ptr::null_mut();
  env.last_error.error_message = std::ptr::null_mut();
  napi_ok
}

pub(crate) fn napi_set_last_error(
  env: *mut Env,
  error_code: napi_status,
) -> napi_status {
  let env = unsafe { &mut *env };
  env.last_error.error_code = error_code;
  error_code
}

#[macro_export]
macro_rules! status_call {
  ($call: expr) => {
    let status = $call;
    if status != napi_ok {
      return status;
    }
  };
}

pub trait Nullable {
  fn is_null(&self) -> bool;
}

impl<T> Nullable for *mut T {
  fn is_null(&self) -> bool {
    (*self).is_null()
  }
}

impl<T> Nullable for *const T {
  fn is_null(&self) -> bool {
    (*self).is_null()
  }
}

impl<T> Nullable for Option<T> {
  fn is_null(&self) -> bool {
    self.is_none()
  }
}

impl<'s> Nullable for napi_value<'s> {
  fn is_null(&self) -> bool {
    self.is_none()
  }
}

#[macro_export]
macro_rules! check_arg {
  ($env: expr, $ptr: expr) => {
    $crate::return_status_if_false!(
      $env,
      !$crate::napi::util::Nullable::is_null(&$ptr),
      napi_invalid_arg
    );
  };
}

#[macro_export]
macro_rules! napi_wrap {
  ( $( # $attr:tt )* fn $name:ident $( < $( $x:lifetime ),* > )? ( $env:ident : & $( $lt:lifetime )? mut Env $( , $ident:ident : $ty:ty )* $(,)? ) -> napi_status $body:block ) => {
    $( # $attr )*
    #[no_mangle]
    pub unsafe extern "C" fn $name $( < $( $x ),* > )? ( env_ptr : *mut Env , $( $ident : $ty ),* ) -> napi_status {
      let env: & $( $lt )? mut Env = $crate::check_env!(env_ptr);

      if env.last_exception.is_some() {
        return napi_pending_exception;
      }

      $crate::napi::util::napi_clear_last_error(env);

      let scope_env = unsafe { &mut *env_ptr };
      let scope = &mut scope_env.scope();
      let try_catch = &mut v8::TryCatch::new(scope);

      #[inline(always)]
      fn inner $( < $( $x ),* > )? ( $env: & $( $lt )? mut Env , $( $ident : $ty ),* ) -> napi_status $body

      log::trace!("NAPI ENTER: {}", stringify!($name));

      let result = inner( env, $( $ident ),* );

      log::trace!("NAPI EXIT: {} {}", stringify!($name), result);

      if let Some(exception) = try_catch.exception() {
        let env = unsafe { &mut *env_ptr };
        let global = v8::Global::new(env.isolate(), exception);
        env.last_exception = Some(global);
        return $crate::napi::util::napi_set_last_error(env_ptr, napi_pending_exception);
      }

      if result != napi_ok {
        return $crate::napi::util::napi_set_last_error(env_ptr, result);
      }

      return result;
    }
  };

  ( $( # $attr:tt )* fn $name:ident $( < $( $x:lifetime ),* > )? ( $( $ident:ident : $ty:ty ),* $(,)? ) -> napi_status $body:block ) => {
    $( # $attr )*
    #[no_mangle]
    pub unsafe extern "C" fn $name $( < $( $x ),* > )? ( $( $ident : $ty ),* ) -> napi_status {
      #[inline(always)]
      fn inner $( < $( $x ),* > )? ( $( $ident : $ty ),* ) -> napi_status $body

      log::trace!("NAPI ENTER: {}", stringify!($name));

      let result = inner( $( $ident ),* );

      log::trace!("NAPI EXIT: {} {}", stringify!($name), result);

      result
    }
  };
}
