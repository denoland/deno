// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::magic::transl8::impl_magic;
use crate::magic::transl8::FromV8;
use crate::magic::transl8::ToV8;

pub struct Global {
  pub v8_value: v8::Global<v8::Value>,
}
impl_magic!(Global);

impl From<v8::Global<v8::Value>> for Global {
  fn from(v8_value: v8::Global<v8::Value>) -> Self {
    Self { v8_value }
  }
}

impl From<Global> for v8::Global<v8::Value> {
  fn from(v: Global) -> Self {
    v.v8_value
  }
}

impl ToV8 for Global {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    Ok(v8::Local::new(scope, self.v8_value.clone()))
  }
}

impl FromV8 for Global {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let global = v8::Global::new(scope, value);
    Ok(global.into())
  }
}
