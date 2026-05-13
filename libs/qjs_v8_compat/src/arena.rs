// Copyright 2018-2026 the Deno authors. MIT license.
//
// Mock-backend arena.
//
// Used only when `link_quickjs` is *off*. Stores refcounted "values" so
// that the GC translation in `scope`/`value` can be exercised by unit
// tests without a real QuickJS runtime linked in.
//
// Each entry has a tag, a refcount, and (optionally) a payload. When the
// refcount hits zero the entry is dropped. The runtime panics on free if
// any entry is still live, surfacing leaks loudly.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[derive(Debug, Clone, PartialEq)]
pub enum MockTag {
  Object,
  Array,
  String,
  Symbol,
  BigInt,
  Function,
  Promise,
  Module,
  ArrayBuffer,
}

#[derive(Debug)]
pub struct MockJSValue {
  pub tag: MockTag,
  pub refcount: u32,
  pub payload: Option<Vec<u8>>,
  pub label: Option<String>,
  /// Numbered properties on objects/arrays — kept simple, not real JS.
  pub indexed: Vec<u64>,
  /// Named properties on objects.
  pub named: HashMap<String, u64>,
}

pub struct Arena {
  next: AtomicU64,
  entries: HashMap<u64, MockJSValue>,
  /// Total of all `alloc_*` calls, useful for leak diagnosis.
  total_allocs: u64,
  total_frees: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ArenaStats {
  pub live: usize,
  pub total_allocs: u64,
  pub total_frees: u64,
}

impl Arena {
  pub fn new() -> Self {
    Self {
      // start at 1: zero is reserved as the null/uninit handle.
      next: AtomicU64::new(1),
      entries: HashMap::new(),
      total_allocs: 0,
      total_frees: 0,
    }
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }
  pub fn live_count(&self) -> usize {
    self.entries.len()
  }
  pub fn stats(&self) -> ArenaStats {
    ArenaStats {
      live: self.entries.len(),
      total_allocs: self.total_allocs,
      total_frees: self.total_frees,
    }
  }

  fn alloc(&mut self, mut v: MockJSValue) -> u64 {
    v.refcount = 1;
    let id = self.next.fetch_add(1, Ordering::SeqCst);
    self.entries.insert(id, v);
    self.total_allocs += 1;
    id
  }

  pub fn alloc_object(&mut self) -> u64 {
    self.alloc(MockJSValue {
      tag: MockTag::Object,
      refcount: 1,
      payload: None,
      label: None,
      indexed: Vec::new(),
      named: HashMap::new(),
    })
  }

  pub fn alloc_array(&mut self) -> u64 {
    self.alloc(MockJSValue {
      tag: MockTag::Array,
      refcount: 1,
      payload: None,
      label: None,
      indexed: Vec::new(),
      named: HashMap::new(),
    })
  }

  pub fn alloc_string(&mut self, s: &str) -> u64 {
    self.alloc(MockJSValue {
      tag: MockTag::String,
      refcount: 1,
      payload: Some(s.as_bytes().to_vec()),
      label: None,
      indexed: Vec::new(),
      named: HashMap::new(),
    })
  }

  pub fn alloc_symbol(&mut self, desc: Option<&str>) -> u64 {
    self.alloc(MockJSValue {
      tag: MockTag::Symbol,
      refcount: 1,
      payload: None,
      label: desc.map(|s| s.to_owned()),
      indexed: Vec::new(),
      named: HashMap::new(),
    })
  }

  pub fn alloc_function(&mut self, name: &str) -> u64 {
    self.alloc(MockJSValue {
      tag: MockTag::Function,
      refcount: 1,
      payload: None,
      label: Some(name.to_owned()),
      indexed: Vec::new(),
      named: HashMap::new(),
    })
  }

  pub fn alloc_promise(&mut self) -> u64 {
    self.alloc(MockJSValue {
      tag: MockTag::Promise,
      refcount: 1,
      payload: None,
      label: None,
      indexed: Vec::new(),
      named: HashMap::new(),
    })
  }

