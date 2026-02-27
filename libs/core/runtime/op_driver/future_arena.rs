// Copyright 2018-2025 the Deno authors. MIT license.

use super::erased_future::TypeErased;
use crate::arena::ArenaBox;
use crate::arena::ArenaUnique;
use pin_project::pin_project;
use std::cell::UnsafeCell;
use std::future::Future;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr::NonNull;
use std::task::Context;
use std::task::Poll;

const MAX_ARENA_FUTURE_SIZE: usize = 1024;

/// The number of futures we'll allow to live in the Arena.
const FUTURE_ARENA_COUNT: usize = 256;

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

#[allow(private_interfaces)]
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

/// An arena of erased futures with associated mapping functions. Futures too large for the arena,
/// or futures allocated when the arena are full, are automatically moved to the heap instead.
///
/// Each future is associated with an output type and a context. The context is used to create the
/// output type.
#[repr(transparent)]
pub struct FutureArena<T, C> {
  arena: ArenaUnique<DynFutureInfoErased<T, C>>,
}

impl<T, C> Default for FutureArena<T, C> {
  fn default() -> Self {
    FutureArena {
      arena: ArenaUnique::with_capacity(FUTURE_ARENA_COUNT),
    }
  }
}

impl<T, C: Clone> FutureArena<T, C> {
  /// Allocate a future to run in this `FuturesUnordered`. If the future is too large, or the arena
  /// is full, allocated in the heap.
  ///
  /// The type of the future provided must convert into the type of the arena itself via [`From`].
  #[inline]
  #[allow(private_bounds)]
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
        if let Some(reservation) = self.arena.reserve_space() {
          let alloc = self.arena.complete_reservation(
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
  use super::*;
  use futures::FutureExt;
  use std::fmt::Display;
  use std::future::ready;
  use std::task::Waker;

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
