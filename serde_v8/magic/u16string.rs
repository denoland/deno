use crate::magic::transl8::impl_magic;
use crate::Error;
use std::ops::{Deref, DerefMut};

use super::transl8::{FromV8, ToV8};

#[derive(Default, PartialEq, Eq, Debug)]
pub struct U16String(pub Vec<u16>);
impl_magic!(U16String);

impl U16String {
  pub fn with_zeroes(length: usize) -> U16String {
    U16String(vec![0u16; length])
  }

  pub fn truncate(&mut self, new_length: usize) {
    self.0.truncate(new_length);
    self.0.shrink_to_fit()
  }
}

impl Deref for U16String {
  type Target = [u16];
  fn deref(&self) -> &[u16] {
    self.0.deref()
  }
}

impl DerefMut for U16String {
  fn deref_mut(&mut self) -> &mut [u16] {
    self.0.deref_mut()
  }
}

impl AsRef<[u16]> for U16String {
  fn as_ref(&self) -> &[u16] {
    self.0.as_ref()
  }
}

impl AsMut<[u16]> for U16String {
  fn as_mut(&mut self) -> &mut [u16] {
    self.0.as_mut()
  }
}

impl ToV8 for U16String {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let v =
      v8::String::new_from_two_byte(scope, self, v8::NewStringType::Normal)
        .unwrap();
    Ok(v.into())
  }
}

impl FromV8 for U16String {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let v8str = v8::Local::<v8::String>::try_from(value)
      .map_err(|_| Error::ExpectedString)?;
    let len = v8str.length();
    let mut buffer = Vec::with_capacity(len);
    // SAFETY: we set length == capacity (see previous line),
    // before immediately writing into that buffer and sanity check with an assert
    #[allow(clippy::uninit_vec)]
    unsafe {
      buffer.set_len(len);
      let written = v8str.write(
        scope,
        &mut buffer,
        0,
        v8::WriteOptions::NO_NULL_TERMINATION,
      );
      assert!(written == len);
    }
    Ok(U16String(buffer))
  }
}
