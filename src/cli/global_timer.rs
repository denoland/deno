// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

//! This module helps deno implement timers.
//!
//! As an optimization, we want to avoid an expensive calls into rust for every
//! setTimeout in JavaScript. Thus in //js/timers.ts a data structure is
//! implemented that calls into Rust for only the smallest timeout.  Thus we
//! only need to be able to start and cancel a single timer (or Delay, as Tokio
//! calls it) for an entire Isolate. This is what is implemented here.

use crate::tokio_util::panic_on_error;
use futures::Future;
use std::time::Instant;
use tokio::sync::oneshot;
use tokio::timer::Delay;

#[derive(Default)]
pub struct GlobalTimer {
  tx: Option<oneshot::Sender<()>>,
}

impl GlobalTimer {
  pub fn new() -> Self {
    Self { tx: None }
  }

  pub fn cancel(&mut self) {
    if let Some(tx) = self.tx.take() {
      tx.send(()).ok();
    }
  }

  pub fn new_timeout(
    &mut self,
    deadline: Instant,
  ) -> impl Future<Item = (), Error = ()> {
    if self.tx.is_some() {
      self.cancel();
    }
    assert!(self.tx.is_none());

    let (tx, rx) = oneshot::channel();
    self.tx = Some(tx);

    let delay = panic_on_error(Delay::new(deadline));
    let rx = panic_on_error(rx);

    delay.select(rx).then(|_| Ok(()))
  }
}
