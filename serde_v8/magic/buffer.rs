// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::fmt::Debug;
use std::ops::Deref;
use std::ops::DerefMut;

use super::transl8::FromV8;
use super::transl8::ToV8;
use super::v8slice::V8Slice;
use crate::magic::transl8::impl_magic;

pub struct JsBuffer(V8Slice);

impl_magic!(JsBuffer);

impl Debug for JsBuffer {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_list().entries(self.0.as_ref().iter()).finish()
  }
}

impl Clone for JsBuffer {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl AsRef<[u8]> for JsBuffer {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl AsMut<[u8]> for JsBuffer {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut self.0
  }
}

impl Deref for JsBuffer {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    &self.0
  }
}

impl DerefMut for JsBuffer {
  fn deref_mut(&mut self) -> &mut [u8] {
    &mut self.0
  }
}

impl FromV8 for JsBuffer {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    Ok(Self(V8Slice::from_v8(scope, value)?))
  }
}

impl From<JsBuffer> for bytes::Bytes {
  fn from(zbuf: JsBuffer) -> bytes::Bytes {
    zbuf.0.into()
  }
}

// NOTE(bartlomieju): we use Option here, because `to_v8()` uses `&mut self`
// instead of `self` which is dictated by the `serde` API.
#[derive(Debug)]
pub struct ToJsBuffer(Option<Box<[u8]>>);

impl_magic!(ToJsBuffer);

impl ToJsBuffer {
  pub fn empty() -> Self {
    ToJsBuffer(Some(vec![0_u8; 0].into_boxed_slice()))
  }
}

impl From<Box<[u8]>> for ToJsBuffer {
  fn from(buf: Box<[u8]>) -> Self {
    ToJsBuffer(Some(buf))
  }
}

impl From<Vec<u8>> for ToJsBuffer {
  fn from(vec: Vec<u8>) -> Self {
    vec.into_boxed_slice().into()
  }
}

impl ToV8 for ToJsBuffer {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let buf: Box<[u8]> = self.0.take().expect("RustToV8Buf was empty");

    if buf.is_empty() {
      let ab = v8::ArrayBuffer::new(scope, 0);
      return Ok(
        v8::Uint8Array::new(scope, ab, 0, 0)
          .expect("Failed to create Uint8Array")
          .into(),
      );
    }
    let buf_len: usize = buf.len();
    let backing_store =
      v8::ArrayBuffer::new_backing_store_from_boxed_slice(buf);
    let backing_store_shared = backing_store.make_shared();
    let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared);
    Ok(
      v8::Uint8Array::new(scope, ab, 0, buf_len)
        .expect("Failed to create Uint8Array")
        .into(),
    )
  }
}
