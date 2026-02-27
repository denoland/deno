// Copyright 2018-2025 the Deno authors. MIT license.

use std::ops::Deref;
use std::ops::DerefMut;

use crate::Error;

use super::transl8::FromV8;
use super::transl8::ToV8;
use super::transl8::impl_magic;

#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub struct U16String(Vec<u16>);
impl_magic!(U16String);

impl Deref for U16String {
  type Target = Vec<u16>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for U16String {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl AsRef<[u16]> for U16String {
  fn as_ref(&self) -> &[u16] {
    &self.0
  }
}

impl AsMut<[u16]> for U16String {
  fn as_mut(&mut self) -> &mut [u16] {
    &mut self.0
  }
}

impl<const N: usize> From<[u16; N]> for U16String {
  fn from(value: [u16; N]) -> Self {
    Self(value.into())
  }
}

impl<const N: usize> From<&[u16; N]> for U16String {
  fn from(value: &[u16; N]) -> Self {
    Self(value.into())
  }
}

impl From<&[u16]> for U16String {
  fn from(value: &[u16]) -> Self {
    Self(value.into())
  }
}

impl From<Vec<u16>> for U16String {
  fn from(value: Vec<u16>) -> Self {
    Self(value)
  }
}

impl ToV8 for U16String {
  fn to_v8<'scope, 'i>(
    &self,
    scope: &mut v8::PinScope<'scope, 'i>,
  ) -> Result<v8::Local<'scope, v8::Value>, crate::Error> {
    let maybe_v =
      v8::String::new_from_two_byte(scope, self, v8::NewStringType::Normal);

    // 'new_from_two_byte' can return 'None' if buffer length > kMaxLength.
    if let Some(v) = maybe_v {
      Ok(v.into())
    } else {
      Err(Error::Message(String::from(
        "Cannot allocate String from UTF-16: buffer exceeds maximum length.",
      )))
    }
  }
}

impl FromV8 for U16String {
  fn from_v8<'scope, 'i>(
    scope: &mut v8::PinScope,
    value: v8::Local<'scope, v8::Value>,
  ) -> Result<Self, crate::Error> {
    let v8str = v8::Local::<v8::String>::try_from(value)
      .map_err(|_| Error::ExpectedString(value.type_repr()))?;
    let len = v8str.length();
    let mut buffer = Vec::with_capacity(len);
    #[allow(clippy::uninit_vec)]
    // SAFETY: we set length == capacity (see previous line),
    // before immediately writing into that buffer and sanity check with an assert
    unsafe {
      buffer.set_len(len);
      v8str.write_v2(scope, 0, &mut buffer, v8::WriteFlags::empty());
    }
    Ok(buffer.into())
  }
}
