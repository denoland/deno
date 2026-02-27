// Copyright 2018-2025 the Deno authors. MIT license.

use crate::OpId;
use crate::PromiseId;
use bit_set::BitSet;
use deno_error::JsErrorClass;
use std::future::Future;
use std::task::Context;
use std::task::Poll;

mod erased_future;
mod future_arena;
mod futures_unordered_driver;
mod op_results;

#[allow(unused)]
pub use futures_unordered_driver::FuturesUnorderedDriver;

pub use self::op_results::OpMappingContext;
pub use self::op_results::OpResult;
use self::op_results::PendingOpInfo;
pub use self::op_results::V8OpMappingContext;
pub use self::op_results::V8RetValMapper;

#[derive(Default)]
/// Returns a set of stats on inflight ops.
pub struct OpInflightStats {
  /// The [`PromiseId`] of any inflight ops by each [`OpId`].
  pub(super) ops: Box<[PendingOpInfo]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpScheduling {
  Eager,
  Lazy,
  Deferred,
}

/// `OpDriver` encapsulates the interface for handling operations within Deno's runtime.
///
/// This trait defines methods for submitting ops and polling readiness inside of the
/// event loop.
///
/// Ops are always submitted with a mapping function that can convert the output of the
/// op to the underlying [`OpMappingContext`] output type. In the case of V8, this is a
/// function that creates [`v8::Local`] values.
///
/// The driver takes an optional [`OpMappingContext`] implementation, which defaults to
/// one compatible with v8. This is used solely for testing purposes.
pub(crate) trait OpDriver<C: OpMappingContext = V8OpMappingContext>:
  Default
{
  /// Submits an operation that is expected to complete successfully without errors.
  fn submit_op_infallible<R: 'static, const LAZY: bool, const DEFERRED: bool>(
    &self,
    op_id: OpId,
    promise_id: i32,
    op: impl Future<Output = R> + 'static,
    rv_map: C::MappingFn<R>,
  ) -> Option<R>;

  /// Submits an operation that is expected to complete successfully without errors.
  #[inline(always)]
  fn submit_op_infallible_scheduling<R: 'static>(
    &self,
    scheduling: OpScheduling,
    op_id: OpId,
    promise_id: i32,
    op: impl Future<Output = R> + 'static,
    rv_map: C::MappingFn<R>,
  ) -> Option<R> {
    match scheduling {
      OpScheduling::Eager => self
        .submit_op_infallible::<R, false, false>(op_id, promise_id, op, rv_map),
      OpScheduling::Lazy => self
        .submit_op_infallible::<R, true, false>(op_id, promise_id, op, rv_map),
      OpScheduling::Deferred => self
        .submit_op_infallible::<R, false, true>(op_id, promise_id, op, rv_map),
    }
  }

  /// Submits an operation that may produce errors during execution.
  ///
  /// This method is similar to `submit_op_infallible` but is used when the op
  /// might return an error (`Result`).
  fn submit_op_fallible<
    R: 'static,
    E: JsErrorClass + 'static,
    const LAZY: bool,
    const DEFERRED: bool,
  >(
    &self,
    op_id: OpId,
    promise_id: i32,
    op: impl Future<Output = Result<R, E>> + 'static,
    rv_map: C::MappingFn<R>,
  ) -> Option<Result<R, E>>;

  /// Submits an operation that is expected to complete successfully without errors.
  #[inline(always)]
  #[allow(clippy::too_many_arguments)]
  fn submit_op_fallible_scheduling<R: 'static, E: JsErrorClass + 'static>(
    &self,
    scheduling: OpScheduling,
    op_id: OpId,
    promise_id: i32,
    op: impl Future<Output = Result<R, E>> + 'static,
    rv_map: C::MappingFn<R>,
  ) -> Option<Result<R, E>> {
    match scheduling {
      OpScheduling::Eager => self.submit_op_fallible::<R, E, false, false>(
        op_id, promise_id, op, rv_map,
      ),
      OpScheduling::Lazy => self
        .submit_op_fallible::<R, E, true, false>(op_id, promise_id, op, rv_map),
      OpScheduling::Deferred => self
        .submit_op_fallible::<R, E, false, true>(op_id, promise_id, op, rv_map),
    }
  }

  #[allow(clippy::type_complexity)]
  /// Polls the readiness of the op driver.
  fn poll_ready(
    &self,
    cx: &mut Context,
  ) -> Poll<(PromiseId, OpId, OpResult<C>)>;

  /// Return the number of in-flight ops currently being polled, or waiting for their results to be
  /// picked up in `poll_ready`.
  fn len(&self) -> usize;

  /// Shuts down this driver, preventing any tasks from being polled beyond this point. It is legal
  /// to call this shutdown method multiple times, and further calls have no effect.
  fn shutdown(&self);

  /// Capture the statistics of in-flight ops, for op sanitizer purposes. Note that this
  /// may not be a cheap operation and calling it large number of times (for example, in an
  /// event loop) may cause slowdowns.
  ///
  /// If `op_exclusions` is passed to this function, any ops in the set are excluded from the stats.
  ///
  /// A [`PromiseId`] will appear in this list until its results have been picked up in `poll_ready`.
  fn stats(&self, op_exclusions: &BitSet) -> OpInflightStats;
}

