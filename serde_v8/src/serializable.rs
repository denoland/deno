use rusty_v8 as v8;
use std::any::TypeId;
use std::mem::transmute_copy;

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
#[derive(Clone, Copy)]
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
}

impl serde::Serialize for Primitive {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    match *self {
      Self::Unit => serializer.serialize_unit(),
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
      Self::Primitive(Primitive::Unit)
    } else if tid == TypeId::of::<bool>() {
      Self::Primitive(Primitive::Bool(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<i8>() {
      Self::Primitive(Primitive::Int8(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<i16>() {
      Self::Primitive(Primitive::Int16(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<i32>() {
      Self::Primitive(Primitive::Int32(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<i64>() {
      Self::Primitive(Primitive::Int64(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<u8>() {
      Self::Primitive(Primitive::UInt8(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<u16>() {
      Self::Primitive(Primitive::UInt16(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<u32>() {
      Self::Primitive(Primitive::UInt32(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<u64>() {
      Self::Primitive(Primitive::UInt64(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<f32>() {
      Self::Primitive(Primitive::Float32(unsafe { transmute_copy(&x) }))
    } else if tid == TypeId::of::<f64>() {
      Self::Primitive(Primitive::Float64(unsafe { transmute_copy(&x) }))
    } else {
      Self::Serializable(Box::new(x))
    }
  }
}
