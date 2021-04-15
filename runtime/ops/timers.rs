// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use crate::permissions::Permissions;

pub fn init(rt: &mut deno_core::JsRuntime) {
  {
    let op_state = rt.op_state();
    let mut state = op_state.borrow_mut();
    state.put(deno_timers::GlobalTimer::default());
    state.put(deno_timers::StartTime::now());
  }
  super::reg_sync(
    rt,
    "op_global_timer_stop",
    deno_timers::op_global_timer_stop,
  );
  super::reg_sync(
    rt,
    "op_global_timer_start",
    deno_timers::op_global_timer_start,
  );
  super::reg_async(rt, "op_global_timer", deno_timers::op_global_timer);
  super::reg_sync(rt, "op_now", deno_timers::op_now::<Permissions>);
  super::reg_sync(
    rt,
    "op_sleep_sync",
    deno_timers::op_sleep_sync::<Permissions>,
  );
}
