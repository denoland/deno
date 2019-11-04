// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;
use futures::Future;
use std;
use std::time::Duration;
use std::time::Instant;
use tokio::timer::Interval;
use crate::resources;
use crate::resources::CoreResource;
use crate::deno_error::bad_resource;
use crate::futures::Stream;
use futures::future::Shared;
use futures::stream::StreamFuture;
use futures::future::poll_fn;
use futures::Poll;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "global_timer_stop",
    s.core_op(json_op(s.stateful_op(op_global_timer_stop))),
  );
  i.register_op(
    "global_timer",
    s.core_op(json_op(s.stateful_op(op_global_timer))),
  );
  i.register_op("now", s.core_op(json_op(s.stateful_op(op_now))));
  i.register_op("set_interval", s.core_op(json_op(s.stateful_op(op_set_interval))));
  i.register_op("await_interval", s.core_op(json_op(s.stateful_op(op_await_interval))));
  i.register_op("clear_interval", s.core_op(json_op(s.stateful_op(op_clear_interval))));
}

struct IntervalResource {
  interval: Interval
}

impl CoreResource for IntervalResource {
  fn inspect_repr(&self) -> &str { "interval" }
}


#[derive(Deserialize)]
struct SetIntervalArgs {
  duration: u64,
}

fn op_set_interval(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SetIntervalArgs = serde_json::from_value(args)?;
  eprintln!("duration {}", args.duration);
  let duration = Duration::from_millis(args.duration);
  let at = Instant::now() + duration.clone();
  let interval = Interval::new(at, duration);
  let interval_resource = IntervalResource { interval };
  let mut table = resources::lock_resource_table();
  let rid = table.add(Box::new(interval_resource));

  Ok(JsonOp::Sync(json!(rid)))
}

#[derive(Deserialize)]
struct AwaitIntervalArgs {
  rid: u32,
}

struct AwaitFut {
  rid: u32,
}

impl Future for AwaitFut {
  type Item = Option<Instant>;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut table = resources::lock_resource_table();
    let interval_resource = table.get_mut::<IntervalResource>(self.rid)
      .ok_or_else(bad_resource)?;
    interval_resource.interval.poll().map_err(ErrBox::from)
  }
}
fn op_await_interval(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: AwaitIntervalArgs = serde_json::from_value(args)?;

  let fut = AwaitFut { rid: args.rid };

  eprintln!("awaiting interval");

  let fut = fut
    .map_err(move |e| {
      eprintln!("awaiting interval err {}", e);
      e
    })
    .and_then(move |maybe_now| {
      eprintln!("awaiting interval {:?}", maybe_now);

      Ok(json!(maybe_now.is_none()))
    });

  Ok(JsonOp::Async(Box::new(fut)))
}

fn op_clear_interval(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: AwaitIntervalArgs = serde_json::from_value(args)?;
  let mut table = resources::lock_resource_table();
  let interval_resource = table.close(args.rid).ok_or_else(bad_resource)?;
  drop(interval_resource);
  Ok(JsonOp::Sync(json!({})))
}


fn op_global_timer_stop(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  t.cancel();
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
struct GlobalTimerArgs {
  timeout: u64,
}

fn op_global_timer(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: GlobalTimerArgs = serde_json::from_value(args)?;
  let val = args.timeout;

  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  let deadline = Instant::now() + Duration::from_millis(val);
  let f = t
    .new_timeout(deadline)
    .then(move |_| futures::future::ok(json!({})));

  Ok(JsonOp::Async(Box::new(f)))
}

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
fn op_now(
  state: &ThreadSafeState,
  _args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let seconds = state.start_time.elapsed().as_secs();
  let mut subsec_nanos = state.start_time.elapsed().subsec_nanos();
  let reduced_time_precision = 2_000_000; // 2ms in nanoseconds

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if !state.permissions.allow_hrtime.is_allow() {
    subsec_nanos -= subsec_nanos % reduced_time_precision
  }

  Ok(JsonOp::Sync(json!({
    "seconds": seconds,
    "subsecNanos": subsec_nanos,
  })))
}
