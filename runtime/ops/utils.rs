// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::futures::task::AtomicWaker;
use deno_core::futures::Future;
use deno_core::parking_lot::Mutex;
use std::collections::LinkedList;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// A utility function to map OsStrings to Strings
pub fn into_string(s: std::ffi::OsString) -> Result<String, AnyError> {
  s.into_string().map_err(|s| {
    let message = format!("File name or path {:?} is not valid UTF-8", s);
    custom_error("InvalidData", message)
  })
}

#[derive(Default)]
struct TaskQueueTaskWaker {
  is_ready: AtomicBool,
  waker: AtomicWaker,
}

#[derive(Default)]
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
#[derive(Clone, Default)]
pub struct TaskQueue {
  tasks: Arc<Mutex<TaskQueueTasks>>,
}

impl TaskQueue {
  /// Alternate API that acquires a permit internally
  /// for the duration of the future.
  #[cfg(test)]
  pub async fn queue<R>(&self, future: impl Future<Output = R>) -> R {
    let _permit = self.acquire().await;
    future.await
  }

  /// Acquires a permit where the tasks are executed one at a time
  /// and in the order that they were acquired.
  pub async fn acquire(&self) -> TaskQueuePermit {
    let acquire = TaskQueuePermitAcquire::new(self.tasks.clone());
    acquire.await;
    TaskQueuePermit {
      tasks: self.tasks.clone(),
    }
  }
}

/// A permit that when dropped will allow another task to proceed.
pub struct TaskQueuePermit {
  tasks: Arc<Mutex<TaskQueueTasks>>,
}

impl Drop for TaskQueuePermit {
  fn drop(&mut self) {
    let next_item = {
      let mut tasks = self.tasks.lock();
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

struct TaskQueuePermitAcquire {
  tasks: Arc<Mutex<TaskQueueTasks>>,
  initialized: AtomicBool,
  waker: Arc<TaskQueueTaskWaker>,
}

impl TaskQueuePermitAcquire {
  pub fn new(tasks: Arc<Mutex<TaskQueueTasks>>) -> Self {
    Self {
      tasks,
      initialized: Default::default(),
      waker: Default::default(),
    }
  }
}

impl Future for TaskQueuePermitAcquire {
  type Output = ();

  fn poll(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    // update with the latest waker
    self.waker.waker.register(cx.waker());

    // ensure this is initialized
    if !self.initialized.swap(true, Ordering::SeqCst) {
      let mut tasks = self.tasks.lock();
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
  use deno_core::futures;
  use deno_core::parking_lot::Mutex;
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
        tokio::task::spawn_blocking(move || {
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
