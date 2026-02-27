// Copyright 2018-2025 the Deno authors. MIT license.

//! This module provides traits and macros that facilitate the conversion of Rust objects into v8 objects.
//!
//! The traits combine various behaviors, including converting values into v8 objects, efficiently placing them
//! into a `v8::ReturnValue`, and handling potential conversion failures.
//!
//! These traits are a result of the intersection of two factors: whether the conversion can be directly stored
//! in a return value and whether the conversion process can fail.
//!
//! For instance, the `RustToV8` and `RustToV8RetVal` traits respectively indicate the ability to convert a Rust value
//! into a v8 object, and to store it in a `v8::ReturnValue`. Similarly, the `RustToV8Fallible` and `RustToV8RetValFallible`
//! traits manage the same conversions that might encounter errors.
//!
//! To illustrate, consider the conversion of a `u32` value. It can be cheaply placed in a `v8::ReturnValue`
//! without any conversion risk. Conversely, storing a string directly in a `v8::ReturnValue` (without using
//! the `set(v8::Local<v8::Value>)` function) is not possible. Moreover, converting a Rust string to a `v8::String`
//! might fail, particularly if its size surpasses v8's limitations.
//!
//! The `#[op2]` proc macro maps the distinct calling patterns to the appropriate trait interfaces.
//! This mapping accounts for conversion nature and potential error occurrences. When a conversion is inherently
//! expected to succeed, `#[op2]` negates the necessity for explicit error checks, potentially boosting execution
//! efficiency. Furthermore, `#[op2]` strives to leverage `v8::ReturnValue` setters when possible, eliminating
//! the need for explicit object handle allocation. This optimization can favorably impact performance and
//! memory management.

use bytes::BytesMut;
use libc::c_void;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::rc::Rc;

/// Convert a value to a `v8::Local`, potentially allocating.
pub trait RustToV8<'a> {
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> v8::Local<'a, v8::Value>;
}

/// Convert a value to a `v8::Local`, not allocating. This is generally not used for
/// anything other than `v8::Local`s themselves, so there is no additional macro support.
pub trait RustToV8NoScope<'a> {
  fn to_v8(self) -> v8::Local<'a, v8::Value>;
}

/// Places a value in a `v8::ReturnValue`, non-allocating.
pub trait RustToV8RetVal<'a>: RustToV8<'a> {
  fn to_v8_rv(self, rv: &mut v8::ReturnValue<'a>);
}

/// Convert a value to a `v8::Local`, potentially allocating or failing.
pub trait RustToV8Fallible<'a> {
  fn to_v8_fallible<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> serde_v8::Result<v8::Local<'a, v8::Value>>;
}

/// Places a value in a `v8::ReturnValue`, potentially allocating or failing.
pub trait RustToV8RetValFallible<'a>: RustToV8Fallible<'a> {
  fn to_v8_rv_fallible<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
    rv: &mut v8::ReturnValue<'a>,
  ) -> serde_v8::Result<()>;
}

/// Implement [`RustToV8`] for an `Option` of [`RustToV8`].
impl<'a, T> RustToV8<'a> for Option<T>
where
  T: RustToV8<'a>,
{
  #[inline(always)]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> v8::Local<'a, v8::Value> {
    match self {
      Some(value) => value.to_v8(scope),
      _ => v8::null(scope).into(),
    }
  }
}

/// Implement [`RustToV8RetVal`] for an `Option` of [`RustToV8RetVal`].
impl<'a, T> RustToV8RetVal<'a> for Option<T>
where
  T: RustToV8RetVal<'a>,
{
  #[inline(always)]
  fn to_v8_rv(self, rv: &mut v8::ReturnValue<'a>) {
    match self {
      Some(value) => value.to_v8_rv(rv),
      _ => rv.set_null(),
    }
  }
}

/// Implement [`RustToV8Fallible`] for an `Option` of [`RustToV8Fallible`].
impl<'a, T> RustToV8Fallible<'a> for Option<T>
where
  T: RustToV8Fallible<'a>,
{
  #[inline(always)]
  fn to_v8_fallible<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> serde_v8::Result<v8::Local<'a, v8::Value>> {
    match self {
      Some(value) => value.to_v8_fallible(scope),
      _ => Ok(v8::null(scope).into()),
    }
  }
}

