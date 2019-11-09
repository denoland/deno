// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::ops::json_op;
use crate::resources;
use crate::resources::Resource;
use crate::state::ThreadSafeState;
use deno::*;
use futures::Future;
use futures::Poll;
use std;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::oneshot;
use tokio::timer::Delay;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op(
    "set_timeout",
    s.core_op(json_op(s.stateful_op(op_set_timeout))),
  );
  i.register_op(
    "poll_timeout",
    s.core_op(json_op(s.stateful_op(op_poll_timeout))),
  );
  i.register_op(
    "clear_timeout",
    s.core_op(json_op(s.stateful_op(op_clear_timeout))),
  );
  i.register_op("now", s.core_op(json_op(s.stateful_op(op_now))));
}

struct TimeoutResource {
  delay: Delay,
  sender: Option<oneshot::Sender<()>>,
  receiver: Option<oneshot::Receiver<()>>,
}

impl Resource for TimeoutResource {}

#[derive(Deserialize)]
struct SetTimeoutArgs {
  timeout: u64,
}

fn op_set_timeout(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SetTimeoutArgs = serde_json::from_value(args)?;
  let deadline = Instant::now() + Duration::from_millis(args.timeout);
  let delay = Delay::new(deadline);
  let (sender, receiver) = oneshot::channel();
  let timeout_resource = TimeoutResource {
    delay,
    sender: Some(sender),
    receiver: Some(receiver),
  };
  let mut table = resources::lock_resource_table();
  let rid = table.add("timeout", Box::new(timeout_resource));
  Ok(JsonOp::Sync(json!(rid)))
}

struct PollTimeout {
  rid: u32,
}

impl Future for PollTimeout {
  type Item = ();
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut table = resources::lock_resource_table();
    let interval_resource = table
      .get_mut::<TimeoutResource>(self.rid)
      .ok_or_else(bad_resource)?;
    interval_resource.delay.poll().map_err(ErrBox::from)
  }
}

#[derive(Deserialize)]
struct TimeoutArgs {
  rid: u32,
}

fn op_poll_timeout(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: TimeoutArgs = serde_json::from_value(args)?;
  let fut = PollTimeout { rid: args.rid };
  let mut table = resources::lock_resource_table();
  let interval_resource = table
    .get_mut::<TimeoutResource>(args.rid)
    .ok_or_else(bad_resource)?;
  let rx = interval_resource.receiver.take().unwrap();
  let rx_fut = rx.map_err(ErrBox::from);
  let fut = fut.select(rx_fut).then(|_| Ok(())).and_then(move |_| {
    let mut table = resources::lock_resource_table();
    table.close(args.rid);
    Ok(json!({}))
  });
  Ok(JsonOp::Async(Box::new(fut)))
}

fn op_clear_timeout(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: TimeoutArgs = serde_json::from_value(args)?;
  let mut table = resources::lock_resource_table();
  let interval_resource = table
    .get_mut::<TimeoutResource>(args.rid)
    .ok_or_else(bad_resource)?;
  if let Some(sender) = interval_resource.sender.take() {
    sender.send(()).ok();
  }
  table.close(args.rid);
  Ok(JsonOp::Sync(json!({})))
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
  let elapsed = state.start_time.elapsed();
  let seconds = elapsed.as_secs();
  let mut subsec_nanos = elapsed.subsec_nanos();
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
