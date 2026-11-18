// Copyright 2018-2026 the Deno authors. MIT license.
// Forked from Gotham:
// https://github.com/gotham-rs/gotham/blob/bcbbf8923789e341b7a0e62c59909428ca4e22e2/gotham/src/state/mod.rs
// Copyright 2017 Gotham Project Developers. MIT license.

use std::any::Any;
use std::any::TypeId;
use std::any::type_name;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::hash::Hasher;

/// A pass-through hasher for [`TypeId`] keys.
///
/// `TypeId`'s own `Hash` impl feeds already well-distributed bits (the low 64
/// bits of a 128-bit compiler-generated hash) straight into the hasher, so
/// there is nothing left to mix: hashing them again only costs cycles. This
/// hasher therefore forwards those bits verbatim.
///
/// Only the integer entry points a `TypeId` can plausibly use are specialized;
/// `write` keeps a cheap byte fold as a correctness backstop in case a future
/// standard library hashes `TypeId` some other way. It must never panic, and
/// it must stay deterministic for a given byte sequence.
#[derive(Default)]
pub struct TypeIdHasher(u64);

impl Hasher for TypeIdHasher {
  #[inline(always)]
  fn write_u64(&mut self, i: u64) {
    self.0 = i;
  }

  #[inline(always)]
  fn write_u128(&mut self, i: u128) {
    // Fold the two halves together; either half alone is a valid hash of a
    // `TypeId`, but xor keeps both in play.
    self.0 = (i as u64) ^ ((i >> 64) as u64);
  }

  #[inline(always)]
  fn finish(&self) -> u64 {
    self.0
  }

  #[inline]
  fn write(&mut self, bytes: &[u8]) {
    // FxHash-style fold. Not expected to be reached with a `TypeId` key.
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    for &b in bytes {
      self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(SEED);
    }
  }
}

type TypeIdMap =
  HashMap<TypeId, Box<dyn Any>, BuildHasherDefault<TypeIdHasher>>;

#[derive(Default)]
pub struct GothamState {
  data: TypeIdMap,
}

impl GothamState {
  /// Puts a value into the `GothamState` storage. One value of each type is retained.
  /// Successive calls to `put` will overwrite the existing value of the same
  /// type.
  pub fn put<T: 'static>(&mut self, t: T) {
    let type_id = TypeId::of::<T>();
    self.data.insert(type_id, Box::new(t));
  }

  /// Determines if the current value exists in `GothamState` storage.
  pub fn has<T: 'static>(&self) -> bool {
    let type_id = TypeId::of::<T>();
    self.data.contains_key(&type_id)
  }

  /// Tries to borrow a value from the `GothamState` storage.
  pub fn try_borrow<T: 'static>(&self) -> Option<&T> {
    let type_id = TypeId::of::<T>();
    self.data.get(&type_id).and_then(|b| b.downcast_ref())
  }

  /// Borrows a value from the `GothamState` storage.
  pub fn borrow<T: 'static>(&self) -> &T {
    self.try_borrow().unwrap_or_else(|| missing::<T>())
  }

  /// Tries to mutably borrow a value from the `GothamState` storage.
  pub fn try_borrow_mut<T: 'static>(&mut self) -> Option<&mut T> {
    let type_id = TypeId::of::<T>();
    self.data.get_mut(&type_id).and_then(|b| b.downcast_mut())
  }

  /// Mutably borrows a value from the `GothamState` storage.
  pub fn borrow_mut<T: 'static>(&mut self) -> &mut T {
    self.try_borrow_mut().unwrap_or_else(|| missing::<T>())
  }

  /// Tries to move a value out of the `GothamState` storage and return ownership.
  pub fn try_take<T: 'static>(&mut self) -> Option<T> {
    let type_id = TypeId::of::<T>();
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
    self.try_take().unwrap_or_else(|| missing::<T>())
  }
}

fn missing<T: 'static>() -> ! {
  panic!(
    "required type {} is not present in GothamState container",
    type_name::<T>()
  );
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

  #[test]
  fn many_types() {
    // Guards against a degenerate hasher: a large number of distinct types
    // must all round-trip, and each must return its own value.
    struct T<const N: usize>(usize);

    macro_rules! put_and_check {
      ($($n:literal),*) => {
        let mut state = GothamState::default();
        $(
          state.put(T::<$n>($n));
        )*
        $(
          assert_eq!(state.borrow::<T<$n>>().0, $n);
        )*
      };
    }
    put_and_check!(
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
      21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31
    );
  }

  #[test]
  #[should_panic(
    expected = "required type deno_core::gotham_state::tests::MyStruct is not present in GothamState container"
  )]
  fn missing() {
    let state = GothamState::default();
    let _ = state.borrow::<MyStruct>();
  }
}
