use deno_core::futures::channel::mpsc;
use deno_core::futures::StreamExt;
use deno_core::napi;
use deno_core::napi::NapiState;
use deno_core::napi::PendingNapiAsyncWork;
use deno_core::napi::ThreadSafeFunctionStatus;
use deno_core::Extension;
use std::task::Poll;

pub fn init() -> Extension {
  Extension::builder()
    .event_loop_middleware(|op_state_rc, cx| {
      // `work` can call back into the runtime. It can also schedule an async task
      // but we don't know that now. We need to make the runtime re-poll to make
      // sure no pending NAPI tasks exist.
      let mut maybe_scheduling = false;

      {
        let mut op_state = op_state_rc.borrow_mut();
        let napi_state = op_state.borrow_mut::<NapiState>();

        while let Poll::Ready(Some(async_work_fut)) =
          napi_state.async_work_receiver.poll_next_unpin(cx)
        {
          napi_state.pending_async_work.push(async_work_fut);
        }

        while let Poll::Ready(Some(tsfn_status)) =
          napi_state.threadsafe_function_receiver.poll_next_unpin(cx)
        {
          match tsfn_status {
            napi::ThreadSafeFunctionStatus::Alive => {
              napi_state.active_threadsafe_functions += 1
            }
            napi::ThreadSafeFunctionStatus::Dead => {
              napi_state.active_threadsafe_functions -= 1
            }
          };
        }

        if napi_state.active_threadsafe_functions > 0 {
          maybe_scheduling = true;
        }
      }

      loop {
        let maybe_work = {
          let mut op_state = op_state_rc.borrow_mut();
          let napi_state = op_state.borrow_mut::<NapiState>();
          napi_state.pending_async_work.pop()
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
      let (async_work_sender, async_work_receiver) =
        mpsc::unbounded::<PendingNapiAsyncWork>();
      let (threadsafe_function_sender, threadsafe_function_receiver) =
        mpsc::unbounded::<ThreadSafeFunctionStatus>();
      state.put(NapiState {
        pending_async_work: Vec::new(),
        async_work_sender,
        async_work_receiver,
        threadsafe_function_sender,
        threadsafe_function_receiver,
        active_threadsafe_functions: 0,
      });

      Ok(())
    })
    .build()
}
