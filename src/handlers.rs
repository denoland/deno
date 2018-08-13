// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use binding;
use binding::{deno_buf, deno_set_response, DenoC};
use flatbuffers;
use from_c;
use fs;
use futures;
use futures::sync::oneshot;
use msg_generated::deno as msg;
use std;
use std::path::Path;

pub extern "C" fn msg_from_js(d: *const DenoC, buf: deno_buf) {
  let bytes = unsafe { std::slice::from_raw_parts(buf.data_ptr, buf.data_len) };
  let base = msg::GetRootAsBase(bytes);
  let msg_type = base.msg_type();
  match msg_type {
    msg::Any::Start => {
      reply_start(d);
    }
    msg::Any::CodeFetch => {
      // TODO base.msg_as_CodeFetch();
      let msg = msg::CodeFetch::init_from_table(base.msg().unwrap());
      let module_specifier = msg.module_specifier().unwrap();
      let containing_file = msg.containing_file().unwrap();
      handle_code_fetch(d, module_specifier, containing_file);
    }
    msg::Any::CodeCache => {
      // TODO base.msg_as_CodeCache();
      let msg = msg::CodeCache::init_from_table(base.msg().unwrap());
      let filename = msg.filename().unwrap();
      let source_code = msg.source_code().unwrap();
      let output_code = msg.output_code().unwrap();
      handle_code_cache(d, filename, source_code, output_code);
    }
    msg::Any::TimerStart => {
      // TODO base.msg_as_TimerStart();
      let msg = msg::TimerStart::init_from_table(base.msg().unwrap());
      handle_timer_start(d, msg.id(), msg.interval(), msg.delay());
    }
    msg::Any::TimerClear => {
      // TODO base.msg_as_TimerClear();
      let msg = msg::TimerClear::init_from_table(base.msg().unwrap());
      handle_timer_clear(d, msg.id());
    }
    msg::Any::Exit => {
      // TODO base.msg_as_Exit();
      let msg = msg::Exit::init_from_table(base.msg().unwrap());
      std::process::exit(msg.code());
    }
    msg::Any::ReadFileSync => {
      // TODO base.msg_as_ReadFileSync();
      let msg = msg::ReadFileSync::init_from_table(base.msg().unwrap());
      let filename = msg.filename().unwrap();
      handle_read_file_sync(d, filename);
    }
    msg::Any::NONE => {
      assert!(false, "Got message with msg_type == Any_NONE");
    }
    _ => {
      assert!(
        false,
        format!("Unhandled message {}", msg::EnumNameAny(msg_type))
      );
    }
  }
}

fn reply_start(d: *const DenoC) {
  let deno = from_c(d);

  let mut builder = flatbuffers::FlatBufferBuilder::new();

  let argv = deno.argv.iter().map(|s| s.as_str()).collect::<Vec<_>>();
  let argv_off = builder.create_vector_of_strings(argv.as_slice());

  let cwd_path = std::env::current_dir().unwrap();
  let cwd_off = builder.create_string(cwd_path.to_str().unwrap());

  let msg = msg::CreateStartRes(
    &mut builder,
    &msg::StartResArgs {
      cwd: Some(cwd_off),
      argv: Some(argv_off),
      debug_flag: false,
      ..Default::default()
    },
  );
  builder.finish(msg);

  set_response_base(
    d,
    &mut builder,
    &msg::BaseArgs {
      msg: Some(flatbuffers::Offset::new(msg.value())),
      msg_type: msg::Any::StartRes,
      ..Default::default()
    },
  )
}

// TODO(ry) Use Deno instead of DenoC as first arg.
fn reply_error(d: *const DenoC, cmd_id: u32, msg: &String) {
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  // println!("reply_error{}", msg);
  let args = msg::BaseArgs {
    cmdId: cmd_id,
    error: Some(builder.create_string(msg)),
    ..Default::default()
  };
  set_response_base(d, &mut builder, &args)
}

fn create_msg(
  builder: &mut flatbuffers::FlatBufferBuilder,
  args: &msg::BaseArgs,
) -> deno_buf {
  let base = msg::CreateBase(builder, &args);
  builder.finish(base);
  let data = builder.get_active_buf_slice();
  deno_buf {
    // TODO(ry)
    // The deno_buf / ImportBuf / ExportBuf semantics should be such that we do not need to yield
    // ownership. Temporarally there is a hack in ImportBuf that when alloc_ptr is null, it will
    // memcpy the deno_buf into V8 instead of doing zero copy.
    alloc_ptr: 0 as *mut u8,
    alloc_len: 0,
    data_ptr: data.as_ptr() as *mut u8,
    data_len: data.len(),
  }
}

// TODO(ry) Use Deno instead of DenoC as first arg.
fn set_response_base(
  d: *const DenoC,
  builder: &mut flatbuffers::FlatBufferBuilder,
  args: &msg::BaseArgs,
) {
  let buf = create_msg(builder, args);
  unsafe { deno_set_response(d, buf) }
}

// TODO(ry) Use Deno instead of DenoC as first arg.
fn send_base(
  d: *const DenoC,
  builder: &mut flatbuffers::FlatBufferBuilder,
  args: &msg::BaseArgs,
) {
  let buf = create_msg(builder, args);
  unsafe { binding::deno_send(d, buf) }
}

