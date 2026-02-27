// Copyright 2018-2025 the Deno authors. MIT license.

use crate::error::DataError;
use crate::runtime::ops;
use deno_error::JsErrorBox;
use deno_error::JsErrorClass;
use smallvec::SmallVec;
use std::convert::Infallible;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ops::DerefMut;
use v8::Local;
use v8::PinScope;

/// A conversion from a rust value to a v8 value.
///
/// When passing data from Rust into JS, either
/// via an op or by calling a JS function directly,
/// you need to serialize the data into a native
/// V8 value. When using the [`op2`][deno_core::op2] macro, the return
/// value is converted to a `v8::Local<Value>` automatically,
/// and the strategy for conversion is controlled by attributes
/// like `#[smi]`, `#[number]`, `#[string]`. For types with support
/// built-in to the op2 macro, like primitives, strings, and buffers,
/// these attributes are sufficient and you don't need to worry about this trait.
///
/// If, however, you want to return a custom type from an op, or
/// simply want more control over the conversion process,
/// you can implement the `ToV8` trait. This allows you the
/// choose the best serialization strategy for your specific use case.
/// You can then use the `#[to_v8]` attribute to indicate
/// that the `#[op2]` macro should call your implementation for the conversion.
///
/// # Example
///
/// ```ignore
/// use deno_core::ToV8;
/// use deno_core::convert::Smi;
/// use deno_core::op2;
///
/// struct Foo(i32);
///
/// impl<'a> ToV8<'a> for Foo {
///   // This conversion can never fail, so we use `Infallible` as the error type.
///   // Any error type that implements `std::error::Error` can be used here.
///   type Error = std::convert::Infallible;
///
///   fn to_v8(self, scope: &mut v8::PinScope<'a, '_>) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
///     // For performance, pass this value as a `v8::Integer` (i.e. a `smi`).
///     // The `Smi` wrapper type implements this conversion for you.
///     Smi(self.0).to_v8(scope)
///   }
/// }
///
/// // using the `#[to_v8]` attribute tells the `op2` macro to call this implementation.
/// #[op2]
/// #[to_v8]
/// fn op_foo() -> Foo {
///   Foo(42)
/// }
/// ```
///
/// # Performance Notes
/// ## Structs
/// The natural representation of a struct in JS is an object with fields
/// corresponding the struct. This, however, is a performance footgun and
/// you should avoid creating and passing objects to V8 whenever possible.
/// In general, if you need to pass a compound type to JS, it is more performant to serialize
/// to a tuple (a `v8::Array`) rather than an object.
/// Object keys are V8 strings, and strings are expensive to pass to V8
/// and they have to be managed by the V8 garbage collector.
/// Tuples, on the other hand, are keyed by `smi`s, which are immediates
/// and don't require allocation or garbage collection.
pub trait ToV8<'a> {
  type Error: JsErrorClass;

  /// Converts the value to a V8 value.
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error>;
}

/// A conversion from a v8 value to a rust value.
///
/// When writing a op, or otherwise writing a function in Rust called
/// from JS, arguments passed from JS are represented as [`v8::Local<v8::Value>>`][deno_core::v8::Value].
/// To convert these values into custom Rust types, you can implement the [`FromV8`] trait.
///
/// Once you've implemented this trait, you can use the `#[from_v8]` attribute
/// to tell the [`op2`][deno_core::op2] macro to use your implementation to convert the argument
/// to the desired type.
///
/// # Example
///
/// ```ignore
/// use deno_core::FromV8;
/// use deno_error::JsErrorBox;
/// use deno_core::convert::Smi;
/// use deno_core::op2;
///
/// struct Foo(i32);
///
/// impl<'a> FromV8<'a> for Foo {
///   // This conversion can fail, so we use `JsErrorBox` as the error type.
///   // Any error type that implements `std::error::Error` can be used here.
///   type Error = JsErrorBox;
///
///   fn from_v8(scope: &mut v8::PinScope<'a, '_>, value: v8::Local<'a, v8::Value>) -> Result<Self, Self::Error> {
///     /// We expect this value to be a `v8::Integer`, so we use the [`Smi`][deno_core::convert::Smi] wrapper type to convert it.
///     Smi::from_v8(scope, value).map(|Smi(v)| Foo(v))
///   }
/// }
///
/// // using the `#[from_v8]` attribute tells the `op2` macro to call this implementation.
/// #[op2]
/// fn op_foo(#[from_v8] foo: Foo) {
///   let Foo(_) = foo;
/// }
/// ```
pub trait FromV8<'a>: Sized {
  type Error: JsErrorClass;

  /// Converts a V8 value to a Rust value.
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error>;
}

/// An alternative to [`FromV8`] that does not require a [`PinScope`].
pub trait FromV8Scopeless<'a>: Sized + FromV8<'a> {
  /// Converts a V8 value to a Rust value.
  fn from_v8(value: v8::Local<'a, v8::Value>) -> Result<Self, Self::Error>;
}

// impls

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Marks a numeric type as being serialized as a v8 `smi` in a `v8::Integer`.
#[repr(transparent)]
pub struct Smi<T: SmallInt>(pub T);

/// A trait for types that can represent a JS `smi`.
pub trait SmallInt {
  const NAME: &'static str;

  #[allow(clippy::wrong_self_convention)]
  fn as_i32(self) -> i32;
  fn from_i32(value: i32) -> Self;
}

macro_rules! impl_smallint {
  (for $($t:ty),*) => {
    $(
      impl SmallInt for $t {
        const NAME: &'static str = stringify!($t);
        #[allow(clippy::wrong_self_convention)]
        #[inline(always)]
        fn as_i32(self) -> i32 {
          self as _
        }

        #[inline(always)]
        fn from_i32(value: i32) -> Self {
            value as _
        }
      }
    )*
  };
}

impl_smallint!(for u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl<'s, T: SmallInt> ToV8<'s> for Smi<T> {
  type Error = Infallible;

  #[inline]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    Ok(v8::Integer::new(scope, self.0.as_i32()).into())
  }
}

impl<'s, T: SmallInt> FromV8<'s> for Smi<T> {
  type Error = DataError;

  fn from_v8<'i>(
    _scope: &mut PinScope<'s, 'i>,
    value: Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    <Self as FromV8Scopeless>::from_v8(value)
  }
}

impl<'s, T: SmallInt> FromV8Scopeless<'s> for Smi<T> {
  #[inline]
  fn from_v8(value: v8::Local<'s, v8::Value>) -> Result<Self, Self::Error> {
    let v = ops::to_i32_option(&value).ok_or_else(|| {
      DataError(v8::DataError::BadType {
        actual: value.type_repr(),
        expected: T::NAME,
      })
    })?;
    Ok(Smi(T::from_i32(v)))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Marks a numeric type as being serialized as a v8 `number` in a `v8::Number`.
#[repr(transparent)]
pub struct Number<T: Numeric>(pub T);

/// A trait for types that can represent a JS `number`.
pub trait Numeric: Sized {
  const NAME: &'static str;
  #[allow(clippy::wrong_self_convention)]
  fn as_f64(self) -> f64;
  fn from_value(value: &v8::Value) -> Option<Self>;
}

macro_rules! impl_numeric {
  ($($t:ty : $from: path ),*) => {
    $(
      impl Numeric for $t {
        const NAME: &'static str = stringify!($t);
        #[inline(always)]
        fn from_value(value: &v8::Value) -> Option<Self> {
          $from(value).map(|v| v as _)
        }

        #[allow(clippy::wrong_self_convention)]
        #[inline(always)]
        fn as_f64(self) -> f64 {
            self as _
        }
      }
    )*
  };
}

impl_numeric!(
  f32   : ops::to_f32_option,
  f64   : ops::to_f64_option,
  u32   : ops::to_u32_option,
  u64   : ops::to_u64_option,
  usize : ops::to_u64_option,
  i32   : ops::to_i32_option,
  i64   : ops::to_i64_option,
  isize : ops::to_i64_option
);

impl<'s, T: Numeric> ToV8<'s> for Number<T> {
  type Error = Infallible;
  #[inline]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    Ok(v8::Number::new(scope, self.0.as_f64()).into())
  }
}

impl<'s, T: Numeric> FromV8<'s> for Number<T> {
  type Error = DataError;

