// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// FuturesList only makes a few allocations whereas FuturesUnordered
/// allocate per item push.
///
/// We see a 9-20% improvement in TCP throughput micro benchmarks.
pub struct FuturesList<Fut>(Vec<Fut>);

impl<Fut: Future + Unpin> FuturesList<Fut> {
  #[inline]
  pub fn new() -> Self {
    Self(Vec::new())
  }

  #[inline]
  pub fn push(&mut self, fut: Fut) {
    self.0.push(fut);
  }

  #[inline]
  pub fn len(&self) -> usize {
    self.0.len()
  }
}

impl<Fut: Future + Unpin> futures::Stream for FuturesList<Fut> {
  type Item = Fut::Output;

  fn poll_next(
    mut self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let futures = &mut self.as_mut().0;
    let mut i = 0;
    while i < futures.len() {
      if let Poll::Ready(r) = Pin::new(&mut futures[i]).poll(cx) {
        futures.swap_remove(i);
        return Poll::Ready(Some(r));
      }
      i += 1;
    }
    Poll::Pending
  }
}

impl<Fut: Future + Unpin> futures::stream::FusedStream for FuturesList<Fut> {
  fn is_terminated(&self) -> bool {
    false
  }
}
