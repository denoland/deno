// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crossbeam_channel::Sender;

pub struct TaskPool<T> {
  sender: Sender<T>,
}

impl<T> TaskPool<T> {
  pub fn new(sender: Sender<T>) -> TaskPool<T> {
    TaskPool { sender }
  }

  pub fn spawn<F>(&mut self, task: F)
  where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
  {
    let sender = self.sender.clone();
    tokio::task::spawn_blocking(move || sender.send(task()).unwrap());
  }
}
