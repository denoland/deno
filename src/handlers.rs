// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use errors::DenoError;
use errors::DenoResult;
use fs as deno_fs;
use isolate::Buf;
use isolate::IsolateState;
use isolate::Op;
use msg;

use flatbuffers::FlatBufferBuilder;
use futures;
use futures::sync::oneshot;
use hyper;
use hyper::rt::{Future, Stream};
use hyper::Client;
use remove_dir_all::remove_dir_all;
use std;
use std::fs;
#[cfg(any(unix))]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use std::time::{Duration, Instant};
use tokio::timer::Delay;

type OpResult = DenoResult<Buf>;

// TODO Ideally we wouldn't have to box the Op being returned.
// The box is just to make it easier to get a prototype refactor working.
type Handler = fn(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op>;

// Hopefully Rust optimizes this away.
fn empty_buf() -> Buf {
  Box::new([])
}

pub fn msg_from_js(state: Arc<IsolateState>, bytes: &[u8]) -> (bool, Box<Op>) {
  let base = msg::get_root_as_base(bytes);
  let is_sync = base.sync();
  let msg_type = base.msg_type();
  let cmd_id = base.cmd_id();
  let handler: Handler = match msg_type {
    msg::Any::Start => handle_start,
    msg::Any::CodeFetch => handle_code_fetch,
    msg::Any::CodeCache => handle_code_cache,
    msg::Any::SetTimeout => handle_set_timeout,
    msg::Any::Environ => handle_env,
    msg::Any::FetchReq => handle_fetch_req,
    msg::Any::TimerStart => handle_timer_start,
    msg::Any::TimerClear => handle_timer_clear,
    msg::Any::MakeTempDir => handle_make_temp_dir,
    msg::Any::Mkdir => handle_mkdir,
    msg::Any::Remove => handle_remove,
    msg::Any::ReadFile => handle_read_file,
    msg::Any::Rename => handle_rename,
    msg::Any::Readlink => handle_read_link,
    msg::Any::Symlink => handle_symlink,
    msg::Any::SetEnv => handle_set_env,
    msg::Any::Stat => handle_stat,
    msg::Any::WriteFile => handle_write_file,
    msg::Any::Exit => handle_exit,
    _ => panic!(format!(
      "Unhandled message {}",
      msg::enum_name_any(msg_type)
    )),
  };

  let op: Box<Op> = handler(state.clone(), &base);
  let boxed_op = Box::new(
    op.or_else(move |err: DenoError| -> DenoResult<Buf> {
      debug!("op err {}", err);
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
    }).and_then(move |buf: Buf| -> DenoResult<Buf> {
      // Handle empty responses. For sync responses we just want
      // to send null. For async we want to send a small message
      // with the cmd_id.
      let buf = if is_sync || buf.len() > 0 {
        buf
      } else {
        // async RPCs that return empty still need to
        // send a message back to signal completion.
        let builder = &mut FlatBufferBuilder::new();
        serialize_response(
          cmd_id,
          builder,
          msg::BaseArgs {
            ..Default::default()
          },
        )
      };
      Ok(buf)
    }),
  );

  debug!(
    "msg_from_js {} sync {}",
    msg::enum_name_any(msg_type),
    base.sync()
  );
  return (base.sync(), boxed_op);
}

fn permission_denied() -> DenoError {
  DenoError::from(std::io::Error::new(
    std::io::ErrorKind::PermissionDenied,
    "permission denied",
  ))
}

fn not_implemented() -> DenoError {
  DenoError::from(std::io::Error::new(
    std::io::ErrorKind::Other,
    "Not implemented",
  ))
}

fn handle_exit(_config: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_exit().unwrap();
  std::process::exit(msg.code())
}

fn handle_start(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let mut builder = FlatBufferBuilder::new();

  let argv = state.argv.iter().map(|s| s.as_str()).collect::<Vec<_>>();
  let argv_off = builder.create_vector_of_strings(argv.as_slice());

  let cwd_path = std::env::current_dir().unwrap();
  let cwd_off =
    builder.create_string(deno_fs::normalize_path(cwd_path.as_ref()).as_ref());

  let msg = msg::StartRes::create(
    &mut builder,
    &msg::StartResArgs {
      cwd: Some(cwd_off),
      argv: Some(argv_off),
      debug_flag: state.flags.log_debug,
      recompile_flag: state.flags.recompile,
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
  vec.into_boxed_slice()
}

fn ok_future(buf: Buf) -> Box<Op> {
  Box::new(futures::future::ok(buf))
}

// Shout out to Earl Sweatshirt.
fn odd_future(err: DenoError) -> Box<Op> {
  Box::new(futures::future::err(err))
}

// https://github.com/denoland/isolate/blob/golang/os.go#L100-L154
fn handle_code_fetch(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_code_fetch().unwrap();
  let cmd_id = base.cmd_id();
  let module_specifier = msg.module_specifier().unwrap();
  let containing_file = msg.containing_file().unwrap();

  assert_eq!(state.dir.root.join("gen"), state.dir.gen, "Sanity check");

  Box::new(futures::future::result(|| -> OpResult {
    let builder = &mut FlatBufferBuilder::new();
    let out = state.dir.code_fetch(module_specifier, containing_file)?;
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

// https://github.com/denoland/isolate/blob/golang/os.go#L156-L169
fn handle_code_cache(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_code_cache().unwrap();
  let filename = msg.filename().unwrap();
  let source_code = msg.source_code().unwrap();
  let output_code = msg.output_code().unwrap();
  Box::new(futures::future::result(|| -> OpResult {
    state.dir.code_cache(filename, source_code, output_code)?;
    Ok(empty_buf())
  }()))
}

fn handle_set_timeout(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_set_timeout().unwrap();
  let val = msg.timeout() as isize;
  state
    .timeout
    .swap(val, std::sync::atomic::Ordering::Relaxed);
  ok_future(empty_buf())
}

fn handle_set_env(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_set_env().unwrap();
  let key = msg.key().unwrap();
  let value = msg.value().unwrap();

  if !state.flags.allow_env {
    return odd_future(permission_denied());
  }

  std::env::set_var(key, value);
  ok_future(empty_buf())
}

fn handle_env(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let cmd_id = base.cmd_id();

  if !state.flags.allow_env {
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
    }).collect();
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

fn handle_fetch_req(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_fetch_req().unwrap();
  let cmd_id = base.cmd_id();
  let id = msg.id();
  let url = msg.url().unwrap();

  if !state.flags.allow_net {
    return odd_future(permission_denied());
  }

  let url = url.parse::<hyper::Uri>().unwrap();
  let client = Client::new();

  debug!("Before fetch {}", url);
  let future = client.get(url).and_then(move |res| {
    let status = res.status().as_u16() as i32;
    debug!("fetch {}", status);

    let headers = {
      let map = res.headers();
      let keys = map
        .keys()
        .map(|s| s.as_str().to_string())
        .collect::<Vec<_>>();
      let values = map
        .values()
        .map(|s| s.to_str().unwrap().to_string())
        .collect::<Vec<_>>();
      (keys, values)
    };

    // TODO Handle streaming body.
    res
      .into_body()
      .concat2()
      .map(move |body| (status, body, headers))
  });

  let future = future.map_err(|err| -> DenoError { err.into() }).and_then(
    move |(status, body, headers)| {
      debug!("fetch body ");
      let builder = &mut FlatBufferBuilder::new();
      // Send the first message without a body. This is just to indicate
      // what status code.
      let body_off = builder.create_vector(body.as_ref());
      let header_keys: Vec<&str> = headers.0.iter().map(|s| &**s).collect();
      let header_keys_off =
        builder.create_vector_of_strings(header_keys.as_slice());
      let header_values: Vec<&str> = headers.1.iter().map(|s| &**s).collect();
      let header_values_off =
        builder.create_vector_of_strings(header_values.as_slice());

      let msg = msg::FetchRes::create(
        builder,
        &msg::FetchResArgs {
          id,
          status,
          body: Some(body_off),
          header_key: Some(header_keys_off),
          header_value: Some(header_values_off),
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
    }).select(cancel_rx)
    .map(|_| ())
    .map_err(|_| ());

  (delay_task, cancel_tx)
}

fn handle_make_temp_dir(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let base = Box::new(*base);
  let msg = base.msg_as_make_temp_dir().unwrap();
  let cmd_id = base.cmd_id();
  let dir = msg.dir();
  let prefix = msg.prefix();
  let suffix = msg.suffix();

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }
  // TODO Use blocking() here.
  Box::new(futures::future::result(|| -> OpResult {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/isolate/issues/627.
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

fn handle_mkdir(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_mkdir().unwrap();
  let mode = msg.mode();
  let path = msg.path().unwrap();

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }
  // TODO Use tokio_threadpool.
  Box::new(futures::future::result(|| -> OpResult {
    debug!("handle_mkdir {}", path);
    deno_fs::mkdir(Path::new(path), mode)?;
    Ok(empty_buf())
  }()))
}

fn handle_remove(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_remove().unwrap();
  let path = msg.path().unwrap();
  let recursive = msg.recursive();

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }
  // TODO Use tokio_threadpool.
  Box::new(futures::future::result(|| -> OpResult {
    debug!("handle_remove {}", path);
    let path_ = Path::new(&path);
    let metadata = fs::metadata(&path_)?;
    if metadata.is_file() {
      fs::remove_file(&path_)?;
    } else {
      if recursive {
        remove_dir_all(&path_)?;
      } else {
        fs::remove_dir(&path_)?;
      }
    }
    Ok(empty_buf())
  }()))
}

// Prototype https://github.com/denoland/isolate/blob/golang/os.go#L171-L184
fn handle_read_file(_config: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
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

#[cfg(any(unix))]
fn get_mode(perm: fs::Permissions) -> u32 {
  perm.mode()
}

#[cfg(not(any(unix)))]
fn get_mode(_perm: fs::Permissions) -> u32 {
  0
}

fn handle_stat(_config: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_stat().unwrap();
  let cmd_id = base.cmd_id();
  let filename = String::from(msg.filename().unwrap());
  let lstat = msg.lstat();

  Box::new(futures::future::result(|| -> OpResult {
    let builder = &mut FlatBufferBuilder::new();
    debug!("handle_stat {} {}", filename, lstat);
    let path = Path::new(&filename);
    let metadata = if lstat {
      fs::symlink_metadata(path)?
    } else {
      fs::metadata(path)?
    };

    let msg = msg::StatRes::create(
      builder,
      &msg::StatResArgs {
        is_file: metadata.is_file(),
        is_symlink: metadata.file_type().is_symlink(),
        len: metadata.len(),
        modified: to_seconds!(metadata.modified()),
        accessed: to_seconds!(metadata.accessed()),
        created: to_seconds!(metadata.created()),
        mode: get_mode(metadata.permissions()),
        has_mode: cfg!(target_family = "unix"),
        ..Default::default()
      },
    );

    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        msg: Some(msg.as_union_value()),
        msg_type: msg::Any::StatRes,
        ..Default::default()
      },
    ))
  }()))
}

fn handle_write_file(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_write_file().unwrap();
  let filename = String::from(msg.filename().unwrap());
  let data = msg.data().unwrap();
  let perm = msg.perm();

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }

  Box::new(futures::future::result(|| -> OpResult {
    debug!("handle_write_file {}", filename);
    deno_fs::write_file(Path::new(&filename), data, perm)?;
    Ok(empty_buf())
  }()))
}

fn remove_timer(state: Arc<IsolateState>, timer_id: u32) {
  let mut timers = state.timers.lock().unwrap();
  timers.remove(&timer_id);
}

// Prototype: https://github.com/ry/isolate/blob/golang/timers.go#L25-L39
fn handle_timer_start(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  debug!("handle_timer_start");
  let msg = base.msg_as_timer_start().unwrap();
  let cmd_id = base.cmd_id();
  let timer_id = msg.id();
  let delay = msg.delay();

  let config2 = state.clone();
  let future = {
    let (delay_task, cancel_delay) = set_timeout(
      move || {
        remove_timer(config2, timer_id);
      },
      delay,
    );
    let mut timers = state.timers.lock().unwrap();
    timers.insert(timer_id, cancel_delay);
    delay_task
  };
  let r = Box::new(future.then(move |result| {
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
  }));
  r
}

// Prototype: https://github.com/ry/isolate/blob/golang/timers.go#L40-L43
fn handle_timer_clear(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_timer_clear().unwrap();
  debug!("handle_timer_clear");
  remove_timer(state, msg.id());
  ok_future(empty_buf())
}

fn handle_rename(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }
  let msg = base.msg_as_rename().unwrap();
  let oldpath = String::from(msg.oldpath().unwrap());
  let newpath = String::from(msg.newpath().unwrap());
  Box::new(futures::future::result(|| -> OpResult {
    debug!("handle_rename {} {}", oldpath, newpath);
    fs::rename(Path::new(&oldpath), Path::new(&newpath))?;
    Ok(empty_buf())
  }()))
}

fn handle_symlink(state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }
  // TODO Use type for Windows.
  if cfg!(windows) {
    return odd_future(not_implemented());
  } else {
    let msg = base.msg_as_symlink().unwrap();
    let oldname = String::from(msg.oldname().unwrap());
    let newname = String::from(msg.newname().unwrap());
    Box::new(futures::future::result(|| -> OpResult {
      debug!("handle_symlink {} {}", oldname, newname);
      #[cfg(any(unix))]
      std::os::unix::fs::symlink(Path::new(&oldname), Path::new(&newname))?;
      Ok(empty_buf())
    }()))
  }
}

fn handle_read_link(_state: Arc<IsolateState>, base: &msg::Base) -> Box<Op> {
  let msg = base.msg_as_readlink().unwrap();
  let cmd_id = base.cmd_id();
  let name = String::from(msg.name().unwrap());
  Box::new(futures::future::result(|| -> OpResult {
    debug!("handle_read_link {}", name);
    let path = fs::read_link(Path::new(&name))?;
    let builder = &mut FlatBufferBuilder::new();
    let path_off = builder.create_string(path.to_str().unwrap());
    let msg = msg::ReadlinkRes::create(
      builder,
      &msg::ReadlinkResArgs {
        path: Some(path_off),
        ..Default::default()
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        msg: Some(msg.as_union_value()),
        msg_type: msg::Any::ReadlinkRes,
        ..Default::default()
      },
    ))
  }()))
}
