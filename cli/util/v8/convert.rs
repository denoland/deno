// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::v8;
use deno_core::FromV8;
use deno_core::ToV8;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A wrapper type for `Option<T>` that (de)serializes `None` as `null`
#[repr(transparent)]
pub struct OptionNull<T>(pub Option<T>);

impl<T> From<Option<T>> for OptionNull<T> {
  fn from(option: Option<T>) -> Self {
    Self(option)
  }
}

impl<T> From<OptionNull<T>> for Option<T> {
  fn from(value: OptionNull<T>) -> Self {
    value.0
  }
}

impl<'a, T> ToV8<'a> for OptionNull<T>
where
  T: ToV8<'a>,
{
  type Error = T::Error;

  fn to_v8(
    self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    match self.0 {
      Some(value) => value.to_v8(scope),
      None => Ok(v8::null(scope).into()),
    }
  }
}

impl<'a, T> FromV8<'a> for OptionNull<T>
where
  T: FromV8<'a>,
{
  type Error = T::Error;

  fn from_v8(
    scope: &mut v8::HandleScope<'a>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    if value.is_null() {
      Ok(OptionNull(None))
    } else {
      T::from_v8(scope, value).map(|v| OptionNull(Some(v)))
    }
  }
}
