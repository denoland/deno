// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! Transerialization extends the set of serde-compatible types (for given de/serializers).
//! By "hackishly" transmuting references across serde boundaries as u64s.
//! Type-safety is enforced using special struct names for each "magic type".
//! Memory-safety relies on transerialized values being "pinned" during de/serialization.

pub(crate) const MAGIC_FIELD: &str = "$__v8_magic_field";

pub(crate) trait MagicType {
  const NAME: &'static str;
  const MAGIC_NAME: &'static str;
}

pub(crate) trait ToV8 {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error>;
}

pub(crate) trait FromV8: Sized {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error>;
}

pub(crate) fn magic_serialize<T, S>(
  serializer: S,
  x: &T,
) -> Result<S::Ok, S::Error>
where
  S: serde::Serializer,
  T: MagicType,
{
  use serde::ser::SerializeStruct;

  let mut s = serializer.serialize_struct(T::MAGIC_NAME, 1)?;
  let ptr = opaque_send(x);
  s.serialize_field(MAGIC_FIELD, &ptr)?;
  s.end()
}

pub(crate) fn magic_deserialize<'de, T, D>(
  deserializer: D,
) -> Result<T, D::Error>
where
  D: serde::Deserializer<'de>,
  T: MagicType,
{
  struct ValueVisitor<T> {
    p1: std::marker::PhantomData<T>,
  }

  impl<'de, T: MagicType> serde::de::Visitor<'de> for ValueVisitor<T> {
    type Value = T;

    fn expecting(
      &self,
      formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
      formatter.write_str("a ")?;
      formatter.write_str(T::NAME)
    }

    fn visit_u64<E>(self, ptr: u64) -> Result<Self::Value, E>
    where
      E: serde::de::Error,
    {
      // SAFETY: opaque ptr originates from visit_magic, which forgets ownership so we can take it
      Ok(unsafe { opaque_take(ptr) })
    }
  }

  deserializer.deserialize_struct(
    T::MAGIC_NAME,
    &[MAGIC_FIELD],
    ValueVisitor::<T> {
      p1: std::marker::PhantomData,
    },
  )
}

pub(crate) fn visit_magic<'de, T, V, E>(visitor: V, x: T) -> Result<V::Value, E>
where
  V: serde::de::Visitor<'de>,
  E: serde::de::Error,
{
  let y = visitor.visit_u64::<E>(opaque_send(&x));
  std::mem::forget(x);
  y
}

/// Constructs an "opaque" ptr from a reference to transerialize
pub(crate) fn opaque_send<T: Sized>(x: &T) -> u64 {
  (x as *const T) as u64
}

/// Copies an "opaque" ptr from a reference to an opaque ptr (transerialized)
/// NOTE: ptr-to-ptr, extra indirection
pub(crate) unsafe fn opaque_recv<T: ?Sized>(ptr: &T) -> u64 {
  *(ptr as *const T as *const u64)
}

/// Transmutes an "opaque" ptr back into a reference
pub(crate) unsafe fn opaque_deref<'a, T>(ptr: u64) -> &'a T {
  std::mem::transmute(ptr)
}

/// Transmutes & copies the value from the "opaque" ptr
/// NOTE: takes ownership & requires other end to forget its ownership
pub(crate) unsafe fn opaque_take<T>(ptr: u64) -> T {
  std::mem::transmute_copy::<T, T>(std::mem::transmute(ptr))
}

macro_rules! impl_magic {
  ($t:ty) => {
    impl crate::magic::transl8::MagicType for $t {
      const NAME: &'static str = stringify!($t);
      const MAGIC_NAME: &'static str = concat!("$__v8_magic_", stringify!($t));
    }

    impl serde::Serialize for $t {
      fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
      where
        S: serde::Serializer,
      {
        crate::magic::transl8::magic_serialize(serializer, self)
      }
    }

    impl<'de> serde::Deserialize<'de> for $t {
      fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
      where
        D: serde::Deserializer<'de>,
      {
        crate::magic::transl8::magic_deserialize(deserializer)
      }
    }
  };
}
pub(crate) use impl_magic;

macro_rules! impl_wrapper {
  ($i:item) => {
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
      derive_more::From,
    )]
    #[as_mut(forward)]
    #[as_ref(forward)]
    #[from(forward)]
    $i
  };
}
pub(crate) use impl_wrapper;
