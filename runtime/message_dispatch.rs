// Copyright 2018-2026 the Deno authors. MIT license.

//! Rust-driven worker / `MessagePort` message delivery.
//!
//! In a latency-bound worker pattern (request/response, ping-pong) the steady
//! state is "one message arrives, dispatch it, wait for the next reply". The
//! classic JS receive loop (`await op_*_recv_message()`) pays a fresh async op,
//! a JS `Promise` allocation, and a microtask checkpoint *per message* for that
//! — the dominant fixed cost measured in deno#11561 / deno#35025.
//!
//! Instead, a receive loop registers its port and a dispatcher function in the
//! `MessageDispatchTable` (see `deno_web`). [`install_message_dispatch`] wires
//! [`drive_message_dispatch`] as the runtime's per-tick event-loop callback, so
//! every event-loop iteration drains each registered port's queue directly and
//! invokes the dispatcher from Rust — no per-message `Promise`, no per-message
//! microtask checkpoint, mirroring Node's `uv_async` "wake, drain, emit" model.
//!
//! A still-pending recv op (now resolving only when the channel *closes*) stays
//! the keep-alive / ref-unref anchor, so worker lifecycle semantics (idle exit,
//! `ref`/`unref`, terminate ordering) are unchanged.

use std::cell::RefCell;
use std::rc::Rc;
use std::task::Context;

use deno_core::JsRuntime;
use deno_core::OpState;
use deno_core::error::CoreError;
use deno_core::error::JsError;
use deno_core::v8;
use deno_web::JsMessageData;
use deno_web::MessageDispatchTable;
use deno_web::serialize_transferables;

/// Upper bound on messages drained from a single port per event-loop iteration,
/// so a flood on one channel cannot starve the rest of the event loop. When the
/// cap is hit the port re-arms the waker, so draining continues next iteration.
const MAX_DRAIN_PER_PORT: usize = 1000;

/// Installs [`drive_message_dispatch`] as `runtime`'s per-tick event-loop
/// callback. Call once after the runtime is constructed. When no dispatch
/// sources are registered (the common non-worker case) the callback is a single
/// cheap `OpState` lookup.
pub fn install_message_dispatch(runtime: &mut JsRuntime) {
  let op_state = runtime.op_state();
  runtime.set_event_loop_tick_callback(Rc::new(move |scope, cx| {
    drive_message_dispatch(scope, cx, &op_state)
  }));
}

/// Drives Rust-side worker / message-port delivery for one event-loop
/// iteration: drains every registered port and invokes its JS dispatcher
/// directly. An exception thrown by a dispatcher is surfaced as the event-loop
/// error, matching the previous behavior where a throw out of the async receive
/// loop became an unhandled rejection.
pub fn drive_message_dispatch(
  scope: &mut v8::PinScope,
  cx: &mut Context,
  op_state: &Rc<RefCell<OpState>>,
) -> Result<(), CoreError> {
  // 1. Snapshot the registered sources (cheap handle clones) so the OpState
  //    borrow is released before draining and before re-entering JS.
  let sources = {
    let state = op_state.borrow();
    match state.try_borrow::<MessageDispatchTable>() {
      Some(table) => table.snapshot(),
      None => return Ok(()),
    }
  };
  if sources.is_empty() {
    return Ok(());
  }

  // 2. Drain each port and build `JsMessageData`. Transferable serialization
  //    needs `&mut OpState`, so do it here, before re-entering JS.
  struct Drained {
    dispatcher: v8::Global<v8::Function>,
    messages: Vec<JsMessageData>,
  }
  let mut drained: Vec<Drained> = Vec::new();
  {
    let mut state = op_state.borrow_mut();
    for (_id, port, dispatcher) in sources {
      let mut raw = Vec::new();
      let closed = port.poll_drain(cx, MAX_DRAIN_PER_PORT, &mut raw);
      if closed {
        // Wake the keep-alive recv op so it resolves `null` and the JS loop can
        // run its close handling after these last messages are dispatched.
        port.mark_closed();
      }
      if raw.is_empty() {
        continue;
      }
      let messages = raw
        .into_iter()
        .map(|(data, transferables)| {
          let transferables = if transferables.is_empty() {
            Vec::new()
          } else {
            serialize_transferables(&mut state, transferables)
          };
          JsMessageData {
            data,
            transferables,
          }
        })
        .collect();
      drained.push(Drained {
        dispatcher,
        messages,
      });
    }
  }
  if drained.is_empty() {
    return Ok(());
  }

  // 3. Dispatch into JS. One microtask checkpoint (run by the subsequent tick
  //    phases) covers the whole batch instead of one per message.
  v8::tc_scope!(let tc, scope);
  for d in &drained {
    let dispatcher = v8::Local::new(tc, &d.dispatcher);
    let undefined: v8::Local<v8::Value> = v8::undefined(tc).into();
    for message in &d.messages {
      let arg = match deno_core::serde_v8::to_v8(tc, message) {
        Ok(v) => v,
        Err(_) => continue,
      };
      dispatcher.call(tc, undefined, &[arg]);
      if let Some(exception) = tc.exception() {
        let js_error = JsError::from_v8_exception(tc, exception);
        return Err(CoreError::from(js_error));
      }
    }
  }
  Ok(())
}
