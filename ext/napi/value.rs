// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::v8;
use std::mem::transmute;
use std::ops::Deref;
use std::os::raw::c_void;
use std::ptr::NonNull;

/// An FFI-opaque, nullable wrapper around v8::Local<v8::Value>.
/// rusty_v8 Local handle cannot be empty but napi_value can be.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct NapiValue<'s>(
  Option<NonNull<v8::Value>>,
  std::marker::PhantomData<&'s ()>,
);

pub type napi_value<'s> = NapiValue<'s>;

impl<'s> Deref for napi_value<'s> {
  type Target = Option<v8::Local<'s, v8::Value>>;
  fn deref(&self) -> &Self::Target {
    // SAFETY: It is safe to transmute `Option<NonNull<T>>` to `Option<*const T>`.
    //         v8::Local guarantees that *const T is not null but napi_value *can* be null.
    unsafe { transmute::<&Self, &Self::Target>(self) }
  }
}

impl<'s, T> From<v8::Local<'s, T>> for napi_value<'s>
where
  v8::Local<'s, T>: Into<v8::Local<'s, v8::Value>>,
{
  fn from(v: v8::Local<'s, T>) -> Self {
    // SAFETY: It is safe to cast v8::Local<T> that implements Into<v8::Local<v8::Value>>.
    //         `fn into(self)` transmutes anyways.
    Self(unsafe { transmute(v) }, std::marker::PhantomData)
  }
}

const _: () = {
  assert!(
    std::mem::size_of::<napi_value>() == std::mem::size_of::<*mut c_void>()
  );
  // Assert "nullable pointer optimization" on napi_value
  unsafe {
    type Src<'a> = napi_value<'a>;
    type Dst = usize;
    assert!(std::mem::size_of::<Src>() == std::mem::size_of::<Dst>());
    union Transmute<'a> {
      src: Src<'a>,
      dst: Dst,
    }
    Transmute {
      src: NapiValue(None, std::marker::PhantomData),
    }
    .dst
  };
};
