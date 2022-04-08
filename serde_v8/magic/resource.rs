// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::magic::transl8::magic_deserialize;
use crate::magic::transl8::magic_serialize;
use crate::magic::transl8::FromV8;
use crate::magic::transl8::MagicType;
use crate::magic::transl8::ToV8;
use std::cell::Cell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

/// A Resource is a Rust object managed by the V8 GC.
/// `T` is reference counted using an `Rc<T>`.
/// When sent to V8, the Resource will be wrapped in a JavaScript object.
/// The JavaScript object will hold a reference to the Rust object.
///
/// The underlying Rc<T> will always have a strong count >= 1 until either
/// the JavaScript object is garbage collected OR `into_inner` is called.
pub struct Resource<T> {
  inner: Option<Rc<T>>,
  cancel_finalization: Rc<Cell<bool>>,
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
  // TODO(@littledivy): Merge both fields into one v8::External.
  const INTERNAL_FIELD_INDEX: usize = 0;
  const CANCEL_FINALIZATION_FIELD_INDEX: usize = 1;

  /// Create a new Resource.
  pub fn new(value: T) -> Self {
    let rc = Rc::new(value);
    Resource {
      inner: Some(rc),
      cancel_finalization: Rc::new(Cell::new(false)),
      _marker: PhantomData,
    }
  }

  /// Returns the underlying owned value held by the Resource.
  /// This will return None if the Resource is already in use.
  pub fn into_inner(mut self) -> Option<T> {
    let rc = self.inner.take()?;
    let ptr = Rc::into_raw(rc);
    // SAFETY: Rc is immediately constructed from it's raw pointer.
    let rc = unsafe { Rc::from_raw(ptr) };

    // Rust wants exclusive access to the Rc<T>.
    // We have to abort then finalization callback.
    // TODO(@littledivy): Prevent finalizer to drop the Rc<T>.

    // Here, we signal the finalizer that it must not drop the Rc
    // and decrement its strong count.
    //
    // `try_unwrap` takes care of pending `Rc`s.
    if Rc::strong_count(&rc) != 1 {
      // SAFETY: The Rc pointer is valid. It is also guaranteed that the finalizer
      // increments the strong count and drops at finalization.
      unsafe {
        Rc::decrement_strong_count(ptr);
      }
    }

    match Rc::try_unwrap(rc) {
      Ok(value) => {
        // Cancel the finalizer.
        self.cancel_finalization.set(true);
        Some(value)
      }
      Err(_) => None,
    }
  }

  pub fn borrow(mut self) -> Rc<T> {
    let rc = self.inner.take().unwrap();
    let ptr = Rc::into_raw(rc);
    // SAFETY: Rc is immediately constructed from it's raw pointer.
    let rc = unsafe { Rc::from_raw(ptr) };
    if Rc::strong_count(&rc) == 1 {
      // SAFETY: We cannot let the Rc<T> drop.
      // TODO(@littledivy): Verify this doesn't cause any side effects!
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
    assert!(
      tpl.set_internal_field_count(2),
      "set_internal_field_count(2) failed"
    );

    let rc = self.inner.clone().unwrap();
    let ptr = Rc::into_raw(rc) as *mut c_void;

    let field = v8::External::new(scope, ptr);
    let wrap = tpl.new_instance(scope).unwrap();
    assert!(
      wrap.set_internal_field(Self::INTERNAL_FIELD_INDEX, field.into()),
      "set_internal_field(0) failed"
    );
    let cancel_field = v8::External::new(
      scope,
      Rc::into_raw(self.cancel_finalization.clone()) as *mut c_void,
    );
    assert!(
      wrap.set_internal_field(
        Self::CANCEL_FINALIZATION_FIELD_INDEX,
        cancel_field.into()
      ),
      "set_internal_field(1) failed"
    );

    let raw_weak: Rc<Cell<NonNull<_>>> =
      Rc::new(Cell::new(NonNull::dangling()));
    let raw_weak_clone = raw_weak.clone();
    let cancel_finalization = self.cancel_finalization.clone();
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
        if cancel_finalization.get() {
          // Rust tells us to prevent dropping the Rc. 
          // It decrements the strong count and is the sole owner of the resource data.
          return;
        }
        // SAFETY: We own the Rc<T>, no other Resource can hold the pointer
        // to it. Here, we say bye-bye to the object.
        unsafe {
          let _ = Rc::from_raw(ptr as *const T);
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
    assert_eq!(
      obj.internal_field_count(),
      2,
      "internal_field_count() != 2"
    );
    let external = obj
      .get_internal_field(scope, Self::INTERNAL_FIELD_INDEX)
      .unwrap();
    let ptr = v8::Local::<v8::External>::try_from(external).unwrap();
    let cancel_external = obj
      .get_internal_field(scope, Self::CANCEL_FINALIZATION_FIELD_INDEX)
      .unwrap();
    let cancel_ptr =
      v8::Local::<v8::External>::try_from(cancel_external).unwrap();

    // SAFETY: The internal field of this Object is a valid External pointer to the Rc<T>.
    let inner = unsafe { Rc::from_raw(ptr.value() as *const _) };
    let cancel_finalization =
      unsafe { Rc::from_raw(cancel_ptr.value() as *const _) };
    Ok(Resource {
      inner: Some(inner),
      cancel_finalization,
      _marker: PhantomData,
    })
  }
}
