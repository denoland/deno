// Copyright 2018-2026 the Deno authors. MIT license.

//! Before/after measurement for the two ordered maps that sat on nearly every
//! op's path: the `ResourceTable` (was a `BTreeMap<ResourceId, _>`, now a
//! slab) and `GothamState` (was a `BTreeMap<TypeId, _>`, now an identity-hash
//! map).
//!
//! Both baselines are re-implemented here verbatim so the old and the new
//! shape are measured in the same binary, on the same machine, in one run —
//! `bencher` interleaves nothing, but running them back to back removes the
//! usual before/after drift between two builds.

#![allow(clippy::undocumented_unsafe_blocks, reason = "bench code")]

use std::any::Any;
use std::any::TypeId;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::rc::Rc;

use bencher::Bencher;
use bencher::benchmark_group;
use bencher::benchmark_main;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ResourceTable;

/// Number of live resources in the table for the "hot get" benchmarks. A real
/// server holds a handful of listeners plus two per in-flight connection.
const LIVE: u32 = 64;
/// Number of add/get/close cycles per iteration.
const CYCLES: u32 = 64;

struct Dummy;

impl Resource for Dummy {
  fn name(&self) -> Cow<'_, str> {
    "dummy".into()
  }
}

// ---------------------------------------------------------------------------
// Baseline: the pre-slab `ResourceTable`, reduced to the operations measured.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct BTreeResourceTable {
  index: BTreeMap<ResourceId, Rc<dyn Resource>>,
  next_rid: ResourceId,
}

impl BTreeResourceTable {
  fn add<T: Resource>(&mut self, resource: T) -> ResourceId {
    let rid = self.next_rid;
    let removed = self
      .index
      .insert(rid, Rc::new(resource) as Rc<dyn Resource>);
    assert!(removed.is_none());
    self.next_rid += 1;
    rid
  }

  fn get<T: Resource>(&self, rid: ResourceId) -> Option<Rc<T>> {
    self
      .index
      .get(&rid)
      .and_then(|rc| rc.downcast_rc::<T>())
      .cloned()
  }

  fn take<T: Resource>(&mut self, rid: ResourceId) -> Option<Rc<T>> {
    let resource = self.get::<T>(rid)?;
    self.index.remove(&rid);
    Some(resource)
  }
}

fn resource_add_get_close_btree(b: &mut Bencher) {
  let mut table = BTreeResourceTable::default();
  b.iter(|| {
    for _ in 0..CYCLES {
      let rid = table.add(Dummy);
      black_box(table.get::<Dummy>(rid).unwrap());
      black_box(table.take::<Dummy>(rid).unwrap());
    }
  });
}

fn resource_add_get_close_slab(b: &mut Bencher) {
  let mut table = ResourceTable::default();
  b.iter(|| {
    for _ in 0..CYCLES {
      let rid = table.add(Dummy);
      black_box(table.get::<Dummy>(rid).unwrap());
      black_box(table.take::<Dummy>(rid).unwrap());
    }
  });
}

fn resource_get_hot_btree(b: &mut Bencher) {
  let mut table = BTreeResourceTable::default();
  let rids = (0..LIVE).map(|_| table.add(Dummy)).collect::<Vec<_>>();
  b.iter(|| {
    for rid in &rids {
      black_box(table.get::<Dummy>(black_box(*rid)).unwrap());
    }
  });
}

fn resource_get_hot_slab(b: &mut Bencher) {
  let mut table = ResourceTable::default();
  let rids = (0..LIVE).map(|_| table.add(Dummy)).collect::<Vec<_>>();
  b.iter(|| {
    for rid in &rids {
      black_box(table.get::<Dummy>(black_box(*rid)).unwrap());
    }
  });
}

