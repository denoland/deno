// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crossbeam_channel::Sender;
use threadpool::ThreadPool;

// TODO(@kitsonk) replace with `tokio-threadpool`?

pub struct TaskPool<T> {
  sender: Sender<T>,
  inner: ThreadPool,
}

impl<T> TaskPool<T> {
  pub fn new(sender: Sender<T>) -> TaskPool<T> {
    TaskPool {
      sender,
      inner: ThreadPool::default(),
    }
  }

  pub fn spawn<F>(&mut self, task: F)
  where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
  {
    self.inner.execute({
      let sender = self.sender.clone();
      move || sender.send(task()).unwrap()
    })
  }

  #[allow(unused)]
  pub fn spawn_with_sender<F>(&mut self, task: F)
  where
    F: FnOnce(Sender<T>) + Send + 'static,
    T: Send + 'static,
  {
    self.inner.execute({
      let sender = self.sender.clone();
      move || task(sender)
    })
  }

  pub fn len(&self) -> usize {
    self.inner.queued_count()
  }
}

impl<T> Drop for TaskPool<T> {
  fn drop(&mut self) {
    self.inner.join()
  }
}
