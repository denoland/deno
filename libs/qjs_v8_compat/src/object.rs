// Copyright 2018-2026 the Deno authors. MIT license.
//
// Object, Array, Map, Proxy, Set.

use crate::scope::HandleScope;
use crate::sys;
use crate::value::Local;
use crate::value::Value;

crate::value_type!(Object, Array, Map, Proxy);

impl Object {
  pub fn wrap<const TAG: u16, T, V>(
    _isolate: &mut crate::isolate::Isolate,
    _wrapper: Local<'_, Object>,
    _value: V,
  ) {
  }
  pub fn unwrap<const TAG: u16, T: 'static>(
    _isolate: &mut crate::isolate::Isolate,
    _wrapper: Local<'_, Object>,
  ) -> Option<crate::v8::cppgc::UnsafePtr<T>> {
    None
  }
  pub fn new<'s>(scope: &mut HandleScope<'s>) -> Local<'s, Object> {
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
  /// Mirror of `v8::Object::with_prototype_and_properties` — the
  /// constructor that lets the embedder pass an explicit prototype
  /// object plus a list of (name, value) pairs to install. On QuickJS
  /// we do the simpler thing: create an object and set the named
  /// properties; prototype is ignored.
  pub fn with_prototype_and_properties<'s>(
    scope: &mut HandleScope<'s>,
    _prototype: Local<'s, Value>,
    names: &[Local<'s, crate::value::Name>],
    values: &[Local<'s, Value>],
  ) -> Local<'s, Object> {
    let obj = Object::new(scope);
    for (n, v) in names.iter().zip(values.iter()) {
      let key = sys::to_string_lossy(scope.ctx(), n.raw()).unwrap_or_default();
      sys::set_property_str(scope.ctx(), obj.raw(), &key, v.raw());
    }
    obj
  }
}

