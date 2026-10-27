// Copyright 2018-2026 the Deno authors. MIT license.

use std::marker::PhantomData;
use std::ops::DerefMut;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use futures::task::AtomicWaker;

type UnsendTask = Box<dyn FnOnce(&mut v8::PinScope) + 'static>;
type SendTask = Box<dyn FnOnce(&mut v8::PinScope) + Send + 'static>;

/// A host-owned stop request registered in the isolate's typed slots. Local
/// execution timeouts must not clear a concurrent external termination request.
pub struct ExternalExecutionTermination(pub Arc<AtomicBool>);

/// Whether the host has asked this isolate to stop. Unlike
/// `is_execution_terminating`, this survives V8 paths that clear a thrown
/// termination, such as a promise hook run while C++ creates a promise.
pub(crate) fn external_execution_termination_requested(
  isolate: &v8::Isolate,
) -> bool {
  isolate
    .get_slot::<ExternalExecutionTermination>()
    .is_some_and(|request| request.0.load(Ordering::SeqCst))
}

/// Re-request a host stop and make it take effect at once. A requested
/// termination is only thrown at V8's next interrupt check, so until then a
/// caller would report it while nothing is pending, and an op would throw a
/// catchable error in its place. Entering a script runs that check. The
/// request is then queued again: V8 drops a thrown termination once it
/// unwinds to the embedder, and a host stop must also stop later entries.
/// The caller rethrows the termination its `TryCatch` caught.
pub(crate) fn raise_external_execution_termination(
  scope: &mut v8::PinScope<'_, '_>,
) {
  scope.terminate_execution();
  let source = v8::String::new_external_onebyte_static(scope, b"0").unwrap();
  if let Some(script) = v8::Script::compile(scope, source, None) {
    let _ = script.run(scope);
  }
  scope.terminate_execution();
}

/// Cancel a local timeout without losing a host-owned stop request.
/// Returns false when execution must remain terminated.
pub fn cancel_local_execution_termination(isolate: &v8::Isolate) -> bool {
  let externally_terminated =
    || external_execution_termination_requested(isolate);
  if externally_terminated() {
    return false;
  }
  let cancelled = isolate.cancel_terminate_execution();
  // Check after cancellation: checking first would let a host request arriving
  // between that check and cancellation be cleared by the local timeout.
  if externally_terminated() {
    isolate.terminate_execution();
    false
  } else {
    cancelled
  }
}

struct BlockingTask<F, T> {
  callback: Option<F>,
  sender: Option<std::sync::mpsc::SyncSender<T>>,
}

impl<F, T> BlockingTask<F, T>
where
  F: FnOnce(&mut v8::PinScope) -> T,
{
  fn run(mut self, scope: &mut v8::PinScope) {
    let result = self.callback.take().unwrap()(scope);
    _ = self.sender.take().unwrap().send(result);
  }
}

impl<F, T> Drop for BlockingTask<F, T> {
  fn drop(&mut self) {
    // Cancellation must finish destroying borrowed callback state before
    // disconnecting the sender and allowing the calling thread to return.
    drop(self.callback.take());
    drop(self.sender.take());
  }
}

static_assertions::assert_not_impl_any!(V8TaskSpawnerFactory: Send);
static_assertions::assert_not_impl_any!(V8TaskSpawner: Send);
static_assertions::assert_impl_all!(V8CrossThreadTaskSpawner: Send);

/// The [`V8TaskSpawnerFactory`] must be created on the same thread as the thread that runs tasks.
///
/// This factory is not [`Send`] because it may contain `!Send` tasks submitted by a
/// [`V8CrossThreadTaskSpawner`]. It is only safe to send this object to another thread if you plan on
/// submitting [`Send`] tasks to it, which is what [`V8CrossThreadTaskSpawner`] does.
#[derive(Default)]
pub(crate) struct V8TaskSpawnerFactory {
  // TODO(mmastrac): ideally we wouldn't box if we could use arena allocation and a max submission size
  // TODO(mmastrac): we may want to split the Send and !Send tasks
  /// The set of tasks, non-empty if `has_tasks` is set.
  tasks: Mutex<Vec<SendTask>>,
  /// A flag we can poll without any locks.
  has_tasks: AtomicBool,
  closed: AtomicBool,
  /// The polled waker, woken on task submission.
  waker: AtomicWaker,
  /// Mark as `!Send`. See note above.
  _unsend_marker: PhantomData<*const ()>,
}

