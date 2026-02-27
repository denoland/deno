// Copyright 2018-2025 the Deno authors. MIT license.

use std::mem::size_of;
use std::ops::Deref;
use std::ops::DerefMut;

use smallvec::SmallVec;

use super::transl8::FromV8;
use super::transl8::ToV8;
use crate::Error;
use crate::magic::transl8::impl_magic;

const USIZE2X: usize = size_of::<usize>() * 2;

#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub struct ByteString(SmallVec<[u8; USIZE2X]>);
impl_magic!(ByteString);

impl Deref for ByteString {
  type Target = SmallVec<[u8; USIZE2X]>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for ByteString {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl AsRef<[u8]> for ByteString {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl AsMut<[u8]> for ByteString {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut self.0
  }
}

// const-assert that Vec<u8> and SmallVec<[u8; size_of::<usize>() * 2]> have a same size.
// Note from https://docs.rs/smallvec/latest/smallvec/#union -
//   smallvec can still be larger than Vec if the inline buffer is
//   larger than two machine words.
const _: () =
  assert!(size_of::<Vec<u8>>() == size_of::<SmallVec<[u8; USIZE2X]>>());

impl ToV8 for ByteString {
  fn to_v8<'scope, 'i>(
    &self,
    scope: &mut v8::PinScope<'scope, 'i>,
  ) -> Result<v8::Local<'scope, v8::Value>, crate::Error> {
    let v =
      v8::String::new_from_one_byte(scope, self, v8::NewStringType::Normal)
        .unwrap();
    Ok(v.into())
  }
}

impl FromV8 for ByteString {
  fn from_v8<'scope, 'i>(
    scope: &mut v8::PinScope,
    value: v8::Local<'scope, v8::Value>,
  ) -> Result<Self, crate::Error> {
    let v8str = v8::Local::<v8::String>::try_from(value)
      .map_err(|_| Error::ExpectedString(value.type_repr()))?;
    if !v8str.contains_only_onebyte() {
      return Err(Error::ExpectedLatin1);
    }
    let len = v8str.length();
    let mut buffer = SmallVec::with_capacity(len);
    #[allow(clippy::uninit_vec)]
    // SAFETY: we set length == capacity (see previous line),
    // before immediately writing into that buffer and sanity check with an assert
    unsafe {
      buffer.set_len(len);
      v8str.write_one_byte_v2(scope, 0, &mut buffer, v8::WriteFlags::empty());
    }
    Ok(Self(buffer))
  }
}

// smallvec does not impl From/Into traits
// like Vec<u8> does. So here we are.

impl From<Vec<u8>> for ByteString {
  fn from(vec: Vec<u8>) -> Self {
    ByteString(SmallVec::from_vec(vec))
  }
}

#[allow(clippy::from_over_into)]
impl Into<Vec<u8>> for ByteString {
  fn into(self) -> Vec<u8> {
    self.0.into_vec()
  }
}

impl From<&[u8]> for ByteString {
  fn from(s: &[u8]) -> Self {
    ByteString(SmallVec::from_slice(s))
  }
}

impl From<&str> for ByteString {
  fn from(s: &str) -> Self {
    let v: Vec<u8> = s.into();
    ByteString::from(v)
  }
}

impl From<String> for ByteString {
  fn from(s: String) -> Self {
    ByteString::from(s.into_bytes())
  }
}
