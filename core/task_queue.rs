// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use futures::task::AtomicWaker;
use futures::Future;
use parking_lot::Mutex;
use std::collections::LinkedList;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Debug, Default)]
struct TaskQueueTaskWaker {
  is_ready: AtomicBool,
  waker: AtomicWaker,
}

#[derive(Debug, Default)]
struct TaskQueueTasks {
  is_running: bool,
  wakers: LinkedList<Arc<TaskQueueTaskWaker>>,
}

/// A queue that executes tasks sequentially one after the other
/// ensuring order and that no task runs at the same time as another.
///
/// Note that tokio's semaphore doesn't seem to maintain order
/// and so we can't use that in the code that uses this or use
/// that here.
#[derive(Debug, Default)]
pub struct TaskQueue {
  tasks: Mutex<TaskQueueTasks>,
}

impl TaskQueue {
  /// Acquires a permit where the tasks are executed one at a time
  /// and in the order that they were acquired.
  pub async fn acquire(&self) -> TaskQueuePermit {
    let acquire = TaskQueuePermitAcquire::new(self);
    acquire.await;
    TaskQueuePermit(self)
  }

  /// Alternate API that acquires a permit internally
  /// for the duration of the future.
  pub async fn queue<R>(&self, future: impl Future<Output = R>) -> R {
    let _permit = self.acquire().await;
    future.await
  }
}

/// A permit that when dropped will allow another task to proceed.
pub struct TaskQueuePermit<'a>(&'a TaskQueue);

impl<'a> Drop for TaskQueuePermit<'a> {
  fn drop(&mut self) {
    let next_item = {
      let mut tasks = self.0.tasks.lock();
      let next_item = tasks.wakers.pop_front();
      tasks.is_running = next_item.is_some();
      next_item
    };
    if let Some(next_item) = next_item {
      next_item.is_ready.store(true, Ordering::SeqCst);
      next_item.waker.wake();
    }
  }
}

struct TaskQueuePermitAcquire<'a> {
  task_queue: &'a TaskQueue,
  initialized: AtomicBool,
  waker: Arc<TaskQueueTaskWaker>,
}

impl<'a> TaskQueuePermitAcquire<'a> {
  pub fn new(task_queue: &'a TaskQueue) -> Self {
    Self {
      task_queue,
      initialized: Default::default(),
      waker: Default::default(),
    }
  }
}

impl<'a> Future for TaskQueuePermitAcquire<'a> {
  type Output = ();

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    // update with the latest waker
    self.waker.waker.register(cx.waker());

    // ensure this is initialized
    if !self.initialized.swap(true, Ordering::SeqCst) {
      let mut tasks = self.task_queue.tasks.lock();
      if !tasks.is_running {
        tasks.is_running = true;
        return std::task::Poll::Ready(());
      }
      tasks.wakers.push_back(self.waker.clone());
      return std::task::Poll::Pending;
    }

    // check if we're ready to run
    if self.waker.is_ready.load(Ordering::SeqCst) {
      std::task::Poll::Ready(())
    } else {
      std::task::Poll::Pending
    }
  }
}

#[cfg(test)]
mod tests {
  use parking_lot::Mutex;
  use std::sync::Arc;

  use super::TaskQueue;

  #[tokio::test]
  async fn task_queue_runs_one_after_other() {
    let task_queue = TaskQueue::default();
    let mut tasks = Vec::new();
    let data = Arc::new(Mutex::new(0));
    for i in 0..100 {
      let data = data.clone();
      tasks.push(task_queue.queue(async move {
        crate::task::spawn_blocking(move || {
          let mut data = data.lock();
          if *data != i {
            panic!("Value was not equal.");
          }
          *data = i + 1;
        })
        .await
        .unwrap();
      }));
    }
    futures::future::join_all(tasks).await;
  }
}
