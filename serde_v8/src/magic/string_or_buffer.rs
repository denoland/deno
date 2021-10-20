use std::ops::Deref;

#[derive(Debug)]
pub struct StringOrBuffer(Vec<u8>);

impl Deref for StringOrBuffer {
  type Target = Vec<u8>;
  fn deref(&self) -> &Vec<u8> {
    &self.0
  }
}

impl<'de> serde::Deserialize<'de> for StringOrBuffer {
  fn deserialize<D>(deserializer: D) -> Result<StringOrBuffer, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    StringOrBufferInner::deserialize(deserializer)
      .map(|x| StringOrBuffer(x.into_bytes()))
  }
}

// TODO(@AaronO): explore if we can make this work with ZeroCopyBuf
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum StringOrBufferInner {
  String(String),
  Buffer(Vec<u8>),
}

impl StringOrBufferInner {
  fn into_bytes(self) -> Vec<u8> {
    match self {
      Self::String(s) => s.into_bytes(),
      Self::Buffer(b) => b,
    }
  }
}
