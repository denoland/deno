// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::impl_magic;
use crate::magic::tr8::FromV8;
use crate::magic::tr8::ToV8;
use std::mem::transmute;

/// serde_v8::Value allows passing through `v8::Value`s untouched
/// when encoding/decoding and allows mixing rust & v8 values in
/// structs, tuples...
/// The implementation mainly breaks down to:
/// 1. Transmuting between u64 <> serde_v8::Value
/// 2. Using special struct/field names to detect these values
/// 3. Then serde "boilerplate"
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
    &self,
    _scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    Ok(unsafe { transmute(self.v8_value) })
  }
}

impl FromV8 for Value<'_> {
  fn from_v8(
    _scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    Ok(unsafe { transmute::<Value, Value>(value.into()) })
  }
}
