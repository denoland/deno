// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::magic::transl8::magic_deserialize;
use crate::magic::transl8::magic_serialize;
use crate::magic::transl8::FromV8;
use crate::magic::transl8::MagicType;
use crate::magic::transl8::ToV8;
use std::borrow::Borrow;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::forget;
use std::mem::transmute;
use std::mem::ManuallyDrop;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::ptr::NonNull;
use std::rc::Rc;
use std::rc::Weak;

// A Resource is a Rust object managed by the V8 GC.
pub struct Resource<T> {
  inner: Option<Rc<T>>,
  shared_access: bool, // Determine if the Rc<T> should be dropped.
  _marker: PhantomData<T>,
}

impl<T> MagicType for Resource<T> {
  const NAME: &'static str = "Resource";
  const MAGIC_NAME: &'static str = "$__v8_magic_Resource";
}

impl<T> serde::Serialize for Resource<T> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    magic_serialize(serializer, self)
  }
}

impl<'de, T> serde::Deserialize<'de> for Resource<T> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    magic_deserialize(deserializer)
  }
}

impl<T> Resource<T> {
  const INTERNAL_FIELD_INDEX: usize = 0;

  /// Create a new Resource. `Resource` leaks the `Rc<T>` reference, and keeps its
  /// `Weak<T>` handle. The `Rc<T>` is dropped when the `Resource` is garbage collected.
  ///
  /// On the other hand, if the strong count of `Rc<T>` is zero,
  pub fn new_boxed(value: T) -> Self {
    let rc = Rc::new(value);
    Resource {
      inner: Some(rc),
      shared_access: false, // This is the owning resource. Don't drop it.
      _marker: PhantomData,
    }
  }

  pub fn into_inner(mut self) -> Option<T> {
    let rc = self.inner.take()?;
    let ptr = Rc::into_raw(rc);
    let rc = unsafe { Rc::from_raw(ptr) };

    // Rust needs exclusive access to the Rc<T>.
    // We pretend to drop the Rc<T> held by the finalizer.
    // `try_unwrap` takes care of pending `Rc`s.
    if Rc::strong_count(&rc) >= 2 {
      unsafe {
        // Leaked Rc<T> in finalizer.
        Rc::decrement_strong_count(ptr);
      }
    }
    Rc::try_unwrap(rc).ok()
  }

  pub fn borrow(mut self) -> Rc<T> {
    let rc = self.inner.take().unwrap();
    let ptr = Rc::into_raw(rc);
    let rc = unsafe { Rc::from_raw(ptr) };
    if Rc::strong_count(&rc) == 1 {
      // We cannot let the Rc<T> drop.
      unsafe {
        Rc::increment_strong_count(ptr);
      }
    }
    rc
  }
}

impl<T> ToV8 for Resource<T> {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let tpl = v8::ObjectTemplate::new(scope);
    assert!(tpl.set_internal_field_count(1));

    let rc = self.inner.clone().unwrap();
    let ptr = Rc::into_raw(rc) as *mut c_void;

    let field = v8::External::new(scope, ptr);
    let wrap = tpl.new_instance(scope).unwrap();
    assert!(wrap.set_internal_field(Self::INTERNAL_FIELD_INDEX, field.into()));

    let mut raw_weak = MaybeUninit::uninit();
    let weak = v8::Weak::with_finalizer(
      scope,
      wrap,
      // finalizer
      Box::new(move |isolate| {
        // SAFETY: We own this object, no other resource can hold the pointer
        // to it. Here, we say bye-bye to the object.
        dbg!("Gc called!");
        unsafe {
          let _weak = v8::Weak::from_raw(isolate, Some(raw_weak.assume_init()));
          let _ = Rc::from_raw(ptr as *const T);
        }
      }),
    );
    let value = weak.to_local(scope).unwrap().into();
    let weak_raw = weak.into_raw().unwrap();
    raw_weak.write(weak_raw);

    Ok(value)
  }
}

impl<T> FromV8 for Resource<T> {
  fn from_v8(
    scope: &mut v8::HandleScope,
    value: v8::Local<v8::Value>,
  ) -> Result<Self, crate::Error> {
    let obj = v8::Local::<v8::Object>::try_from(value).unwrap();
    assert_eq!(obj.internal_field_count(), 1);
    let external = obj
      .get_internal_field(scope, Self::INTERNAL_FIELD_INDEX)
      .unwrap();
    let ptr = v8::Local::<v8::External>::try_from(external).unwrap();

    let inner = unsafe { Rc::from_raw(ptr.value() as *const _) };
    Ok(Resource {
      inner: Some(inner),
      shared_access: true, // This is a shared resource. Safe to decrement the strong count of Rc<T>.
      _marker: PhantomData,
    })
  }
}
