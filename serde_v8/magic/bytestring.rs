// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use super::transl8::FromV8;
use super::transl8::ToV8;
use crate::magic::transl8::impl_magic;
use crate::Error;
use smallvec::SmallVec;
use std::mem::size_of;

const USIZE2X: usize = size_of::<usize>() * 2;

#[derive(
  PartialEq,
  Eq,
  Clone,
  Debug,
  Default,
  derive_more::Deref,
  derive_more::DerefMut,
  derive_more::AsRef,
  derive_more::AsMut,
)]
#[as_mut(forward)]
#[as_ref(forward)]
pub struct ByteString(SmallVec<[u8; USIZE2X]>);
impl_magic!(ByteString);

// const-assert that Vec<u8> and SmallVec<[u8; size_of::<usize>() * 2]> have a same size.
// Note from https://docs.rs/smallvec/latest/smallvec/#union -
//   smallvec can still be larger than Vec if the inline buffer is
//   larger than two machine words.
const _: () =
  assert!(size_of::<Vec<u8>>() == size_of::<SmallVec<[u8; USIZE2X]>>());

impl ToV8 for ByteString {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let v =
      v8::String::new_from_one_byte(scope, self, v8::NewStringType::Normal)
        .unwrap();
    Ok(v.into())
  }
}

impl FromV8 for ByteString {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let v8str = v8::Local::<v8::String>::try_from(value)
      .map_err(|_| Error::ExpectedString)?;
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
      let written = v8str.write_one_byte(
        scope,
        &mut buffer,
        0,
        v8::WriteOptions::NO_NULL_TERMINATION,
      );
      assert!(written == len);
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
