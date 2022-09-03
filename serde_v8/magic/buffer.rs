// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::fmt::Debug;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Mutex;

use super::transl8::FromV8;
use super::transl8::ToV8;
use super::v8slice::V8Slice;
use crate::magic::transl8::impl_magic;

// An asymmetric wrapper around V8Slice,
// allowing us to use a single type for familiarity
pub enum ZeroCopyBuf {
  FromV8(V8Slice),
  ToV8(Mutex<Option<Box<[u8]>>>),
  // Variant of the ZeroCopyBuf than is never exposed to the JS.
  // Generally used to pass Vec<u8> backed buffers to resource methods.
  Temp(Vec<u8>),
}

impl_magic!(ZeroCopyBuf);

impl Debug for ZeroCopyBuf {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_list().entries(self.as_ref().iter()).finish()
  }
}

impl ZeroCopyBuf {
  pub fn empty() -> Self {
    ZeroCopyBuf::ToV8(Mutex::new(Some(vec![0_u8; 0].into_boxed_slice())))
  }

  pub fn new_temp(vec: Vec<u8>) -> Self {
    ZeroCopyBuf::Temp(vec)
  }

  // TODO(@littledivy): Temporary, this needs a refactor.
  pub fn to_temp(self) -> Vec<u8> {
    match self {
      ZeroCopyBuf::Temp(vec) => vec,
      _ => unreachable!(),
    }
  }
}

impl Clone for ZeroCopyBuf {
  fn clone(&self) -> Self {
    match self {
      Self::FromV8(zbuf) => Self::FromV8(zbuf.clone()),
      Self::Temp(vec) => Self::Temp(vec.clone()),
      Self::ToV8(_) => panic!("Don't Clone a ZeroCopyBuf sent to v8"),
    }
  }
}

impl AsRef<[u8]> for ZeroCopyBuf {
  fn as_ref(&self) -> &[u8] {
    self
  }
}

impl AsMut<[u8]> for ZeroCopyBuf {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut *self
  }
}

impl Deref for ZeroCopyBuf {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    match self {
      Self::FromV8(buf) => buf,
      Self::Temp(vec) => vec,
      Self::ToV8(_) => panic!("Don't Deref a ZeroCopyBuf sent to v8"),
    }
  }
}

impl DerefMut for ZeroCopyBuf {
  fn deref_mut(&mut self) -> &mut [u8] {
    match self {
      Self::FromV8(buf) => &mut *buf,
      Self::Temp(vec) => &mut *vec,
      Self::ToV8(_) => panic!("Don't Deref a ZeroCopyBuf sent to v8"),
    }
  }
}

impl From<Box<[u8]>> for ZeroCopyBuf {
  fn from(buf: Box<[u8]>) -> Self {
    ZeroCopyBuf::ToV8(Mutex::new(Some(buf)))
  }
}

impl From<Vec<u8>> for ZeroCopyBuf {
  fn from(vec: Vec<u8>) -> Self {
    vec.into_boxed_slice().into()
  }
}

impl ToV8 for ZeroCopyBuf {
  fn to_v8<'a>(
    &mut self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let buf: Box<[u8]> = match self {
      Self::FromV8(buf) => {
        let value: &[u8] = buf;
        value.into()
      }
      Self::Temp(_) => unreachable!(),
      Self::ToV8(x) => {
        x.get_mut().unwrap().take().expect("ZeroCopyBuf was empty")
      }
    };

    if buf.is_empty() {
      let ab = v8::ArrayBuffer::new(scope, 0);
      return Ok(
        v8::Uint8Array::new(scope, ab, 0, 0)
          .expect("Failed to create Uint8Array")
          .into(),
      );
    }
    let buf_len = buf.len();
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

impl FromV8 for ZeroCopyBuf {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    Ok(Self::FromV8(V8Slice::from_v8(scope, value)?))
  }
}

impl From<ZeroCopyBuf> for bytes::Bytes {
  fn from(zbuf: ZeroCopyBuf) -> bytes::Bytes {
    match zbuf {
      ZeroCopyBuf::FromV8(v) => v.into(),
      // WARNING(AaronO): potential footgun, but will disappear in future ZeroCopyBuf refactor
      ZeroCopyBuf::ToV8(v) => v
        .lock()
        .unwrap()
        .take()
        .expect("ZeroCopyBuf was empty")
        .into(),
      ZeroCopyBuf::Temp(v) => v.into(),
    }
  }
}