  pub fn alloc_array_buffer(&mut self, bytes: Vec<u8>) -> u64 {
    self.alloc(MockJSValue {
      tag: MockTag::ArrayBuffer,
      refcount: 1,
      payload: Some(bytes),
      label: None,
      indexed: Vec::new(),
      named: HashMap::new(),
    })
  }

  pub fn dup(&mut self, h: u64) {
    if let Some(e) = self.entries.get_mut(&h) {
      e.refcount = e.refcount.saturating_add(1);
    } else if h != 0 {
      panic!(
        "qjs_v8_compat mock: JS_DupValue on unknown handle {} \
         (likely use-after-free)",
        h
      );
    }
  }

  pub fn free(&mut self, h: u64) {
    if h == 0 {
      return;
    }
    let drop = if let Some(e) = self.entries.get_mut(&h) {
      e.refcount = e.refcount.saturating_sub(1);
      e.refcount == 0
    } else {
      panic!(
        "qjs_v8_compat mock: JS_FreeValue on unknown handle {} \
         (double free or use-after-free)",
        h
      );
    };
    if drop {
      // Take ownership of the entry so we can recursively decrement the
      // refcounts on the children it owned (property values, array slots).
      // QuickJS does this transitively via JS_FreeValueRT; we mirror.
      let entry = self.entries.remove(&h).unwrap();
      self.total_frees += 1;
      // Collect child handles before recursing — gives a deterministic
      // free order independent of HashMap iteration.
      let children: Vec<u64> = entry
        .indexed
        .iter()
        .copied()
        .chain(entry.named.values().copied())
        .filter(|h| *h != 0)
        .collect();
      for c in children {
        self.free(c);
      }
    }
  }

  pub fn refcount(&self, h: u64) -> Option<u32> {
    self.entries.get(&h).map(|e| e.refcount)
  }

  pub fn tag(&self, h: u64) -> Option<MockTag> {
    self.entries.get(&h).map(|e| e.tag.clone())
  }

  pub fn string_value(&self, h: u64) -> Option<String> {
    self
      .entries
      .get(&h)
      .and_then(|e| e.payload.as_ref())
      .and_then(|p| std::str::from_utf8(p).ok().map(|s| s.to_owned()))
  }

  pub fn set_indexed(&mut self, obj: u64, idx: usize, val: u64) {
    if let Some(e) = self.entries.get_mut(&obj) {
      if idx >= e.indexed.len() {
        e.indexed.resize(idx + 1, 0);
      }
      e.indexed[idx] = val;
    }
  }

  pub fn get_indexed(&self, obj: u64, idx: usize) -> Option<u64> {
    self
      .entries
      .get(&obj)
      .and_then(|e| e.indexed.get(idx).copied())
  }

  pub fn set_named(&mut self, obj: u64, name: &str, val: u64) {
    if let Some(e) = self.entries.get_mut(&obj) {
      e.named.insert(name.to_owned(), val);
    }
  }

  pub fn get_named(&self, obj: u64, name: &str) -> Option<u64> {
    self
      .entries
      .get(&obj)
      .and_then(|e| e.named.get(name))
      .copied()
  }

  pub fn delete_named(&mut self, obj: u64, name: &str) -> bool {
    if let Some(e) = self.entries.get_mut(&obj) {
      e.named.remove(name).is_some()
    } else {
      false
    }
  }

  /// Snapshot of an object's named entries in deterministic (key, handle)
  /// order. Used by the bytecode-cache serializer.
  pub fn entries_named(&self, obj: u64) -> Vec<(String, u64)> {
    let Some(e) = self.entries.get(&obj) else {
      return Vec::new();
    };
    let mut out: Vec<(String, u64)> =
      e.named.iter().map(|(k, v)| (k.clone(), *v)).collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
  }
}

impl Default for Arena {
  fn default() -> Self {
    Self::new()
  }
}