  fn from_v8<'i>(
    _scope: &mut PinScope<'s, 'i>,
    value: Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    <Self as FromV8Scopeless>::from_v8(value)
  }
}

impl<'s, T: Numeric> FromV8Scopeless<'s> for Number<T> {
  #[inline]
  fn from_v8(value: v8::Local<'s, v8::Value>) -> Result<Self, Self::Error> {
    T::from_value(&value).map(Number).ok_or_else(|| {
      DataError(v8::DataError::BadType {
        actual: value.type_repr(),
        expected: T::NAME,
      })
    })
  }
}

macro_rules! impl_number_types {
  ($($t:ty),*) => {
    $(
      impl<'a> FromV8<'a> for $t {
        type Error = DataError;
        #[inline]
        fn from_v8<'i>(
          _scope: &mut v8::PinScope<'a, 'i>,
          value: v8::Local<'a, v8::Value>,
        ) -> Result<Self, Self::Error> {
          <Self as FromV8Scopeless>::from_v8(value)
        }
      }
      impl<'a> FromV8Scopeless<'a> for $t {
        #[inline]
        fn from_v8(value: v8::Local<'a, v8::Value>) -> Result<Self, Self::Error> {
          let n = value.try_cast::<v8::Number>()?;
          Ok(n.value() as Self)
        }
      }

      impl<'a> ToV8<'a> for $t {
        type Error = Infallible;
        #[inline]
        fn to_v8<'i>(
          self,
          scope: &mut v8::PinScope<'a, 'i>,
        ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
          Ok(v8::Number::new(scope, self as f64).into())
        }
      }
    )*
  };
}

impl_number_types!(
  u8, i8, u16, i16, u32, i32, u64, i64, usize, isize, f32, f64
);

pub struct BigInt {
  pub sign_bit: bool,
  pub words: Vec<u64>,
}

impl<'s> ToV8<'s> for BigInt {
  type Error = JsErrorBox;

  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    v8::BigInt::new_from_words(scope, self.sign_bit, &self.words)
      .map(Into::into)
      .ok_or_else(|| JsErrorBox::type_error("Failed to create BigInt"))
  }
}

impl<'s> FromV8<'s> for BigInt {
  type Error = DataError;

  fn from_v8<'i>(
    _scope: &mut v8::PinScope<'s, 'i>,
    value: v8::Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let bigint = value.try_cast::<v8::BigInt>()?;

    let word_count = bigint.word_count();
    let mut words = vec![0u64; word_count];
    let (sign_bit, _) = bigint.to_words_array(&mut words);

    Ok(BigInt { sign_bit, words })
  }
}

impl<'s> ToV8<'s> for bool {
  type Error = Infallible;
  #[inline]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    Ok(v8::Boolean::new(scope, self).into())
  }
}

impl<'s> FromV8<'s> for bool {
  type Error = DataError;

  fn from_v8<'i>(
    _scope: &mut PinScope<'s, 'i>,
    value: Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    <Self as FromV8Scopeless>::from_v8(value)
  }
}

impl<'s> FromV8Scopeless<'s> for bool {
  #[inline]
  fn from_v8(value: v8::Local<'s, v8::Value>) -> Result<Self, Self::Error> {
    value
      .try_cast::<v8::Boolean>()
      .map(|v| v.is_true())
      .map_err(DataError)
  }
}

impl<'s> FromV8<'s> for String {
  type Error = Infallible;
  #[inline]
  fn from_v8<'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    value: v8::Local<'s, v8::Value>,
  ) -> Result<String, Self::Error> {
    Ok(value.to_rust_string_lossy(scope))
  }
}
impl<'s> ToV8<'s> for String {
  type Error = Infallible;
  #[inline]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    Ok(v8::String::new(scope, &self).unwrap().into()) // TODO
  }
}

impl<'s> ToV8<'s> for &'static str {
  type Error = Infallible;
  #[inline]
  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    Ok(v8::String::new(scope, self).unwrap().into()) // TODO
  }
}

const USIZE2X: usize = std::mem::size_of::<usize>() * 2;
#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub struct ByteString(SmallVec<[u8; USIZE2X]>);

