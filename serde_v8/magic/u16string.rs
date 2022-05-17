use super::transl8::{impl_magic, impl_wrapper, FromV8, ToV8};
use crate::Error;

impl_wrapper!(
  pub struct U16String(Vec<u16>);
);
impl_magic!(U16String);

impl ToV8 for U16String {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let maybe_v =
      v8::String::new_from_two_byte(scope, self, v8::NewStringType::Normal);

    // 'new_from_two_byte' can return 'None' if buffer length > kMaxLength.
    if let Some(v) = maybe_v {
      Ok(v.into())
    } else {
      Err(Error::Message(String::from(
        "Cannot allocate String from UTF-16: buffer exceeds maximum length.",
      )))
    }
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
    Ok(buffer.into())
  }
}