impl V8TaskSpawnerFactory {
  pub fn new_same_thread_spawner(self: Arc<Self>) -> V8TaskSpawner {
    V8TaskSpawner {
      tasks: self,
      _unsend_marker: PhantomData,
    }
  }

  pub fn new_cross_thread_spawner(self: Arc<Self>) -> V8CrossThreadTaskSpawner {
    V8CrossThreadTaskSpawner { tasks: self }
  }

  /// `false` guarantees that there are no queued tasks, while `true` means that it is likely (but not guaranteed)
  /// that tasks exist.
  ///
  /// Calls should prefer using the waker, but this is left while we rework the event loop.
  pub fn has_pending_tasks(&self) -> bool {
    // Ensure that all reads after this point happen-after we load from the atomic
    self.has_tasks.load(Ordering::Acquire)
  }

  /// Poll this set of tasks, returning a non-empty set of tasks if there have
  /// been any queued, or registering the waker if not.
  pub fn poll_inner(&self, cx: &mut Context) -> Poll<Vec<UnsendTask>> {
    // Check the flag first -- if it's false we definitely have no tasks. AcqRel semantics ensure
    // that this read happens-before the vector read below, and that any writes happen-after the success
    // case.
    if self
      .has_tasks
      .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
      .is_err()
    {
      self.waker.register(cx.waker());
      return Poll::Pending;
    }

    let mut lock = self.tasks.lock().unwrap();
    let tasks = std::mem::take(lock.deref_mut());
    if tasks.is_empty() {
      // Unlikely race lost -- the task submission to the queue and flag are not atomic, so it's
      // possible we ended up with an extra poll here. This only shows up under Miri, but as it is
      // possible we do need to handle it.
      self.waker.register(cx.waker());
      return Poll::Pending;
    }

    // SAFETY: we are removing the Send trait as we return the tasks here to prevent
    // these tasks from accidentally leaking to another thread.
    let tasks =
      unsafe { std::mem::transmute::<Vec<SendTask>, Vec<UnsendTask>>(tasks) };
    Poll::Ready(tasks)
  }

  pub(crate) fn shutdown(&self) {
    let tasks = {
      let mut queue = self.tasks.lock().unwrap();
      self.closed.store(true, Ordering::Release);
      self.has_tasks.store(false, Ordering::Release);
      std::mem::take(&mut *queue)
    };
    // Drop on the owning runtime thread, outside the lock: same-thread tasks
    // may own !Send values, and their destructors may submit another task.
    drop(tasks);
  }

  fn spawn(&self, task: SendTask) {
    let mut queue = self.tasks.lock().unwrap();
    if self.closed.load(Ordering::Acquire) {
      drop(queue);
      drop(task);
      return;
    }
    queue.push(task);
    // TODO(mmastrac): can we skip the mutex here?
    // Release ordering means that the writes in the above lock happen-before the atomic store
    self.has_tasks.store(true, Ordering::Release);
    drop(queue);
    self.waker.wake();
  }
}

/// Allows for submission of v8 tasks on the same thread.
#[derive(Clone)]
pub struct V8TaskSpawner {
  // TODO(mmastrac): can we split the waker into a send and !send one?
  tasks: Arc<V8TaskSpawnerFactory>,
  _unsend_marker: PhantomData<*const ()>,
}

