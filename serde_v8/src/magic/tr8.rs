// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

pub(crate) const MAGIC_FIELD: &str = "$__v8_magic_field";

pub(crate) trait MagicType {
  fn magic_field() -> &'static str {
    MAGIC_FIELD
  }

  fn name() -> &'static str {
    Self::magic_name()
  }

  fn magic_name() -> &'static str {
    std::any::type_name::<Self>()
  }

  // TODO(@AaronO): blocked on https://github.com/rust-lang/rust/issues/63084
  // const MAGIC_NAME: &'static str = std::any::type_name::<Self>();
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

  let mut s = serializer.serialize_struct(T::magic_name(), 1)?;
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
      formatter.write_str(std::any::type_name::<T>())
    }

    fn visit_u64<E>(self, ptr: u64) -> Result<Self::Value, E>
    where
      E: serde::de::Error,
    {
      Ok(opaque_take(ptr))
    }
  }

  deserializer.deserialize_struct(
    T::magic_name(),
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
  let y = visitor.visit_u64::<E>(unsafe { std::mem::transmute(&x) });
  std::mem::forget(x);
  y
}

pub(crate) fn opaque_send<T: Sized>(x: &T) -> u64 {
  (x as *const T as *const u8) as u64
}

pub(crate) fn opaque_recv<T: ?Sized>(ptr: &T) -> u64 {
  unsafe { *(ptr as *const T as *const u8 as *const u64) }
}

pub(crate) fn opaque_deref<'a, T>(ptr: u64) -> &'a T {
  unsafe { std::mem::transmute(ptr) }
}

pub(crate) fn opaque_take<T>(ptr: u64) -> T {
  unsafe { std::mem::transmute_copy::<T, T>(std::mem::transmute(ptr)) }
}

#[macro_export]
macro_rules! impl_magic {
  ($t:ty) => {
    impl crate::magic::tr8::MagicType for $t {}

    impl serde::Serialize for $t {
      fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
      where
        S: serde::Serializer,
      {
        crate::magic::tr8::magic_serialize(serializer, self)
      }
    }

    impl<'de> serde::Deserialize<'de> for $t {
      fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
      where
        D: serde::Deserializer<'de>,
      {
        crate::magic::tr8::magic_deserialize(deserializer)
      }
    }
  };
}
