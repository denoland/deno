// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::future::Future;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr::NonNull;
use std::task::Context;
use std::task::Poll;

use pin_project::pin_project;

use super::erased_future::TypeErased;
use crate::arena::ArenaBox;
use crate::arena::ArenaUnique;
use crate::arena::ArenaUniqueReservation;

const MAX_ARENA_FUTURE_SIZE: usize = 1024;

/// The number of futures the arena has room for before it needs to grow. This
/// is allocated up-front by every runtime, so it is kept small: workloads that
/// need more get it on demand.
const FUTURE_ARENA_INITIAL_COUNT: usize = 16;

/// The number of futures we'll allow to live in the Arena. Once the arena has
/// grown this far, further futures are allocated on the heap.
const FUTURE_ARENA_MAX_COUNT: usize = 256;

/// The [`FutureArena`] requires context for each submitted future. This mapper provides the context, as well
/// as finalizes the output of the future to the correct output type for this arena.
pub trait FutureContextMapper<T, C, R> {
  fn context(&self) -> C;
  fn map(&self, r: R) -> T;
}

struct DynFutureInfoErased<T, C> {
  ptr: MaybeUninit<NonNull<dyn ContextFuture<T, C>>>,
  data: UnsafeCell<TypeErased<MAX_ARENA_FUTURE_SIZE>>,
}

pub trait ContextFuture<T, C>: Future<Output = T> {
  fn context(&self) -> C;
}

#[pin_project]
struct DynFutureInfo<
  T: 'static,
  C: 'static,
  M: FutureContextMapper<T, C, F::Output>,
  F: Future,
> {
  /// The future metadata
  #[pin]
  context: M,

  /// The underlying [`Future`], [`Pin`]-projectable.
  #[pin]
  future: F,

  _phantom: PhantomData<(T, C)>,
}

impl<T, C, M: FutureContextMapper<T, C, F::Output>, F: Future>
  ContextFuture<T, C> for DynFutureInfo<T, C, M, F>
{
  fn context(&self) -> C {
    self.context.context()
  }
}

impl<T, C, M: FutureContextMapper<T, C, F::Output>, F: Future> Future
  for DynFutureInfo<T, C, M, F>
{
  type Output = T;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.project();
    match F::poll(this.future, cx) {
      Poll::Pending => Poll::Pending,
      Poll::Ready(v) => Poll::Ready(this.context.map(v)),
    }
  }
}

#[allow(
  private_interfaces,
  reason = "variants are implementation details not exposed publicly"
)]
pub enum FutureAllocation<T: 'static, C: 'static> {
  /// The future and metadata are small enough to fit in the arena, so let's put it there
  Arena(ArenaBox<DynFutureInfoErased<T, C>>),
  /// If this future doesn't fit in the arena (because the arena is full or the future is too
  /// large), it is stored in the heap.
  Box(Pin<Box<dyn ContextFuture<T, C>>>),
}

impl<T, C> FutureAllocation<T, C> {
  pub fn context(&self) -> C {
    unsafe {
      match self {
        Self::Arena(a) => (a.ptr.assume_init().as_ref()).context(),
        Self::Box(b) => b.context(),
      }
    }
  }
}

impl<T, C> Unpin for FutureAllocation<T, C> {}

impl<T, C> Future for FutureAllocation<T, C> {
  type Output = T;

  #[inline(always)]
  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    // SAFETY: We know the underlying futures are both pinned by their allocations
    unsafe {
      match self.get_mut() {
        Self::Arena(a) => {
          Pin::new_unchecked(a.ptr.assume_init().as_mut()).poll(cx)
        }
        Self::Box(b) => b.as_mut().poll(cx),
      }
    }
  }
}

/// A [`FutureAllocation`] that has not been erased yet. This may be polled using its original
/// type of [`Future`].
pub struct TypedFutureAllocation<
  T: 'static,
  C: 'static,
  M: FutureContextMapper<T, C, F::Output>,
  F: Future,
> {
  inner: FutureAllocation<T, C>,
  /// Maintain a pointer to the raw type until we erase it
  ptr: NonNull<DynFutureInfo<T, C, M, F>>,
}

impl<T, C, M: FutureContextMapper<T, C, F::Output>, F: Future> Unpin
  for TypedFutureAllocation<T, C, M, F>
{
}

impl<T, C, M: FutureContextMapper<T, C, F::Output>, F: Future>
  TypedFutureAllocation<T, C, M, F>
{
  #[inline(always)]
  pub fn erase(self) -> FutureAllocation<T, C> {
    self.inner
  }
}