impl V8TaskSpawner {
  /// Spawn a task that runs within the [`crate::JsRuntime`] event loop from the same thread
  /// that the runtime is running on. This function is re-entrant-safe and may be called from
  /// ops, from outside of a [`v8::HandleScope`] in a plain `async`` task, or even from within
  /// another, previously-spawned task.
  ///
  /// The task is handed off to be run the next time the event loop is polled, and there are
  /// no guarantees as to when this may happen.
  ///
  /// # Safety
  ///
  /// The task shares the same [`v8::HandleScope`] as the core event loop, which means that it
  /// must maintain the scope in a valid state to avoid corrupting or destroying the runtime.
  ///
  /// For example, if the code called by this task can raise an exception, the task must ensure
  /// that it calls that code within a new [`v8::TryCatch`] to avoid the exception leaking to the
  /// event loop's [`v8::HandleScope`].
  pub fn spawn<F>(&self, f: F)
  where
    F: FnOnce(&mut v8::PinScope<'_, '_>) + 'static,
  {
    let task: Box<dyn FnOnce(&mut v8::PinScope<'_, '_>)> = Box::new(f);
    // SAFETY: we are transmuting Send into a !Send handle but we guarantee this object will never
    // leave the current thread because `V8TaskSpawner` is !Send.
    let task: Box<dyn FnOnce(&mut v8::PinScope<'_, '_>) + Send> =
      unsafe { std::mem::transmute(task) };
    self.tasks.spawn(task)
  }
}

/// Allows for submission of v8 tasks on any thread.
#[derive(Clone)]
pub struct V8CrossThreadTaskSpawner {
  tasks: Arc<V8TaskSpawnerFactory>,
}

// SAFETY: the underlying V8TaskSpawnerFactory is not Send, but we always submit Send tasks
// to it from this spawner.
unsafe impl Send for V8CrossThreadTaskSpawner {}

impl V8CrossThreadTaskSpawner {
  /// Spawn a task that runs within the [`crate::JsRuntime`] event loop, potentially (but not
  /// required to be) from a different thread than the runtime is running on.
  ///
  /// The task is handed off to be run the next time the event loop is polled, and there are
  /// no guarantees as to when this may happen.
  ///
  /// # Safety
  ///
  /// The task shares the same [`v8::HandleScope`] as the core event loop, which means that it
  /// must maintain the scope in a valid state to avoid corrupting or destroying the runtime.
  ///
  /// For example, if the code called by this task can raise an exception, the task must ensure
  /// that it calls that code within a new [`v8::TryCatch`] to avoid the exception leaking to the
  /// event loop's [`v8::HandleScope`].
  pub fn spawn<F>(&self, f: F)
  where
    F: FnOnce(&mut v8::PinScope<'_, '_>) + Send + 'static,
  {
    self.tasks.spawn(Box::new(f))
  }

  /// Spawn a task that runs within the [`crate::JsRuntime`] event loop from a different thread
  /// than the runtime is running on.
  ///
  /// This function will deadlock if called from the same thread as the [`crate::JsRuntime`], and
  /// there are no checks for this case.
  ///
  /// As this function blocks until the task has run to completion (or panics/deadlocks), it is
  /// safe to borrow data from the local environment and use it within the closure.
  ///
  /// The task is handed off to be run the next time the event loop is polled, and there are
  /// no guarantees as to when this may happen, however the function will not return until the
  /// task has been fully run to completion.
  ///
  /// # Safety
  ///
  /// The task shares the same [`v8::HandleScope`] as the core event loop, which means that it
  /// must maintain the scope in a valid state to avoid corrupting or destroying the runtime.
  ///
  /// For example, if the code called by this task can raise an exception, the task must ensure
  /// that it calls that code within a new [`v8::TryCatch`] to avoid the exception leaking to the
  /// event loop's [`v8::HandleScope`].
  pub fn spawn_blocking<'a, F, T>(&self, f: F) -> T
  where
    F: FnOnce(&mut v8::PinScope) -> T + Send + 'a,
    T: Send + 'a,
  {
    self
      .try_spawn_blocking(f)
      .expect("JavaScript runtime has stopped")
  }

  /// Whether the runtime has shut down this spawner. After that its isolate
  /// may already be gone, so callers must not enter it.
  pub fn is_closed(&self) -> bool {
    self.tasks.closed.load(Ordering::Acquire)
  }

  /// Like `spawn_blocking`, but returns an error when runtime shutdown cancels
  /// the callback before it executes. This is safe for FFI callers that cannot
  /// unwind through a native callback boundary.
  pub fn try_spawn_blocking<'a, F, T>(
    &self,
    f: F,
  ) -> Result<T, std::sync::mpsc::RecvError>
  where
    F: FnOnce(&mut v8::PinScope) -> T + Send + 'a,
    T: Send + 'a,
  {
    let (tx, rx) = std::sync::mpsc::sync_channel(0);
    let task = BlockingTask {
      callback: Some(f),
      sender: Some(tx),
    };
    let task: Box<dyn FnOnce(&mut v8::PinScope<'_, '_>) + Send> =
      Box::new(move |scope| task.run(scope));
    // SAFETY: The receive cannot finish until execution completes or cancellation
    // destroys the borrowed callback before disconnecting its sender. Thus no
    // callback capture can outlive this call, despite the erased lifetime.
    let task: SendTask = unsafe { std::mem::transmute(task) };
    self.tasks.spawn(task);
    rx.recv()
  }
}