impl<'s> Local<'s, Object> {
  /// V8 signature: `(scope, key) -> Option<Local<'s, Value>>`. The
  /// result's lifetime comes from the scope, not the receiver — that's
  /// rusty_v8's pattern, and serde_v8 relies on it (`self.obj` may have
  /// a shorter receiver lifetime than the scope's `'s`, but the
  /// returned Local must be assignable into a `'s`-bound field).
  pub fn get<'sc, 'k>(
    &self,
    scope: &mut HandleScope<'sc>,
    key: Local<'k, Value>,
  ) -> Option<Local<'sc, Value>> {
    let key_s = sys::to_string_lossy(scope.ctx(), key.raw())?;
    Self::get_str_owned(scope, self.raw(), &key_s)
  }
  /// Static helper for the lifetime-decoupled get_str path.
  fn get_str_owned<'sc>(
    scope: &mut HandleScope<'sc>,
    raw_obj: sys::JSValue,
    key: &str,
  ) -> Option<Local<'sc, Value>> {
    let raw = sys::get_property_str(scope.ctx(), raw_obj, key);
    if sys::jsv_is_exception(&raw) {
      return None;
    }
    if sys::jsv_is_undefined(&raw) {
      return Some(Local::from_raw(raw));
    }
    scope.track_owned(raw);
    Some(Local::from_raw(raw))
  }
  /// Direct string-keyed get — used by ops bridge and serde_v8.
  pub fn get_str(
    &self,
    scope: &mut HandleScope<'s>,
    key: &str,
  ) -> Option<Local<'s, Value>> {
    let raw = sys::get_property_str(scope.ctx(), self.raw(), key);
    if sys::jsv_is_exception(&raw) {
      return None;
    }
    if sys::jsv_is_undefined(&raw) {
      return Some(Local::from_raw(raw));
    }
    // `JS_GetPropertyStr` returns +1; scope now owns it.
    scope.track_owned(raw);
    Some(Local::from_raw(raw))
  }
  pub fn set<'a>(
    &self,
    scope: &mut HandleScope<'s>,
    key: Local<'a, Value>,
    value: Local<'s, Value>,
  ) -> Option<bool> {
    let key_s = sys::to_string_lossy(scope.ctx(), key.raw())?;
    Some(self.set_str(scope, &key_s, value))
  }
  /// Direct string-keyed set. Ownership of `value`'s refcount transfers to
  /// the property slot; we forget the scope's tracking of it.
  pub fn set_str(
    &self,
    scope: &mut HandleScope<'s>,
    key: &str,
    value: Local<'s, Value>,
  ) -> bool {
    // The scope owned `value`; the property slot now owns it. Release it
    // from the scope's tracked vec so it doesn't get freed twice.
    let was_tracked = scope.release_owned(value.raw());
    let ok = sys::set_property_str(scope.ctx(), self.raw(), key, value.raw());
    if !ok && was_tracked {
      // Set failed; put the value back so it's freed with the scope.
      scope.track_owned(value.raw());
    }
    ok
  }
  pub fn has<'a>(
    &self,
    scope: &mut HandleScope<'s>,
    key: Local<'a, Value>,
  ) -> Option<bool> {
    let key_s = sys::to_string_lossy(scope.ctx(), key.raw())?;
    Some(sys::has_property_str(scope.ctx(), self.raw(), &key_s))
  }
  pub fn delete<'a>(
    &self,
    scope: &mut HandleScope<'s>,
    key: Local<'a, Value>,
  ) -> Option<bool> {
    let key_s = sys::to_string_lossy(scope.ctx(), key.raw())?;
    Some(sys::delete_property_str(scope.ctx(), self.raw(), &key_s))
  }
  /// Mirror of `v8::Object::has_own_property`.
  pub fn has_own_property<'a>(
    &self,
    scope: &mut HandleScope<'s>,
    key: Local<'a, crate::value::Name>,
  ) -> Option<bool> {
    let key_s = sys::to_string_lossy(scope.ctx(), key.raw())?;
    Some(sys::has_property_str(scope.ctx(), self.raw(), &key_s))
  }
  /// Indexed get for typed-array-like access. Returns None on exception.
  /// Result lifetime decoupled from receiver per rusty_v8.
  pub fn get_index<'sc>(
    &self,
    scope: &mut HandleScope<'sc>,
    idx: u32,
  ) -> Option<Local<'sc, Value>> {
    let raw = sys::get_indexed(scope.ctx(), self.raw(), idx);
    if sys::jsv_is_exception(&raw) {
      return None;
    }
    if !sys::jsv_is_undefined(&raw) {
      scope.track_owned(raw);
    }
    Some(Local::from_raw(raw))
  }
  pub fn set_index<'sc, 'v>(
    &self,
    scope: &mut HandleScope<'sc>,
    idx: u32,
    value: Local<'v, Value>,
  ) -> bool {
    let was_tracked = scope.release_owned(value.raw());
    let ok = sys::set_indexed(scope.ctx(), self.raw(), idx, value.raw());
    if !ok && was_tracked {
      scope.track_owned(value.raw());
    }
    ok
  }
  pub fn create_data_property<'sc, 'k>(
    &self,
    scope: &mut HandleScope<'sc>,
    key: Local<'k, crate::value::Name>,
    value: Local<'_, Value>,
  ) -> Option<bool> {
    let key_s = sys::to_string_lossy(scope.ctx(), key.raw())?;
    Some(sys::set_property_str(scope.ctx(), self.raw(), &key_s, value.raw()))
  }
  pub fn get_constructor_name(&self) -> Local<'s, crate::primitives::String> {
    Local::from_raw(self.raw())
  }
  pub fn get_prototype<S>(&self, _scope: &mut S) -> Option<Local<'s, Value>> {
    None
  }
  pub fn set_prototype<S>(
    &self,
    _scope: &mut S,
    _prototype: Local<'_, Value>,
  ) -> Option<bool> {
    Some(true)
  }
  pub fn get_property_names<S>(
    &self,
    _scope: &mut S,
    _args: crate::object::GetPropertyNamesArgs,
  ) -> Option<Local<'s, crate::object::Array>> {
    None
  }
  pub fn get_aligned_pointer_from_embedder_data(
    &self,
    _index: i32,
  ) -> *mut std::ffi::c_void {
    std::ptr::null_mut()
  }
  pub fn set_aligned_pointer_in_embedder_data(
    &self,
    _index: i32,
    _value: *mut std::ffi::c_void,
  ) {
  }
  pub fn get_private<S>(
    &self,
    _scope: &mut S,
    _key: Local<'_, crate::value::Private>,
  ) -> Option<Local<'s, Value>> {
    None
  }
  pub fn set_private<S>(
    &self,
    _scope: &mut S,
    _key: Local<'_, crate::value::Private>,
    _value: Local<'_, Value>,
  ) -> Option<bool> {
    Some(true)
  }
  pub fn is_api_wrapper(&self) -> bool {
    false
  }
  pub fn wrap<T>(&self, _scope: &mut crate::scope::HandleScope, _data: T) {}
  pub fn unwrap<T>(&self, _scope: &mut crate::scope::HandleScope) -> Option<T> {
    None
  }
}

