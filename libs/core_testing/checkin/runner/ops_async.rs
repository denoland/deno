// Copyright 2018-2025 the Deno authors. MIT license.

use super::Output;
use super::TestData;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::V8TaskSpawner;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use std::cell::RefCell;
use std::future::Future;
use std::future::poll_fn;
use std::rc::Rc;

#[op2]
pub fn op_task_submit(
  state: &mut OpState,
  #[scoped] f: v8::Global<v8::Function>,
) {
  state.borrow_mut::<V8TaskSpawner>().spawn(move |scope| {
    let f = v8::Local::new(scope, f);
    let recv = v8::undefined(scope);
    f.call(scope, recv.into(), &[]);
  });
}

#[op2]
pub async fn op_async_yield() {
  tokio::task::yield_now().await
}

#[op2(fast)]
pub fn op_async_barrier_create(
  state: &mut OpState,
  #[string] name: String,
  count: u32,
) {
  let barrier = Rc::new(tokio::sync::Barrier::new(count as _));
  state.borrow_mut::<TestData>().insert(name, barrier);
}

#[op2]
pub fn op_async_barrier_await(
  state: &OpState,
  #[string] name: String,
) -> impl Future<Output = ()> + use<> {
  let test_data = state.borrow::<TestData>();
  let barrier: &Rc<tokio::sync::Barrier> = test_data.get(name);
  let barrier = barrier.clone();
  async move {
    barrier.wait().await;
  }
}

#[op2]
pub async fn op_async_spin_on_state(state: Rc<RefCell<OpState>>) {
  poll_fn(|cx| {
    // Ensure that we never get polled when the state has been emptied
    state.borrow().borrow::<Output>();
    cx.waker().wake_by_ref();
    std::task::Poll::Pending
  })
  .await
}

pub struct TestResource {
  value: u32,
}

unsafe impl GarbageCollected for TestResource {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"TestResource"
  }
}

#[op2]
#[cppgc]
pub async fn op_async_make_cppgc_resource() -> TestResource {
  TestResource { value: 42 }
}

#[op2]
#[smi]
pub async fn op_async_get_cppgc_resource(
  #[cppgc] resource: &TestResource,
) -> u32 {
  resource.value
}

#[op2]
pub fn op_async_never_resolves() -> impl Future<Output = ()> {
  std::future::pending::<()>()
}

#[op2(async(fake))]
pub fn op_async_fake() -> Result<u32, JsErrorBox> {
  Ok(1)
}

#[op2(promise_id)]
pub async fn op_async_promise_id(#[smi] promise_id: u32) -> u32 {
  promise_id
}