/// Implement [`RustToV8RetValFallible`] for an `Option` of [`RustToV8RetValFallible`].
impl<'a, T> RustToV8RetValFallible<'a> for Option<T>
where
  T: RustToV8RetValFallible<'a>,
{
  #[inline(always)]
  fn to_v8_rv_fallible<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
    rv: &mut v8::ReturnValue<'a>,
  ) -> serde_v8::Result<()> {
    match self {
      Some(value) => value.to_v8_rv_fallible(scope, rv),
      _ => {
        rv.set_null();
        Ok(())
      }
    }
  }
}

/// Transparent marker struct that can be used to alter the conversion in various ways. The
/// main uses of this are to mark something as being `serde`-serializable. This may also be
/// useful to alter the serialization of various types. For example, a `NumberMarker` and
/// `BigIntMarker` could control the serialization of `i64`/`u64`/`f64` as `v8::Number` or
/// `v8::BigInt` objects.
#[repr(transparent)]
pub struct RustToV8Marker<M, T>(T, PhantomData<M>);
impl<M: Marker, T> From<T> for RustToV8Marker<M, T> {
  #[inline(always)]
  fn from(value: T) -> Self {
    RustToV8Marker(value, PhantomData)
  }
}

/// This struct should be serialized using `serde`.
pub struct SerdeMarker;
impl Marker for SerdeMarker {}

/// This primitive should be serialized as an SMI.
pub struct SmiMarker;
impl Marker for SmiMarker {}

/// This primitive should be serialized as a number.
pub struct NumberMarker;
impl Marker for NumberMarker {}

/// This buffer should be serialized as an ArrayBuffer.
pub struct ArrayBufferMarker;
impl Marker for ArrayBufferMarker {}

/// This struct should be wrapped with cppgc.
pub struct CppGcMarker;
impl Marker for CppGcMarker {}

pub struct CppGcProtoMarker;
impl Marker for CppGcProtoMarker {}

pub struct ToV8Marker;
impl Marker for ToV8Marker {}

pub struct Undefined;
impl Marker for Undefined {}

trait Marker {}

/// Helper macro for [`RustToV8`] to reduce boilerplate.
///
/// Implements a Rust-to-v8 conversion that cannot fail. Multiple types may be specified
/// to apply the same implementation.
///
/// ```no_compile
/// to_v8!(bool: |value, scope| v8::Boolean::new(scope, value as _));
/// ```
macro_rules! to_v8 {
  (( $( $ty:ty ),+ ) : |$value:ident, $scope:ident| $block:expr_2021) => {
    $(
      impl <'a> RustToV8<'a> for $ty {
        #[inline(always)]
        fn to_v8<'i>(self, scope: &mut v8::PinScope<'a, 'i>) -> v8::Local<'a, v8::Value> {
          let $value = self;
          let $scope = scope;
          v8::Local::<v8::Value>::from($block)
        }
      }
    )+
  };
  ($ty:ty : |$value:ident, $scope:ident| $block:expr_2021) => {
    to_v8!(( $ty ) : |$value, $scope| $block);
  };
}

/// Helper macro for [`RustToV8RetVal`] to reduce boilerplate.
///
/// Implements a Rust-to-v8 conversion that cannot allocate or fail. Multiple types may be specified
/// to apply the same implementation. Places the return value in a `v8::ReturnValue`.
macro_rules! to_v8_retval {
  (( $( $ty:ty ),+ ) : |$value:ident, $rv:ident| $block:expr_2021) => {
    $(
      impl <'a> RustToV8RetVal<'a> for $ty {
        #[inline(always)]
        fn to_v8_rv(self, rv: &mut v8::ReturnValue<'a>) {
          let $value = self;
          let $rv = rv;
          $block
        }
      }
    )+
  };
  ($ty:ty : |$rv:ident, $scope:ident| $block:expr_2021) => {
    to_v8_retval!(( $ty ) : |$rv, $scope| $block);
  };
}

