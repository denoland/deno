// Copyright 2018 the Deno authors. All rights reserved. MIT license.
use errors::DenoError;
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
use msg;
use std;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;
use std::time::{Duration, Instant};
use tokio::timer::Delay;

// Buf represents a byte array returned from a "Op".
// The message might be empty (which will be translated into a null object on
// the javascript side) or it is a heap allocated opaque sequence of bytes.
// Usually a flatbuffer message.
type Buf = Option<Box<[u8]>>;

// JS promises in Deno map onto a specific Future
// which yields either a DenoError or a byte array.
type Op = Future<Item = Buf, Error = DenoError>;

type OpResult = DenoResult<Buf>;

// TODO Ideally we wouldn't have to box the Op being returned.
// The box is just to make it easier to get a prototype refactor working.
type Handler = fn(d: *const DenoC, base: &msg::Base) -> Box<Op>;

pub extern "C" fn msg_from_js(d: *const DenoC, buf: deno_buf) {
  let bytes = unsafe { std::slice::from_raw_parts(buf.data_ptr, buf.data_len) };
  let base = msg::get_root_as_base(bytes);
  let msg_type = base.msg_type();
  let cmd_id = base.cmd_id();
  let handler: Handler = match msg_type {
    msg::Any::Start => handle_start,
    msg::Any::CodeFetch => handle_code_fetch,
    msg::Any::CodeCache => handle_code_cache,
    msg::Any::Environ => handle_env,
    msg::Any::FetchReq => handle_fetch_req,
    msg::Any::TimerStart => handle_timer_start,
    msg::Any::TimerClear => handle_timer_clear,
    msg::Any::MakeTempDir => handle_make_temp_dir,
    msg::Any::Mkdir => handle_mkdir,
    msg::Any::ReadFile => handle_read_file,
    msg::Any::RenameSync => handle_rename_sync,
    msg::Any::SetEnv => handle_set_env,
    msg::Any::StatSync => handle_stat_sync,
    msg::Any::WriteFile => handle_write_file,
    msg::Any::Exit => handle_exit,
    _ => panic!(format!(
      "Unhandled message {}",
      msg::enum_name_any(msg_type)
    )),
  };

  let future = handler(d, &base);
  let future = future.or_else(move |err| {
    // No matter whether we got an Err or Ok, we want a serialized message to
    // send back. So transform the DenoError into a deno_buf.
    let builder = &mut FlatBufferBuilder::new();
    let errmsg_offset = builder.create_string(&format!("{}", err));
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        error: Some(errmsg_offset),
        error_kind: err.kind(),
        ..Default::default()
      },
    ))
  });

  let deno = from_c(d);
  if base.sync() {
    // Execute future synchronously.
    // println!("sync handler {}", msg::enum_name_any(msg_type));
    let maybe_box_u8 = future.wait().unwrap();
    match maybe_box_u8 {
      None => {}
      Some(box_u8) => {
        let buf = deno_buf_from(box_u8);
        // Set the synchronous response, the value returned from deno.send().
        unsafe { libdeno::deno_set_response(d, buf) }
      }
    }
  } else {
    // Execute future asynchornously.
    let future = future.and_then(move |maybe_box_u8| {
      let buf = match maybe_box_u8 {
        Some(box_u8) => deno_buf_from(box_u8),
        None => {
          // async RPCs that return None still need to
          // send a message back to signal completion.
          let builder = &mut FlatBufferBuilder::new();
          deno_buf_from(
            serialize_response(
              cmd_id,
              builder,
              msg::BaseArgs {
                ..Default::default()
              },
            ).unwrap(),
          )
        }
      };
      // TODO(ry) make this thread safe.
      unsafe { libdeno::deno_send(d, buf) };
      Ok(())
    });
    deno.rt.spawn(future);
  }
}

fn deno_buf_from(x: Box<[u8]>) -> deno_buf {
  let len = x.len();
  let ptr = Box::into_raw(x);
  deno_buf {
    alloc_ptr: 0 as *mut u8,
    alloc_len: 0,
    data_ptr: ptr as *mut u8,
    data_len: len,
  }
}

fn permission_denied() -> DenoError {
  DenoError::from(std::io::Error::new(
    std::io::ErrorKind::PermissionDenied,
    "permission denied",
  ))
}

fn handle_exit(_d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_exit().unwrap();
  std::process::exit(msg.code())
}

fn handle_start(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let deno = from_c(d);
  let mut builder = FlatBufferBuilder::new();

  let argv = deno.argv.iter().map(|s| s.as_str()).collect::<Vec<_>>();
  let argv_off = builder.create_vector_of_strings(argv.as_slice());

  let cwd_path = std::env::current_dir().unwrap();
  let cwd_off =
    builder.create_string(deno_fs::normalize_path(cwd_path.as_ref()).as_ref());

  let msg = msg::StartRes::create(
    &mut builder,
    &msg::StartResArgs {
      cwd: Some(cwd_off),
      argv: Some(argv_off),
      debug_flag: deno.flags.log_debug,
      ..Default::default()
    },
  );

  ok_future(serialize_response(
    base.cmd_id(),
    &mut builder,
    msg::BaseArgs {
      msg_type: msg::Any::StartRes,
      msg: Some(msg.as_union_value()),
      ..Default::default()
    },
  ))
}

