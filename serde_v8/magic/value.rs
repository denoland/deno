// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::magic::transl8::impl_magic;
use crate::magic::transl8::FromV8;
use crate::magic::transl8::ToV8;
use std::mem::transmute;

/// serde_v8::Value allows passing through `v8::Value`s untouched
/// when de/serializing & allows mixing rust & v8 values in structs, tuples...
//
// SAFETY: caveat emptor, the rust-compiler can no longer link lifetimes to their
// original scope, you must take special care in ensuring your handles don't outlive their scope
pub struct Value<'s> {
  pub v8_value: v8::Local<'s, v8::Value>,
}
impl_magic!(Value<'_>);

impl<'s> From<v8::Local<'s, v8::Value>> for Value<'s> {
  fn from(v8_value: v8::Local<'s, v8::Value>) -> Self {
    Self { v8_value }
  }
}

impl<'s> From<Value<'s>> for v8::Local<'s, v8::Value> {
  fn from(v: Value<'s>) -> Self {
    v.v8_value
  }
}

impl ToV8 for Value<'_> {
  fn to_v8<'a>(
    &mut self,
    _scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    // SAFETY: not fully safe, since lifetimes are detached from original scope
    Ok(unsafe { transmute(self.v8_value) })
  }
}

impl FromV8 for Value<'_> {
  fn from_v8(
    _scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    // SAFETY: not fully safe, since lifetimes are detached from original scope
    Ok(unsafe { transmute::<Value, Value>(value.into()) })
  }
}