/// Helper macro for [`RustToV8Fallible`] to reduce boilerplate.
///
/// Implements a Rust-to-v8 conversion that can fail. Multiple types may be specified
/// to apply the same implementation.
///
/// ```no_compile
/// to_v8_fallible!(serde_v8::V8Slice: |value, scope| {
///   // Implementation
///   Ok(result)
/// });
/// ```
macro_rules! to_v8_fallible {
  (( $( $ty:ty ),+ ) : |$value:ident, $scope:ident| $block:expr_2021) => {
    $(
      #[allow(clippy::needless_borrow)]
      impl <'a> RustToV8Fallible<'a> for $ty {
        #[inline(always)]
        fn to_v8_fallible<'i>(self, scope: &mut v8::PinScope<'a, 'i>) -> serde_v8::Result<v8::Local<'a, v8::Value>> {
          let $value = self;
          let $scope = scope;
          let res = $block;
          match res {
            Ok(v) => Ok(v.into()),
            Err(err) => Err(err),
          }
        }
      }
    )+
  };
  ($ty:ty : |$value:ident, $scope:ident| $block:expr_2021) => {
    to_v8_fallible!(( $ty ) : |$value, $scope| $block);
  };
}

/// Helper macro for [`RustToV8RetValFallible`] to reduce boilerplate.
///
/// Implements a Rust-to-v8 conversion that can fail. Multiple types may be specified
/// to apply the same implementation. Will place the output in a `v8::ReturnValue`, but
/// will not allocate.
macro_rules! to_v8_retval_fallible {
  (( $( $ty:ty ),+ ) : |$value:ident, $scope: ident, $rv:ident| $block:expr_2021) => {
    $(
      impl <'a> RustToV8RetValFallible<'a> for $ty {
        #[inline(always)]
        fn to_v8_rv_fallible<'i>(self, scope: &mut v8::PinScope<'a, 'i>, rv: &mut v8::ReturnValue<'a>) -> serde_v8::Result<()>{
          let $value = self;
          let $scope = scope;
          let $rv = rv;
          $block
        }
      }
    )+
  };
  ($ty:ty : |$value:ident, $scope: ident, $rv:ident| $block:expr_2021) => {
    to_v8_retval_fallible!(( $ty ) : |$value, $scope, $rv| $block);
  };
}

//
// Integer/primitive conversions (cheap)
//

to_v8!((): |_value, scope| v8::null(scope));
to_v8_retval!((): |_value, rv| rv.set_null());
to_v8!(bool: |value, scope| v8::Boolean::new(scope, value as _));
to_v8_retval!(bool: |value, rv| rv.set_bool(value));
to_v8!((u8, u16, u32): |value, scope| v8::Integer::new_from_unsigned(scope, value as _));
to_v8_retval!((u8, u16, u32): |value, rv| rv.set_uint32(value as _));
to_v8!((i8, i16, i32): |value, scope| v8::Integer::new(scope, value as _));
to_v8_retval!((i8, i16, i32): |value, rv| rv.set_int32(value as _));
to_v8!((f32, f64): |value, scope| v8::Number::new(scope, value as _));
to_v8_retval!((f32, f64): |value, rv| rv.set_double(value as _));

//
// Heavier primitives with no retval shortcuts
//

to_v8!((*const c_void, *mut c_void): |value, scope| {
  if value.is_null() {
    v8::Local::<v8::Value>::from(v8::null(scope))
  } else {
    v8::Local::<v8::Value>::from(v8::External::new(scope, value as _))
  }
});
to_v8!((u64, usize): |value, scope| v8::BigInt::new_from_u64(scope, value as _));
to_v8!((i64, isize): |value, scope| v8::BigInt::new_from_i64(scope, value as _));

//
// Strings
//

to_v8_fallible!((String, Cow<'a, str>, &'a str): |value, scope| v8::String::new(scope, &value).ok_or_else(|| serde_v8::Error::Message("failed to allocate string; buffer exceeds maximum length".into())));
to_v8_retval_fallible!((String, Cow<'a, str>, &'a str): |value, scope, rv| {
  if value.is_empty() {
    rv.set_empty_string();
  } else {
    rv.set(value.to_v8_fallible(scope)?);
  }
  Ok(())
});
to_v8_fallible!(Cow<'a, [u8]>: |value, scope| v8::String::new_from_one_byte(scope, &value, v8::NewStringType::Normal).ok_or_else(|| serde_v8::Error::Message("failed to allocate string; buffer exceeds maximum length".into())));
to_v8_retval_fallible!(Cow<'a, [u8]>: |value, scope, rv| {
  if value.is_empty() {
    rv.set_empty_string();
  } else {
    rv.set(value.to_v8_fallible(scope)?);
  }
  Ok(())
});

//
// Buffers
//