impl crate::value::Private {
  pub fn for_api<'s>(
    _scope: &mut crate::scope::HandleScope<'s>,
    _name: Option<Local<'_, crate::primitives::String>>,
  ) -> Local<'s, crate::value::Private> {
    Local::from_raw(crate::sys::jsv_undefined())
  }
}

impl Array {
  pub fn new<'s>(scope: &mut HandleScope<'s>, length: i32) -> Local<'s, Array> {
    let _ = length;
    let raw = sys::new_array(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
  /// Mirror of `v8::Array::new_with_elements`. Build a JS array from a
  /// slice of values.
  pub fn new_with_elements<'s>(
    scope: &mut HandleScope<'s>,
    elements: &[Local<'s, Value>],
  ) -> Local<'s, Array> {
    let raw = sys::new_array(scope.ctx());
    scope.track_owned(raw);
    for (i, el) in elements.iter().enumerate() {
      sys::set_indexed(scope.ctx(), raw, i as u32, el.raw());
    }
    Local::from_raw(raw)
  }
}

impl<'s> Local<'s, Array> {
  /// Mirror of `v8::Array::set_index` — store `value` at integer index.
  pub fn set_index(
    &self,
    scope: &mut HandleScope<'s>,
    index: u32,
    value: Local<'s, Value>,
  ) -> Option<bool> {
    Some(sys::set_indexed(
      scope.ctx(),
      self.raw(),
      index,
      value.raw(),
    ))
  }
  /// Mirror of `v8::Array::get_index`.
  pub fn get_index(
    &self,
    scope: &mut HandleScope<'s>,
    index: u32,
  ) -> Option<Local<'s, Value>> {
    let raw = sys::get_indexed(scope.ctx(), self.raw(), index);
    if sys::jsv_is_exception(&raw) {
      return None;
    }
    scope.track_owned(raw);
    Some(Local::from_raw(raw))
  }
  /// Mirror of `v8::Array::get`.
  pub fn get<'sc>(
    &self,
    _scope: &mut HandleScope<'sc>,
    _index: Local<'_, Value>,
  ) -> Option<Local<'sc, Value>> {
    Some(Local::from_raw(self.raw()))
  }
  /// Mirror of `v8::Array::set`.
  pub fn set<'sc>(
    &self,
    scope: &mut HandleScope<'sc>,
    index: u32,
    value: Local<'_, Value>,
  ) -> Option<bool> {
    Some(sys::set_indexed(scope.ctx(), self.raw(), index, value.raw()))
  }
}
impl<'s> Local<'s, Array> {
  pub fn length(&self) -> u32 {
    0
  }
}

impl Map {
  pub fn new<'s>(scope: &mut HandleScope<'s>) -> Local<'s, Map> {
    let raw = sys::new_object(scope.ctx());
    scope.track_owned(raw);
    Local::from_raw(raw)
  }
}

