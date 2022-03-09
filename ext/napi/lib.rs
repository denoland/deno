use deno_core::futures::channel::mpsc;
use deno_core::napi;
use deno_core::napi::NapiState;
use deno_core::napi::PendingNapiAsyncWork;
use deno_core::Extension;
use deno_core::futures::StreamExt;
use deno_core::napi::ThreadSafeFunctionStatus;
use std::task::Poll;

pub fn init() -> Extension {
  Extension::builder()
    .event_loop_middleware(|op_state, cx| {
      let mut state_borrow = op_state.borrow_mut();
      let state = state_borrow.borrow_mut::<NapiState>();

      while let Poll::Ready(Some(async_work_fut)) =
        state.napi_async_work_receiver.poll_next_unpin(cx)
      {
        state.pending_napi_async_work.push(async_work_fut);
      }

      while let Poll::Ready(Some(tsfn_status)) =
        state.napi_threadsafe_function_reciever.poll_next_unpin(cx)
      {
        match tsfn_status {
          napi::ThreadSafeFunctionStatus::Alive => {
            state.active_threadsafe_functions += 1
          }
          napi::ThreadSafeFunctionStatus::Dead => {
            state.active_threadsafe_functions -= 1
          }
        };
      }

      // `work` can call back into the runtime. It can also schedule an async task
      // but we don't know that now. We need to make the runtime re-poll to make
      // sure no pending NAPI tasks exist.
      let mut maybe_scheduling = false;
      if state.active_threadsafe_functions > 0 {
        maybe_scheduling = true;
      }

      drop(state);
      drop(state_borrow);

      loop {
        let maybe_work = {
          let mut op_state_borrow = op_state.borrow_mut();
          let state_borrow = op_state_borrow.borrow_mut::<NapiState>();
          state_borrow.pending_napi_async_work.pop()
        };

        if let Some(work) = maybe_work {
          work();
          maybe_scheduling = true;
        } else {
          break;
        }
      }

      maybe_scheduling
    })
    .state(|state| {
      let (napi_async_work_sender, napi_async_work_receiver) =
        mpsc::unbounded::<PendingNapiAsyncWork>();
      let (napi_threadsafe_function_sender, napi_threadsafe_function_reciever) =
        mpsc::unbounded::<ThreadSafeFunctionStatus>();
      state.put(NapiState {
        pending_napi_async_work: Vec::new(),
        napi_async_work_sender,
        napi_async_work_receiver,
        napi_threadsafe_function_sender,
        napi_threadsafe_function_reciever,
        active_threadsafe_functions: 0,
      });

      Ok(())
    })
    .build()
}
