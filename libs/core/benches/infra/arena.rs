// Copyright 2018-2025 the Deno authors. MIT license.

#![allow(clippy::needless_range_loop)]
use bencher::Bencher;
use bencher::benchmark_group;
use bencher::benchmark_main;
use deno_core::arena::ArenaShared;
use deno_core::arena::ArenaSharedAtomic;
use deno_core::arena::ArenaUnique;
use deno_core::arena::RawArena;
use std::alloc::Layout;
use std::cell::RefCell;
use std::hint::black_box;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;

const COUNT: usize = 10_000;
type TestType = RefCell<usize>;

fn validate_array<T>(v: &[T]) {
  assert_eq!(v.len(), COUNT);
  for t in v {
    let _ = black_box(t);
  }
}

fn initialize_data<T: AsRef<TestType>>() -> [Option<T>; COUNT] {
  unsafe { std::mem::zeroed() }
}

fn bench_arc_arena(b: &mut Bencher) {
  let arena = ArenaSharedAtomic::<TestType>::with_capacity(COUNT);
  b.iter(|| {
    let mut data = initialize_data();
    for i in 0..COUNT {
      data[i] = Some(arena.allocate(Default::default()));
    }
    validate_array(&data);
  });
}

fn bench_rc_arena(b: &mut Bencher) {
  let arena = ArenaShared::<TestType>::with_capacity(COUNT);
  b.iter(|| {
    let mut data = initialize_data();
    for i in 0..COUNT {
      data[i] = Some(arena.allocate(Default::default()));
    }
    validate_array(&data);
  });
}

fn bench_rc_of_raw_arena(b: &mut Bencher) {
  struct RcData(NonNull<TestType>, Rc<RawArena<TestType>>);
  impl AsRef<TestType> for RcData {
    fn as_ref(&self) -> &TestType {
      unsafe { self.0.as_ref() }
    }
  }

  let arena = Rc::new(RawArena::<TestType>::with_capacity(COUNT));
  b.iter(|| {
    let mut data = initialize_data();
    for i in 0..COUNT {
      unsafe {
        let ptr = arena.allocate();
        std::ptr::write(ptr.as_ptr(), Default::default());
        data[i] = Some(RcData(ptr, arena.clone()));
      }
    }
    validate_array(&data);
    for i in 0..COUNT {
      unsafe {
        let data = data[i].as_ref().unwrap();
        data.1.recycle(data.0);
      }
    }
  });
}

fn bench_box_arena(b: &mut Bencher) {
  let arena = ArenaUnique::<TestType>::with_capacity(COUNT);
  b.iter(|| {
    let mut data = initialize_data();
    for i in 0..COUNT {
      data[i] = Some(arena.allocate(Default::default()));
    }
    validate_array(&data);
  });
}

#[allow(clippy::arc_with_non_send_sync)]
fn bench_arc(b: &mut Bencher) {
  b.iter(|| {
    let mut data = initialize_data();
    for i in 0..COUNT {
      data[i] = Some(Arc::<TestType>::new(Default::default()));
    }
    validate_array(&data);
  })
}

fn bench_rc(b: &mut Bencher) {
  b.iter(|| {
    let mut data = initialize_data();
    for i in 0..COUNT {
      data[i] = Some(Rc::<TestType>::new(Default::default()));
    }
    validate_array(&data);
  })
}

fn bench_box(b: &mut Bencher) {
  b.iter(|| {
    let mut data = initialize_data();
    for i in 0..COUNT {
      data[i] = Some(Box::<TestType>::default());
    }
    validate_array(&data);
  })
}

fn bench_raw_arena_init(b: &mut Bencher) {
  let arena = RawArena::<TestType>::with_capacity(COUNT);
  b.iter(|| {
    let mut data = [NonNull::dangling(); 10000];
    for i in 0..COUNT {
      unsafe {
        let ptr = arena.allocate();
        std::ptr::write(ptr.as_ptr(), Default::default());
        data[i] = ptr;
      }
    }
    validate_array(&data);
    for i in 0..COUNT {
      unsafe {
        arena.recycle(data[i]);
      }
    }
  });
}

fn bench_raw_arena_uninit(b: &mut Bencher) {
  let arena = RawArena::<TestType>::with_capacity(COUNT);
  b.iter(|| {
    let mut data = [NonNull::dangling(); 10000];
    for i in 0..COUNT {
      unsafe {
        data[i] = arena.allocate();
      }
    }
    validate_array(&data);
    for i in 0..COUNT {
      unsafe {
        arena.recycle_without_drop(data[i]);
      }
    }
  });
}

fn bench_raw_alloc(b: &mut Bencher) {
  b.iter(|| {
    let mut data = [std::ptr::null_mut(); COUNT];
    for i in 0..COUNT {
      unsafe {
        data[i] = std::alloc::alloc(Layout::new::<TestType>());
      }
    }
    validate_array(&data);
    for i in 0..COUNT {
      unsafe {
        std::alloc::dealloc(data[i], Layout::new::<TestType>());
      }
    }
  })
}

benchmark_main!(benches);

benchmark_group!(
  benches,
  bench_arc,
  bench_arc_arena,
  bench_rc,
  bench_rc_arena,
  bench_rc_of_raw_arena,
  bench_box,
  bench_box_arena,
  bench_raw_alloc,
  bench_raw_arena_init,
  bench_raw_arena_uninit,
);