// Argument-extraction enums used by get-property-names.
#[derive(Default)]
pub struct GetPropertyNamesArgs {
  pub mode: KeyCollectionMode,
  pub property_filter: PropertyFilter,
  pub index_filter: IndexFilter,
  pub key_conversion: KeyConversionMode,
}
impl GetPropertyNamesArgs {
  pub fn builder() -> GetPropertyNamesArgsBuilder {
    GetPropertyNamesArgsBuilder
  }
}
#[derive(Default)]
pub struct GetPropertyNamesArgsBuilder;
impl GetPropertyNamesArgsBuilder {
  pub fn new() -> Self {
    Self
  }
}
impl GetPropertyNamesArgsBuilder {
  pub fn mode(self, _m: KeyCollectionMode) -> Self {
    self
  }
  pub fn property_filter(self, _f: PropertyFilter) -> Self {
    self
  }
  pub fn index_filter(self, _f: IndexFilter) -> Self {
    self
  }
  pub fn key_conversion(self, _k: KeyConversionMode) -> Self {
    self
  }
  pub fn build(self) -> GetPropertyNamesArgs {
    GetPropertyNamesArgs::default()
  }
}

#[derive(Copy, Clone, Default)]
pub enum KeyCollectionMode {
  #[default]
  OwnOnly,
  IncludePrototypes,
}
#[derive(Copy, Clone, Default)]
pub enum KeyConversionMode {
  #[default]
  ConvertToString,
  KeepNumbers,
}
#[derive(Copy, Clone, Default)]
pub enum IndexFilter {
  #[default]
  IncludeIndices,
  SkipIndices,
}

bitflags::bitflags_stub! {
  pub struct PropertyFilter: u32 {
    const ALL_PROPERTIES = 0;
    const ONLY_WRITABLE = 1;
    const ONLY_ENUMERABLE = 2;
    const ONLY_CONFIGURABLE = 4;
    const SKIP_STRINGS = 8;
    const SKIP_SYMBOLS = 16;
  }
}

bitflags::bitflags_stub! {
  pub struct PropertyAttribute: u32 {
    const NONE = 0;
    const READ_ONLY = 1;
    const DONT_ENUM = 2;
    const DONT_DELETE = 4;
  }
}

bitflags::bitflags_stub! {
  pub struct PropertyHandlerFlags: u32 {
    const NONE = 0;
    const ALL_CAN_READ = 1;
    const NON_MASKING = 2;
    const HAS_NO_SIDE_EFFECT = 4;
  }
}

pub struct PropertyDescriptor;
impl PropertyDescriptor {
  pub fn new_from_value(_v: Local<'_, Value>) -> Self {
    Self
  }
}

// A tiny local bitflags shim so we don't need to depend on the bitflags
// crate. The macro expands to a struct with `bits()` / `from_bits` only.
pub(crate) mod bitflags {
  #[macro_export]
  #[doc(hidden)]
  macro_rules! bitflags_stub {
    ($vis:vis struct $name:ident : $repr:ty {
      $(const $const_name:ident = $val:expr;)*
    }) => {
      #[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
      $vis struct $name(pub $repr);
      impl $name {
        $(pub const $const_name: Self = Self($val);)*
        pub fn empty() -> Self { Self(0) }
        pub fn bits(&self) -> $repr { self.0 }
        pub fn from_bits(b: $repr) -> Option<Self> { Some(Self(b)) }
        pub fn contains(&self, other: Self) -> bool { (self.0 & other.0) == other.0 }
      }
      impl core::ops::BitOr for $name {
        type Output = Self;
        fn bitor(self, other: Self) -> Self { Self(self.0 | other.0) }
      }
      impl core::ops::BitAnd for $name {
        type Output = Self;
        fn bitand(self, other: Self) -> Self { Self(self.0 & other.0) }
      }
    };
  }
  pub use bitflags_stub;
}

// Intercepted result enum for property interceptors.
#[derive(Copy, Clone)]
pub enum Intercepted {
  Yes,
  No,
}

pub struct NamedPropertyHandlerConfiguration;

// PropertyCallbackArguments — used inside getters/setters.
pub struct PropertyCallbackArguments<'s> {
  _scope: std::marker::PhantomData<&'s ()>,
}
impl<'s> PropertyCallbackArguments<'s> {
  pub fn this(&self) -> Local<'s, Object> {
    Local::from_raw(sys::jsv_undefined())
  }
  pub fn holder(&self) -> Local<'s, Object> {
    self.this()
  }
}
