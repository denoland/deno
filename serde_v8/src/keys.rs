// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use fxhash::FxHashMap;

use std::cell::RefCell;
use std::hash::Hash;
use std::hash::Hasher;
use std::intrinsics::transmute;
use std::rc::Rc;

// KeyCache stores a pool struct keys mapped to v8,
// to minimize allocs and speed up decoding/encoding `v8::Object`s.
#[derive(Default)]
pub struct KeyCache(FxHashMap<StaticKey, v8::Global<v8::String>>);

// The `StaticKey` wraps a statically allocated &'str. The purpose of this is to
// compare and hash the string's address rather than its contents.
#[derive(Copy, Clone, Debug, Eq)]
struct StaticKey(pub &'static str);

impl StaticKey {
  #[inline]
  fn address(&self) -> usize {
    self.0.as_ptr() as _
  }
}

impl PartialEq for StaticKey {
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.address() == other.address()
  }
}

impl Hash for StaticKey {
  #[inline]
  fn hash<H: Hasher>(&self, state: &mut H) {
    // Hash the pointer rather than the string contents.
    state.write_usize(self.address())
  }
}

// creates an optimized v8::String for a struct field
// TODO: experiment with external strings
// TODO: evaluate if own KeyCache is better than v8's dedupe
pub fn v8_struct_key<'s>(
  scope: &mut v8::HandleScope<'s>,
  key: &'static str,
  key_cache: &mut KeyCache,
) -> v8::Local<'s, v8::String> {
  let key_global = key_cache.0.entry(StaticKey(key)).or_insert_with(|| {
    let key_local = v8::String::new_from_utf8(
      scope,
      key.as_ref(),
      v8::NewStringType::Internalized,
    )
    .unwrap();
    v8::Global::new(scope, key_local)
  });

  // The safe, but rather inefficient, way to do this is:
  // ```
  // v8::Local::new(scope, key_global.to_owned())
  // ```
  // TODO(piscisaureus): allow `v8::Local::new()` to accept a `&Global` by
  // reference instead of an owned `Global`.
  unsafe {
    let key_ref = key_global.open(scope);
    let key_pseudo_local = transmute::<_, v8::Local<v8::String>>(key_ref);
    v8::Local::new(scope, key_pseudo_local)
  }

  // TODO: consider external strings later
  // right now non-deduped external strings (without KeyCache)
  // are slower than the deduped internalized strings by ~2.5x
  // since they're a new string in v8's eyes and needs to be hashed, etc...
  // v8::String::new_external_onebyte_static(scope, field).unwrap()
}

pub(crate) fn get_key_cache(
  isolate: &mut v8::Isolate,
) -> Rc<RefCell<KeyCache>> {
  match isolate.get_slot::<Rc<RefCell<KeyCache>>>() {
    Some(key_cache_rc) => key_cache_rc.clone(),
    None => {
      let key_cache_rc = Rc::new(RefCell::new(KeyCache::default()));
      isolate.set_slot(key_cache_rc.clone());
      key_cache_rc
    }
  }
}

pub fn clear_key_cache(isolate: &mut v8::Isolate) {
  isolate.remove_slot::<Rc<RefCell<KeyCache>>>();
}
