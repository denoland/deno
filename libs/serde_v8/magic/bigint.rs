// Copyright 2018-2025 the Deno authors. MIT license.

use std::ops::Deref;
use std::ops::DerefMut;

use smallvec::SmallVec;
use smallvec::smallvec;

use super::transl8::FromV8;
use super::transl8::ToV8;
use crate::Error;
use crate::magic::transl8::impl_magic;

#[derive(PartialEq, Eq, Clone, Debug, Default)]
pub struct BigInt(num_bigint::BigInt);
impl_magic!(BigInt);

impl Deref for BigInt {
  type Target = num_bigint::BigInt;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl DerefMut for BigInt {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl ToV8 for BigInt {
  fn to_v8<'scope, 'i>(
    &self,
    scope: &mut v8::PinScope<'scope, 'i>,
  ) -> Result<v8::Local<'scope, v8::Value>, crate::Error> {
    let (sign, words) = self.0.to_u64_digits();
    let sign_bit = sign == num_bigint::Sign::Minus;
    let v = v8::BigInt::new_from_words(scope, sign_bit, &words).unwrap();
    Ok(v.into())
  }
}

impl FromV8 for BigInt {
  fn from_v8<'scope, 'i>(
    _scope: &mut v8::PinScope,
    value: v8::Local<'scope, v8::Value>,
  ) -> Result<Self, crate::Error> {
    let v8bigint = v8::Local::<v8::BigInt>::try_from(value)
      .map_err(|_| Error::ExpectedBigInt(value.type_repr()))?;
    let word_count = v8bigint.word_count();
    let mut words: SmallVec<[u64; 1]> = smallvec![0u64; word_count];
    let (sign_bit, _words) = v8bigint.to_words_array(&mut words);
    let sign = match sign_bit {
      true => num_bigint::Sign::Minus,
      false => num_bigint::Sign::Plus,
    };
    // SAFETY: Because the alignment of u64 is 8, the alignment of u32 is 4, and
    // the size of u64 is 8, the size of u32 is 4, the alignment of u32 is a
    // factor of the alignment of u64, and the size of u32 is a factor of the
    // size of u64, we can safely transmute the slice of u64 to a slice of u32.
    let (prefix, slice, suffix) = unsafe { words.align_to::<u32>() };
    assert!(prefix.is_empty());
    assert!(suffix.is_empty());
    assert_eq!(slice.len(), words.len() * 2);
    let big_int = num_bigint::BigInt::from_slice(sign, slice);
    Ok(Self(big_int))
  }
}

impl From<num_bigint::BigInt> for BigInt {
  fn from(big_int: num_bigint::BigInt) -> Self {
    Self(big_int)
  }
}

impl From<BigInt> for num_bigint::BigInt {
  fn from(big_int: BigInt) -> Self {
    big_int.0
  }
}
