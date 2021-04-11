// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// Forked from Gotham:
// https://github.com/gotham-rs/gotham/blob/bcbbf8923789e341b7a0e62c59909428ca4e22e2/gotham/src/state/mod.rs
// Copyright 2017 Gotham Project Developers. MIT license.

use log::trace;
use std::any::Any;
use std::any::TypeId;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct GothamState {
  data: BTreeMap<TypeId, Box<dyn Any>>,
}

impl GothamState {
  /// Puts a value into the `GothamState` storage. One value of each type is retained.
  /// Successive calls to `put` will overwrite the existing value of the same
  /// type.
  pub fn put<T: 'static>(&mut self, t: T) {
    let type_id = TypeId::of::<T>();
    trace!(" inserting record to state for type_id `{:?}`", type_id);
    self.data.insert(type_id, Box::new(t));
  }

  /// Determines if the current value exists in `GothamState` storage.
  pub fn has<T: 'static>(&self) -> bool {
    let type_id = TypeId::of::<T>();
    self.data.get(&type_id).is_some()
  }

  /// Tries to borrow a value from the `GothamState` storage.
  pub fn try_borrow<T: 'static>(&self) -> Option<&T> {
    let type_id = TypeId::of::<T>();
    trace!(" borrowing state data for type_id `{:?}`", type_id);
    self.data.get(&type_id).and_then(|b| b.downcast_ref())
  }

  /// Borrows a value from the `GothamState` storage.
  pub fn borrow<T: 'static>(&self) -> &T {
    self
      .try_borrow()
      .expect("required type is not present in GothamState container")
  }

  /// Tries to mutably borrow a value from the `GothamState` storage.
  pub fn try_borrow_mut<T: 'static>(&mut self) -> Option<&mut T> {
    let type_id = TypeId::of::<T>();
    trace!(" mutably borrowing state data for type_id `{:?}`", type_id);
    self.data.get_mut(&type_id).and_then(|b| b.downcast_mut())
  }

  /// Mutably borrows a value from the `GothamState` storage.
  pub fn borrow_mut<T: 'static>(&mut self) -> &mut T {
    self
      .try_borrow_mut()
      .expect("required type is not present in GothamState container")
  }

  /// Tries to move a value out of the `GothamState` storage and return ownership.
  pub fn try_take<T: 'static>(&mut self) -> Option<T> {
    let type_id = TypeId::of::<T>();
    trace!(
      " taking ownership from state data for type_id `{:?}`",
      type_id
    );
    self
      .data
      .remove(&type_id)
      .and_then(|b| b.downcast().ok())
      .map(|b| *b)
  }

  /// Moves a value out of the `GothamState` storage and returns ownership.
  ///
  /// # Panics
  ///
  /// If a value of type `T` is not present in `GothamState`.
  pub fn take<T: 'static>(&mut self) -> T {
    self
      .try_take()
      .expect("required type is not present in GothamState container")
  }
}

#[cfg(test)]
mod tests {
  use super::GothamState;

  struct MyStruct {
    value: i32,
  }

  struct AnotherStruct {
    value: &'static str,
  }

  type Alias1 = String;
  type Alias2 = String;

  #[test]
  fn put_borrow1() {
    let mut state = GothamState::default();
    state.put(MyStruct { value: 1 });
    assert_eq!(state.borrow::<MyStruct>().value, 1);
  }

  #[test]
  fn put_borrow2() {
    let mut state = GothamState::default();
    assert!(!state.has::<AnotherStruct>());
    state.put(AnotherStruct { value: "a string" });
    assert!(state.has::<AnotherStruct>());
    assert!(!state.has::<MyStruct>());
    state.put(MyStruct { value: 100 });
    assert!(state.has::<MyStruct>());
    assert_eq!(state.borrow::<MyStruct>().value, 100);
    assert_eq!(state.borrow::<AnotherStruct>().value, "a string");
  }

  #[test]
  fn try_borrow() {
    let mut state = GothamState::default();
    state.put(MyStruct { value: 100 });
    assert!(state.try_borrow::<MyStruct>().is_some());
    assert_eq!(state.try_borrow::<MyStruct>().unwrap().value, 100);
    assert!(state.try_borrow::<AnotherStruct>().is_none());
  }

  #[test]
  fn try_borrow_mut() {
    let mut state = GothamState::default();
    state.put(MyStruct { value: 100 });
    if let Some(a) = state.try_borrow_mut::<MyStruct>() {
      a.value += 10;
    }
    assert_eq!(state.borrow::<MyStruct>().value, 110);
  }

  #[test]
  fn borrow_mut() {
    let mut state = GothamState::default();
    state.put(MyStruct { value: 100 });
    {
      let a = state.borrow_mut::<MyStruct>();
      a.value += 10;
    }
    assert_eq!(state.borrow::<MyStruct>().value, 110);
    assert!(state.try_borrow_mut::<AnotherStruct>().is_none());
  }

  #[test]
  fn try_take() {
    let mut state = GothamState::default();
    state.put(MyStruct { value: 100 });
    assert_eq!(state.try_take::<MyStruct>().unwrap().value, 100);
    assert!(state.try_take::<MyStruct>().is_none());
    assert!(state.try_borrow_mut::<MyStruct>().is_none());
    assert!(state.try_borrow::<MyStruct>().is_none());
    assert!(state.try_take::<AnotherStruct>().is_none());
  }

  #[test]
  fn take() {
    let mut state = GothamState::default();
    state.put(MyStruct { value: 110 });
    assert_eq!(state.take::<MyStruct>().value, 110);
    assert!(state.try_take::<MyStruct>().is_none());
    assert!(state.try_borrow_mut::<MyStruct>().is_none());
    assert!(state.try_borrow::<MyStruct>().is_none());
  }

  #[test]
  fn type_alias() {
    let mut state = GothamState::default();
    state.put::<Alias1>("alias1".to_string());
    state.put::<Alias2>("alias2".to_string());
    assert_eq!(state.take::<Alias1>(), "alias2");
    assert!(state.try_take::<Alias1>().is_none());
    assert!(state.try_take::<Alias2>().is_none());
  }
}