#[cfg(test)]
mod tests {
  use std::future::poll_fn;

  use tokio::task::LocalSet;

  use super::*;

  // Needs a real V8 isolate, which Miri cannot run.
  #[cfg(not(miri))]
  #[test]
  fn runtime_drop_releases_queued_blocking_callbacks() {
    let runtime = crate::JsRuntime::new(Default::default());
    let spawner = runtime
      .op_state()
      .borrow()
      .borrow::<V8CrossThreadTaskSpawner>()
      .clone();
    let factory = spawner.tasks.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
      let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          spawner.spawn_blocking(|_| 7)
        }));
      sender.send(result.is_err()).unwrap();
    });
    let deadline =
      std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !factory.has_pending_tasks() && std::time::Instant::now() < deadline {
      std::thread::yield_now();
    }
    assert!(
      factory.has_pending_tasks(),
      "callback must be queued before drop"
    );
    drop(runtime);
    let stopped = receiver.recv_timeout(std::time::Duration::from_millis(100));
    if stopped.is_err() {
      // This test callback ignores its scope and captures no runtime handles,
      // so it is safe to execute solely for cleanup in a fresh runtime. The
      // broken implementation borrows its sender, so dropping it cannot wake
      // the waiting thread.
      let tasks = std::mem::take(&mut *factory.tasks.lock().unwrap());
      let mut cleanup_runtime = crate::JsRuntime::new(Default::default());
      crate::scope!(scope, &mut cleanup_runtime);
      for task in tasks {
        task(scope);
      }
    }
    thread.join().unwrap();
    assert!(
      stopped.is_ok(),
      "runtime drop must release queued blocking callbacks"
    );
    assert!(stopped.unwrap(), "shutdown must cancel rather than execute");
  }

  // https://github.com/tokio-rs/tokio/issues/6155
  #[test]
  #[cfg(not(all(miri, target_os = "linux")))]
  fn test_spawner_serial() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
      .worker_threads(1)
      .build()
      .unwrap();
    runtime.block_on(async {
      let factory = Arc::<V8TaskSpawnerFactory>::default();
      let cross_thread_spawner = factory.clone().new_cross_thread_spawner();
      let local_set = LocalSet::new();

      const COUNT: usize = 1000;

      let task = runtime.spawn(async move {
        for _ in 0..COUNT {
          cross_thread_spawner.spawn(|_| {});
        }
      });

      local_set.spawn_local(async move {
        let mut count = 0;
        loop {
          count += poll_fn(|cx| factory.poll_inner(cx)).await.len();
          if count >= COUNT {
            break;
          }
        }
      });

      local_set.await;
      _ = task.await;
    });
  }

  // https://github.com/tokio-rs/tokio/issues/6155
  #[test]
  #[cfg(not(all(miri, target_os = "linux")))]
  fn test_spawner_parallel() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
      .worker_threads(1)
      .build()
      .unwrap();
    runtime.block_on(async {
      let factory = Arc::<V8TaskSpawnerFactory>::default();
      let cross_thread_spawner = factory.clone().new_cross_thread_spawner();
      let local_set = LocalSet::new();

      const COUNT: usize = 100;
      let mut tasks = vec![];
      for _ in 0..COUNT {
        let cross_thread_spawner = cross_thread_spawner.clone();
        tasks.push(runtime.spawn(async move {
          cross_thread_spawner.spawn(|_| {});
        }));
      }

      local_set.spawn_local(async move {
        let mut count = 0;
        loop {
          count += poll_fn(|cx| factory.poll_inner(cx)).await.len();
          if count >= COUNT {
            break;
          }
        }
      });

      local_set.await;
      for task in tasks.drain(..) {
        _ = task.await;
      }
    });
  }
}