impl Deref for ByteString {
  type Target = SmallVec<[u8; USIZE2X]>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for ByteString {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl AsRef<[u8]> for ByteString {
  fn as_ref(&self) -> &[u8] {
    &self.0
  }
}

impl AsMut<[u8]> for ByteString {
  fn as_mut(&mut self) -> &mut [u8] {
    &mut self.0
  }
}

impl<'a> ToV8<'a> for ByteString {
  type Error = Infallible;

  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'a, 'i>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let v = v8::String::new_from_one_byte(
      scope,
      self.as_ref(),
      v8::NewStringType::Normal,
    )
    .unwrap();
    Ok(v.into())
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum ByteStringError {
  #[error("Invalid type, expected: string but got: {0}")]
  ExpectedString(&'static str),
  #[error("Invalid type, expected: latin1")]
  ExpectedLatin1,
}

impl<'a> FromV8<'a> for ByteString {
  type Error = ByteStringError;

  fn from_v8<'i>(
    scope: &mut v8::PinScope<'a, 'i>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let v8str = v8::Local::<v8::String>::try_from(value)
      .map_err(|_| ByteStringError::ExpectedString(value.type_repr()))?;
    if !v8str.contains_only_onebyte() {
      return Err(ByteStringError::ExpectedLatin1);
    }
    let len = v8str.length();
    let mut buffer = SmallVec::with_capacity(len);
    #[allow(clippy::uninit_vec)]
    // SAFETY: we set length == capacity (see previous line),
    // before immediately writing into that buffer and sanity check with an assert
    unsafe {
      buffer.set_len(len);
      v8str.write_one_byte_v2(scope, 0, &mut buffer, v8::WriteFlags::empty());
    }
    Ok(Self(buffer))
  }
}

impl From<Vec<u8>> for ByteString {
  fn from(vec: Vec<u8>) -> Self {
    ByteString(SmallVec::from_vec(vec))
  }
}

#[allow(clippy::from_over_into)]
impl Into<Vec<u8>> for ByteString {
  fn into(self) -> Vec<u8> {
    self.0.into_vec()
  }
}

impl From<&[u8]> for ByteString {
  fn from(s: &[u8]) -> Self {
    ByteString(SmallVec::from_slice(s))
  }
}

impl From<&str> for ByteString {
  fn from(s: &str) -> Self {
    let v: Vec<u8> = s.into();
    ByteString::from(v)
  }
}

impl From<String> for ByteString {
  fn from(s: String) -> Self {
    ByteString::from(s.into_bytes())
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A wrapper type for `Option<T>` that (de)serializes `None` as `null`
#[repr(transparent)]
pub struct OptionNull<T>(pub Option<T>);

impl<T> From<Option<T>> for OptionNull<T> {
  fn from(option: Option<T>) -> Self {
    Self(option)
  }
}

impl<T> From<OptionNull<T>> for Option<T> {
  fn from(value: OptionNull<T>) -> Self {
    value.0
  }
}

impl<'s, T> ToV8<'s> for OptionNull<T>
where
  T: ToV8<'s>,
{
  type Error = T::Error;

  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    match self.0 {
      Some(value) => value.to_v8(scope),
      None => Ok(v8::null(scope).into()),
    }
  }
}

impl<'s, T> FromV8<'s> for OptionNull<T>
where
  T: FromV8<'s>,
{
  type Error = T::Error;

  fn from_v8<'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    value: v8::Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    if value.is_null() {
      Ok(OptionNull(None))
    } else {
      T::from_v8(scope, value).map(|v| OptionNull(Some(v)))
    }
  }
}

impl<'s, T> FromV8Scopeless<'s> for OptionNull<T>
where
  T: FromV8Scopeless<'s>,
{
  fn from_v8(value: v8::Local<'s, v8::Value>) -> Result<Self, Self::Error> {
    if value.is_null() {
      Ok(OptionNull(None))
    } else {
      <T as FromV8Scopeless>::from_v8(value).map(|v| OptionNull(Some(v)))
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// A wrapper type for `Option<T>` that (de)serializes `None` as `undefined`
#[repr(transparent)]
pub struct OptionUndefined<T>(pub Option<T>);

impl<T> From<Option<T>> for OptionUndefined<T> {
  fn from(option: Option<T>) -> Self {
    Self(option)
  }
}

impl<T> From<OptionUndefined<T>> for Option<T> {
  fn from(value: OptionUndefined<T>) -> Self {
    value.0
  }
}

impl<'s, T> ToV8<'s> for OptionUndefined<T>
where
  T: ToV8<'s>,
{
  type Error = T::Error;

  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    match self.0 {
      Some(value) => value.to_v8(scope),
      None => Ok(v8::undefined(scope).into()),
    }
  }
}

impl<'s, T> FromV8<'s> for OptionUndefined<T>
where
  T: FromV8<'s>,
{
  type Error = T::Error;

  fn from_v8<'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    value: v8::Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    if value.is_undefined() {
      Ok(OptionUndefined(None))
    } else {
      T::from_v8(scope, value).map(|v| OptionUndefined(Some(v)))
    }
  }
}

impl<'s, T> FromV8Scopeless<'s> for OptionUndefined<T>
where
  T: FromV8Scopeless<'s>,
{
  fn from_v8(value: v8::Local<'s, v8::Value>) -> Result<Self, Self::Error> {
    if value.is_undefined() {
      Ok(OptionUndefined(None))
    } else {
      <T as FromV8Scopeless>::from_v8(value).map(|v| OptionUndefined(Some(v)))
    }
  }
}

unsafe fn abview_to_box<T>(
  ab_view: v8::Local<v8::ArrayBufferView>,
) -> Box<[T]> {
  if ab_view.byte_length() == 0 {
    return Box::new([]);
  }
  let data = ab_view.data();
  let len = ab_view.byte_length() / std::mem::size_of::<T>();
  let mut out = Box::<[T]>::new_uninit_slice(len);
  unsafe {
    std::ptr::copy_nonoverlapping(
      data.cast::<T>(),
      out.as_mut_ptr().cast::<T>(),
      len,
    );
    out.assume_init()
  }
}

macro_rules! typedarray_to_v8 {
  ($ty:ty, $v8ty:ident, $v8fn:ident) => {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct $v8ty(pub Box<[$ty]>);

    impl std::ops::Deref for $v8ty {
      type Target = [$ty];
      fn deref(&self) -> &Self::Target {
        &self.0
      }
    }

    impl From<Vec<$ty>> for $v8ty {
      fn from(value: Vec<$ty>) -> Self {
        Self(value.into_boxed_slice())
      }
    }

    impl From<Box<[$ty]>> for $v8ty {
      fn from(value: Box<[$ty]>) -> Self {
        Self(value)
      }
    }

    impl<'a> ToV8<'a> for $v8ty {
      type Error = JsErrorBox;

      fn to_v8<'i>(
        self,
        scope: &mut v8::PinScope<'a, 'i>,
      ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
        let len = self.0.len();
        if self.0.is_empty() {
          let ab = v8::ArrayBuffer::new(scope, 0);
          return v8::$v8ty::new(scope, ab, 0, 0)
            .ok_or_else(|| {
              JsErrorBox::type_error("Failed to create typed array")
            })
            .map(|v| v.into());
        }
        let backing = v8::ArrayBuffer::new_backing_store_from_bytes(self.0);
        let backing_shared = backing.make_shared();
        let ab = v8::ArrayBuffer::with_backing_store(scope, &backing_shared);
        v8::$v8ty::new(scope, ab, 0, len)
          .ok_or_else(|| JsErrorBox::type_error("Failed to create typed array"))
          .map(|v| v.into())
      }
    }

    impl<'s> FromV8<'s> for $v8ty {
      type Error = DataError;

      fn from_v8<'i>(
        _scope: &mut PinScope<'s, 'i>,
        value: Local<'s, v8::Value>,
      ) -> Result<Self, Self::Error> {
        <Self as FromV8Scopeless>::from_v8(value)
      }
    }

    impl<'a> FromV8Scopeless<'a> for $v8ty {
      fn from_v8(value: v8::Local<'a, v8::Value>) -> Result<Self, Self::Error> {
        if value.$v8fn() {
          Ok($v8ty(unsafe {
            abview_to_box::<$ty>(value.cast::<v8::ArrayBufferView>())
          }))
        } else {
          Err(DataError(v8::DataError::BadType {
            actual: value.type_repr(),
            expected: stringify!($v8ty),
          }))
        }
      }
    }
  };
}

typedarray_to_v8!(i8, Int8Array, is_int8_array);
typedarray_to_v8!(u8, Uint8Array, is_uint8_array);
typedarray_to_v8!(i16, Int16Array, is_int16_array);
typedarray_to_v8!(u16, Uint16Array, is_uint16_array);
typedarray_to_v8!(i32, Int32Array, is_int32_array);
typedarray_to_v8!(u32, Uint32Array, is_uint32_array);
typedarray_to_v8!(i64, BigInt64Array, is_big_int64_array);
typedarray_to_v8!(u64, BigUint64Array, is_big_uint64_array);

pub enum ArrayBufferView {
  Int8Array(Int8Array),
  Uint8Array(Uint8Array),
  Int16Array(Int16Array),
  Uint16Array(Uint16Array),
  Int32Array(Int32Array),
  Uint32Array(Uint32Array),
  BigInt64Array(BigInt64Array),
  BigUint64Array(BigUint64Array),
}

impl<'a> ToV8<'a> for ArrayBufferView {
  type Error = JsErrorBox;

