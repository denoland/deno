// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use super::transl8::{FromV8, ToV8};
use crate::magic::transl8::{impl_magic, impl_wrapper};
use crate::Error;

impl_wrapper! { pub struct ByteString(Vec<u8>); }
impl_magic!(ByteString);

impl ToV8 for ByteString {
  fn to_v8<'a>(
    &self,
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
    let mut buffer = Vec::with_capacity(len);
    // SAFETY: we set length == capacity (see previous line),
    // before immediately writing into that buffer and sanity check with an assert
    #[allow(clippy::uninit_vec)]
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
    Ok(buffer.into())
  }
}

#[allow(clippy::from_over_into)]
impl Into<Vec<u8>> for ByteString {
  fn into(self) -> Vec<u8> {
    self.0
  }
}
