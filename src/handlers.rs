// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use binding;
use binding::{deno_buf, deno_set_response, DenoC};
use flatbuffers;
use flatbuffers::ByteStringOffset;
use flatbuffers::LabeledUOffsetT;
use from_c;
use fs;
use futures;
use futures::sync::oneshot;
use libc::c_char;
use mem;
use msg_generated::deno as msg;
use std::ffi::CStr;
use std::path::Path;

// Help. Is there a way to do this without macros?
// Want: fn str_from_ptr(*const c_char) -> &str
macro_rules! str_from_ptr {
  ($ptr:expr) => {{
    let cstr = unsafe { CStr::from_ptr($ptr as *const i8) };
    cstr.to_str().unwrap()
  }};
}

/*
// reply_start partially implemented here https://gist.github.com/ry/297c83e0ac8722c045db1b097cdb6afc
pub fn deno_handle_msg_from_js(d: *const DenoC, buf: deno_buf) {
    let s = std::slice::from_raw_parts(buf.data_ptr, buf.data_len);
    buf.data_ptr
    get_root()
}
*/

// TODO(ry) Use Deno instead of DenoC as first arg.
fn reply_error(d: *const DenoC, cmd_id: u32, msg: &String) {
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  // println!("reply_error{}", msg);
  let args = msg::BaseArgs {
    cmdId: cmd_id,
    error: builder.create_string(msg),
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
#[no_mangle]
pub extern "C" fn handle_code_fetch(
  d: *const DenoC,
  cmd_id: u32,
  module_specifier_: *const c_char,
  containing_file_: *const c_char,
) {
  let module_specifier = str_from_ptr!(module_specifier_);
  let containing_file = str_from_ptr!(containing_file_);

  let deno = from_c(d);

  assert!(deno.dir.root.join("gen") == deno.dir.gen, "Sanity check");

  let result = deno
    .dir
    .code_fetch(module_specifier, containing_file)
    .map_err(|err| {
      let errmsg = format!("{}", err);
      reply_error(d, cmd_id, &errmsg);
    });
  if result.is_err() {
    return;
  }
  let out = result.unwrap();
  // reply_code_fetch
  let mut builder = flatbuffers::FlatBufferBuilder::new();
  let mut msg_args = msg::CodeFetchResArgs {
    module_name: builder.create_string(&out.module_name),
    filename: builder.create_string(&out.filename),
    source_code: builder.create_string(&out.source_code),
    ..Default::default()
  };
  match out.maybe_output_code {
    Some(ref output_code) => {
      msg_args.output_code = builder.create_string(output_code);
    }
    _ => (),
  };
  let msg = msg::CreateCodeFetchRes(&mut builder, &msg_args);
  builder.finish(msg);
  let args = msg::BaseArgs {
    cmdId: cmd_id,
    msg: Some(msg.union()),
    msg_type: msg::Any::CodeFetchRes,
    ..Default::default()
  };
  set_response_base(d, &mut builder, &args)
}

// https://github.com/denoland/deno/blob/golang/os.go#L156-L169
#[no_mangle]
pub extern "C" fn handle_code_cache(
  d: *const DenoC,
  cmd_id: u32,
  filename_: *const c_char,
  source_code_: *const c_char,
  output_code_: *const c_char,
) {
  let deno = from_c(d);
  let filename = str_from_ptr!(filename_);
  let source_code = str_from_ptr!(source_code_);
  let output_code = str_from_ptr!(output_code_);
  let result = deno.dir.code_cache(filename, source_code, output_code);
  if result.is_err() {
    let err = result.unwrap_err();
    let errmsg = format!("{}", err);
    reply_error(d, cmd_id, &errmsg);
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
      msg: Some(msg.union()),
      msg_type: msg::Any::TimerReady,
      ..Default::default()
    },
  );
}

// Prototype https://github.com/denoland/deno/blob/golang/os.go#L171-L184
#[no_mangle]
pub extern "C" fn handle_read_file_sync(
  d: *const DenoC,
  cmd_id: u32,
  filename: *const c_char,
) {
  let filename = str_from_ptr!(filename);

  debug!("handle_read_file_sync {}", filename);
  let result = fs::read_file_sync(Path::new(filename));
  if result.is_err() {
    let err = result.unwrap_err();
    let errmsg = format!("{}", err);
    reply_error(d, cmd_id, &errmsg);
    return;
  }

  // Build the response message. memcpy data into msg.
  let mut builder = flatbuffers::FlatBufferBuilder::new();

  let vec = result.unwrap();
  //let data =
  //  flatbuffers::LabeledUOffsetT::new(builder.push_bytes(vec.as_slice()));

  let data_ = builder.create_byte_vector(vec.as_slice());

  // TODO(ry) This is a hack that can be removed once builder.create_byte_vector
  // works properly.
  let data = unsafe {
    mem::transmute::<LabeledUOffsetT<ByteStringOffset>, LabeledUOffsetT<&[i8]>>(
      data_,
    )
  };

  let msg = msg::CreateReadFileSyncRes(
    &mut builder,
    &msg::ReadFileSyncResArgs {
      data,
      ..Default::default()
    },
  );
  builder.finish(msg);
  set_response_base(
    d,
    &mut builder,
    &msg::BaseArgs {
      msg: Some(msg.union()),
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
#[no_mangle]
pub extern "C" fn handle_timer_start(
  d: *const DenoC,
  cmd_id: u32,
  timer_id: u32,
  interval: bool,
  delay: u32,
) {
  assert!(cmd_id == 0);
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
#[no_mangle]
pub extern "C" fn handle_timer_clear(
  d: *const DenoC,
  cmd_id: u32,
  timer_id: u32,
) {
  assert!(cmd_id == 0);
  debug!("handle_timer_clear");
  remove_timer(d, timer_id);
}
