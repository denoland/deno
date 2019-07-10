use std::any::{Any, TypeId};
use std::convert::From;
use std::error::Error;
use std::fmt;
use std::ops::Deref;

// The Send and Sync traits are required because deno is multithreaded and we
// need to beable to handle errors across threads.
pub trait AnyError: Any + Error + Send + Sync + 'static {}
impl<T> AnyError for T where T: Any + Error + Send + Sync + Sized + 'static {}

#[derive(Debug)]
pub struct ErrBox(Box<dyn AnyError>);

impl dyn AnyError {
  pub fn downcast_ref<T: AnyError>(&self) -> Option<&T> {
    if Any::type_id(self) == TypeId::of::<T>() {
      let target = self as *const Self as *const T;
      let target = unsafe { &*target };
      Some(target)
    } else {
      None
    }
  }
}

impl ErrBox {
  pub fn downcast<T: AnyError>(self) -> Result<T, Self> {
    if Any::type_id(&*self.0) == TypeId::of::<T>() {
      let target = Box::into_raw(self.0) as *mut T;
      let target = unsafe { Box::from_raw(target) };
      Ok(*target)
    } else {
      Err(self)
    }
  }
}

impl AsRef<dyn AnyError> for ErrBox {
  fn as_ref(&self) -> &dyn AnyError {
    self.0.as_ref()
  }
}

impl Deref for ErrBox {
  type Target = Box<AnyError>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: AnyError> From<T> for ErrBox {
  fn from(error: T) -> Self {
    Self(Box::new(error))
  }
}

impl From<Box<dyn AnyError>> for ErrBox {
  fn from(boxed: Box<dyn AnyError>) -> Self {
    Self(boxed)
  }
}

impl fmt::Display for ErrBox {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    self.0.fmt(f)
  }
}
