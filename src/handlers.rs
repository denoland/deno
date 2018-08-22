// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use binding;
use binding::{deno_buf, DenoC};
use errors::DenoResult;
use flatbuffers;
use flatbuffers::FlatBufferBuilder;
use from_c;
use fs;
use futures;
use futures::sync::oneshot;
use hyper;
use hyper::rt::{Future, Stream};
use hyper::Client;
use msg_generated::deno as msg;
use std;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::prelude::future;
use tokio::prelude::*;
use tokio::timer::{Delay, Interval};

type HandlerResult = DenoResult<binding::deno_buf>;

pub extern "C" fn msg_from_js(d: *const DenoC, buf: deno_buf) {
  let bytes = unsafe { std::slice::from_raw_parts(buf.data_ptr, buf.data_len) };
  let base = msg::get_root_as_base(bytes);
  let mut builder = FlatBufferBuilder::new();
  let msg_type = base.msg_type();
  let result: HandlerResult = match msg_type {
    msg::Any::Start => handle_start(d, &mut builder),
    msg::Any::CodeFetch => {
      // TODO base.msg_as_CodeFetch();
      let msg = msg::CodeFetch::init_from_table(base.msg().unwrap());
      let module_specifier = msg.module_specifier().unwrap();
      let containing_file = msg.containing_file().unwrap();
      handle_code_fetch(d, &mut builder, module_specifier, containing_file)
    }
    msg::Any::CodeCache => {
      // TODO base.msg_as_CodeCache();
      let msg = msg::CodeCache::init_from_table(base.msg().unwrap());
      let filename = msg.filename().unwrap();
      let source_code = msg.source_code().unwrap();
      let output_code = msg.output_code().unwrap();
      handle_code_cache(d, &mut builder, filename, source_code, output_code)
    }
    msg::Any::FetchReq => {
      // TODO base.msg_as_FetchReq();
      let msg = msg::FetchReq::init_from_table(base.msg().unwrap());
      let url = msg.url().unwrap();
      handle_fetch_req(d, &mut builder, msg.id(), url)
    }
    msg::Any::TimerStart => {
      // TODO base.msg_as_TimerStart();
      let msg = msg::TimerStart::init_from_table(base.msg().unwrap());
      handle_timer_start(d, &mut builder, msg.id(), msg.interval(), msg.delay())
    }
    msg::Any::TimerClear => {
      // TODO base.msg_as_TimerClear();
      let msg = msg::TimerClear::init_from_table(base.msg().unwrap());
      handle_timer_clear(d, &mut builder, msg.id())
    }
    msg::Any::Exit => {
      // TODO base.msg_as_Exit();
      let msg = msg::Exit::init_from_table(base.msg().unwrap());
      std::process::exit(msg.code())
    }
    msg::Any::ReadFileSync => {
      // TODO base.msg_as_ReadFileSync();
      let msg = msg::ReadFileSync::init_from_table(base.msg().unwrap());
      let filename = msg.filename().unwrap();
      handle_read_file_sync(d, &mut builder, filename)
    }
    _ => panic!(format!(
      "Unhandled message {}",
      msg::enum_name_any(msg_type)
    )),
  };

  // No matter whether we got an Err or Ok, we want a serialized message to
  // send back. So transform the DenoError into a deno_buf.
  let buf = match result {
    Err(ref err) => {
      let errmsg_offset = builder.create_string(&format!("{}", err));
      create_msg(
        &mut builder,
        &msg::BaseArgs {
          error: Some(errmsg_offset),
          error_kind: err.kind(),
          ..Default::default()
        },
      )
    }
    Ok(buf) => buf,
  };

  // Set the synchronous response, the value returned from deno.send().
  // null_buf is a special case that indicates success.
  if buf != null_buf() {
    unsafe { binding::deno_set_response(d, buf) }
  }
}

fn null_buf() -> deno_buf {
  deno_buf {
    alloc_ptr: 0 as *mut u8,
    alloc_len: 0,
    data_ptr: 0 as *mut u8,
    data_len: 0,
  }
}

fn handle_start(
  d: *const DenoC,
  builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let deno = from_c(d);

  let argv = deno.argv.iter().map(|s| s.as_str()).collect::<Vec<_>>();
  let argv_off = builder.create_vector_of_strings(argv.as_slice());

  let cwd_path = std::env::current_dir().unwrap();
  let cwd_off = builder.create_string(cwd_path.to_str().unwrap());

  let msg = msg::StartRes::create(
    builder,
    &msg::StartResArgs {
      cwd: Some(cwd_off),
      argv: Some(argv_off),
      debug_flag: deno.flags.log_debug,
      ..Default::default()
    },
  );

  Ok(create_msg(
    builder,
    &msg::BaseArgs {
      msg: Some(flatbuffers::Offset::new(msg.value())),
      msg_type: msg::Any::StartRes,
      ..Default::default()
    },
  ))
}