fn serialize_response(
  cmd_id: u32,
  builder: &mut FlatBufferBuilder,
  mut args: msg::BaseArgs,
) -> Buf {
  args.cmd_id = cmd_id;
  let base = msg::Base::create(builder, &args);
  msg::finish_base_buffer(builder, base);
  let data = builder.finished_data();
  // println!("serialize_response {:x?}", data);
  let vec = data.to_vec();
  Some(vec.into_boxed_slice())
}

fn ok_future(buf: Buf) -> Box<Op> {
  Box::new(futures::future::ok(buf))
}

// Shout out to Earl Sweatshirt.
fn odd_future(err: DenoError) -> Box<Op> {
  Box::new(futures::future::err(err))
}

// https://github.com/denoland/deno/blob/golang/os.go#L100-L154
fn handle_code_fetch(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_code_fetch().unwrap();
  let cmd_id = base.cmd_id();
  let module_specifier = msg.module_specifier().unwrap();
  let containing_file = msg.containing_file().unwrap();
  let deno = from_c(d);

  assert_eq!(deno.dir.root.join("gen"), deno.dir.gen, "Sanity check");

  Box::new(futures::future::result(|| -> OpResult {
    let builder = &mut FlatBufferBuilder::new();
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
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        msg: Some(msg.as_union_value()),
        msg_type: msg::Any::CodeFetchRes,
        ..Default::default()
      },
    ))
  }()))
}

// https://github.com/denoland/deno/blob/golang/os.go#L156-L169
fn handle_code_cache(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_code_cache().unwrap();
  let filename = msg.filename().unwrap();
  let source_code = msg.source_code().unwrap();
  let output_code = msg.output_code().unwrap();
  Box::new(futures::future::result(|| -> OpResult {
    let deno = from_c(d);
    deno.dir.code_cache(filename, source_code, output_code)?;
    Ok(None)
  }()))
}

fn handle_set_env(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_set_env().unwrap();
  let key = msg.key().unwrap();
  let value = msg.value().unwrap();

  let deno = from_c(d);
  if !deno.flags.allow_env {
    return odd_future(permission_denied());
  }

  std::env::set_var(key, value);
  ok_future(None)
}

fn handle_env(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let deno = from_c(d);
  let cmd_id = base.cmd_id();
  if !deno.flags.allow_env {
    return odd_future(permission_denied());
  }

  let builder = &mut FlatBufferBuilder::new();
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
  ok_future(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      msg: Some(msg.as_union_value()),
      msg_type: msg::Any::EnvironRes,
      ..Default::default()
    },
  ))
}

fn handle_fetch_req(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_fetch_req().unwrap();
  let cmd_id = base.cmd_id();
  let id = msg.id();
  let url = msg.url().unwrap();
  let deno = from_c(d);

  if !deno.flags.allow_net {
    return odd_future(permission_denied());
  }

  let url = url.parse::<hyper::Uri>().unwrap();
  let client = Client::new();

  let future = client.get(url).and_then(|res| {
    let status = res.status().as_u16() as i32;
    // TODO Handle streaming body.
    res.into_body().concat2().map(move |body| (status, body))
  });

  let future = future.map_err(|err| -> DenoError { err.into() }).and_then(
    move |(status, body)| {
      let builder = &mut FlatBufferBuilder::new();
      // Send the first message without a body. This is just to indicate
      // what status code.
      let body_off = builder.create_vector(body.as_ref());
      let msg = msg::FetchRes::create(
        builder,
        &msg::FetchResArgs {
          id,
          status,
          body: Some(body_off),
          ..Default::default()
        },
      );
      Ok(serialize_response(
        cmd_id,
        builder,
        msg::BaseArgs {
          msg: Some(msg.as_union_value()),
          msg_type: msg::Any::FetchRes,
          ..Default::default()
        },
      ))
    },
  );
  Box::new(future)
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

fn handle_make_temp_dir(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let base = Box::new(*base);
  let msg = base.msg_as_make_temp_dir().unwrap();
  let cmd_id = base.cmd_id();
  let dir = msg.dir();
  let prefix = msg.prefix();
  let suffix = msg.suffix();

  let deno = from_c(d);
  if !deno.flags.allow_write {
    return Box::new(futures::future::err(permission_denied()));
  }
  // TODO Use blocking() here.
  Box::new(futures::future::result(|| -> OpResult {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/deno/issues/627.
    // We can't assume that paths are always valid utf8 strings.
    let path = deno_fs::make_temp_dir(dir.map(Path::new), prefix, suffix)?;
    let builder = &mut FlatBufferBuilder::new();
    let path_off = builder.create_string(path.to_str().unwrap());
    let msg = msg::MakeTempDirRes::create(
      builder,
      &msg::MakeTempDirResArgs {
        path: Some(path_off),
        ..Default::default()
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        msg: Some(msg.as_union_value()),
        msg_type: msg::Any::MakeTempDirRes,
        ..Default::default()
      },
    ))
  }()))
}

