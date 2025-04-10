// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::error::ResourceError;
use deno_core::op2;
use deno_core::v8;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::ResourceId;

pub struct AsyncId(i64);

impl Default for AsyncId {
  // `kAsyncIdCounter` should start at `1` because that'll be the id the execution
  // context during bootstrap.
  fn default() -> Self {
    Self(1)
  }
}

impl AsyncId {
  // Increment the internal id counter and return the value.
  fn next(&mut self) -> i64 {
    self.0 += 1;
    self.0
  }
}

fn next_async_id(state: &mut OpState) -> i64 {
  let async_id = state.borrow_mut::<AsyncId>().next();
  async_id
}

#[op2(fast)]
pub fn op_node_new_async_id(state: &mut OpState) -> f64 {
  next_async_id(state) as f64
}

pub struct AsyncWrap {
  provider: i32,
  async_id: i64,
}

impl GarbageCollected for AsyncWrap {}

impl AsyncWrap {
  pub(crate) fn create(state: &mut OpState, provider: i32) -> Self {
    let async_id = next_async_id(state);

    Self { provider, async_id }
  }
}

#[op2(base)]
impl AsyncWrap {
  #[getter]
  fn provider(&self) -> i32 {
    self.provider
  }

  #[fast]
  fn get_async_id(&self) -> f64 {
    self.async_id as f64
  }

  #[fast]
  fn get_provider_type(&self) -> i32 {
    self.provider
  }
}

#[derive(Copy, Clone, PartialEq, Eq, Default)]
enum State {
  #[default]
  Initialized,
  Closing,
  Closed,
}

pub struct HandleWrap {
  handle: Option<ResourceId>,
  state: Rc<Cell<State>>,
}

impl GarbageCollected for HandleWrap {}

impl HandleWrap {
  pub(crate) fn create(handle: Option<ResourceId>) -> Self {
    Self {
      handle,
      state: Rc::new(Cell::new(State::Initialized)),
    }
  }

  fn is_alive(&self) -> bool {
    self.state.get() != State::Closed
  }
}

static ON_CLOSE_STR: deno_core::FastStaticString =
  deno_core::ascii_str!("_onClose");

#[op2(inherit = AsyncWrap)]
impl HandleWrap {
  #[constructor]
  #[cppgc]
  fn new(
    state: &mut OpState,
    #[smi] provider: i32,
    #[smi] handle: Option<ResourceId>,
  ) -> (AsyncWrap, HandleWrap) {
    (
      AsyncWrap::create(state, provider),
      HandleWrap::create(handle),
    )
  }

  // Ported from Node.js
  //
  // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L65-L85
  #[reentrant]
  fn close(
    &self,
    op_state: Rc<RefCell<OpState>>,
    #[this] this: v8::Global<v8::Object>,
    scope: &mut v8::HandleScope,
    #[global] cb: Option<v8::Global<v8::Function>>,
  ) -> Result<(), ResourceError> {
    if self.state.get() != State::Initialized {
      return Ok(());
    }

    let state = self.state.clone();
    // This effectively mimicks Node's OnClose callback.
    //
    // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L135-L157
    let on_close = move |scope: &mut v8::HandleScope| {
      assert!(state.get() == State::Closing);
      state.set(State::Closed);

      // Workaround for https://github.com/denoland/deno/pull/24656
      //
      // We need to delay 'cb' at least 2 ticks to avoid "close" event happening before "error"
      // event in net.Socket.
      //
      // This is a temporary solution. We should support async close like `uv_close`.
      if let Some(cb) = cb {
        let recv = v8::undefined(scope);
        cb.open(scope).call(scope, recv.into(), &[]);
      }
    };

    uv_close(scope, op_state, this, on_close);
    self.state.set(State::Closing);

    Ok(())
  }

  // Ported from Node.js
  //
  // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L58-L62
  #[fast]
  fn has_ref(&self, state: &mut OpState) -> bool {
    if let Some(handle) = self.handle {
      return state.has_ref(handle);
    }

    true
  }

  // Ported from Node.js
  //
  // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L40-L46
  #[fast]
  #[rename("r#ref")]
  fn ref_(&self, state: &mut OpState) {
    if self.is_alive() {
      if let Some(handle) = self.handle {
        state.uv_ref(handle);
      }
    }
  }

  // Ported from Node.js
  //
  // https://github.com/nodejs/node/blob/038d82980ab26cd79abe4409adc2fecad94d7c93/src/handle_wrap.cc#L49-L55
  #[fast]
  fn unref(&self, state: &mut OpState) {
    if self.is_alive() {
      if let Some(handle) = self.handle {
        state.uv_unref(handle);
      }
    }
  }
}

fn uv_close<F>(
  scope: &mut v8::HandleScope,
  op_state: Rc<RefCell<OpState>>,
  this: v8::Global<v8::Object>,
  on_close: F,
) where
  F: FnOnce(&mut v8::HandleScope) + 'static,
{
  // Call _onClose() on the JS handles. Not needed for Rust handles.
  let this = v8::Local::new(scope, this);
  let on_close_str = ON_CLOSE_STR.v8_string(scope).unwrap();
  let onclose = this.get(scope, on_close_str.into());

  if let Some(onclose) = onclose {
    let fn_: v8::Local<v8::Function> = onclose.try_into().unwrap();
    fn_.call(scope, this.into(), &[]);
  }

  op_state
    .borrow()
    .borrow::<deno_core::V8TaskSpawner>()
    .spawn(on_close);
}

#[cfg(test)]
mod tests {
  use std::future::poll_fn;
  use std::task::Poll;

  use deno_core::JsRuntime;
  use deno_core::RuntimeOptions;

  async fn js_test(source_code: &'static str) {
    deno_core::extension!(
      test_ext,
      objects = [super::AsyncWrap, super::HandleWrap,],
      state = |state| {
        state.put::<super::AsyncId>(super::AsyncId::default());
      }
    );

    let mut runtime = JsRuntime::new(RuntimeOptions {
      extensions: vec![test_ext::init_ops()],
      ..Default::default()
    });

    poll_fn(move |cx| {
      runtime
        .execute_script("file://handle_wrap_test.js", source_code)
        .unwrap();

      let result = runtime.poll_event_loop(cx, Default::default());
      assert!(matches!(result, Poll::Ready(Ok(()))));
      Poll::Ready(())
    })
    .await;
  }

  #[tokio::test]
  async fn test_handle_wrap() {
    js_test(
      r#"
        const { HandleWrap } = Deno.core.ops;

        let called = false;
        class MyHandleWrap extends HandleWrap {
          constructor() {
            super(0, null);
          }

          _onClose() {
            called = true;
          }
        }

        const handleWrap = new MyHandleWrap();
        handleWrap.close();

        if (!called) {
          throw new Error("HandleWrap._onClose was not called");
        }
      "#,
    )
    .await;
  }
}
