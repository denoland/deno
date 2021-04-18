// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use rusty_v8 as v8;
use std::any::TypeId;

/// SerializablePkg exists to provide a fast path for op returns,
/// allowing them to avoid boxing primtives (ints/floats/bool/unit/...)
pub enum SerializablePkg {
  MinValue(MinValue),
  Serializable(Box<dyn serde_v8::Serializable>),
}

impl SerializablePkg {
  pub fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, serde_v8::Error> {
    match &*self {
      Self::MinValue(x) => serde_v8::to_v8(scope, x),
      Self::Serializable(x) => x.to_v8(scope),
    }
  }
}

/// MinValue serves as a lightweight serializable wrapper around primitives
/// so that we can use them for async values
#[derive(Clone, Copy)]
pub enum MinValue {
  Unit(()),
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
}

impl serde::Serialize for MinValue {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match *self {
      Self::Unit(_) => serializer.serialize_unit(),
      Self::Bool(x) => serializer.serialize_bool(x),
      Self::Int8(x) => serializer.serialize_i8(x),
      Self::Int16(x) => serializer.serialize_i16(x),
      Self::Int32(x) => serializer.serialize_i32(x),
      Self::Int64(x) => serializer.serialize_i64(x),
      Self::UInt8(x) => serializer.serialize_u8(x),
      Self::UInt16(x) => serializer.serialize_u16(x),
      Self::UInt32(x) => serializer.serialize_u32(x),
      Self::UInt64(x) => serializer.serialize_u64(x),
      Self::Float32(x) => serializer.serialize_f32(x),
      Self::Float64(x) => serializer.serialize_f64(x),
    }
  }
}

impl<T: serde::Serialize + 'static> From<T> for SerializablePkg {
  fn from(x: T) -> Self {
    let tid = TypeId::of::<T>();

    if tid == TypeId::of::<()>() {
      Self::MinValue(MinValue::Unit(()))
    } else if tid == TypeId::of::<bool>() {
      Self::MinValue(MinValue::Bool(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<i8>() {
      Self::MinValue(MinValue::Int8(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<i16>() {
      Self::MinValue(MinValue::Int16(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<i32>() {
      Self::MinValue(MinValue::Int32(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<i64>() {
      Self::MinValue(MinValue::Int64(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<u8>() {
      Self::MinValue(MinValue::UInt8(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<u16>() {
      Self::MinValue(MinValue::UInt16(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<u32>() {
      Self::MinValue(MinValue::UInt32(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<u64>() {
      Self::MinValue(MinValue::UInt64(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<f32>() {
      Self::MinValue(MinValue::Float32(unsafe { std::mem::transmute_copy(&x) }))
    } else if tid == TypeId::of::<f64>() {
      Self::MinValue(MinValue::Float64(unsafe { std::mem::transmute_copy(&x) }))
    } else {
      Self::Serializable(Box::new(x))
    }
  }
}
