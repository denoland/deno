use deno::AnyError as DenoAnyError;
use std::any::{Any, TypeId};
use std::convert::From;
use std::error::Error;
use std::fmt;
use std::ops::Deref;

pub trait GetErrorKind: Error {
  fn kind(&self) -> &str;
}

impl GetErrorKind for dyn DenoAnyError {
  fn kind(&self) -> &str {
    "UnkownErrorKind"
  }
}

impl GetErrorKind for serde_json::error::Error {
  fn kind(&self) -> &str {
    use serde_json::error::*;
    match self.classify() {
      Category::Io => "InvalidInput",
      Category::Syntax => "InvalidInput",
      Category::Data => "InvalidData",
      Category::Eof => "UnexpectedEof",
    }
  }
}

// The Send and Sync traits are required because deno is multithreaded and we
// need to beable to handle errors across threads.
pub trait JsonAnyError:
  Any + GetErrorKind + Error + Send + Sync + 'static
{
}
impl<T> JsonAnyError for T where
  T: Any + GetErrorKind + Error + Send + Sync + Sized + 'static
{
}

#[derive(Debug)]
pub struct JsonErrBox(Box<dyn JsonAnyError>);

impl dyn JsonAnyError {
  pub fn downcast_ref<T: JsonAnyError>(&self) -> Option<&T> {
    if Any::type_id(self) == TypeId::of::<T>() {
      let target = self as *const Self as *const T;
      let target = unsafe { &*target };
      Some(target)
    } else {
      None
    }
  }
}

impl JsonErrBox {
  pub fn downcast<T: JsonAnyError>(self) -> Result<T, Self> {
    if Any::type_id(&*self.0) == TypeId::of::<T>() {
      let target = Box::into_raw(self.0) as *mut T;
      let target = unsafe { Box::from_raw(target) };
      Ok(*target)
    } else {
      Err(self)
    }
  }
}

impl AsRef<dyn JsonAnyError> for JsonErrBox {
  fn as_ref(&self) -> &dyn JsonAnyError {
    self.0.as_ref()
  }
}

impl Deref for JsonErrBox {
  type Target = Box<dyn JsonAnyError>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: JsonAnyError> From<T> for JsonErrBox {
  fn from(error: T) -> Self {
    Self(Box::new(error))
  }
}

impl From<Box<dyn JsonAnyError>> for JsonErrBox {
  fn from(boxed: Box<dyn JsonAnyError>) -> Self {
    Self(boxed)
  }
}

impl fmt::Display for JsonErrBox {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.0.fmt(f)
  }
}

impl Error for JsonErrBox {}
