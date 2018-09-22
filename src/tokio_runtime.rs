use futures;
use std;
use tokio_current_thread;
pub use tokio_current_thread::{RunError, Turn, TurnError};
use tokio_executor;
use tokio_reactor;
use tokio_timer;

type Timer = tokio_timer::timer::Timer<tokio_reactor::Reactor>;

#[derive(Debug)]
pub struct Runtime {
  reactor_handle: tokio_reactor::Handle,
  timer_handle: tokio_timer::timer::Handle,
  clock: tokio_timer::clock::Clock,
  executor: tokio_current_thread::CurrentThread<Timer>,
}

#[derive(Debug, Clone)]
pub struct Handle(tokio_current_thread::Handle);

impl Handle {
  // Spawns a future onto the runtime instance that this handle is for.
  #[allow(dead_code)]
  pub fn spawn<F>(&self, future: F) -> Result<(), tokio_executor::SpawnError>
  where
    F: futures::Future<Item = (), Error = ()> + Send + 'static,
  {
    self.0.spawn(future)
  }
}

impl Runtime {
  pub fn new() -> std::io::Result<Runtime> {
    let reactor = tokio_reactor::Reactor::new()?;
    let reactor_handle = reactor.handle();

    let clock = tokio_timer::clock::Clock::new();

    let timer = tokio_timer::timer::Timer::new_with_now(reactor, clock.clone());
    let timer_handle = timer.handle();

    let executor = tokio_current_thread::CurrentThread::new_with_park(timer);

    Ok(Runtime {
      reactor_handle: reactor_handle,
      timer_handle: timer_handle,
      clock,
      executor,
    })
  }

  #[allow(dead_code)]
  pub fn handle(&self) -> Handle {
    Handle(self.executor.handle().clone())
  }

  // Returns true if the event loop is idle, that is, there are no more futures
  // to complete.
  #[allow(dead_code)]
  pub fn is_idle(&self) -> bool {
    self.executor.is_idle()
  }

  // Spawns a future onto this (single-threaded) runtime.
  pub fn spawn<F>(&mut self, future: F) -> &mut Self
  where
    F: futures::Future<Item = (), Error = ()> + 'static,
  {
    self.executor.spawn(future);
    self
  }

  // Runs the event loop until the specified future has completed.
  #[allow(dead_code)]
  pub fn block_on<F>(&mut self, f: F) -> Result<F::Item, F::Error>
  where
    F: futures::Future,
  {
    self.enter(|executor| {
      let ret = executor.block_on(f);
      // Map error to Future::Error.
      ret.map_err(|e| e.into_inner().expect("unexpected execution error"))
    })
  }

  // Runs the event loop until all futures have completed.
  #[allow(dead_code)]
  pub fn run(&mut self) -> Result<(), RunError> {
    self.enter(|executor| executor.run())
  }

  // Runs the event loop until any future has completed or the timeout expires.
  #[allow(dead_code)]
  pub fn turn(
    &mut self,
    max_wait: Option<std::time::Duration>,
  ) -> Result<Turn, TurnError> {
    self.enter(|executor| executor.turn(max_wait))
  }

  fn enter<F, R>(&mut self, f: F) -> R
  where
    F: FnOnce(&mut tokio_current_thread::Entered<Timer>) -> R,
  {
    let Runtime {
      ref reactor_handle,
      ref timer_handle,
      ref clock,
      ref mut executor,
      ..
    } = *self;

    // Binds an executor to this thread.
    let mut enter =
      tokio_executor::enter().expect("Multiple executors at once");

    // This will set the default handle and timer to use inside the closure
    // and run the future.
    tokio_reactor::with_default(&reactor_handle, &mut enter, |enter| {
      tokio_timer::clock::with_default(clock, enter, |enter| {
        tokio_timer::timer::with_default(&timer_handle, enter, |enter| {
          // The TaskExecutor is a fake executor that looks into the
          // current single-threaded executor when used. This is a trick,
          // because we need two mutable references to the executor (one
          // to run the provided future, another to install as the default
          // one). We use the fake one here as the default one.
          let mut default_executor =
            tokio_current_thread::TaskExecutor::current();
          tokio_executor::with_default(&mut default_executor, enter, |enter| {
            let mut executor = executor.enter(enter);
            f(&mut executor)
          })
        })
      })
    })
  }
}
