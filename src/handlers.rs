// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use errors::DenoResult;
use flatbuffers::FlatBufferBuilder;
use from_c;
use fs as deno_fs;
use futures;
use futures::sync::oneshot;
use hyper;
use hyper::rt::{Future, Stream};
use hyper::Client;
use libdeno;
use libdeno::{deno_buf, DenoC};
use msg_generated::deno as msg;
use std;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;
use std::time::{Duration, Instant};
use tokio::prelude::future;
use tokio::prelude::*;
use tokio::timer::{Delay, Interval};

type HandlerResult = DenoResult<libdeno::deno_buf>;
type Handler =
  fn(d: *const DenoC, base: msg::Base, builder: &mut FlatBufferBuilder)
    -> HandlerResult;

pub extern "C" fn msg_from_js(d: *const DenoC, buf: deno_buf) {
  let bytes = unsafe { std::slice::from_raw_parts(buf.data_ptr, buf.data_len) };
  let base = msg::get_root_as_base(bytes);
  let msg_type = base.msg_type();
  let handler: Handler = match msg_type {
    msg::Any::Start => handle_start,
    msg::Any::CodeFetch => handle_code_fetch,
    msg::Any::CodeCache => handle_code_cache,
    msg::Any::Environ => handle_env,
    msg::Any::FetchReq => handle_fetch_req,
    msg::Any::TimerStart => handle_timer_start,
    msg::Any::TimerClear => handle_timer_clear,
    msg::Any::MakeTempDir => handle_make_temp_dir,
    msg::Any::MkdirSync => handle_mkdir_sync,
    msg::Any::ReadFileSync => handle_read_file_sync,
    msg::Any::RenameSync => handle_rename_sync,
    msg::Any::SetEnv => handle_set_env,
    msg::Any::StatSync => handle_stat_sync,
    msg::Any::WriteFileSync => handle_write_file_sync,
    msg::Any::Exit => handle_exit,
    _ => panic!(format!(
      "Unhandled message {}",
      msg::enum_name_any(msg_type)
    )),
  };

  let builder = &mut FlatBufferBuilder::new();
  let result = handler(d, base, builder);

  // No matter whether we got an Err or Ok, we want a serialized message to
  // send back. So transform the DenoError into a deno_buf.
  let buf = match result {
    Err(ref err) => {
      let errmsg_offset = builder.create_string(&format!("{}", err));
      create_msg(
        builder,
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
    unsafe { libdeno::deno_set_response(d, buf) }
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

fn handle_exit(
  _d: *const DenoC,
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_exit().unwrap();
  std::process::exit(msg.code())
}

fn handle_start(
  d: *const DenoC,
  _base: msg::Base,
  builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let deno = from_c(d);

  let argv = deno.argv.iter().map(|s| s.as_str()).collect::<Vec<_>>();
  let argv_off = builder.create_vector_of_strings(argv.as_slice());

  let cwd_path = std::env::current_dir().unwrap();
  let cwd_off =
    builder.create_string(deno_fs::normalize_path(cwd_path.as_ref()).as_ref());

  let msg = msg::StartRes::create(
    builder,
    &msg::StartResArgs {
      cwd: Some(cwd_off),
      argv: Some(argv_off),
      debug_flag: deno.flags.log_debug,
      deps_flag: deno.flags.deps_flag,
      ..Default::default()
    },
  );

  Ok(create_msg(
    builder,
    &msg::BaseArgs {
      msg_type: msg::Any::StartRes,
      msg: Some(msg.as_union_value()),
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
  let data = builder.finished_data();
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
  unsafe { libdeno::deno_send(d, buf) }
}

// https://github.com/denoland/deno/blob/golang/os.go#L100-L154
fn handle_code_fetch(
  d: *const DenoC,
  base: msg::Base,
  builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_code_fetch().unwrap();
  let module_specifier = msg.module_specifier().unwrap();
  let containing_file = msg.containing_file().unwrap();
  let deno = from_c(d);

  assert!(deno.dir.root.join("gen") == deno.dir.gen, "Sanity check");

  let out = deno.dir.code_fetch(module_specifier, containing_file)?;
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
      msg: Some(msg.as_union_value()),
      msg_type: msg::Any::CodeFetchRes,
      ..Default::default()
    },
  ))
}

// https://github.com/denoland/deno/blob/golang/os.go#L156-L169
fn handle_code_cache(
  d: *const DenoC,
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_code_cache().unwrap();
  let filename = msg.filename().unwrap();
  let source_code = msg.source_code().unwrap();
  let output_code = msg.output_code().unwrap();
  let deno = from_c(d);
  deno.dir.code_cache(filename, source_code, output_code)?;
  Ok(null_buf()) // null response indicates success.
}

fn handle_set_env(
  d: *const DenoC,
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_set_env().unwrap();
  let key = msg.key().unwrap();
  let value = msg.value().unwrap();

  let deno = from_c(d);
  if !deno.flags.allow_env {
    let err = std::io::Error::new(
      std::io::ErrorKind::PermissionDenied,
      "allow_env is off.",
    );
    return Err(err.into());
  }

  std::env::set_var(key, value);
  Ok(null_buf())
}

fn handle_env(
  d: *const DenoC,
  _base: msg::Base,
  builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let deno = from_c(d);
  if !deno.flags.allow_env {
    let err = std::io::Error::new(
      std::io::ErrorKind::PermissionDenied,
      "allow_env is off.",
    );
    return Err(err.into());
  }

  let vars: Vec<_> = std::env::vars()
    .map(|(key, value)| {
      let key = builder.create_string(&key);
      let value = builder.create_string(&value);

      msg::EnvPair::create(
        builder,
        &msg::EnvPairArgs {
          key: Some(key),
          value: Some(value),
          ..Default::default()
        },
      )
    })
    .collect();

  let tables = builder.create_vector(&vars);

  let msg = msg::EnvironRes::create(
    builder,
    &msg::EnvironResArgs {
      map: Some(tables),
      ..Default::default()
    },
  );

  Ok(create_msg(
    builder,
    &msg::BaseArgs {
      msg: Some(msg.as_union_value()),
      msg_type: msg::Any::EnvironRes,
      ..Default::default()
    },
  ))
}

fn handle_fetch_req(
  d: *const DenoC,
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let deno = from_c(d);
  if !deno.flags.allow_net {
    let err = std::io::Error::new(
      std::io::ErrorKind::PermissionDenied,
      "allow_net is off.",
    );
    return Err(err.into());
  }
  let msg = base.msg_as_fetch_req().unwrap();
  let id = msg.id();
  let url = msg.url().unwrap();
  let url = url.parse::<hyper::Uri>().unwrap();
  let client = Client::new();

  deno.rt.spawn(
    client
      .get(url)
      .map(move |res| {
        let status = res.status().as_u16() as i32;

        let mut builder = FlatBufferBuilder::new();
        // Send the first message without a body. This is just to indicate
        // what status code.
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
            msg: Some(msg.as_union_value()),
            msg_type: msg::Any::FetchRes,
            ..Default::default()
          },
        );
        res
      })
      .and_then(move |res| {
        // Send the body as a FetchRes message.
        res.into_body().concat2().map(move |body_buffer| {
          let mut builder = FlatBufferBuilder::new();
          let data_off = builder.create_vector(body_buffer.as_ref());
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
              msg: Some(msg.as_union_value()),
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
        let mut builder = FlatBufferBuilder::new();
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
            msg: Some(msg.as_union_value()),
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
      msg: Some(msg.as_union_value()),
      msg_type: msg::Any::TimerReady,
      ..Default::default()
    },
  );
}

fn handle_make_temp_dir(
  d: *const DenoC,
  base: msg::Base,
  builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_make_temp_dir().unwrap();
  let dir = msg.dir();
  let prefix = msg.prefix();
  let suffix = msg.suffix();
  let deno = from_c(d);
  if !deno.flags.allow_write {
    let err = std::io::Error::new(
      std::io::ErrorKind::PermissionDenied,
      "allow_write is off.",
    );
    return Err(err.into());
  }
  // TODO(piscisaureus): use byte vector for paths, not a string.
  // See https://github.com/denoland/deno/issues/627.
  // We can't assume that paths are always valid utf8 strings.
  let path = deno_fs::make_temp_dir(dir.map(Path::new), prefix, suffix)?;
  let path_off = builder.create_string(path.to_str().unwrap());
  let msg = msg::MakeTempDirRes::create(
    builder,
    &msg::MakeTempDirResArgs {
      path: Some(path_off),
      ..Default::default()
    },
  );
  Ok(create_msg(
    builder,
    &msg::BaseArgs {
      msg: Some(msg.as_union_value()),
      msg_type: msg::Any::MakeTempDirRes,
      ..Default::default()
    },
  ))
}

fn handle_mkdir_sync(
  d: *const DenoC,
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_mkdir_sync().unwrap();
  let path = msg.path().unwrap();
  // TODO let mode = msg.mode();
  let deno = from_c(d);

  debug!("handle_mkdir_sync {}", path);
  if deno.flags.allow_write {
    // TODO(ry) Use mode.
    deno_fs::mkdir(Path::new(path))?;
    Ok(null_buf())
  } else {
    let err = std::io::Error::new(
      std::io::ErrorKind::PermissionDenied,
      "allow_write is off.",
    );
    Err(err.into())
  }
}

// Prototype https://github.com/denoland/deno/blob/golang/os.go#L171-L184
fn handle_read_file_sync(
  _d: *const DenoC,
  base: msg::Base,
  builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_read_file_sync().unwrap();
  let filename = msg.filename().unwrap();
  debug!("handle_read_file_sync {}", filename);
  let vec = fs::read(Path::new(filename))?;
  // Build the response message. memcpy data into msg.
  // TODO(ry) zero-copy.
  let data_off = builder.create_vector(vec.as_slice());
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
      msg: Some(msg.as_union_value()),
      msg_type: msg::Any::ReadFileSyncRes,
      ..Default::default()
    },
  ))
}

macro_rules! to_seconds {
  ($time:expr) => {{
    // Unwrap is safe here as if the file is before the unix epoch
    // something is very wrong.
    $time
      .and_then(|t| Ok(t.duration_since(UNIX_EPOCH).unwrap().as_secs()))
      .unwrap_or(0)
  }};
}

fn handle_stat_sync(
  _d: *const DenoC,
  base: msg::Base,
  builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_stat_sync().unwrap();
  let filename = msg.filename().unwrap();
  let lstat = msg.lstat();

  debug!("handle_stat_sync {} {}", filename, lstat);
  let path = Path::new(filename);
  let metadata = if lstat {
    fs::symlink_metadata(path)?
  } else {
    fs::metadata(path)?
  };

  let msg = msg::StatSyncRes::create(
    builder,
    &msg::StatSyncResArgs {
      is_file: metadata.is_file(),
      is_symlink: metadata.file_type().is_symlink(),
      len: metadata.len(),
      modified: to_seconds!(metadata.modified()),
      accessed: to_seconds!(metadata.accessed()),
      created: to_seconds!(metadata.created()),
      ..Default::default()
    },
  );

  Ok(create_msg(
    builder,
    &msg::BaseArgs {
      msg: Some(msg.as_union_value()),
      msg_type: msg::Any::StatSyncRes,
      ..Default::default()
    },
  ))
}

fn handle_write_file_sync(
  d: *const DenoC,
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_write_file_sync().unwrap();
  let filename = msg.filename().unwrap();
  let data = msg.data().unwrap();
  // TODO let perm = msg.perm();
  let deno = from_c(d);

  debug!("handle_write_file_sync {}", filename);
  if deno.flags.allow_write {
    // TODO(ry) Use perm.
    deno_fs::write_file_sync(Path::new(filename), data)?;
    Ok(null_buf())
  } else {
    let err = std::io::Error::new(
      std::io::ErrorKind::PermissionDenied,
      "allow_write is off.",
    );
    Err(err.into())
  }
}

// TODO(ry) Use Deno instead of DenoC as first arg.
fn remove_timer(d: *const DenoC, timer_id: u32) {
  let deno = from_c(d);
  deno.timers.remove(&timer_id);
}

// Prototype: https://github.com/ry/deno/blob/golang/timers.go#L25-L39
fn handle_timer_start(
  d: *const DenoC,
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  debug!("handle_timer_start");
  let msg = base.msg_as_timer_start().unwrap();
  let timer_id = msg.id();
  let interval = msg.interval();
  let delay = msg.delay();
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
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_timer_clear().unwrap();
  debug!("handle_timer_clear");
  remove_timer(d, msg.id());
  Ok(null_buf())
}

fn handle_rename_sync(
  d: *const DenoC,
  base: msg::Base,
  _builder: &mut FlatBufferBuilder,
) -> HandlerResult {
  let msg = base.msg_as_rename_sync().unwrap();
  let oldpath = msg.oldpath().unwrap();
  let newpath = msg.newpath().unwrap();
  let deno = from_c(d);

  debug!("handle_rename_sync {} {}", oldpath, newpath);
  if !deno.flags.allow_write {
    let err = std::io::Error::new(
      std::io::ErrorKind::PermissionDenied,
      "allow_write is off.",
    );
    return Err(err.into());
  }
  fs::rename(Path::new(oldpath), Path::new(newpath))?;
  Ok(null_buf())
}
