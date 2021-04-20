// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use rusty_v8 as v8;

/// Deserializable allows custom v8 deserializables beyond `serde::Deserializable`s,
/// enabling things such as deserializing `v8::Value`s to value serialized buffers
pub trait Deserializable
where
  Self: Sized,
{
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error>;
}

/// Allows all implementors of `serde::Deserialize` to implement Deserializable
impl<T: serde::de::DeserializeOwned> Deserializable for T {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<T, crate::Error> {
    crate::from_v8(scope, value)
  }
}
