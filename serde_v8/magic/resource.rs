// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use crate::magic::transl8::magic_deserialize;
use crate::magic::transl8::magic_serialize;
use crate::magic::transl8::FromV8;
use crate::magic::transl8::MagicType;
use crate::magic::transl8::ToV8;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::forget;
use std::mem::transmute;
use std::rc::Rc;
use std::borrow::Borrow;

pub struct Resource<T> {
  pub wrap: Rc<T>,
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

  pub fn new_boxed(value: T) -> Self {
    Resource {
      wrap: Rc::new(value),
      _marker: PhantomData,
    }
  }
  
  pub fn borrow(self) -> Rc<T> {
    self.wrap.clone()
  }
}

impl<T> ToV8 for Resource<T> {
  fn to_v8<'a>(
    &self,
    scope: &mut v8::HandleScope<'a>,
  ) -> Result<v8::Local<'a, v8::Value>, crate::Error> {
    let tpl = v8::ObjectTemplate::new(scope);
    assert!(tpl.set_internal_field_count(1));

    let ptr = Rc::into_raw(self.wrap.clone()) as *mut c_void;
    let field = v8::External::new(scope, ptr);
    let wrap = tpl.new_instance(scope).unwrap();
    assert!(wrap.set_internal_field(Self::INTERNAL_FIELD_INDEX, field.into()));
    let weak = v8::Weak::with_finalizer(
      scope,
      wrap,
      // finalizer
      Box::new(move || {
        // SAFETY: We own this object, no other resource can hold the pointer
        // to it. Here, we say bye-bye to the object.
        println!("Gc called!");
        unsafe {
          let _ = Rc::from_raw(ptr);
        }
      }),
    );
    Ok(weak.to_local(scope).unwrap().into())
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
    let ptr = unsafe {
      transmute::<v8::Local<v8::Value>, v8::Local<v8::External>>(external)
    };
    Ok(Resource {
      wrap: unsafe { Rc::from_raw(ptr.value() as *const _) },
      _marker: PhantomData,
    })
  }
}
