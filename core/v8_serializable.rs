use rusty_v8 as v8;

/// V8Serializable exists to allow boxing values as "objects" to be serialized later,
/// this is particularly useful for async op-responses. This trait is a more efficient
/// replacement for erased-serde that makes less allocations, since it's specific to serde_v8
/// (and thus doesn't have to have generic outputs, etc...)
pub trait V8Serializable {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, serde_v8::Error>;
}

/// Allows all implementors of `serde::Serialize` to implement V8Serializable
impl<T: serde::Serialize> V8Serializable for T {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, serde_v8::Error> {
    serde_v8::to_v8(scope, self)
  }
}