  fn to_v8<'i>(
    self,
    scope: &mut PinScope<'a, 'i>,
  ) -> Result<Local<'a, v8::Value>, Self::Error> {
    match self {
      ArrayBufferView::Int8Array(view) => view.to_v8(scope),
      ArrayBufferView::Uint8Array(view) => view.to_v8(scope),
      ArrayBufferView::Int16Array(view) => view.to_v8(scope),
      ArrayBufferView::Uint16Array(view) => view.to_v8(scope),
      ArrayBufferView::Int32Array(view) => view.to_v8(scope),
      ArrayBufferView::Uint32Array(view) => view.to_v8(scope),
      ArrayBufferView::BigInt64Array(view) => view.to_v8(scope),
      ArrayBufferView::BigUint64Array(view) => view.to_v8(scope),
    }
  }
}

impl<'s> FromV8<'s> for ArrayBufferView {
  type Error = DataError;

  fn from_v8<'i>(
    _scope: &mut PinScope<'s, 'i>,
    value: Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    <Self as FromV8Scopeless>::from_v8(value)
  }
}

impl<'a> FromV8Scopeless<'a> for ArrayBufferView {
  fn from_v8(value: Local<'a, v8::Value>) -> Result<Self, Self::Error> {
    if value.is_int8_array() {
      Ok(Self::Int8Array(<Int8Array as FromV8Scopeless>::from_v8(
        value,
      )?))
    } else if value.is_uint8_array() {
      Ok(Self::Uint8Array(<Uint8Array as FromV8Scopeless>::from_v8(
        value,
      )?))
    } else if value.is_int16_array() {
      Ok(Self::Int16Array(<Int16Array as FromV8Scopeless>::from_v8(
        value,
      )?))
    } else if value.is_uint16_array() {
      Ok(Self::Uint16Array(
        <Uint16Array as FromV8Scopeless>::from_v8(value)?,
      ))
    } else if value.is_int32_array() {
      Ok(Self::Int32Array(<Int32Array as FromV8Scopeless>::from_v8(
        value,
      )?))
    } else if value.is_uint32_array() {
      Ok(Self::Uint32Array(
        <Uint32Array as FromV8Scopeless>::from_v8(value)?,
      ))
    } else if value.is_big_int64_array() {
      Ok(Self::BigInt64Array(
        <BigInt64Array as FromV8Scopeless>::from_v8(value)?,
      ))
    } else if value.is_big_uint64_array() {
      Ok(Self::BigUint64Array(
        <BigUint64Array as FromV8Scopeless>::from_v8(value)?,
      ))
    } else {
      Err(DataError(v8::DataError::BadType {
        actual: value.type_repr(),
        expected: "ArrayBufferView",
      }))
    }
  }
}

impl<'a, T> ToV8<'a> for Vec<T>
where
  T: ToV8<'a>,
{
  type Error = T::Error;

  fn to_v8(
    self,
    scope: &mut v8::PinScope<'a, '_>,
  ) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
    let buf = self
      .into_iter()
      .map(|v| v.to_v8(scope))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(v8::Array::new_with_elements(scope, &buf).into())
  }
}

impl<'a, T> FromV8<'a> for Vec<T>
where
  T: FromV8<'a>,
{
  type Error = JsErrorBox;

  fn from_v8(
    scope: &mut v8::PinScope<'a, '_>,
    value: v8::Local<'a, v8::Value>,
  ) -> Result<Self, Self::Error> {
    let arr = v8::Local::<v8::Array>::try_from(value)
      .map_err(|e| JsErrorBox::from_err(DataError(e)))?;
    let len = arr.length() as usize;

    let mut out = maybe_uninit_vec::<T>(len);

    for i in 0..len {
      let v = arr.get_index(scope, i as u32).unwrap();
      match T::from_v8(scope, v) {
        Ok(v) => {
          out[i].write(v);
        }
        Err(e) => {
          // need to drop the elements we've already written
          for elem in out.iter_mut().take(i) {
            // SAFETY: we've initialized these elements
            unsafe {
              elem.assume_init_drop();
            }
          }
          return Err(JsErrorBox::from_err(e));
        }
      }
    }

    // SAFETY: all elements have been initialized, and `MaybeUninit<T>`
    // is transmutable to `T`
    let out = unsafe { transmute_vec::<MaybeUninit<T>, T>(out) };

    Ok(out)
  }
}

fn maybe_uninit_vec<T>(len: usize) -> Vec<std::mem::MaybeUninit<T>> {
  let mut v = Vec::with_capacity(len);
  // SAFETY: `MaybeUninit` is allowed to be uninitialized and
  // the length is the same as the capacity.
  unsafe {
    v.set_len(len);
  }
  v
}

/// Transmutes a `Vec` of one type to a `Vec` of another type.
///
/// # Safety
/// `T` must be transmutable to `U`
unsafe fn transmute_vec<T, U>(v: Vec<T>) -> Vec<U> {
  const {
    assert!(std::mem::size_of::<T>() == std::mem::size_of::<U>());
    assert!(std::mem::align_of::<T>() == std::mem::align_of::<U>());
  }

  // make sure the original vector is not dropped
  let mut v = std::mem::ManuallyDrop::new(v);
  let len = v.len();
  let cap = v.capacity();
  let ptr = v.as_mut_ptr();

  // SAFETY: the original vector is not dropped, the caller upholds the
  // transmutability invariants, and the length and capacity are not changed.
  unsafe { Vec::from_raw_parts(ptr as *mut U, len, cap) }
}

macro_rules! impl_tuple {
  ($($len: expr; ($($name: ident),*)),+) => {
    $(
      impl<'a, $($name),+> ToV8<'a> for ($($name,)+)
      where
        $($name: ToV8<'a>,)+
      {
        type Error = deno_error::JsErrorBox;
        fn to_v8(self, scope: &mut v8::PinScope<'a, '_>) -> Result<v8::Local<'a, v8::Value>, Self::Error> {
          #[allow(non_snake_case)]
          let ($($name,)+) = self;
          let elements = &[$($name.to_v8(scope).map_err(deno_error::JsErrorBox::from_err)?),+];
          Ok(v8::Array::new_with_elements(scope, elements).into())
        }
      }
      impl<'a, $($name),+> FromV8<'a> for ($($name,)+)
      where
        $($name: FromV8<'a>,)+
      {
        type Error = deno_error::JsErrorBox;

        fn from_v8(
          scope: &mut v8::PinScope<'a, '_>,
          value: v8::Local<'a, v8::Value>,
        ) -> Result<Self, Self::Error> {
          let array = v8::Local::<v8::Array>::try_from(value)
            .map_err(|e| deno_error::JsErrorBox::from_err(crate::error::DataError(e)))?;
          if array.length() != $len {
            return Err(deno_error::JsErrorBox::type_error(format!("Expected {} elements, got {}", $len, array.length())));
          }
          let mut i = 0;
          #[allow(non_snake_case)]
          let ($($name,)+) = (
            $(
              {
                let element = array.get_index(scope, i).unwrap();
                let res = $name::from_v8(scope, element).map_err(deno_error::JsErrorBox::from_err)?;
                #[allow(unused)]
                {
                  i += 1;
                }
                res
              },
            )+
          );
          Ok(($($name,)+))
        }
      }
    )+
  };
}

impl_tuple!(
  1; (A),
  2; (A, B),
  3; (A, B, C),
  4; (A, B, C, D),
  5; (A, B, C, D, E),
  6; (A, B, C, D, E, F),
  7; (A, B, C, D, E, F, G),
  8; (A, B, C, D, E, F, G, H),
  9; (A, B, C, D, E, F, G, H, I),
  10; (A, B, C, D, E, F, G, H, I, J),
  11; (A, B, C, D, E, F, G, H, I, J, K),
  12; (A, B, C, D, E, F, G, H, I, J, K, L)
);

impl<'s, T> ToV8<'s> for Option<T>
where
  T: ToV8<'s>,
{
  type Error = T::Error;

  fn to_v8<'i>(
    self,
    scope: &mut v8::PinScope<'s, 'i>,
  ) -> Result<v8::Local<'s, v8::Value>, Self::Error> {
    match self {
      Some(value) => value.to_v8(scope),
      None => Ok(v8::null(scope).into()),
    }
  }
}