impl<T, C, M: FutureContextMapper<T, C, F::Output>, F: Future> Future
  for TypedFutureAllocation<T, C, M, F>
{
  type Output = F::Output;
  #[inline(always)]
  fn poll(
    mut self: Pin<&mut Self>,
    cx: &mut Context<'_>,
  ) -> Poll<Self::Output> {
    // SAFETY: We know the underlying futures are both pinned by their allocations
    unsafe { F::poll(Pin::new_unchecked(&mut self.ptr.as_mut().future), cx) }
  }
}

type Chunk<T, C> = ArenaUnique<DynFutureInfoErased<T, C>>;
type Reservation<T, C> = ArenaUniqueReservation<DynFutureInfoErased<T, C>>;

/// An arena of erased futures with associated mapping functions. Futures too large for the arena,
/// or futures allocated when the arena are full, are automatically moved to the heap instead.
///
/// Each future is associated with an output type and a context. The context is used to create the
/// output type.
///
/// The arena starts out with room for [`FUTURE_ARENA_INITIAL_COUNT`] futures
/// and grows on demand -- by doubling -- up to [`FUTURE_ARENA_MAX_COUNT`].
/// Growth appends a new chunk rather than reallocating: futures in the arena
/// are pinned in place while they are in flight, so an existing slot must never
/// move.
pub struct FutureArena<T, C> {
  /// The chunk we're currently handing slots out of. This is a separate field
  /// rather than an index into `retired` so that the common case -- allocating
  /// from a chunk that has room -- is the same single pointer load it was when
  /// the arena was one fixed block.
  ///
  /// Only ever mutated by [`FutureArena::reserve_slow`], which holds no other
  /// reference to it while doing so.
  current: UnsafeCell<Chunk<T, C>>,
  /// Chunks that were full at the point we last needed a slot from them. They
  /// may since have had futures complete, so we look here for a free slot
  /// before growing.
  retired: RefCell<Vec<Chunk<T, C>>>,
  /// The total number of slots across `current` and `retired`.
  capacity: Cell<usize>,
}

impl<T, C> Default for FutureArena<T, C> {
  fn default() -> Self {
    FutureArena {
      current: UnsafeCell::new(ArenaUnique::with_capacity(
        FUTURE_ARENA_INITIAL_COUNT,
      )),
      retired: RefCell::new(Vec::new()),
      capacity: Cell::new(FUTURE_ARENA_INITIAL_COUNT),
    }
  }
}

impl<T, C> FutureArena<T, C> {
  /// Reserve a slot in the current chunk, growing the arena (or recycling a
  /// retired chunk that has since freed a slot) if it is full. Returns [`None`]
  /// if the arena is at its maximum size and every chunk is full.
  ///
  /// On success the reservation always belongs to `current`, so the caller
  /// completes it against `current` in both the fast and the slow case.
  ///
  /// # Safety
  ///
  /// The returned reservation must be completed or forgotten against
  /// `self.current`, and must not outlive another call to this method.
  #[inline(always)]
  unsafe fn reserve(&self) -> Option<Reservation<T, C>> {
    // SAFETY: no other reference to `current` is alive at this point, and
    // `reserve_space` does not touch the `UnsafeCell` itself.
    if let Some(reservation) = unsafe { (*self.current.get()).reserve_space() }
    {
      return Some(reservation);
    }
    self.reserve_slow()
  }

  #[cold]
  #[inline(never)]
  fn reserve_slow(&self) -> Option<Reservation<T, C>> {
    let mut retired = self.retired.borrow_mut();

    // Did a future in one of the retired chunks complete? Take the emptiest one
    // so we come back here as rarely as possible.
    let emptiest = retired
      .iter()
      .enumerate()
      .map(|(i, chunk)| (chunk.remaining(), i))
      .filter(|&(remaining, _)| remaining > 0)
      .max();
    if let Some((_, i)) = emptiest {
      let chunk = retired.swap_remove(i);
      // SAFETY: the chunk has room, so this returns a reservation belonging to
      // `chunk`, which we immediately install as `current`. Moving the
      // `ArenaUnique` handle does not move the slots it owns.
      let reservation = unsafe { chunk.reserve_space() };
      debug_assert!(reservation.is_some());
      // SAFETY: `retired` is borrowed, but nothing borrows `current` here.
      let full = unsafe { std::mem::replace(&mut *self.current.get(), chunk) };
      retired.push(full);
      return reservation;
    }

    // Everything is full: double the arena if we're still under the cap.
    let capacity = self.capacity.get();
    let growth = std::cmp::min(capacity, FUTURE_ARENA_MAX_COUNT - capacity);
    if growth == 0 {
      return None;
    }

    let chunk = ArenaUnique::with_capacity(growth);
    // SAFETY: a fresh chunk always has room, and moving the `ArenaUnique`
    // handle does not move the slots it owns.
    let reservation = unsafe { chunk.reserve_space() };
    debug_assert!(reservation.is_some());
    // SAFETY: `retired` is borrowed, but nothing borrows `current` here.
    let full = unsafe { std::mem::replace(&mut *self.current.get(), chunk) };
    retired.push(full);
    self.capacity.set(capacity + growth);
    reservation
  }

