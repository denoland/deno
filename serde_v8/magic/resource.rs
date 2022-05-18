// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::magic::transl8::magic_deserialize;
use crate::magic::transl8::magic_serialize;
use crate::magic::transl8::FromV8;
use crate::magic::transl8::MagicType;
use crate::magic::transl8::ToV8;
use std::cell::Cell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::forget;
use std::ptr::NonNull;
use std::rc::Rc;

/// A Resource is a Rust object managed by the V8 GC.
/// `T` is reference counted using an `Rc<T>`.
/// When sent to V8, the Resource will be wrapped in a JavaScript object.
/// The JavaScript object will hold a reference to the Rust object.
///
/// The underlying Rc<T> will always have a strong count >= 1 until either
/// the JavaScript object is garbage collected OR `into_inner` is called.
pub struct Resource<T: ?Sized> {
  inner: Option<Rc<T>>,
  _marker: PhantomData<T>,
  from_v8: bool,
}

impl<T: ?Sized> Drop for Resource<T> {
  fn drop(&mut self) {
    if let Some(inner) = self.inner.take() {
      if self.from_v8 {
        forget(inner); // Don't drop.
      }
    }
  }
}

impl<T: ?Sized> MagicType for Resource<T> {
  const NAME: &'static str = "Resource";
  const MAGIC_NAME: &'static str = "$__v8_magic_Resource";
}

impl<T: ?Sized> serde::Serialize for Resource<T> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    magic_serialize(serializer, self)
  }
}

impl<'de, T: ?Sized> serde::Deserialize<'de> for Resource<T> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    magic_deserialize(deserializer)
  }
}

impl<T: ?Sized> Resource<T> {
  const INTERNAL_FIELD_INDEX: usize = 0;

  pub fn borrow(&self) -> Rc<T> {
    self.inner.clone().unwrap()
  }
}

impl<T> Resource<T> {
  /// Create a new Resource.
  pub fn new(value: T) -> Self {
    Resource {
      inner: Some(Rc::new(value)),
      _marker: PhantomData,
      from_v8: false,
    }
  }

  /// Returns the underlying owned value held by the Resource.
  /// This will return None if the Resource is already in use.
  pub fn into_inner(mut self) -> Option<T> {
    let rc = self.inner.take().unwrap();
    match Rc::try_unwrap(rc) {
      Ok(value) => Some(value),
      Err(_) => None,
    }
  }
}

fn try_close_callback<'a, 's, T>(
  _scope: &mut v8::HandleScope<'a>,
  args: v8::FunctionCallbackArguments<'a>,
  _rv: v8::ReturnValue<'s>,
) {
  let _resource = args.this();
}

impl<T: 'static> ToV8 for Resource<T> {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let tpl = v8::ObjectTemplate::new(scope);
    assert!(
      tpl.set_internal_field_count(1),
      "set_internal_field_count(1) failed"
    );

    let func_name = v8::String::new(scope, "close").unwrap();
    let func =
      v8::FunctionBuilder::<'_, v8::Function>::new(try_close_callback::<T>)
        .build(scope)
        .unwrap();

    let rc = self.inner.clone().unwrap();
    let weak_ptr = Rc::downgrade(&rc);
    let rc_ptr = Rc::into_raw(rc) as *mut c_void;

    let field = v8::External::new(scope, rc_ptr);
    let wrap = tpl.new_instance(scope).unwrap();
    assert!(
      wrap.set_internal_field(Self::INTERNAL_FIELD_INDEX, field.into()),
      "set_internal_field(0) failed"
    );
    wrap.set(scope, func_name.into(), func.into()).unwrap();

    let raw_weak: Rc<Cell<NonNull<_>>> =
      Rc::new(Cell::new(NonNull::dangling()));
    let raw_weak_clone = raw_weak.clone();
    let weak = v8::Weak::with_finalizer(
      scope,
      wrap,
      // finalizer
      Box::new(move |isolate| {
        // SAFETY: 1. The finalizer is guaranteed by V8 to run on the isolate thread.
        // 2. The backing memory for WeakData is initialized immediately after callback is registered.
        // 3. The second-pass callback calls finalizer before attempting to drop the WeakData.
        unsafe {
          // Mark this weak as dropped, so the WeakData can be dropped by the second-pass callback.
          let raw_weak = raw_weak_clone.get();
          let _weak = v8::Weak::from_raw(isolate, Some(raw_weak));
        }
        if let Some(rc) = weak_ptr.upgrade() {
          drop(rc);
          // To debug: println!("works 😭");
        }
      }),
    );
    let value = weak.to_local(scope).unwrap().into();
    // Leak and initialize memory.
    let weak_raw = weak.into_raw().unwrap();
    raw_weak.set(weak_raw);
    Ok(value)
  }
}

impl<T> FromV8 for Resource<T> {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let obj = v8::Local::<v8::Object>::try_from(value).unwrap();
    assert_eq!(obj.internal_field_count(), 1, "internal_field_count() != 1");
    let external = obj
      .get_internal_field(scope, Self::INTERNAL_FIELD_INDEX)
      .unwrap();
    let ptr = v8::Local::<v8::External>::try_from(external).unwrap();

    let inner = unsafe { Rc::from_raw(ptr.value() as *const _) };

    Ok(Resource {
      inner: Some(inner),
      _marker: PhantomData,
      from_v8: true,
    })
  }
}