impl<'s, T> FromV8<'s> for Option<T>
where
  T: FromV8<'s>,
{
  type Error = T::Error;

  fn from_v8<'i>(
    scope: &mut v8::PinScope<'s, 'i>,
    value: v8::Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    if value.is_null_or_undefined() {
      Ok(None)
    } else {
      T::from_v8(scope, value).map(|v| Some(v))
    }
  }
}

impl<'s, T> FromV8Scopeless<'s> for Option<T>
where
  T: FromV8Scopeless<'s>,
{
  fn from_v8(value: v8::Local<'s, v8::Value>) -> Result<Self, Self::Error> {
    if value.is_null_or_undefined() {
      Ok(None)
    } else {
      <T as FromV8Scopeless>::from_v8(value).map(|v| Some(v))
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum V8ConvertError {
  #[class(inherit)]
  #[error(transparent)]
  Infallible(#[from] Infallible),
  #[class(inherit)]
  #[error(transparent)]
  DataError(DataError),
}

impl From<v8::DataError> for V8ConvertError {
  fn from(value: v8::DataError) -> Self {
    Self::DataError(value.into())
  }
}

impl<'s, T, E> ToV8<'s> for v8::Global<T>
where
  Local<'s, T>: TryInto<Local<'s, v8::Value>, Error = E>,
  E: Into<V8ConvertError>,
{
  type Error = V8ConvertError;

  fn to_v8<'i>(
    self,
    scope: &mut PinScope<'s, 'i>,
  ) -> Result<Local<'s, v8::Value>, Self::Error> {
    let local: Local<'s, T> = Local::new(scope, self);
    local.try_into().map_err(Into::into)
  }
}

impl<'s, T, E> FromV8<'s> for v8::Global<T>
where
  Local<'s, v8::Value>: TryInto<Local<'s, T>, Error = E>,
  E: Into<V8ConvertError>,
{
  type Error = V8ConvertError;

  fn from_v8<'i>(
    scope: &mut PinScope<'s, 'i>,
    value: Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    Ok(v8::Global::new(
      scope,
      value.try_into().map_err(Into::into)?,
    ))
  }
}

impl<'s, T, E> ToV8<'s> for v8::Local<'s, T>
where
  Local<'s, T>: TryInto<Local<'s, v8::Value>, Error = E>,
  E: Into<V8ConvertError>,
{
  type Error = V8ConvertError;

  fn to_v8<'i>(
    self,
    _scope: &mut PinScope<'s, 'i>,
  ) -> Result<Local<'s, v8::Value>, Self::Error> {
    self.try_into().map_err(Into::into)
  }
}

impl<'s, T, E> FromV8<'s> for v8::Local<'s, T>
where
  Local<'s, v8::Value>: TryInto<Local<'s, T>, Error = E>,
  E: Into<V8ConvertError>,
{
  type Error = V8ConvertError;

  fn from_v8<'i>(
    _scope: &mut PinScope<'s, 'i>,
    value: Local<'s, v8::Value>,
  ) -> Result<Self, Self::Error> {
    <Self as FromV8Scopeless>::from_v8(value)
  }
}

impl<'s, T, E> FromV8Scopeless<'s> for v8::Local<'s, T>
where
  Local<'s, v8::Value>: TryInto<Local<'s, T>, Error = E>,
  E: Into<V8ConvertError>,
{
  fn from_v8(value: Local<'s, v8::Value>) -> Result<Self, Self::Error> {
    value.try_into().map_err(Into::into)
  }
}

#[cfg(all(test, not(miri)))]
mod tests {
  use super::*;
  use std::sync::atomic::{AtomicUsize, Ordering};

  use deno_error::JsErrorClass;

  use crate::JsRuntime;
  use crate::scope as scope_macro;
  use std::collections::HashMap;
  use v8::Local;

  static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

  fn next_id() -> usize {
    ID_COUNTER.fetch_add(1, Ordering::Relaxed)
  }

  // minified vendored code from @std/assert
  static ASSERT_CODE: &str = r#"
function equal(a, b) {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((v, i) => equal(v, b[i]));
  if (typeof a === 'object' && typeof b === 'object') {
    const keysA = Object.keys(a);
    const keysB = Object.keys(b);
    if (keysA.length !== keysB.length) return false;
    for (const key of keysA) {
      if (!equal(a[key], b[key])) return false;
    }
    return true;
  }
  return false;
}
"#;

  fn cast_closure<F>(f: F) -> F
  where
    F: for<'a, 'b> Fn(
        &mut v8::PinScope<'a, 'b>,
        v8::FunctionCallbackArguments<'a>,
        v8::ReturnValue<'a>,
      ) + 'static,
  {
    f
  }

