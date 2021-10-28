// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::fmt;
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Mutex;

use super::zero_copy_buf::ZeroCopyBuf;

// An asymmetric wrapper around ZeroCopyBuf,
// allowing us to use a single type for familiarity
pub enum MagicBuffer {
  FromV8(ZeroCopyBuf),
  ToV8(Mutex<Option<Box<[u8]>>>),
}

impl MagicBuffer {
  pub fn new<'s>(
    scope: &mut v8::HandleScope<'s>,
    view: v8::Local<v8::ArrayBufferView>,
  ) -> Self {
    Self::try_new(scope, view).unwrap()
  }

  pub fn try_new<'s>(
    scope: &mut v8::HandleScope<'s>,
    view: v8::Local<v8::ArrayBufferView>,
  ) -> Result<Self, v8::DataError> {
    Ok(Self::FromV8(ZeroCopyBuf::try_new(scope, view)?))
  }

  pub fn empty() -> Self {
    MagicBuffer::ToV8(Mutex::new(Some(vec![0_u8; 0].into_boxed_slice())))
  }
}

impl Clone for MagicBuffer {
  fn clone(&self) -> Self {
    match self {
      Self::FromV8(zbuf) => Self::FromV8(zbuf.clone()),
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
      Self::ToV8(_) => panic!("Don't Deref a MagicBuffer sent to v8"),
    }
  }
}

impl DerefMut for MagicBuffer {
  fn deref_mut(&mut self) -> &mut [u8] {
    match self {
      Self::FromV8(buf) => &mut *buf,
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

pub const BUF_NAME: &str = "$__v8_magic_Buffer";
pub const BUF_FIELD_1: &str = "$__v8_magic_buffer_1";
pub const BUF_FIELD_2: &str = "$__v8_magic_buffer_2";

impl serde::Serialize for MagicBuffer {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    use serde::ser::SerializeStruct;

    let mut s = serializer.serialize_struct(BUF_NAME, 1)?;
    let boxed: Box<[u8]> = match self {
      Self::FromV8(buf) => {
        let value: &[u8] = buf;
        value.into()
      }
      Self::ToV8(x) => x.lock().unwrap().take().expect("MagicBuffer was empty"),
    };
    let hack: [usize; 2] = unsafe { std::mem::transmute(boxed) };
    let f1: u64 = hack[0] as u64;
    let f2: u64 = hack[1] as u64;
    s.serialize_field(BUF_FIELD_1, &f1)?;
    s.serialize_field(BUF_FIELD_2, &f2)?;
    s.end()
  }
}

impl<'de, 's> serde::Deserialize<'de> for MagicBuffer {
  fn deserialize<D>(deserializer: D) -> Result<MagicBuffer, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct ValueVisitor {}

    impl<'de> serde::de::Visitor<'de> for ValueVisitor {
      type Value = MagicBuffer;

      fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a serde_v8::MagicBuffer")
      }

      fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        let p1: &[usize] = unsafe { &*(v as *const [u8] as *const [usize]) };
        let p2: [usize; 4] = [p1[0], p1[1], p1[2], p1[3]];
        let zero_copy: ZeroCopyBuf = unsafe { std::mem::transmute(p2) };
        Ok(MagicBuffer::FromV8(zero_copy))
      }
    }

    static FIELDS: [&str; 0] = [];
    let visitor = ValueVisitor {};
    deserializer.deserialize_struct(BUF_NAME, &FIELDS, visitor)
  }
}