fn create_msg(
  builder: &mut FlatBufferBuilder,
  args: &msg::BaseArgs,
) -> deno_buf {
  let base = msg::Base::create(builder, &args);
  msg::finish_base_buffer(builder, base);
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
fn send_base(
  d: *const DenoC,
  builder: &mut FlatBufferBuilder,
  args: &msg::BaseArgs,
) {
  let buf = create_msg(builder, args);
  unsafe { binding::deno_send(d, buf) }
}

// https://github.com/denoland/deno/blob/golang/os.go#L100-L154
fn handle_code_fetch(
  d: *const DenoC,
  builder: &mut FlatBufferBuilder,
  module_specifier: &str,
  containing_file: &str,
) -> HandlerResult {
  let deno = from_c(d);

  assert!(deno.dir.root.join("gen") == deno.dir.gen, "Sanity check");

  let out = deno.dir.code_fetch(module_specifier, containing_file)?;
  // reply_code_fetch
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
  let msg = msg::CodeFetchRes::create(builder, &msg_args);
  Ok(create_msg(
    builder,
    &msg::BaseArgs {
      msg: Some(flatbuffers::Offset::new(msg.value())),
      msg_type: msg::Any::CodeFetchRes,
      ..Default::default()
    },
  ))
}

// https://github.com/denoland/deno/blob/golang/os.go#L156-L169
fn handle_code_cache(
  d: *const DenoC,
  _builder: &mut FlatBufferBuilder,
  filename: &str,
  source_code: &str,
  output_code: &str,
) -> HandlerResult {
  let deno = from_c(d);
  deno.dir.code_cache(filename, source_code, output_code)?;
  Ok(null_buf()) // null response indicates success.
}

fn handle_fetch_req(
  d: *const DenoC,
  _builder: &mut FlatBufferBuilder,
  id: u32,
  url: &str,
) -> HandlerResult {
  let deno = from_c(d);
  let url = url.parse::<hyper::Uri>().unwrap();
  let client = Client::new();

  deno.rt.spawn(
    client
      .get(url)
      .map(move |res| {
        let status = res.status().as_u16() as i32;

        // Send the first message without a body. This is just to indicate
        // what status code.
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let msg = msg::FetchRes::create(
          &mut builder,
          &msg::FetchResArgs {
            id,
            status,
            ..Default::default()
          },
        );
        send_base(
          d,
          &mut builder,
          &msg::BaseArgs {
            msg: Some(flatbuffers::Offset::new(msg.value())),
            msg_type: msg::Any::FetchRes,
            ..Default::default()
          },
        );
        res
      })
      .and_then(move |res| {
        // Send the body as a FetchRes message.
        res.into_body().concat2().map(move |body_buffer| {
          let mut builder = flatbuffers::FlatBufferBuilder::new();
          let data_off = builder.create_byte_vector(body_buffer.as_ref());
          let msg = msg::FetchRes::create(
            &mut builder,
            &msg::FetchResArgs {
              id,
              body: Some(data_off),
              ..Default::default()
            },
          );
          send_base(
            d,
            &mut builder,
            &msg::BaseArgs {
              msg: Some(flatbuffers::Offset::new(msg.value())),
              msg_type: msg::Any::FetchRes,
              ..Default::default()
            },
          );
        })
      })
      .map_err(move |err| {
        let errmsg = format!("{}", err);

        // TODO This is obviously a lot of duplicated code from the success case.
        // Leaving it here now jsut to get a first pass implementation, but this
        // needs to be cleaned up.
        let mut builder = flatbuffers::FlatBufferBuilder::new();
        let err_off = builder.create_string(errmsg.as_str());
        let msg = msg::FetchRes::create(
          &mut builder,
          &msg::FetchResArgs {
            id,
            ..Default::default()
          },
        );
        send_base(
          d,
          &mut builder,
          &msg::BaseArgs {
            msg: Some(flatbuffers::Offset::new(msg.value())),
            msg_type: msg::Any::FetchRes,
            error: Some(err_off),
            ..Default::default()
          },
        );
      }),
  );
  Ok(null_buf()) // null response indicates success.
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
  let mut builder = FlatBufferBuilder::new();
  let msg = msg::TimerReady::create(
    &mut builder,
    &msg::TimerReadyArgs {
      id: timer_id,
      done,
      ..Default::default()
    },
  );
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
fn handle_read_file_sync(
  _d: *const DenoC,
  builder: &mut FlatBufferBuilder,
  filename: &str,
) -> HandlerResult {
  debug!("handle_read_file_sync {}", filename);
  let vec = fs::read_file_sync(Path::new(filename))?;
  // Build the response message. memcpy data into msg.
  // TODO(ry) zero-copy.
  let data_off = builder.create_byte_vector(vec.as_slice());
  let msg = msg::ReadFileSyncRes::create(
    builder,
    &msg::ReadFileSyncResArgs {
      data: Some(data_off),
      ..Default::default()
    },
  );
  Ok(create_msg(
    builder,
    &msg::BaseArgs {
      msg: Some(flatbuffers::Offset::new(msg.value())),
      msg_type: msg::Any::ReadFileSyncRes,
      ..Default::default()
    },
  ))
}

// TODO(ry) Use Deno instead of DenoC as first arg.
fn remove_timer(d: *const DenoC, timer_id: u32) {
  let deno = from_c(d);
  deno.timers.remove(&timer_id);
}

// Prototype: https://github.com/ry/deno/blob/golang/timers.go#L25-L39
fn handle_timer_start(
  d: *const DenoC,
  _builder: &mut FlatBufferBuilder,
  timer_id: u32,
  interval: bool,
  delay: u32,
) -> HandlerResult {
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
  Ok(null_buf())
}

// Prototype: https://github.com/ry/deno/blob/golang/timers.go#L40-L43
fn handle_timer_clear(
  d: *const DenoC,
  _builder: &mut FlatBufferBuilder,
  timer_id: u32,
) -> HandlerResult {
  debug!("handle_timer_clear");
  remove_timer(d, timer_id);
  Ok(null_buf())
}
