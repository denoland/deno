// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::LinkedList;
use std::future::Future;
use std::sync::Arc;

use deno_core::futures::task::AtomicWaker;
use deno_core::parking_lot::Mutex;

use super::AtomicFlag;

#[derive(Debug, Default)]
struct TaskQueueTaskItem {
  is_ready: AtomicFlag,
  is_future_dropped: AtomicFlag,
  waker: AtomicWaker,
}

#[derive(Debug, Default)]
struct TaskQueueTasks {
  is_running: bool,
  items: LinkedList<Arc<TaskQueueTaskItem>>,
}

/// A queue that executes tasks sequentially one after the other
/// ensuring order and that no task runs at the same time as another.
///
/// Note that this differs from tokio's semaphore in that the order
/// is acquired synchronously.
#[derive(Debug, Default)]
pub struct TaskQueue {
  tasks: Mutex<TaskQueueTasks>,
}

impl TaskQueue {
  /// Acquires a permit where the tasks are executed one at a time
  /// and in the order that they were acquired.
  pub fn acquire(&self) -> TaskQueuePermitAcquireFuture {
    TaskQueuePermitAcquireFuture::new(self)
  }

  /// Alternate API that acquires a permit internally
  /// for the duration of the future.
  #[allow(unused)]
  pub fn run<'a, R>(
    &'a self,
    future: impl Future<Output = R> + 'a,
  ) -> impl Future<Output = R> + 'a {
    let acquire_future = self.acquire();
    async move {
      let permit = acquire_future.await;
      let result = future.await;
      drop(permit); // explicit for clarity
      result
    }
  }

  fn raise_next(&self) {
    let front_item = {
      let mut tasks = self.tasks.lock();

      // clear out any wakers for futures that were dropped
      while let Some(front_waker) = tasks.items.front() {
        if front_waker.is_future_dropped.is_raised() {
          tasks.items.pop_front();
        } else {
          break;
        }
      }
      let front_item = tasks.items.pop_front();
      tasks.is_running = front_item.is_some();
      front_item
    };

    // wake up the next waker
    if let Some(front_item) = front_item {
      front_item.is_ready.raise();
      front_item.waker.wake();
    }
  }
}

/// A permit that when dropped will allow another task to proceed.
pub struct TaskQueuePermit<'a>(&'a TaskQueue);

impl Drop for TaskQueuePermit<'_> {
  fn drop(&mut self) {
    self.0.raise_next();
  }
}

pub struct TaskQueuePermitAcquireFuture<'a> {
  task_queue: Option<&'a TaskQueue>,
  item: Arc<TaskQueueTaskItem>,
}

impl<'a> TaskQueuePermitAcquireFuture<'a> {
  pub fn new(task_queue: &'a TaskQueue) -> Self {
    // acquire the waker position synchronously
    let mut tasks = task_queue.tasks.lock();
    let item = if !tasks.is_running {
      tasks.is_running = true;
      let item = Arc::new(TaskQueueTaskItem::default());
      item.is_ready.raise();
      item
    } else {
      let item = Arc::new(TaskQueueTaskItem::default());
      tasks.items.push_back(item.clone());
      item
    };
    drop(tasks);
    Self {
      task_queue: Some(task_queue),
      item,
    }
  }
}

impl Drop for TaskQueuePermitAcquireFuture<'_> {
  fn drop(&mut self) {
    if let Some(task_queue) = self.task_queue.take() {
      if self.item.is_ready.is_raised() {
        task_queue.raise_next();
      } else {
        self.item.is_future_dropped.raise();
      }
    }
  }
}

impl<'a> Future for TaskQueuePermitAcquireFuture<'a> {
  type Output = TaskQueuePermit<'a>;

  fn poll(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<Self::Output> {
    if self.item.is_ready.is_raised() {
      std::task::Poll::Ready(TaskQueuePermit(self.task_queue.take().unwrap()))
    } else {
      self.item.waker.register(cx.waker());
      std::task::Poll::Pending
    }
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use deno_core::futures;
  use deno_core::parking_lot::Mutex;

  use super::*;

  #[tokio::test]
  async fn task_queue_runs_one_after_other() {
    let task_queue = TaskQueue::default();
    let mut tasks = Vec::new();
    let data = Arc::new(Mutex::new(0));
    for i in 0..100 {
      let data = data.clone();
      tasks.push(task_queue.run(async move {
        deno_core::unsync::spawn_blocking(move || {
          let mut data = data.lock();
          assert_eq!(*data, i);
          *data = i + 1;
        })
        .await
        .unwrap();
      }));
    }
    futures::future::join_all(tasks).await;
  }

  #[tokio::test]
  async fn task_queue_run_in_sequence() {
    let task_queue = TaskQueue::default();
    let data = Arc::new(Mutex::new(0));

    let first = task_queue.run(async {
      *data.lock() = 1;
    });
    let second = task_queue.run(async {
      assert_eq!(*data.lock(), 1);
      *data.lock() = 2;
    });
    let _ = tokio::join!(first, second);

    assert_eq!(*data.lock(), 2);
  }

  #[tokio::test]
  async fn task_queue_future_dropped_before_poll() {
    let task_queue = Arc::new(TaskQueue::default());

    // acquire a future, but do not await it
    let future = task_queue.acquire();

    // this task tries to acquire another permit, but will be blocked by the first permit.
    let enter_flag = Arc::new(AtomicFlag::default());
    let delayed_task = deno_core::unsync::spawn({
      let enter_flag = enter_flag.clone();
      let task_queue = task_queue.clone();
      async move {
        enter_flag.raise();
        task_queue.acquire().await;
        true
      }
    });

    // ensure the task gets a chance to be scheduled and blocked
    tokio::task::yield_now().await;
    assert!(enter_flag.is_raised());

    // now, drop the first future
    drop(future);

    assert!(delayed_task.await.unwrap());
  }

  #[tokio::test]
  async fn task_queue_many_future_dropped_before_poll() {
    let task_queue = Arc::new(TaskQueue::default());

    // acquire a future, but do not await it
    let mut futures = Vec::new();
    for _ in 0..=10_000 {
      futures.push(task_queue.acquire());
    }

    // this task tries to acquire another permit, but will be blocked by the first permit.
    let enter_flag = Arc::new(AtomicFlag::default());
    let delayed_task = deno_core::unsync::spawn({
      let task_queue = task_queue.clone();
      let enter_flag = enter_flag.clone();
      async move {
        enter_flag.raise();
        task_queue.acquire().await;
        true
      }
    });

    // ensure the task gets a chance to be scheduled and blocked
    tokio::task::yield_now().await;
    assert!(enter_flag.is_raised());

    // now, drop the futures
    drop(futures);

    assert!(delayed_task.await.unwrap());
  }

  #[tokio::test]
  async fn task_queue_middle_future_dropped_while_permit_acquired() {
    let task_queue = TaskQueue::default();

    let fut1 = task_queue.acquire();
    let fut2 = task_queue.acquire();
    let fut3 = task_queue.acquire();

    // should not hang
    drop(fut2);
    drop(fut1.await);
    drop(fut3.await);
  }
}