fn handle_mkdir(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_mkdir().unwrap();
  // TODO let mode = msg.mode();
  let path = msg.path().unwrap();
  let deno = from_c(d);
  if !deno.flags.allow_write {
    return odd_future(permission_denied());
  }
  // TODO Use tokio_threadpool.
  Box::new(futures::future::result(|| -> OpResult {
    debug!("handle_mkdir {}", path);
    // TODO(ry) Use mode.
    deno_fs::mkdir(Path::new(path))?;
    Ok(None)
  }()))
}

// Prototype https://github.com/denoland/deno/blob/golang/os.go#L171-L184
fn handle_read_file(_d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_read_file().unwrap();
  let cmd_id = base.cmd_id();
  let filename = String::from(msg.filename().unwrap());
  Box::new(futures::future::result(|| -> OpResult {
    debug!("handle_read_file {}", filename);
    let vec = fs::read(Path::new(&filename))?;
    // Build the response message. memcpy data into msg.
    // TODO(ry) zero-copy.
    let builder = &mut FlatBufferBuilder::new();
    let data_off = builder.create_vector(vec.as_slice());
    let msg = msg::ReadFileRes::create(
      builder,
      &msg::ReadFileResArgs {
        data: Some(data_off),
        ..Default::default()
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        msg: Some(msg.as_union_value()),
        msg_type: msg::Any::ReadFileRes,
        ..Default::default()
      },
    ))
  }()))
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

fn handle_stat_sync(_d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_stat_sync().unwrap();
  let cmd_id = base.cmd_id();
  let filename = String::from(msg.filename().unwrap());
  let lstat = msg.lstat();

  Box::new(futures::future::result(|| -> OpResult {
    let builder = &mut FlatBufferBuilder::new();
    debug!("handle_stat_sync {} {}", filename, lstat);
    let path = Path::new(&filename);
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

    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        msg: Some(msg.as_union_value()),
        msg_type: msg::Any::StatSyncRes,
        ..Default::default()
      },
    ))
  }()))
}

fn handle_write_file(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_write_file().unwrap();
  let filename = String::from(msg.filename().unwrap());
  let data = msg.data().unwrap();
  let perm = msg.perm();
  let deno = from_c(d);

  debug!("handle_write_file {}", filename);
  Box::new(futures::future::result(|| -> OpResult {
    if !deno.flags.allow_write {
      Err(permission_denied())
    } else {
      deno_fs::write_file(Path::new(&filename), data, perm)?;
      Ok(None)
    }
  }()))
}

// TODO(ry) Use Deno instead of DenoC as first arg.
fn remove_timer(d: *const DenoC, timer_id: u32) {
  let deno = from_c(d);
  assert!(deno.timers.contains_key(&timer_id));
  deno.timers.remove(&timer_id);
  assert!(!deno.timers.contains_key(&timer_id));
}

// Prototype: https://github.com/ry/deno/blob/golang/timers.go#L25-L39
fn handle_timer_start(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  debug!("handle_timer_start");
  let msg = base.msg_as_timer_start().unwrap();
  let cmd_id = base.cmd_id();
  let timer_id = msg.id();
  let delay = msg.delay();
  let deno = from_c(d);

  let future = {
    let (delay_task, cancel_delay) = set_timeout(
      move || {
        remove_timer(d, timer_id);
      },
      delay,
    );
    deno.timers.insert(timer_id, cancel_delay);
    delay_task
  };
  Box::new(future.then(move |result| {
    let builder = &mut FlatBufferBuilder::new();
    let msg = msg::TimerReady::create(
      builder,
      &msg::TimerReadyArgs {
        id: timer_id,
        canceled: result.is_err(),
        ..Default::default()
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        msg: Some(msg.as_union_value()),
        msg_type: msg::Any::TimerReady,
        ..Default::default()
      },
    ))
  }))
}

// Prototype: https://github.com/ry/deno/blob/golang/timers.go#L40-L43
fn handle_timer_clear(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_timer_clear().unwrap();
  debug!("handle_timer_clear");
  remove_timer(d, msg.id());
  ok_future(None)
}

fn handle_rename_sync(d: *const DenoC, base: &msg::Base) -> Box<Op> {
  let deno = from_c(d);
  if !deno.flags.allow_write {
    return Box::new(futures::future::err(permission_denied()));
  };
  let msg = base.msg_as_rename_sync().unwrap();
  let oldpath = String::from(msg.oldpath().unwrap());
  let newpath = String::from(msg.newpath().unwrap());
  // TODO use blocking()
  Box::new(futures::future::result(|| -> OpResult {
    debug!("handle_rename {} {}", oldpath, newpath);
    fs::rename(Path::new(&oldpath), Path::new(&newpath))?;
    Ok(None)
  }()))
}