to_v8_fallible!(serde_v8::V8Slice<u8>: |value, scope| {
  value.into_v8_local(scope).ok_or_else(|| serde_v8::Error::Message("failed to allocate array".into()))
});
to_v8!(RustToV8Marker<ArrayBufferMarker, serde_v8::V8Slice<u8>>: |value, scope| {
  value.0.into_v8_unsliced_arraybuffer_local(scope)
});
to_v8_fallible!(serde_v8::V8Slice<u32>: |value, scope| {
  value.into_v8_local(scope).ok_or_else(|| serde_v8::Error::Message("failed to allocate array".into()))
});
to_v8!(RustToV8Marker<ArrayBufferMarker, serde_v8::JsBuffer>: |value, scope| {
  RustToV8Marker::<ArrayBufferMarker, _>::from(value.0.into_parts()).to_v8(scope)
});
to_v8_fallible!(serde_v8::JsBuffer: |value, scope| {
  value.into_parts().to_v8_fallible(scope)
});
to_v8!(RustToV8Marker<ArrayBufferMarker, BytesMut>: |value, scope| {
  let value = value.0;
  let ptr = value.as_ptr();
  let len = value.len() as _;
  let rc = Rc::into_raw(Rc::new(value)) as *const c_void;

  extern "C" fn drop_rc(_ptr: *mut c_void, _len: usize, data: *mut c_void) {
    // SAFETY: We know that data is a raw Rc from above
    unsafe { drop(Rc::<BytesMut>::from_raw(data as _)) }
  }

  // SAFETY: We are using the BytesMut backing store here
  let backing_store_shared = unsafe {
    v8::ArrayBuffer::new_backing_store_from_ptr(
      ptr as _, len, drop_rc, rc as _,
    )
  }
  .make_shared();
  v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared)
});
to_v8_fallible!(BytesMut: |buf, scope| {
  let len = buf.len();
  let ab = unsafe { v8::Local::cast_unchecked(RustToV8Marker::<ArrayBufferMarker, _>::from(buf).to_v8(scope)) };
  v8::Uint8Array::new(scope, ab, 0, len).ok_or_else(|| serde_v8::Error::Message("failed to allocate array".into()))
});

macro_rules! typedarray {
  ($rty:ty, $v8ty:ident) => {
    to_v8!(RustToV8Marker<ArrayBufferMarker, Box<[$rty]>>: |buf, scope| {
      let buf = buf.0;
      if buf.is_empty() {
        v8::ArrayBuffer::new(scope, 0)
      } else {
        let backing_store =
          v8::ArrayBuffer::new_backing_store_from_bytes(buf);
        let backing_store_shared = backing_store.make_shared();
        v8::ArrayBuffer::with_backing_store(scope, &backing_store_shared)
      }
    });
    to_v8_fallible!(Box<[$rty]>: |buf, scope| {
      let len = buf.len();
      let ab = unsafe {
        v8::Local::cast_unchecked(RustToV8Marker::<ArrayBufferMarker, _>::from(buf).to_v8(scope))
      };
      v8::$v8ty::new(scope, ab, 0, len).ok_or_else(|| serde_v8::Error::Message("failed to allocate array".into()))
    });
    to_v8!(RustToV8Marker<ArrayBufferMarker, Vec<$rty>>: |value, scope| {
      RustToV8Marker::<ArrayBufferMarker, _>::from(value.0.into_boxed_slice()).to_v8(scope)
    });
    to_v8_fallible!(Vec<$rty>: |value, scope| value.into_boxed_slice().to_v8_fallible(scope));
  }
}

typedarray!(u8, Uint8Array);
typedarray!(u16, Uint16Array);
typedarray!(u32, Uint32Array);
typedarray!(u64, BigUint64Array);
typedarray!(i8, Int8Array);
typedarray!(i16, Int16Array);
typedarray!(i32, Int32Array);
typedarray!(i64, BigInt64Array);
typedarray!(f32, Float32Array);
typedarray!(f64, Float64Array);

//
// Serde
//

impl<'a, T: serde::Serialize> RustToV8Fallible<'a>
  for RustToV8Marker<SerdeMarker, T>
{
  #[inline(always)]
  fn to_v8_fallible<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> serde_v8::Result<v8::Local<'a, v8::Value>> {
    serde_v8::to_v8(scope, self.0)
  }
}