/// The shape of a busy server: a stable set of live resources, with one
/// resource churning through open/read/close on top of it.
fn resource_churn_btree(b: &mut Bencher) {
  let mut table = BTreeResourceTable::default();
  let rids = (0..LIVE).map(|_| table.add(Dummy)).collect::<Vec<_>>();
  b.iter(|| {
    let rid = table.add(Dummy);
    for existing in &rids {
      black_box(table.get::<Dummy>(black_box(*existing)).unwrap());
    }
    black_box(table.take::<Dummy>(rid).unwrap());
  });
}

fn resource_churn_slab(b: &mut Bencher) {
  let mut table = ResourceTable::default();
  let rids = (0..LIVE).map(|_| table.add(Dummy)).collect::<Vec<_>>();
  b.iter(|| {
    let rid = table.add(Dummy);
    for existing in &rids {
      black_box(table.get::<Dummy>(black_box(*existing)).unwrap());
    }
    black_box(table.take::<Dummy>(rid).unwrap());
  });
}

// ---------------------------------------------------------------------------
// Baseline: the pre-hash `GothamState`, reduced to the operations measured.
// ---------------------------------------------------------------------------

#[derive(Default)]
struct BTreeGothamState {
  data: BTreeMap<TypeId, Box<dyn Any>>,
}

impl BTreeGothamState {
  fn put<T: 'static>(&mut self, t: T) {
    self.data.insert(TypeId::of::<T>(), Box::new(t));
  }

  fn borrow<T: 'static>(&self) -> &T {
    self
      .data
      .get(&TypeId::of::<T>())
      .and_then(|b| b.downcast_ref())
      .unwrap()
  }
}

/// ~30 registered types is what a real `deno` runtime's `OpState` carries.
macro_rules! with_types {
  ($mac:ident) => {
    $mac!(
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
      21, 22, 23, 24, 25, 26, 27, 28, 29
    )
  };
}

struct Ty<const N: usize>(usize);

fn gotham_borrow_btree(b: &mut Bencher) {
  let mut state = BTreeGothamState::default();
  macro_rules! fill {
    ($($n:literal),*) => { $(state.put(Ty::<$n>($n));)* };
  }
  with_types!(fill);
  b.iter(|| {
    macro_rules! read {
      ($($n:literal),*) => { $(black_box(state.borrow::<Ty<$n>>().0);)* };
    }
    with_types!(read);
  });
}

fn gotham_borrow_hash(b: &mut Bencher) {
  let mut state = OpState::new(None);
  macro_rules! fill {
    ($($n:literal),*) => { $(state.put(Ty::<$n>($n));)* };
  }
  with_types!(fill);
  b.iter(|| {
    macro_rules! read {
      ($($n:literal),*) => { $(black_box(state.borrow::<Ty<$n>>().0);)* };
    }
    with_types!(read);
  });
}

/// A single op typically touches one or two pieces of state, not all thirty;
/// this is the per-op cost in isolation.
fn gotham_borrow_one_btree(b: &mut Bencher) {
  let mut state = BTreeGothamState::default();
  macro_rules! fill {
    ($($n:literal),*) => { $(state.put(Ty::<$n>($n));)* };
  }
  with_types!(fill);
  b.iter(|| black_box(state.borrow::<Ty<29>>().0));
}

fn gotham_borrow_one_hash(b: &mut Bencher) {
  let mut state = OpState::new(None);
  macro_rules! fill {
    ($($n:literal),*) => { $(state.put(Ty::<$n>($n));)* };
  }
  with_types!(fill);
  b.iter(|| black_box(state.borrow::<Ty<29>>().0));
}

benchmark_group!(
  benches,
  resource_add_get_close_btree,
  resource_add_get_close_slab,
  resource_get_hot_btree,
  resource_get_hot_slab,
  resource_churn_btree,
  resource_churn_slab,
  gotham_borrow_btree,
  gotham_borrow_hash,
  gotham_borrow_one_btree,
  gotham_borrow_one_hash,
);
benchmark_main!(benches);