// https://github.com/denoland/deno/blob/golang/os.go#L100-L154
fn handle_code_fetch(
  d: *const DenoC,
  module_specifier: &str,
  containing_file: &str,
) {
  let deno = from_c(d);

  assert!(deno.dir.root.join("gen") == deno.dir.gen, "Sanity check");

  let result = deno
    .dir
    .code_fetch(module_specifier, containing_file)
    .map_err(|err| {
      let errmsg = format!("{}", err);
      reply_error(d, 0, &errmsg);
    });
  if result.is_err() {
    return;
  }
  let out = result.unwrap();
  // reply_code_fetch
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  let mut msg_args = msg::CodeFetchResArgs {
    module_name: Some(builder.create_string(&out.module_name)),
    filename: Some(builder.create_string(&out.filename)),
    source_code: Some(builder.create_string(&out.source_code)),
    ..Default::default()
  };
  match out.maybe_output_code {
    Some(ref output_code) => {
      msg_args.output_code = Some(builder.create_string(output_code));
    }
    _ => (),
  };
  let msg = msg::CreateCodeFetchRes(&mut builder, &msg_args);
  builder.finish(msg);
  let args = msg::BaseArgs {
    msg: Some(flatbuffers::Offset::new(msg.value())),
    msg_type: msg::Any::CodeFetchRes,
    ..Default::default()
  };
  set_response_base(d, &mut builder, &args)
}

// https://github.com/denoland/deno/blob/golang/os.go#L156-L169
fn handle_code_cache(
  d: *const DenoC,
  filename: &str,
  source_code: &str,
  output_code: &str,
) {
  let deno = from_c(d);
  let result = deno.dir.code_cache(filename, source_code, output_code);
  if result.is_err() {
    let err = result.unwrap_err();
    let errmsg = format!("{}", err);
    reply_error(d, 0, &errmsg);
  }
  // null response indicates success.
}

fn set_timeout<F>(
  cb: F,
  delay: u32,
) -> (
  impl Future<Item = (), Error = ()>,
  futures::sync::oneshot::Sender<()>,
)
where
  F: FnOnce() -> (),
{
  let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
  let when = Instant::now() + Duration::from_millis(delay.into());
  let delay_task = Delay::new(when)
    .map_err(|e| panic!("timer failed; err={:?}", e))
    .and_then(|_| {
      cb();
      Ok(())
    })
    .select(cancel_rx)
    .map(|_| ())
    .map_err(|_| ());

  (delay_task, cancel_tx)
}

fn set_interval<F>(
  cb: F,
  delay: u32,
) -> (
  impl Future<Item = (), Error = ()>,
  futures::sync::oneshot::Sender<()>,
)
where
  F: Fn() -> (),
{
  let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
  let delay = Duration::from_millis(delay.into());
  let interval_task = future::lazy(move || {
    Interval::new(Instant::now() + delay, delay)
      .for_each(move |_| {
        cb();
        future::ok(())
      })
      .into_future()
      .map_err(|_| panic!())
  }).select(cancel_rx)
    .map(|_| ())
    .map_err(|_| ());

  (interval_task, cancel_tx)
}

// TODO(ry) Use Deno instead of DenoC as first arg.
fn send_timer_ready(d: *const DenoC, timer_id: u32, done: bool) {
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  let msg = msg::CreateTimerReady(
    &mut builder,
    &msg::TimerReadyArgs {
      id: timer_id,
      done,
      ..Default::default()
    },
  );
  builder.finish(msg);
  send_base(
    d,
    &mut builder,
    &msg::BaseArgs {
      msg: Some(flatbuffers::Offset::new(msg.value())),
      msg_type: msg::Any::TimerReady,
      ..Default::default()
    },
  );
}

// Prototype https://github.com/denoland/deno/blob/golang/os.go#L171-L184
fn handle_read_file_sync(d: *const DenoC, filename: &str) {
  debug!("handle_read_file_sync {}", filename);
  let result = fs::read_file_sync(Path::new(filename));
  if result.is_err() {
    let err = result.unwrap_err();
    let errmsg = format!("{}", err);
    reply_error(d, 0, &errmsg);
    return;
  }

  // Build the response message. memcpy data into msg.
  // TODO(ry) zero-copy.
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  let vec = result.unwrap();
  let data_off = builder.create_byte_vector(vec.as_slice());
  let msg = msg::CreateReadFileSyncRes(
    &mut builder,
    &msg::ReadFileSyncResArgs {
      data: Some(data_off),
      ..Default::default()
    },
  );
  builder.finish(msg);
  set_response_base(
    d,
    &mut builder,
    &msg::BaseArgs {
      msg: Some(flatbuffers::Offset::new(msg.value())),
      msg_type: msg::Any::ReadFileSyncRes,
      ..Default::default()
    },
  );
}

// TODO(ry) Use Deno instead of DenoC as first arg.
fn remove_timer(d: *const DenoC, timer_id: u32) {
  let deno = from_c(d);
  deno.timers.remove(&timer_id);
}

use std::time::{Duration, Instant};
use tokio::prelude::*;
use tokio::timer::{Delay, Interval};
// Prototype: https://github.com/ry/deno/blob/golang/timers.go#L25-L39
fn handle_timer_start(
  d: *const DenoC,
  timer_id: u32,
  interval: bool,
  delay: u32,
) {
  debug!("handle_timer_start");
  let deno = from_c(d);

  if interval {
    let (interval_task, cancel_interval) = set_interval(
      move || {
        send_timer_ready(d, timer_id, false);
      },
      delay,
    );

    deno.timers.insert(timer_id, cancel_interval);
    deno.rt.spawn(interval_task);
  } else {
    let (delay_task, cancel_delay) = set_timeout(
      move || {
        remove_timer(d, timer_id);
        send_timer_ready(d, timer_id, true);
      },
      delay,
    );

    deno.timers.insert(timer_id, cancel_delay);
    deno.rt.spawn(delay_task);
  }
}

// Prototype: https://github.com/ry/deno/blob/golang/timers.go#L40-L43
fn handle_timer_clear(d: *const DenoC, timer_id: u32) {
  debug!("handle_timer_clear");
  remove_timer(d, timer_id);
}