#[cfg(test)]
mod tests {
  use super::op_results::*;
  use super::*;
  use bit_set::BitSet;
  use deno_error::JsErrorBox;
  use rstest::rstest;
  use std::future::poll_fn;
  use std::marker::PhantomData;

  struct TestMappingContext {}
  impl<'s, 'i> OpMappingContextLifetime<'s, 'i> for TestMappingContext {
    type Context
      = PhantomData<&'s mut &'i mut ()>
    where
      'i: 's;
    type Result = String;
    type MappingError = anyhow::Error;

    fn map_error(
      _context: &mut Self::Context,
      err: JsErrorBox,
    ) -> Self::Result {
      format!("{err:?}")
    }

    fn map_mapping_error(
      _context: &mut Self::Context,
      err: Self::MappingError,
    ) -> Self::Result {
      format!("{err:?}")
    }
  }
  impl OpMappingContext for TestMappingContext {
    type MappingFn<R: 'static> = for<'s> fn(R) -> Result<String, anyhow::Error>;
    fn erase_mapping_fn<R: 'static>(f: Self::MappingFn<R>) -> *const fn() {
      f as _
    }

    fn unerase_mapping_fn<'s, 'i, R: 'static>(
      f: *const fn(),
      _context: &mut <Self as OpMappingContextLifetime<'s, 'i>>::Context,
      r: R,
    ) -> UnmappedResult<'s, 'i, Self> {
      let f: Self::MappingFn<R> = unsafe { std::mem::transmute(f) };
      f(r)
    }
  }

  fn submit_task(
    driver: &impl OpDriver<TestMappingContext>,
    scheduling: OpScheduling,
    id: usize,
    op: impl Future<Output = i32> + 'static,
  ) {
    assert_eq!(
      None,
      driver.submit_op_infallible_scheduling(
        scheduling,
        1234,
        id as _,
        op,
        |r| { Ok(format!("{r}")) }
      )
    );
  }

  fn submit_task_eager_ready(
    driver: &impl OpDriver<TestMappingContext>,
    id: usize,
    op: impl Future<Output = i32> + 'static,
    result: i32,
  ) {
    assert_eq!(
      Some(result),
      driver.submit_op_infallible_scheduling(
        OpScheduling::Eager,
        1234,
        id as _,
        op,
        |r| { Ok(format!("{r}")) }
      )
    );
  }

  async fn reap_task(
    driver: &impl OpDriver<TestMappingContext>,
    bitset: &mut BitSet,
    expected: &str,
  ) {
    let (promise_id, op_id, result) = poll_fn(|cx| driver.poll_ready(cx)).await;
    assert!(bitset.insert(promise_id as usize));
    assert_eq!(1234, op_id);
    assert_eq!(expected, &(result.unwrap(&mut PhantomData).unwrap()));
  }

  #[rstest]
  #[case::futures_unordered(FuturesUnorderedDriver::<TestMappingContext>::default()
  )]
  fn test_driver<D: OpDriver<TestMappingContext>>(
    #[case] driver: D,
    #[values(2, 16)] count: usize,
    #[values(OpScheduling::Eager, OpScheduling::Lazy, OpScheduling::Deferred)]
    scheduling: OpScheduling,
  ) {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .build()
      .unwrap();
    runtime.block_on(async {
      for i in 0..count {
        if scheduling == OpScheduling::Eager {
          submit_task_eager_ready(&driver, i, async { 1 }, 1);
        } else {
          submit_task(&driver, scheduling, i, async { 1 });
        }
      }
      if scheduling != OpScheduling::Eager {
        let mut bitset = BitSet::default();
        for i in 0..count {
          assert_eq!(driver.len(), count - i);
          reap_task(&driver, &mut bitset, "1").await;
        }
        assert_eq!(bitset.len(), count);
      }
    });
  }

  #[rstest]
  #[case::futures_unordered(FuturesUnorderedDriver::<TestMappingContext>::default()
  )]
  fn test_driver_yield<D: OpDriver<TestMappingContext>>(
    #[case] driver: D,
    #[values(2, 16)] count: usize,
    #[values(1, 5)] outer: usize,
    #[values(OpScheduling::Eager, OpScheduling::Lazy, OpScheduling::Deferred)]
    scheduling: OpScheduling,
  ) {
    async fn task() -> i32 {
      let v = [0_u8, 1, 2, 3];
      for i in &v {
        for _ in 0..*i {
          tokio::task::yield_now().await;
        }
      }
      v.len() as _
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
      .build()
      .unwrap();
    runtime.block_on(async {
      for _ in 0..outer {
        for i in 0..count {
          submit_task(&driver, scheduling, i, task());
        }
        let mut bitset = BitSet::default();
        for i in 0..count {
          assert_eq!(driver.len(), count - i);
          reap_task(&driver, &mut bitset, "4").await;
        }
        assert_eq!(bitset.len(), count);
      }
    });
  }

  #[rstest]
  #[case::futures_unordered(FuturesUnorderedDriver::<TestMappingContext>::default()
  )]
  fn test_driver_large<D: OpDriver<TestMappingContext>>(
    #[case] driver: D,
    #[values(2, 16)] count: usize,
    #[values(OpScheduling::Eager, OpScheduling::Lazy, OpScheduling::Deferred)]
    scheduling: OpScheduling,
  ) {
    async fn task() -> i32 {
      let mut v = [0; 10 * 1024];
      #[allow(clippy::needless_range_loop)]
      for i in 0..10 {
        tokio::task::yield_now().await;
        v[i] = 1;
      }
      let mut s = 0;
      for i in v {
        s += i;
      }
      s
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
      .build()
      .unwrap();
    runtime.block_on(async {
      for i in 0..count {
        submit_task(&driver, scheduling, i, task());
      }
      let mut bitset = BitSet::default();
      for i in 0..count {
        assert_eq!(driver.len(), count - i);
        reap_task(&driver, &mut bitset, "10").await;
      }
      assert_eq!(bitset.len(), count);
    });
  }

  #[cfg(not(miri))] // needs I/O
  #[rstest]
  #[case::futures_unordered(FuturesUnorderedDriver::<TestMappingContext>::default()
  )]
  fn test_driver_io<D: OpDriver<TestMappingContext>>(
    #[case] driver: D,
    #[values(2, 16)] count: usize,
    #[values(OpScheduling::Eager, OpScheduling::Lazy, OpScheduling::Deferred)]
    scheduling: OpScheduling,
  ) {
    async fn task() -> i32 {
      use tokio::net::TcpSocket;
      let socket = TcpSocket::new_v4().unwrap();
      socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
      let listen = socket.listen(1).unwrap();
      let connect = TcpSocket::new_v4().unwrap();
      let f = tokio::spawn(connect.connect(listen.local_addr().unwrap()));
      listen.accept().await.unwrap();
      f.await.unwrap().unwrap();
      1
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_io()
      .build()
      .unwrap();
    runtime.block_on(async {
      for i in 0..count {
        submit_task(&driver, scheduling, i, task());
      }
      let mut bitset = BitSet::default();
      for i in 0..count {
        assert_eq!(driver.len(), count - i);
        reap_task(&driver, &mut bitset, "1").await;
      }
      assert_eq!(bitset.len(), count);
    });
  }
}
