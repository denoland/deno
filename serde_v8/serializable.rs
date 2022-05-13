// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use std::any::TypeId;
use std::mem::transmute_copy;

use crate::ByteString;
use crate::U16String;
use crate::ZeroCopyBuf;

/// Serializable exists to allow boxing values as "objects" to be serialized later,
/// this is particularly useful for async op-responses. This trait is a more efficient
/// replacement for erased-serde that makes less allocations, since it's specific to serde_v8
/// (and thus doesn't have to have generic outputs, etc...)
pub trait Serializable {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error>;
}

/// Allows all implementors of `serde::Serialize` to implement Serializable
impl<T: serde::Serialize> Serializable for T {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    crate::to_v8(scope, self)
  }
}

/// SerializablePkg exists to provide a fast path for op returns,
/// allowing them to avoid boxing primtives (ints/floats/bool/unit/...)
pub enum SerializablePkg {
  Primitive(Primitive),
  Serializable(Box<dyn Serializable>),
}

impl SerializablePkg {
  pub fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    match &*self {
      Self::Primitive(x) => crate::to_v8(scope, x),
      Self::Serializable(x) => x.to_v8(scope),
    }
  }
}

/// Primitive serves as a lightweight serializable wrapper around primitives
/// so that we can use them for async values
pub enum Primitive {
  Unit,
  Bool(bool),
  Int8(i8),
  Int16(i16),
  Int32(i32),
  Int64(i64),
  UInt8(u8),
  UInt16(u16),
  UInt32(u32),
  UInt64(u64),
  Float32(f32),
  Float64(f64),
  String(String),
  ZeroCopyBuf(ZeroCopyBuf),
  ByteString(ByteString),
  U16String(U16String),
}

impl serde::Serialize for Primitive {
  fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match self {
      Self::Unit => ().serialize(s),
      Self::Bool(x) => x.serialize(s),
      Self::Int8(x) => x.serialize(s),
      Self::Int16(x) => x.serialize(s),
      Self::Int32(x) => x.serialize(s),
      Self::Int64(x) => x.serialize(s),
      Self::UInt8(x) => x.serialize(s),
      Self::UInt16(x) => x.serialize(s),
      Self::UInt32(x) => x.serialize(s),
      Self::UInt64(x) => x.serialize(s),
      Self::Float32(x) => x.serialize(s),
      Self::Float64(x) => x.serialize(s),
      Self::String(x) => x.serialize(s),
      Self::ZeroCopyBuf(x) => x.serialize(s),
      Self::ByteString(x) => x.serialize(s),
      Self::U16String(x) => x.serialize(s),
    }
  }
}

impl<T: serde::Serialize + 'static> From<T> for SerializablePkg {
  fn from(x: T) -> Self {
    #[inline(always)]
    fn tc<T, U>(src: T) -> U {
      let x = unsafe { transmute_copy(&src) };
      std::mem::forget(src);
      x
    }

    let tid = TypeId::of::<T>();
    if tid == TypeId::of::<()>() {
      Self::Primitive(Primitive::Unit)
    } else if tid == TypeId::of::<bool>() {
      Self::Primitive(Primitive::Bool(tc(x)))
    } else if tid == TypeId::of::<i8>() {
      Self::Primitive(Primitive::Int8(tc(x)))
    } else if tid == TypeId::of::<i16>() {
      Self::Primitive(Primitive::Int16(tc(x)))
    } else if tid == TypeId::of::<i32>() {
      Self::Primitive(Primitive::Int32(tc(x)))
    } else if tid == TypeId::of::<i64>() {
      Self::Primitive(Primitive::Int64(tc(x)))
    } else if tid == TypeId::of::<u8>() {
      Self::Primitive(Primitive::UInt8(tc(x)))
    } else if tid == TypeId::of::<u16>() {
      Self::Primitive(Primitive::UInt16(tc(x)))
    } else if tid == TypeId::of::<u32>() {
      Self::Primitive(Primitive::UInt32(tc(x)))
    } else if tid == TypeId::of::<u64>() {
      Self::Primitive(Primitive::UInt64(tc(x)))
    } else if tid == TypeId::of::<f32>() {
      Self::Primitive(Primitive::Float32(tc(x)))
    } else if tid == TypeId::of::<f64>() {
      Self::Primitive(Primitive::Float64(tc(x)))
    } else if tid == TypeId::of::<String>() {
      Self::Primitive(Primitive::String(tc(x)))
    } else if tid == TypeId::of::<ZeroCopyBuf>() {
      Self::Primitive(Primitive::ZeroCopyBuf(tc(x)))
    } else if tid == TypeId::of::<ByteString>() {
      Self::Primitive(Primitive::ByteString(tc(x)))
    } else if tid == TypeId::of::<U16String>() {
      Self::Primitive(Primitive::U16String(tc(x)))
    } else {
      Self::Serializable(Box::new(x))
    }
  }
}
