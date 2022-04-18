// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Mutex;

use super::transl8::FromV8;
use super::transl8::ToV8;
use super::zero_copy_buf::ZeroCopyBuf;
use crate::magic::transl8::impl_magic;

// An asymmetric wrapper around ZeroCopyBuf,
// allowing us to use a single type for familiarity
pub enum MagicBuffer {
  FromV8(ZeroCopyBuf),
  ToV8(Mutex<Option<Box<[u8]>>>),
  // Variant of the MagicBuffer than is never exposed to the JS.
  // Generally used to pass Vec<u8> backed buffers to resource methods.
  Temp(Vec<u8>),
}

impl_magic!(MagicBuffer);

impl MagicBuffer {
  pub fn empty() -> Self {
    MagicBuffer::ToV8(Mutex::new(Some(vec![0_u8; 0].into_boxed_slice())))
  }

  pub fn new_temp(vec: Vec<u8>) -> Self {
    MagicBuffer::Temp(vec)
  }
}

impl Clone for MagicBuffer {
  fn clone(&self) -> Self {
    match self {
      Self::FromV8(zbuf) => Self::FromV8(zbuf.clone()),
      Self::Temp(vec) => Self::Temp(vec.clone()),
      Self::ToV8(_) => panic!("Don't Clone a MagicBuffer sent to v8"),
    }
  }
}

impl AsRef<[u8]> for MagicBuffer {
  fn as_ref(&self) -> &[u8] {
    &*self
  }
}

impl AsMut<[u8]> for MagicBuffer {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut *self
  }
}

impl Deref for MagicBuffer {
  type Target = [u8];
  fn deref(&self) -> &[u8] {
    match self {
      Self::FromV8(buf) => &*buf,
      Self::Temp(vec) => &*vec,
      Self::ToV8(_) => panic!("Don't Deref a MagicBuffer sent to v8"),
    }
  }
}

impl DerefMut for MagicBuffer {
  fn deref_mut(&mut self) -> &mut [u8] {
    match self {
      Self::FromV8(buf) => &mut *buf,
      Self::Temp(vec) => &mut *vec,
      Self::ToV8(_) => panic!("Don't Deref a MagicBuffer sent to v8"),
    }
  }
}

impl From<Box<[u8]>> for MagicBuffer {
  fn from(buf: Box<[u8]>) -> Self {
    MagicBuffer::ToV8(Mutex::new(Some(buf)))
  }
}

impl From<Vec<u8>> for MagicBuffer {
  fn from(vec: Vec<u8>) -> Self {
    vec.into_boxed_slice().into()
  }
}

impl ToV8 for MagicBuffer {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let buf: Box<[u8]> = match self {
      Self::FromV8(buf) => {
        let value: &[u8] = buf;
        value.into()
      }
      Self::Temp(_) => unreachable!(),
      Self::ToV8(x) => x.lock().unwrap().take().expect("MagicBuffer was empty"),
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

impl FromV8 for MagicBuffer {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    Ok(Self::FromV8(ZeroCopyBuf::from_v8(scope, value)?))
  }
}