  fn key<'a>(
    scope: &mut v8::PinScope<'a, '_>,
    name: &str,
  ) -> v8::Local<'a, v8::Value> {
    v8::String::new(scope, name).unwrap().into()
  }

  macro_rules! make_test_fn {
    ($scope:expr, $f:expr) => {
      let global = $scope.get_current_context().global($scope);
      let test_fn_key = key($scope, "test_fn");
      let test_fn = v8::FunctionBuilder::<v8::Function>::new(cast_closure($f))
        .build($scope)
        .unwrap();
      global.set($scope, test_fn_key, test_fn.into()).unwrap();
    };
  }

  macro_rules! to_v8_test {
    ($runtime:ident, |$scope: ident, $args: ident| $to_v8:expr, $assertion:expr) => {{
      scope_macro!($scope, &mut $runtime);
      make_test_fn!($scope, |$scope, $args, mut rv| {
        let v = $to_v8;
        rv.set(v);
      });
    }
    {
      let test_name = format!("test_{}", next_id());
      let assertion = format!("{}{};", ASSERT_CODE, $assertion);
      let result = $runtime.execute_script(test_name, assertion).unwrap();
      scope_macro!(scope, &mut $runtime);
      let local = v8::Local::new(scope, result);
      assert!(local.is_true());
    }};
  }

  macro_rules! from_v8_test {
    ($runtime:ident, $js:expr, |$scope: ident, $result: ident| $assertion:expr) => {{
      let js = format!("{}{};", ASSERT_CODE, $js);
      let $result = $runtime
        .execute_script(format!("test_{}", next_id()), js)
        .unwrap();
      scope_macro!($scope, &mut $runtime);
      let $result = v8::Local::new($scope, $result);
      $assertion;
    }};
  }

  #[test]
  fn test_option_undefined() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| OptionUndefined::<Number<f64>>(None).to_v8(scope).unwrap(),
      "test_fn() === undefined"
    );
    to_v8_test!(
      runtime,
      |scope, _args| OptionUndefined::<Number<f64>>(Some(Number(1.0)))
        .to_v8(scope)
        .unwrap(),
      "test_fn() === 1.0"
    );
    from_v8_test!(runtime, "undefined", |scope, result| {
      let r = <OptionUndefined<Number<f64>> as FromV8>::from_v8(scope, result)
        .unwrap();
      assert_eq!(r, OptionUndefined(None));
    });

    from_v8_test!(runtime, "1.0", |scope, result| {
      assert_eq!(
        <OptionUndefined::<Number<f64>> as FromV8>::from_v8(scope, result)
          .unwrap(),
        OptionUndefined(Some(Number(1.0)))
      )
    });
  }

  #[test]
  fn test_option_null() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| OptionNull::<Number<f64>>(None).to_v8(scope).unwrap(),
      "test_fn() === null"
    );

    to_v8_test!(
      runtime,
      |scope, _args| {
        OptionNull::<Number<f64>>(Some(Number(1.0)))
          .to_v8(scope)
          .unwrap()
      },
      "test_fn() === 1.0"
    );

    from_v8_test!(runtime, "null", |scope, result| assert_eq!(
      <OptionNull::<Number<f64>> as FromV8>::from_v8(scope, result).unwrap(),
      OptionNull(None)
    ));

    from_v8_test!(runtime, "1.0", |scope, result| assert_eq!(
      <OptionNull::<Number<f64>> as FromV8>::from_v8(scope, result).unwrap(),
      OptionNull(Some(Number(1.0)))
    ));
  }

  #[test]
  fn test_tuple() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| (Number(1.0), Number(2.0)).to_v8(scope).unwrap(),
      "var result = test_fn(); equal(result, [1.0, 2.0])"
    );

    to_v8_test!(
      runtime,
      |scope, _args| (Number(1.0), true).to_v8(scope).unwrap(),
      "var result = test_fn(); equal(result, [1.0, true])"
    );

    to_v8_test!(
      runtime,
      |scope, _args| (
        Number(1.0),
        Number(2.0),
        OptionNull::<Number<f64>>(Some(Number(3.0))),
        OptionUndefined::<bool>(None)
      )
        .to_v8(scope)
        .unwrap(),
      "equal(test_fn(), [1.0, 2.0, 3.0, undefined])"
    );

    from_v8_test!(runtime, "[1.0, 2.0]", |scope, result| {
      assert_eq!(
        <(Number<f64>, Number<f64>)>::from_v8(scope, result).unwrap(),
        (Number(1.0), Number(2.0))
      )
    });

    from_v8_test!(runtime, "[1.0, 2.0, 3.0, undefined]", |scope, result| {
      assert_eq!(
        <(
          Number<f64>,
          Number<f64>,
          OptionNull::<Number<f64>>,
          OptionUndefined::<bool>
        )>::from_v8(scope, result)
        .unwrap(),
        (
          Number(1.0),
          Number(2.0),
          OptionNull(Some(Number(3.0))),
          OptionUndefined(None)
        )
      )
    });

    from_v8_test!(runtime, "[1.0]", |scope, result| {
      let err = <(Number<f64>, bool)>::from_v8(scope, result).unwrap_err();
      assert!(
        err.to_string().contains("Expected 2 elements"),
        "expected length mismatch error, got: {}",
        err
      );
    });
  }

  #[test]
  fn test_vec() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| vec![Number(1.0), Number(2.0)].to_v8(scope).unwrap(),
      "equal(test_fn(), [1.0, 2.0])"
    );

    from_v8_test!(runtime, "[1.0, 2.0]", |scope, result| {
      assert_eq!(
        <Vec<Number<f64>>>::from_v8(scope, result).unwrap(),
        vec![Number(1.0), Number(2.0)]
      )
    });

    to_v8_test!(
      runtime,
      |scope, _args| Vec::<Number<f64>>::new().to_v8(scope).unwrap(),
      "equal(test_fn(), [])"
    );

    from_v8_test!(runtime, "[]", |scope, result| {
      let v = <Vec<Number<f64>>>::from_v8(scope, result).unwrap();
      assert_eq!(v, vec![]);
    });

    // Test with nested Vec
    to_v8_test!(
      runtime,
      |scope, _args| vec![vec![Number(1.0), Number(2.0)], vec![Number(3.0)]]
        .to_v8(scope)
        .unwrap(),
      "equal(test_fn(), [[1.0,2.0],[3.0]])"
    );
    from_v8_test!(runtime, "[[1.0,2.0],[3.0]]", |scope, result| {
      let v = <Vec<Vec<Number<f64>>>>::from_v8(scope, result).unwrap();
      assert_eq!(v, vec![vec![Number(1.0), Number(2.0)], vec![Number(3.0)]]);
    });

    // Test Vec<Option<T>>
    to_v8_test!(
      runtime,
      |scope, _args| vec![Some(Number(1.0)), None, Some(Number(2.0))]
        .to_v8(scope)
        .unwrap(),
      "equal(test_fn(), [1.0, null, 2.0])"
    );
    from_v8_test!(runtime, "[1.0, undefined, 2.0]", |scope, result| {
      let v = <Vec<Option<Number<f64>>>>::from_v8(scope, result).unwrap();
      assert_eq!(v, vec![Some(Number(1.0)), None, Some(Number(2.0))]);
    });

    // Test failure case: element conversion error
    from_v8_test!(runtime, "[1.0, 'notanumber']", |scope, result| {
      let err = <Vec<Number<f64>>>::from_v8(scope, result).unwrap_err();
      let err = err.get_ref().downcast_ref::<DataError>().unwrap();
      assert_eq!(
        err,
        &DataError(v8::DataError::BadType {
          actual: "string",
          expected: "f64",
        })
      );
    });
  }

  #[test]
  fn test_uint8array() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| Uint8Array::from(vec![1, 2, 3]).to_v8(scope).unwrap(),
      "equal(test_fn(), new Uint8Array([1, 2, 3]))"
    );

    to_v8_test!(
      runtime,
      |scope, _args| Uint8Array::from(Vec::<u8>::new()).to_v8(scope).unwrap(),
      "equal(test_fn(), new Uint8Array([]))"
    );

    from_v8_test!(runtime, "new Uint8Array([1, 2, 3])", |scope, result| {
      assert_eq!(
        *<Uint8Array as FromV8>::from_v8(scope, result).unwrap(),
        vec![1u8, 2, 3]
      )
    });

    from_v8_test!(runtime, "new Uint8Array([])", |scope, result| {
      assert_eq!(
        *<Uint8Array as FromV8>::from_v8(scope, result).unwrap(),
        Vec::<u8>::new()
      )
    });

    from_v8_test!(
      runtime,
      "new Uint8Array([1, 2, 3, 4, 5]).subarray(2)",
      |scope, result| {
        assert_eq!(
          *<Uint8Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![3u8, 4, 5]
        )
      }
    );

    from_v8_test!(
      runtime,
      "new Uint8Array([1, 2, 3, 4, 5]).subarray(1, 4)",
      |scope, result| {
        assert_eq!(
          *<Uint8Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![2u8, 3, 4]
        )
      }
    );

    from_v8_test!(
      runtime,
      "new Uint8Array([1, 2, 3]).subarray(2, 2)",
      |scope, result| {
        assert_eq!(
          *<Uint8Array as FromV8>::from_v8(scope, result).unwrap(),
          Vec::<u8>::new()
        )
      }
    );
  }

  #[test]
  fn test_uint16array() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| Uint16Array::from(vec![1, 2, 3]).to_v8(scope).unwrap(),
      "equal(test_fn(), new Uint16Array([1, 2, 3]))"
    );

    from_v8_test!(runtime, "new Uint16Array([1, 2, 3])", |scope, result| {
      assert_eq!(
        *<Uint16Array as FromV8>::from_v8(scope, result).unwrap(),
        vec![1u16, 2, 3]
      )
    });

    from_v8_test!(runtime, "new Uint16Array([])", |scope, result| {
      assert_eq!(
        *<Uint16Array as FromV8>::from_v8(scope, result).unwrap(),
        Vec::<u16>::new()
      )
    });

    from_v8_test!(
      runtime,
      "new Uint16Array([1, 2, 3, 4, 5]).subarray(2)",
      |scope, result| {
        assert_eq!(
          *<Uint16Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![3u16, 4, 5]
        )
      }
    );

    from_v8_test!(
      runtime,
      "new Uint16Array([1, 2, 3, 4, 5]).subarray(1, 4)",
      |scope, result| {
        assert_eq!(
          *<Uint16Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![2u16, 3, 4]
        )
      }
    );
  }

  #[test]
  fn test_uint32array() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| Uint32Array::from(vec![1, 2, 3]).to_v8(scope).unwrap(),
      "equal(test_fn(), new Uint32Array([1, 2, 3]))"
    );

    from_v8_test!(runtime, "new Uint32Array([1, 2, 3])", |scope, result| {
      assert_eq!(
        *<Uint32Array as FromV8>::from_v8(scope, result).unwrap(),
        vec![1u32, 2, 3]
      )
    });

    from_v8_test!(runtime, "new Uint32Array([])", |scope, result| {
      assert_eq!(
        *<Uint32Array as FromV8>::from_v8(scope, result).unwrap(),
        Vec::<u32>::new()
      )
    });

    from_v8_test!(
      runtime,
      "new Uint32Array([1, 2, 3, 4, 5]).subarray(2)",
      |scope, result| {
        assert_eq!(
          *<Uint32Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![3u32, 4, 5]
        )
      }
    );

    from_v8_test!(
      runtime,
      "new Uint32Array([1, 2, 3, 4, 5]).subarray(1, 4)",
      |scope, result| {
        assert_eq!(
          *<Uint32Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![2u32, 3, 4]
        )
      }
    );
  }

  #[test]
  fn test_int32array() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| Int32Array::from(vec![1, 2, 3]).to_v8(scope).unwrap(),
      "equal(test_fn(), new Int32Array([1, 2, 3]))"
    );

    from_v8_test!(runtime, "new Int32Array([1, 2, 3])", |scope, result| {
      assert_eq!(
        *<Int32Array as FromV8>::from_v8(scope, result).unwrap(),
        vec![1, 2, 3]
      )
    });

    from_v8_test!(runtime, "new Int32Array([])", |scope, result| {
      assert_eq!(
        *<Int32Array as FromV8>::from_v8(scope, result).unwrap(),
        Vec::<i32>::new()
      )
    });

    from_v8_test!(
      runtime,
      "new Int32Array([1, 2, 3, 4, 5]).subarray(2)",
      |scope, result| {
        assert_eq!(
          *<Int32Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![3i32, 4, 5]
        )
      }
    );

    from_v8_test!(
      runtime,
      "new Int32Array([-1, -2, -3, -4, -5]).subarray(1, 4)",
      |scope, result| {
        assert_eq!(
          *<Int32Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![-2i32, -3, -4]
        )
      }
    );
  }

  #[test]
  fn test_biguint64array() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| BigUint64Array::from(vec![1, 2, 3]).to_v8(scope).unwrap(),
      "equal(test_fn(), new BigUint64Array([1n, 2n, 3n]))"
    );

    from_v8_test!(
      runtime,
      "new BigUint64Array([1n, 2n, 3n])",
      |scope, result| {
        assert_eq!(
          *<BigUint64Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![1u64, 2, 3]
        )
      }
    );

    from_v8_test!(
      runtime,
      "new BigUint64Array([1n, 2n, 3n, 4n, 5n]).subarray(2)",
      |scope, result| {
        assert_eq!(
          *<BigUint64Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![3u64, 4, 5]
        )
      }
    );

    from_v8_test!(
      runtime,
      "new BigUint64Array([1n, 2n, 3n, 4n, 5n]).subarray(1, 4)",
      |scope, result| {
        assert_eq!(
          *<BigUint64Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![2u64, 3, 4]
        )
      }
    );
  }

  #[test]
  fn test_bigint64array() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| BigInt64Array::from(vec![1, 2, 3]).to_v8(scope).unwrap(),
      "equal(test_fn(), new BigInt64Array([1n, 2n, 3n]))"
    );

    from_v8_test!(
      runtime,
      "new BigInt64Array([1n, 2n, 3n])",
      |scope, result| {
        assert_eq!(
          *<BigInt64Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![1, 2, 3]
        )
      }
    );

    from_v8_test!(runtime, "new BigInt64Array([])", |scope, result| {
      assert_eq!(
        *<BigInt64Array as FromV8>::from_v8(scope, result).unwrap(),
        Vec::<i64>::new()
      )
    });

    from_v8_test!(
      runtime,
      "new BigInt64Array([1n, 2n, 3n, 4n, 5n]).subarray(2)",
      |scope, result| {
        assert_eq!(
          *<BigInt64Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![3i64, 4, 5]
        )
      }
    );

    from_v8_test!(
      runtime,
      "new BigInt64Array([-1n, -2n, -3n, -4n, -5n]).subarray(1, 4)",
      |scope, result| {
        assert_eq!(
          *<BigInt64Array as FromV8>::from_v8(scope, result).unwrap(),
          vec![-2i64, -3, -4]
        )
      }
    );
  }

  #[test]
  fn derive_struct() {
    #[derive(deno_ops::FromV8, deno_ops::ToV8, Eq, PartialEq, Clone, Debug)]
    pub struct Struct {
      a: u8,
      #[from_v8(default = Some(3))]
      c: Option<u32>,
      #[v8(rename = "e")]
      f: String,
      #[v8(serde)]
      b: HashMap<String, u32>,
    }

    let mut runtime = JsRuntime::new(Default::default());
    deno_core::scope!(scope, runtime);

    let value = Struct {
      a: 242,
      c: Some(102),
      f: "foo".to_string(),
      b: Default::default(),
    };
    let to = ToV8::to_v8(value.clone(), scope).unwrap();
    let from = FromV8::from_v8(scope, to).unwrap();
    assert_eq!(value, from);
  }

  #[test]
  fn derive_from_struct() {
    #[derive(deno_ops::FromV8, Eq, PartialEq, Clone, Debug)]
    pub struct Struct {
      a: u8,
      #[from_v8(default = Some(3))]
      c: Option<u32>,
      #[v8(rename = "e")]
      f: String,
      #[v8(serde)]
      b: HashMap<String, u32>,
    }

    let mut runtime = JsRuntime::new(Default::default());
    let val = runtime
      .execute_script("", "({ a: 1, c: 70000, e: 'foo', b: { 'bar': 1 } })")
      .unwrap();

    let val2 = runtime
      .execute_script("", "({ a: 1, e: 'foo', b: {} })")
      .unwrap();

    deno_core::scope!(scope, runtime);

    let val = Local::new(scope, val);
    let from = Struct::from_v8(scope, val).unwrap();
    assert_eq!(
      from,
      Struct {
        a: 1,
        c: Some(70000),
        f: "foo".to_string(),
        b: HashMap::from([("bar".to_string(), 1)]),
      }
    );

    let val2 = Local::new(scope, val2);
    let from = Struct::from_v8(scope, val2).unwrap();
    assert_eq!(
      from,
      Struct {
        a: 1,
        c: Some(3),
        f: "foo".to_string(),
        b: HashMap::default(),
      }
    );
  }

  #[test]
  fn derive_from_tuple_struct() {
    #[derive(deno_ops::FromV8, Eq, PartialEq, Clone, Debug)]
    pub struct Tuple(u8, String);
    #[derive(deno_ops::FromV8, Eq, PartialEq, Clone, Debug)]
    pub struct TupleSingle(u8);

    let mut runtime = JsRuntime::new(Default::default());
    let val = runtime.execute_script("", "([1, 'foo'])").unwrap();
    let val2 = runtime.execute_script("", "(1)").unwrap();

    deno_core::scope!(scope, runtime);
    let val = Local::new(scope, val);

    let from = Tuple::from_v8(scope, val).unwrap();
    assert_eq!(from, Tuple(1, "foo".to_string()));

    let val2 = Local::new(scope, val2);

    let from = TupleSingle::from_v8(scope, val2).unwrap();
    assert_eq!(from, TupleSingle(1));
  }

  #[test]
  fn derive_to_struct() {
    #[derive(deno_ops::ToV8, Eq, PartialEq, Clone, Debug)]
    pub struct Struct {
      a: u8,
      c: Option<u32>,
      #[v8(rename = "e")]
      f: String,
      #[v8(serde)]
      b: HashMap<String, u32>,
    }

    let mut runtime = JsRuntime::new(Default::default());
    deno_core::scope!(scope, runtime);

    let from = ToV8::to_v8(
      Struct {
        a: 1,
        c: Some(70000),
        f: "foo".to_string(),
        b: HashMap::from([("bar".to_string(), 1)]),
      },
      scope,
    )
    .unwrap();
    let obj = from.cast::<v8::Object>();
    let key = v8::String::new(scope, "a").unwrap();
    assert_eq!(
      obj
        .get(scope, key.into())
        .unwrap()
        .number_value(scope)
        .unwrap(),
      1.0
    );
    let key = v8::String::new(scope, "c").unwrap();
    assert_eq!(
      obj
        .get(scope, key.into())
        .unwrap()
        .number_value(scope)
        .unwrap(),
      70000.0
    );
    let key = v8::String::new(scope, "e").unwrap();
    assert_eq!(
      obj
        .get(scope, key.into())
        .unwrap()
        .to_rust_string_lossy(scope),
      "foo"
    );
    let key = v8::String::new(scope, "b").unwrap();
    let record_key = v8::String::new(scope, "bar").unwrap();
    assert_eq!(
      obj
        .get(scope, key.into())
        .unwrap()
        .cast::<v8::Object>()
        .get(scope, record_key.into())
        .unwrap()
        .number_value(scope)
        .unwrap(),
      1.0
    );

    let from = ToV8::to_v8(
      Struct {
        a: 1,
        c: Some(3),
        f: "foo".to_string(),
        b: HashMap::default(),
      },
      scope,
    )
    .unwrap();
    let obj = from.cast::<v8::Object>();
    let key = v8::String::new(scope, "a").unwrap();
    assert_eq!(
      obj
        .get(scope, key.into())
        .unwrap()
        .number_value(scope)
        .unwrap(),
      1.0
    );
    let key = v8::String::new(scope, "c").unwrap();
    assert_eq!(
      obj
        .get(scope, key.into())
        .unwrap()
        .number_value(scope)
        .unwrap(),
      3.0
    );
    let key = v8::String::new(scope, "e").unwrap();
    assert_eq!(
      obj
        .get(scope, key.into())
        .unwrap()
        .to_rust_string_lossy(scope),
      "foo"
    );
    let key = v8::String::new(scope, "b").unwrap();
    assert!(
      obj
        .get(scope, key.into())
        .unwrap()
        .try_cast::<v8::Object>()
        .is_ok()
    );
  }

  #[test]
  fn derive_to_tuple_struct() {
    #[derive(deno_ops::ToV8, Eq, PartialEq, Clone, Debug)]
    pub struct Tuple(u8, String);
    #[derive(deno_ops::ToV8, Eq, PartialEq, Clone, Debug)]
    pub struct TupleSingle(u8);

    let mut runtime = JsRuntime::new(Default::default());
    deno_core::scope!(scope, runtime);

    let from = ToV8::to_v8(Tuple(1, "foo".to_string()), scope).unwrap();
    let arr = from.cast::<v8::Array>();
    assert_eq!(
      arr
        .get_index(scope, 0)
        .unwrap()
        .number_value(scope)
        .unwrap(),
      1.0
    );
    assert_eq!(
      arr.get_index(scope, 1).unwrap().to_rust_string_lossy(scope),
      "foo"
    );

    let from = ToV8::to_v8(TupleSingle(1), scope).unwrap();
    assert_eq!(from.number_value(scope).unwrap(), 1.0);
  }

  #[test]
  fn test_bigint() {
    let mut runtime = JsRuntime::new(Default::default());

    to_v8_test!(
      runtime,
      |scope, _args| BigInt {
        sign_bit: false,
        words: vec![42],
      }
      .to_v8(scope)
      .unwrap(),
      "test_fn() === 42n"
    );

    to_v8_test!(
      runtime,
      |scope, _args| BigInt {
        sign_bit: true,
        words: vec![42],
      }
      .to_v8(scope)
      .unwrap(),
      "test_fn() === -42n"
    );

    to_v8_test!(
      runtime,
      |scope, _args| BigInt {
        sign_bit: false,
        words: vec![0],
      }
      .to_v8(scope)
      .unwrap(),
      "test_fn() === 0n"
    );

    to_v8_test!(
      runtime,
      |scope, _args| BigInt {
        sign_bit: false,
        words: vec![0, 1],
      }
      .to_v8(scope)
      .unwrap(),
      "test_fn() === 18446744073709551616n"
    );

    from_v8_test!(runtime, "42n", |scope, result| {
      let bigint = BigInt::from_v8(scope, result).unwrap();
      assert!(!bigint.sign_bit);
      assert_eq!(bigint.words, vec![42]);
    });

    from_v8_test!(runtime, "-42n", |scope, result| {
      let bigint = BigInt::from_v8(scope, result).unwrap();
      assert!(bigint.sign_bit);
      assert_eq!(bigint.words, vec![42]);
    });

    from_v8_test!(runtime, "0n", |scope, result| {
      let bigint = BigInt::from_v8(scope, result).unwrap();
      assert!(!bigint.sign_bit);
      assert!(bigint.words.is_empty());
    });

    from_v8_test!(runtime, "18446744073709551616n", |scope, result| {
      let bigint = BigInt::from_v8(scope, result).unwrap();
      assert!(!bigint.sign_bit);
      assert_eq!(bigint.words, vec![0, 1]);
    });

    from_v8_test!(
      runtime,
      "123456789012345678901234567890n",
      |scope, result| {
        let bigint = BigInt::from_v8(scope, result).unwrap();
        let to_v8 = bigint.to_v8(scope).unwrap();
        let back = BigInt::from_v8(scope, to_v8).unwrap();

        assert!(!back.sign_bit);
        assert!(!back.words.is_empty());
      }
    );
  }
}
