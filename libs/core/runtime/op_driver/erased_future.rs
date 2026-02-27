// Copyright 2018-2025 the Deno authors. MIT license.

use std::future::Future;
use std::marker::PhantomData;
use std::marker::PhantomPinned;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr::NonNull;
use std::task::Context;
use std::task::Poll;

#[repr(C, align(16))]
pub struct TypeErased<const MAX_SIZE: usize> {
  memory: [MaybeUninit<u8>; MAX_SIZE],
  drop: fn(this: NonNull<()>),
  _unpin: PhantomPinned,
}

impl<const MAX_SIZE: usize> TypeErased<MAX_SIZE> {
  #[inline(always)]
  pub unsafe fn take<R: 'static>(self) -> R {
    unsafe {
      assert!(
        std::mem::size_of::<R>() <= std::mem::size_of_val(&self.memory),
        "Invalid size for {}: {} (max {})",
        std::any::type_name::<R>(),
        std::mem::size_of::<R>(),
        std::mem::size_of_val(&self.memory)
      );
      assert!(
        std::mem::align_of::<R>() <= std::mem::align_of_val(&self),
        "Invalid alignment for {}: {} (max {})",
        std::any::type_name::<R>(),
        std::mem::align_of::<R>(),
        std::mem::align_of_val(&self)
      );
      let r = std::ptr::read(self.memory.as_ptr() as _);
      std::mem::forget(self);
      r
    }
  }

  #[inline(always)]
  /// Safety: This uses a place projection to `this.memory`, which must be in-bounds.
  pub unsafe fn raw_ptr<R>(this: *mut Self) -> NonNull<R> {
    unsafe {
      NonNull::new_unchecked(std::ptr::addr_of_mut!((*this).memory) as *mut _)
    }
  }

  #[inline(always)]
  pub fn new<R>(r: R) -> Self {
    let mut new = Self {
      memory: [MaybeUninit::uninit(); MAX_SIZE],
      drop: |this| unsafe { std::ptr::drop_in_place::<R>(this.as_ptr() as _) },
      _unpin: PhantomPinned,
    };
    assert!(
      std::mem::size_of::<R>() <= std::mem::size_of_val(&new.memory),
      "Invalid size for {}: {} (max {})",
      std::any::type_name::<R>(),
      std::mem::size_of::<R>(),
      std::mem::size_of_val(&new.memory)
    );
    assert!(
      std::mem::align_of::<R>() <= std::mem::align_of_val(&new),
      "Invalid alignment for {}: {} (max {})",
      std::any::type_name::<R>(),
      std::mem::align_of::<R>(),
      std::mem::align_of_val(&new),
    );
    unsafe { std::ptr::write(new.memory.as_mut_ptr() as *mut R, r) };
    new
  }
}

impl<const MAX_SIZE: usize> Drop for TypeErased<MAX_SIZE> {
  fn drop(&mut self) {
    (self.drop)(unsafe { Self::raw_ptr(self) })
  }
}

// TODO(mmastrac): This code is also testing TypeErased above
#[allow(dead_code)]
pub struct ErasedFuture<const MAX_SIZE: usize, Output> {
  erased: TypeErased<MAX_SIZE>,
  poll: unsafe fn(
    this: Pin<&mut TypeErased<MAX_SIZE>>,
    cx: &mut Context,
  ) -> Poll<Output>,
  _output: PhantomData<Output>,
}

impl<const MAX_SIZE: usize, Output> ErasedFuture<MAX_SIZE, Output> {
  #[allow(dead_code)]
  unsafe fn poll<F>(
    pin: Pin<&mut TypeErased<MAX_SIZE>>,
    cx: &mut Context<'_>,
  ) -> Poll<Output>
  where
    F: Future<Output = Output>,
  {
    unsafe {
      F::poll(
        std::mem::transmute::<Pin<&mut TypeErased<MAX_SIZE>>, Pin<&mut F>>(pin),
        cx,
      )
    }
  }

  #[allow(dead_code)]
  pub fn new<F>(f: F) -> Self
  where
    F: Future<Output = Output>,
  {
    Self {
      erased: TypeErased::new(f),
      poll: Self::poll::<F>,
      _output: PhantomData,
    }
  }
}

impl<const MAX_SIZE: usize, Output> Future for ErasedFuture<MAX_SIZE, Output> {
  type Output = Output;
  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    unsafe {
      (self.poll)(Pin::new_unchecked(&mut self.get_unchecked_mut().erased), cx)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::arena::ArenaUnique;

  struct Countdown(usize);

  impl Future for Countdown {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
      if self.0 == 0 {
        Poll::Ready(())
      } else {
        self.get_mut().0 -= 1;
        cx.waker().wake_by_ref();
        Poll::Pending
      }
    }
  }

  #[inline(never)]
  async fn add_one_to_reference(r: &mut usize) {
    Countdown(10).await;
    *r += 1;
  }

  #[test]
  fn test_outside_arena() {
    let future = Box::pin(async {
      let mut v = 1;
      add_one_to_reference(&mut v).await;
      v
    });
    assert_eq!(2, futures::executor::block_on(future));
  }

  #[test]
  fn test_in_arena_simple() {
    let arena = ArenaUnique::<ErasedFuture<256, usize>>::with_capacity(16);
    let future = arena.allocate(ErasedFuture::new(async { 1 }));
    assert_eq!(1, futures::executor::block_on(future));
  }

  #[test]
  fn test_in_arena_selfref_easy() {
    let arena = ArenaUnique::<ErasedFuture<256, usize>>::with_capacity(16);
    #[allow(clippy::useless_vec)]
    let future = arena.allocate(ErasedFuture::new(async {
      let mut v = vec![1];
      let v = v.get_mut(0).unwrap();
      Countdown(10).await;
      *v = 2;
      *v
    }));
    assert_eq!(2, futures::executor::block_on(future));
  }

  // This test doesn't pass in Miri because we're waiting on https://github.com/rust-lang/rfcs/pull/3467
  // Miri believes this is invalid because the internal future has a &mut alive when it suspends, and we need
  // to re-create the &mut to properly Pin it during the next poll cycle.
  #[cfg(not(miri))]
  #[test]
  fn test_in_arena_selfref_hard() {
    let arena = ArenaUnique::<ErasedFuture<256, usize>>::with_capacity(16);
    let future = arena.allocate(ErasedFuture::new(async {
      let mut v = 1;
      add_one_to_reference(&mut v).await;
      v
    }));
    assert_eq!(2, futures::executor::block_on(future));
  }
}