//
// CppGc
//
impl<'a, T: crate::cppgc::GarbageCollected + 'static> RustToV8<'a>
  for RustToV8Marker<CppGcMarker, T>
{
  #[inline(always)]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> v8::Local<'a, v8::Value> {
    v8::Local::<v8::Value>::from(deno_core::cppgc::make_cppgc_object(
      scope, self.0,
    ))
  }
}

//
// Undefined
//
impl<'a> RustToV8<'a> for RustToV8Marker<Undefined, ()> {
  #[inline(always)]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> v8::Local<'a, v8::Value> {
    v8::undefined(scope).into()
  }
}

impl<'a, T: crate::ToV8<'a>> RustToV8Fallible<'a>
  for RustToV8Marker<ToV8Marker, T>
{
  #[inline(always)]
  fn to_v8_fallible<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> serde_v8::Result<v8::Local<'a, v8::Value>> {
    self
      .0
      .to_v8(scope)
      .map_err(|e| serde_v8::Error::Custom(Box::new(e)))
  }
}

//
// SMI
//

/// Implement an infallible SMI conversion from each of the integer types. SMI has a subset
/// of the range of a normal integer, so we just do a bitwise cast.
// TODO(mmastrac): We should be able to make these things much faster and without a scope
macro_rules! smi_to_v8 {
  ($($ty:ident),*) => {
    $(
      impl<'a> RustToV8<'a> for RustToV8Marker<SmiMarker, $ty>
      {
        #[inline(always)]
        fn to_v8<'i>(
          self,
          scope: &mut v8::PinScope<'a, 'i>,
        ) -> v8::Local<'a, v8::Value> {
          v8::Integer::new(scope, self.0 as _).into()
        }
      }

      impl <'a> RustToV8RetVal<'a> for RustToV8Marker<SmiMarker, $ty> {
        #[inline(always)]
        fn to_v8_rv(self, rv: &mut v8::ReturnValue<'a>) {
          rv.set_int32(self.0 as _)
        }
      }
    )*
  };
}

smi_to_v8!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

//
// 64-bit numbers
//

/// Implement an infallible SMI conversion from each of the integer types. SMI has a subset
/// of the range of a normal integer, so we just do a bitwise cast.
// TODO(mmastrac): We should be able to make these things much faster and without a scope
macro_rules! number_64_bit_to_v8 {
  ($($ty:ident),*) => {
    $(
      impl<'a> RustToV8<'a> for RustToV8Marker<NumberMarker, $ty>
      {
        #[inline(always)]
        fn to_v8<'i>(
          self,
          scope: &mut v8::PinScope<'a, 'i>,
        ) -> v8::Local<'a, v8::Value> {
          v8::Number::new(scope, self.0 as _).into()
        }
      }

      impl <'a> RustToV8RetVal<'a> for RustToV8Marker<NumberMarker, $ty> {
        #[inline(always)]
        fn to_v8_rv(self, rv: &mut v8::ReturnValue<'a>) {
          rv.set_double(self.0 as _)
        }
      }
    )*
  };
}

number_64_bit_to_v8!(u64, usize, i64, isize);

//
// Globals
//

impl<'a, T> RustToV8<'a> for v8::Global<T>
where
  v8::Local<'a, v8::Value>: From<v8::Local<'a, T>>,
{
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> v8::Local<'a, v8::Value> {
    v8::Local::new(scope, self).into()
  }
}

//
// Locals
//

impl<'a, T> RustToV8NoScope<'a> for v8::Local<'a, T>
where
  v8::Local<'a, v8::Value>: From<v8::Local<'a, T>>,
{
  #[inline(always)]
  fn to_v8(self) -> v8::Local<'a, v8::Value> {
    self.into()
  }
}

impl<'a, T> RustToV8<'a> for Option<v8::Local<'a, T>>
where
  v8::Local<'a, v8::Value>: From<v8::Local<'a, T>>,
{
  #[inline(always)]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> v8::Local<'a, v8::Value> {
    if let Some(v) = self {
      v.to_v8()
    } else {
      v8::null(scope).into()
    }
  }
}

impl<'a, T> RustToV8RetVal<'a> for Option<v8::Local<'a, T>>
where
  v8::Local<'a, v8::Value>: From<v8::Local<'a, T>>,
{
  fn to_v8_rv(self, rv: &mut v8::ReturnValue<'a>) {
    if let Some(v) = self {
      rv.set(v.to_v8())
    } else {
      rv.set_null()
    }
  }
}
