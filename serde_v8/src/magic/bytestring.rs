// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use std::ops::{Deref, DerefMut};

use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};

pub const NAME: &str = "$__v8_magic_bytestring";
pub const FIELD_PTR: &str = "$__v8_magic_bytestring_ptr";
pub const FIELD_LEN: &str = "$__v8_magic_bytestring_len";

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ByteString(pub Vec<u8>);

impl ByteString {
  pub fn new() -> ByteString {
    ByteString(Vec::new())
  }

  pub fn with_capacity(capacity: usize) -> ByteString {
    ByteString(Vec::with_capacity(capacity))
  }

  pub fn capacity(&self) -> usize {
    self.0.capacity()
  }

  pub fn reserve(&mut self, additional: usize) {
    self.0.reserve(additional)
  }

  pub fn reserve_exact(&mut self, additional: usize) {
    self.0.reserve_exact(additional)
  }

  pub fn shrink_to_fit(&mut self) {
    self.0.shrink_to_fit()
  }

  pub fn truncate(&mut self, len: usize) {
    self.0.truncate(len)
  }

  pub fn push(&mut self, value: u8) {
    self.0.push(value)
  }

  pub fn pop(&mut self) -> Option<u8> {
    self.0.pop()
  }
}

impl Default for ByteString {
  fn default() -> Self {
    ByteString::new()
  }
}

impl Deref for ByteString {
  type Target = [u8];

  fn deref(&self) -> &[u8] {
    self.0.deref()
  }
}

impl DerefMut for ByteString {
  fn deref_mut(&mut self) -> &mut [u8] {
    self.0.deref_mut()
  }
}

impl AsRef<[u8]> for ByteString {
  fn as_ref(&self) -> &[u8] {
    self.0.as_ref()
  }
}

impl AsMut<[u8]> for ByteString {
  fn as_mut(&mut self) -> &mut [u8] {
    self.0.as_mut()
  }
}

impl Serialize for ByteString {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    use serde::ser::SerializeStruct;

    let mut s = serializer.serialize_struct(NAME, 1)?;
    s.serialize_field(FIELD_PTR, &(self.0.as_ptr() as usize))?;
    s.serialize_field(FIELD_LEN, &self.0.len())?;
    s.end()
  }
}

impl<'de> Deserialize<'de> for ByteString {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    struct ValueVisitor {}

    impl<'de> Visitor<'de> for ValueVisitor {
      type Value = ByteString;

      fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
      ) -> std::fmt::Result {
        formatter.write_str("a serde_v8::ByteString")
      }

      fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        Ok(ByteString(v))
      }
    }

    deserializer.deserialize_struct(NAME, &[], ValueVisitor {})
  }
}