  /// The number of futures this arena has room for without growing. Grows over
  /// the life of the arena, up to [`FUTURE_ARENA_MAX_COUNT`].
  #[cfg(test)]
  fn capacity(&self) -> usize {
    self.capacity.get()
  }
}

impl<T, C: Clone> FutureArena<T, C> {
  /// Allocate a future to run in this `FuturesUnordered`. If the future is too large, or the arena
  /// is full, allocated in the heap.
  ///
  /// The type of the future provided must convert into the type of the arena itself via [`From`].
  #[inline]
  #[allow(
    private_bounds,
    reason = "bounds are implementation details not exposed publicly"
  )]
  pub fn allocate<F, R, M: FutureContextMapper<T, C, R> + 'static>(
    &self,
    context: M,
    future: F,
  ) -> TypedFutureAllocation<T, C, M, F>
  where
    F: Future<Output = R> + 'static,
    DynFutureInfo<T, C, M, F>: ContextFuture<T, C>,
  {
    if std::mem::size_of::<DynFutureInfo<T, C, M, F>>() <= MAX_ARENA_FUTURE_SIZE
    {
      unsafe {
        if let Some(reservation) = self.reserve() {
          // SAFETY: `reserve` always hands back a reservation belonging to the
          // current chunk.
          let alloc = (*self.current.get()).complete_reservation(
            reservation,
            DynFutureInfoErased {
              ptr: MaybeUninit::uninit(),
              data: UnsafeCell::new(TypeErased::new(DynFutureInfo {
                context,
                future,
                _phantom: PhantomData,
              })),
            },
          );
          let ptr =
            TypeErased::raw_ptr::<DynFutureInfo<T, C, M, F>>(alloc.data.get());
          (*alloc.deref_data().as_ptr()).ptr.write(ptr);
          return TypedFutureAllocation {
            inner: FutureAllocation::Arena(alloc),
            ptr,
          };
        }
      }
    }

    let mut future = Box::pin(DynFutureInfo {
      context,
      future,
      _phantom: PhantomData,
    });

    let ptr = unsafe { NonNull::from(future.as_mut().get_unchecked_mut()) };

    TypedFutureAllocation {
      inner: FutureAllocation::Box(future),
      ptr,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::fmt::Display;
  use std::future::ready;
  use std::task::Waker;

  use futures::FutureExt;

  use super::*;

  const INFO: usize = 0;

  #[derive(Debug, PartialEq, Eq)]
  struct Stringish(String);

  impl<R: Display> FutureContextMapper<Stringish, usize, R> for usize {
    fn context(&self) -> usize {
      *self
    }

    fn map(&self, r: R) -> Stringish {
      Stringish(format!("{r}"))
    }
  }

  #[test]
  fn test_mapping() {
    let arena = FutureArena::<Stringish, usize>::default();

    // Poll unmapped
    let mut f = arena.allocate(INFO, async { 1 });
    let Poll::Ready(v) = f.poll_unpin(&mut Context::from_waker(Waker::noop()))
    else {
      panic!();
    };
    assert_eq!(v, 1);

    // Poll Mapped
    let mut f = arena.allocate(INFO, async { 1 }).erase();
    let Poll::Ready(v) = f.poll_unpin(&mut Context::from_waker(Waker::noop()))
    else {
      panic!();
    };
    assert_eq!(v.0, "1".to_owned());
  }

  #[test]
  fn test_double_free() {
    let arena = FutureArena::<Stringish, usize>::default();
    let f = arena.allocate(INFO, async { 1 });
    drop(f);
    let f = arena.allocate(INFO, Box::pin(async { 1 }));
    drop(f);
    let f = arena.allocate(INFO, ready(Box::new(1_i32)));
    drop(f);
  }

  #[test]
  fn test_exceed_arena() {
    let arena = FutureArena::<Stringish, usize>::default();
    let mut v = vec![];
    for _ in 0..1000 {
      v.push(arena.allocate(INFO, ready(Box::new(1_i32))));
    }
    drop(v);
  }

  fn is_arena<T, C, M: FutureContextMapper<T, C, F::Output>, F: Future>(
    alloc: &TypedFutureAllocation<T, C, M, F>,
  ) -> bool {
    matches!(alloc.inner, FutureAllocation::Arena(..))
  }

  /// The arena starts small and doubles, and futures already in flight keep
  /// their address as it grows.
  #[test]
  fn test_growth_keeps_futures_pinned() {
    let arena = FutureArena::<Stringish, usize>::default();
    assert_eq!(arena.capacity(), FUTURE_ARENA_INITIAL_COUNT);

    let mut v = vec![];
    let mut addresses = vec![];
    let mut capacities = vec![];
    for _ in 0..FUTURE_ARENA_MAX_COUNT {
      let f = arena.allocate(INFO, ready(1_i32));
      assert!(is_arena(&f));
      addresses.push(f.ptr.as_ptr() as usize);
      v.push(f);
      // Every allocated future must still live where it was put.
      for (f, address) in v.iter().zip(addresses.iter()) {
        assert_eq!(f.ptr.as_ptr() as usize, *address);
      }
      if capacities.last() != Some(&arena.capacity()) {
        capacities.push(arena.capacity());
      }
    }
    assert_eq!(capacities, vec![16, 32, 64, 128, 256]);
    assert_eq!(arena.capacity(), FUTURE_ARENA_MAX_COUNT);

    // The arena is full now, so this one goes to the heap.
    let boxed = arena.allocate(INFO, ready(1_i32));
    assert!(!is_arena(&boxed));
    assert_eq!(arena.capacity(), FUTURE_ARENA_MAX_COUNT);

    // ... and everything still polls to completion.
    for mut f in v {
      let Poll::Ready(v) =
        f.poll_unpin(&mut Context::from_waker(Waker::noop()))
      else {
        panic!();
      };
      assert_eq!(v, 1);
    }
  }

  /// Completed futures return their slot to the arena, and we don't grow while
  /// there are slots to reuse.
  #[test]
  fn test_slots_are_reused() {
    let arena = FutureArena::<Stringish, usize>::default();
    for _ in 0..100 {
      let v = (0..FUTURE_ARENA_INITIAL_COUNT)
        .map(|_| arena.allocate(INFO, ready(1_i32)))
        .collect::<Vec<_>>();
      assert!(v.iter().all(is_arena));
      drop(v);
      assert_eq!(arena.capacity(), FUTURE_ARENA_INITIAL_COUNT);
    }
  }

  /// A slot freed in a chunk we've already moved past is picked up again rather
  /// than growing the arena.
  #[test]
  fn test_retired_chunk_slots_are_reused() {
    let arena = FutureArena::<Stringish, usize>::default();
    // Fill the first chunk, then the second: capacity 16 + 16.
    let mut v = (0..FUTURE_ARENA_INITIAL_COUNT * 2)
      .map(|_| arena.allocate(INFO, ready(1_i32)))
      .collect::<Vec<_>>();
    assert_eq!(arena.capacity(), FUTURE_ARENA_INITIAL_COUNT * 2);

    // Free a slot in the first (retired) chunk. The current chunk is still
    // full, so the next allocation has to find this one.
    let address = v.remove(0).ptr.as_ptr() as usize;
    let reused = arena.allocate(INFO, ready(1_i32));
    assert!(is_arena(&reused));
    assert_eq!(reused.ptr.as_ptr() as usize, address);
    assert_eq!(arena.capacity(), FUTURE_ARENA_INITIAL_COUNT * 2);
  }

  /// Futures too large for a slot skip the arena entirely.
  #[test]
  fn test_oversized_future_is_boxed() {
    let arena = FutureArena::<Stringish, usize>::default();
    let big = arena.allocate(INFO, async {
      let large = [0_u8; MAX_ARENA_FUTURE_SIZE];
      ready(1_i32).await;
      large[0] as i32
    });
    assert!(!is_arena(&big));
    // An oversized future doesn't consume arena capacity.
    assert_eq!(arena.capacity(), FUTURE_ARENA_INITIAL_COUNT);
    let small = arena.allocate(INFO, ready(1_i32));
    assert!(is_arena(&small));
  }

  #[test]
  fn test_drop_after_arena() {
    let arena = FutureArena::<Stringish, usize>::default();
    let mut v = vec![];
    for _ in 0..1000 {
      v.push(arena.allocate(INFO, ready(Box::new(1_i32))));
    }
    drop(arena);
    drop(v);
  }
}
