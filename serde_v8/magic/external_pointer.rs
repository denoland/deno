// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::ffi::c_void;

use super::transl8::impl_magic;
use super::transl8::FromV8;
use super::transl8::ToV8;

pub struct ExternalPointer(*mut c_void);

// SAFETY: Nonblocking FFI is user controller and we must trust user to have it right.
unsafe impl Send for ExternalPointer {}
// SAFETY: Nonblocking FFI is user controller and we must trust user to have it right.
unsafe impl Sync for ExternalPointer {}

impl_magic!(ExternalPointer);

impl ToV8 for ExternalPointer {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    if self.0.is_null() {
      Ok(v8::null(scope).into())
    } else {
      Ok(v8::External::new(scope, self.0).into())
    }
  }
}

impl FromV8 for ExternalPointer {
  fn from_v8(
    _scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    if value.is_null() {
      Ok(ExternalPointer(std::ptr::null_mut()))
    } else if let Ok(external) = v8::Local::<v8::External>::try_from(value) {
      Ok(ExternalPointer(external.value()))
    } else {
      Err(crate::Error::ExpectedExternal)
    }
  }
}

impl From<*mut c_void> for ExternalPointer {
  fn from(value: *mut c_void) -> Self {
    ExternalPointer(value)
  }
}

impl From<*const c_void> for ExternalPointer {
  fn from(value: *const c_void) -> Self {
    ExternalPointer(value as *mut c_void)
  }
}
