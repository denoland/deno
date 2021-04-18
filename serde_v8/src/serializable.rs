// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use rusty_v8 as v8;

/// Serializable exists to allow boxing values as "objects" to be serialized later,
/// this is particularly useful for async op-responses. This trait is a more efficient
/// replacement for erased-serde that makes less allocations, since it's specific to serde_v8
/// (and thus doesn't have to have generic outputs, etc...)
pub trait Serializable {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error>;
}

/// Allows all implementors of `serde::Serialize` to implement Serializable
impl<T: serde::Serialize> Serializable for T {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    crate::to_v8(scope, self)
  }
}
