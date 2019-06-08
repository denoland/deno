// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use atty;
use crate::ansi;
use crate::deno_dir::resolve_path;
use crate::dispatch_minimal::dispatch_minimal;
use crate::dispatch_minimal::parse_min_record;
use crate::errors;
use crate::errors::{DenoError, DenoResult, ErrorKind};
use crate::fs as deno_fs;
use crate::http_util;
use crate::js_errors::apply_source_map;
use crate::js_errors::JSErrorColor;
use crate::msg;
use crate::msg_util;
use crate::rand;
use crate::repl;
use crate::resolve_addr::resolve_addr;
use crate::resources;
use crate::resources::table_entries;
use crate::resources::Resource;
use crate::signal::kill;
use crate::startup_data;
use crate::state::ThreadSafeState;
use crate::tokio_util;
use crate::tokio_write;
use crate::version;
use crate::worker::root_specifier_to_url;
use crate::worker::Worker;
use deno::js_check;
use deno::Buf;
use deno::JSError;
use deno::Op;
use deno::PinnedBuf;
use flatbuffers::FlatBufferBuilder;
use futures;
use futures::Async;
use futures::Poll;
use futures::Sink;
use futures::Stream;
use futures::future;
use hyper;
use hyper::rt::Future;
use rand::{thread_rng, Rng};
use remove_dir_all::remove_dir_all;
use std;
use std::convert::From;
use std::fs;
use std::net::Shutdown;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tokio;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_process::CommandExt;
use tokio_threadpool;
use utime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

type OpResult = DenoResult<Buf>;

pub type OpWithError = dyn Future<Item = Buf, Error = DenoError> + Send;

// TODO Ideally we wouldn't have to box the OpWithError being returned.
// The box is just to make it easier to get a prototype refactor working.
type OpCreator =
  fn(state: &ThreadSafeState, base: &msg::Base<'_>, data: Option<PinnedBuf>)
    -> Box<OpWithError>;

pub type OpSelector = fn(inner_type: msg::Any) -> Option<OpCreator>;

#[inline]
fn empty_buf() -> Buf {
  Box::new([])
}

pub fn dispatch_all(
  state: &ThreadSafeState,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
  op_selector: OpSelector,
) -> Op {
  let bytes_sent_control = control.len();
  let bytes_sent_zero_copy = zero_copy.as_ref().map(|b| b.len()).unwrap_or(0);
  let op = if let Some(min_record) = parse_min_record(control) {
    dispatch_minimal(state, min_record, zero_copy)
  } else {
    dispatch_all_legacy(state, control, zero_copy, op_selector)
  };
  state.metrics_op_dispatched(bytes_sent_control, bytes_sent_zero_copy);
  op
}

/// Processes raw messages from JavaScript.
/// This functions invoked every time Deno.core.dispatch() is called.
/// control corresponds to the first argument of Deno.core.dispatch().
/// data corresponds to the second argument of Deno.core.dispatch().
pub fn dispatch_all_legacy(
  state: &ThreadSafeState,
  control: &[u8],
  zero_copy: Option<PinnedBuf>,
  op_selector: OpSelector,
) -> Op {
  let base = msg::get_root_as_base(&control);
  let is_sync = base.sync();
  let inner_type = base.inner_type();
  let cmd_id = base.cmd_id();

  let op_func: OpCreator = match op_selector(inner_type) {
    Some(v) => v,
    None => panic!("Unhandled message {}", msg::enum_name_any(inner_type)),
  };

  let op: Box<OpWithError> = op_func(state, &base, zero_copy);

  let state = state.clone();

  let fut = Box::new(
    op.or_else(move |err: DenoError| -> Result<Buf, ()> {
      debug!("op err {}", err);
      // No matter whether we got an Err or Ok, we want a serialized message to
      // send back. So transform the DenoError into a Buf.
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
    }).and_then(move |buf: Buf| -> Result<Buf, ()> {
      // Handle empty responses. For sync responses we just want
      // to send null. For async we want to send a small message
      // with the cmd_id.
      let buf = if is_sync || buf.len() > 0 {
        buf
      } else {
        let builder = &mut FlatBufferBuilder::new();
        serialize_response(
          cmd_id,
          builder,
          msg::BaseArgs {
            ..Default::default()
          },
        )
      };
      state.metrics_op_completed(buf.len());
      Ok(buf)
    }).map_err(|err| panic!("unexpected error {:?}", err)),
  );

  debug!(
    "msg_from_js {} sync {}",
    msg::enum_name_any(inner_type),
    base.sync()
  );

  if base.sync() {
    // TODO(ry) This is not correct! If the sync op is not actually synchronous
    // (like in the case of op_fetch_module_meta_data) this wait() will block
    // a thread in the Tokio runtime. Depending on the size of the runtime's
    // thread pool, this may result in a dead lock!
    //
    // The solution is that ops should return an Op directly. Op::Sync contains
    // the result value, so if its returned directly from the OpCreator, we
    // know it has actually be evaluated synchronously.
    Op::Sync(fut.wait().unwrap())
  } else {
    Op::Async(fut)
  }
}

/// Standard ops set for most isolates
pub fn op_selector_std(inner_type: msg::Any) -> Option<OpCreator> {
  match inner_type {
    msg::Any::Accept => Some(op_accept),
    msg::Any::Cache => Some(op_cache),
    msg::Any::Chdir => Some(op_chdir),
    msg::Any::Chmod => Some(op_chmod),
    msg::Any::Chown => Some(op_chown),
    msg::Any::Close => Some(op_close),
    msg::Any::CopyFile => Some(op_copy_file),
    msg::Any::CreateWorker => Some(op_create_worker),
    msg::Any::Cwd => Some(op_cwd),
    msg::Any::Dial => Some(op_dial),
    msg::Any::Environ => Some(op_env),
    msg::Any::Exit => Some(op_exit),
    msg::Any::Fetch => Some(op_fetch),
    msg::Any::FetchModuleMetaData => Some(op_fetch_module_meta_data),
    msg::Any::FormatError => Some(op_format_error),
    msg::Any::GetRandomValues => Some(op_get_random_values),
    msg::Any::GlobalTimer => Some(op_global_timer),
    msg::Any::GlobalTimerStop => Some(op_global_timer_stop),
    msg::Any::HostGetMessage => Some(op_host_get_message),
    msg::Any::HostGetWorkerClosed => Some(op_host_get_worker_closed),
    msg::Any::HostPostMessage => Some(op_host_post_message),
    msg::Any::IsTTY => Some(op_is_tty),
    msg::Any::Kill => Some(op_kill),
    msg::Any::Link => Some(op_link),
    msg::Any::Listen => Some(op_listen),
    msg::Any::MakeTempDir => Some(op_make_temp_dir),
    msg::Any::Metrics => Some(op_metrics),
    msg::Any::Mkdir => Some(op_mkdir),
    msg::Any::Now => Some(op_now),
    msg::Any::Open => Some(op_open),
    msg::Any::PermissionRevoke => Some(op_revoke_permission),
    msg::Any::Permissions => Some(op_permissions),
    msg::Any::Read => Some(op_read),
    msg::Any::ReadDir => Some(op_read_dir),
    msg::Any::Readlink => Some(op_read_link),
    msg::Any::Remove => Some(op_remove),
    msg::Any::Rename => Some(op_rename),
    msg::Any::ReplReadline => Some(op_repl_readline),
    msg::Any::ReplStart => Some(op_repl_start),
    msg::Any::Resources => Some(op_resources),
    msg::Any::Run => Some(op_run),
    msg::Any::RunStatus => Some(op_run_status),
    msg::Any::Seek => Some(op_seek),
    msg::Any::SetEnv => Some(op_set_env),
    msg::Any::Shutdown => Some(op_shutdown),
    msg::Any::Start => Some(op_start),
    msg::Any::Stat => Some(op_stat),
    msg::Any::Symlink => Some(op_symlink),
    msg::Any::Truncate => Some(op_truncate),
    msg::Any::Utime => Some(op_utime),
    msg::Any::Write => Some(op_write),

    // TODO(ry) split these out so that only the appropriate Workers can access
    // them.
    msg::Any::WorkerGetMessage => Some(op_worker_get_message),
    msg::Any::WorkerPostMessage => Some(op_worker_post_message),

    _ => None,
  }
}

// Returns a milliseconds and nanoseconds subsec
// since the start time of the deno runtime.
// If the High precision flag is not set, the
// nanoseconds are rounded on 2ms.
fn op_now(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let seconds = state.start_time.elapsed().as_secs();
  let mut subsec_nanos = state.start_time.elapsed().subsec_nanos();
  let reduced_time_precision = 2_000_000; // 2ms in nanoseconds

  // If the permission is not enabled
  // Round the nano result on 2 milliseconds
  // see: https://developer.mozilla.org/en-US/docs/Web/API/DOMHighResTimeStamp#Reduced_time_precision
  if !state.permissions.allows_hrtime() {
    subsec_nanos -= subsec_nanos % reduced_time_precision
  }

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::NowRes::create(
    builder,
    &msg::NowResArgs {
      seconds,
      subsec_nanos,
    },
  );
  ok_future(serialize_response(
    base.cmd_id(),
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::NowRes,
      ..Default::default()
    },
  ))
}

fn op_is_tty(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  _data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::IsTTYRes::create(
    builder,
    &msg::IsTTYResArgs {
      stdin: atty::is(atty::Stream::Stdin),
      stdout: atty::is(atty::Stream::Stdout),
      stderr: atty::is(atty::Stream::Stderr),
    },
  );
  ok_future(serialize_response(
    base.cmd_id(),
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::IsTTYRes,
      ..Default::default()
    },
  ))
}

fn op_exit(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  _data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  let inner = base.inner_as_exit().unwrap();
  std::process::exit(inner.code())
}

fn op_start(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let mut builder = FlatBufferBuilder::new();

  let state = state;
  let argv = state.argv.iter().map(String::as_str).collect::<Vec<_>>();
  let argv_off = builder.create_vector_of_strings(argv.as_slice());

  let cwd_path = std::env::current_dir().unwrap();
  let cwd_off =
    builder.create_string(deno_fs::normalize_path(cwd_path.as_ref()).as_ref());

  let exec_path =
    builder.create_string(std::env::current_exe().unwrap().to_str().unwrap());

  let v8_version = version::v8();
  let v8_version_off = builder.create_string(v8_version);

  let deno_version = version::DENO;
  let deno_version_off = builder.create_string(deno_version);

  let main_module = state.main_module().map(|m| builder.create_string(&m));

  let xeval_delim = state
    .flags
    .xeval_delim
    .clone()
    .map(|m| builder.create_string(&m));

  let inner = msg::StartRes::create(
    &mut builder,
    &msg::StartResArgs {
      cwd: Some(cwd_off),
      pid: std::process::id(),
      argv: Some(argv_off),
      main_module,
      debug_flag: state.flags.log_debug,
      version_flag: state.flags.version,
      v8_version: Some(v8_version_off),
      deno_version: Some(deno_version_off),
      no_color: !ansi::use_color(),
      exec_path: Some(exec_path),
      xeval_delim,
      ..Default::default()
    },
  );

  ok_future(serialize_response(
    base.cmd_id(),
    &mut builder,
    msg::BaseArgs {
      inner_type: msg::Any::StartRes,
      inner: Some(inner.as_union_value()),
      ..Default::default()
    },
  ))
}

fn op_format_error(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_format_error().unwrap();
  let orig_error = String::from(inner.error().unwrap());

  let js_error = JSError::from_v8_exception(&orig_error).unwrap();
  let js_error_mapped = apply_source_map(&js_error, &state.dir);
  let js_error_string = JSErrorColor(&js_error_mapped).to_string();

  let mut builder = FlatBufferBuilder::new();
  let new_error = builder.create_string(&js_error_string);

  let inner = msg::FormatErrorRes::create(
    &mut builder,
    &msg::FormatErrorResArgs {
      error: Some(new_error),
    },
  );

  ok_future(serialize_response(
    base.cmd_id(),
    &mut builder,
    msg::BaseArgs {
      inner_type: msg::Any::FormatErrorRes,
      inner: Some(inner.as_union_value()),
      ..Default::default()
    },
  ))
}

fn serialize_response(
  cmd_id: u32,
  builder: &mut FlatBufferBuilder<'_>,
  mut args: msg::BaseArgs<'_>,
) -> Buf {
  args.cmd_id = cmd_id;
  let base = msg::Base::create(builder, &args);
  msg::finish_base_buffer(builder, base);
  let data = builder.finished_data();
  // println!("serialize_response {:x?}", data);
  data.into()
}

#[inline]
pub fn ok_future(buf: Buf) -> Box<OpWithError> {
  Box::new(futures::future::ok(buf))
}

// Shout out to Earl Sweatshirt.
#[inline]
pub fn odd_future(err: DenoError) -> Box<OpWithError> {
  Box::new(futures::future::err(err))
}

fn op_cache(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_cache().unwrap();
  let extension = inner.extension().unwrap();
  let module_id = inner.module_id().unwrap();
  let contents = inner.contents().unwrap();

  state.mark_compiled(&module_id);

  // TODO It shouldn't be necessary to call fetch_module_meta_data() here.
  // However, we need module_meta_data.source_code in order to calculate the
  // cache path. In the future, checksums will not be used in the cache
  // filenames and this requirement can be removed. See
  // https://github.com/denoland/deno/issues/2057
  let r = state.dir.fetch_module_meta_data(module_id, ".", true, true);
  if let Err(err) = r {
    return odd_future(err);
  }
  let module_meta_data = r.unwrap();

  let (js_cache_path, source_map_path) = state
    .dir
    .cache_path(&module_meta_data.filename, &module_meta_data.source_code);

  if extension == ".map" {
    debug!("cache {:?}", source_map_path);
    let r = fs::write(source_map_path, contents);
    if let Err(err) = r {
      return odd_future(err.into());
    }
  } else if extension == ".js" {
    debug!("cache {:?}", js_cache_path);
    let r = fs::write(js_cache_path, contents);
    if let Err(err) = r {
      return odd_future(err.into());
    }
  } else {
    unreachable!();
  }

  ok_future(empty_buf())
}

// https://github.com/denoland/deno/blob/golang/os.go#L100-L154
fn op_fetch_module_meta_data(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_fetch_module_meta_data().unwrap();
  let cmd_id = base.cmd_id();
  let specifier = inner.specifier().unwrap();
  let referrer = inner.referrer().unwrap();

  assert_eq!(state.dir.root.join("gen"), state.dir.gen, "Sanity check");

  let use_cache = !state.flags.reload;
  let no_fetch = state.flags.no_fetch;

  let fut = state
    .dir
    .fetch_module_meta_data_async(specifier, referrer, use_cache, no_fetch)
    .and_then(move |out| {
      let builder = &mut FlatBufferBuilder::new();
      let data_off = builder.create_vector(out.source_code.as_slice());
      let msg_args = msg::FetchModuleMetaDataResArgs {
        module_name: Some(builder.create_string(&out.module_name)),
        filename: Some(builder.create_string(&out.filename)),
        media_type: out.media_type,
        data: Some(data_off),
      };
      let inner = msg::FetchModuleMetaDataRes::create(builder, &msg_args);
      Ok(serialize_response(
        cmd_id,
        builder,
        msg::BaseArgs {
          inner: Some(inner.as_union_value()),
          inner_type: msg::Any::FetchModuleMetaDataRes,
          ..Default::default()
        },
      ))
    });

  // Unfortunately TypeScript's CompilerHost interface does not leave room for
  // asynchronous source code fetching. This complicates things greatly and
  // requires us to use tokio_util::block_on() below.
  assert!(base.sync());

  // WARNING: Here we use tokio_util::block_on() which starts a new Tokio
  // runtime for executing the future. This is so we don't inadvernently run
  // out of threads in the main runtime.
  Box::new(futures::future::result(tokio_util::block_on(fut)))
}

fn op_chdir(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_chdir().unwrap();
  let directory = inner.directory().unwrap();
  Box::new(futures::future::result(|| -> OpResult {
    std::env::set_current_dir(&directory)?;
    Ok(empty_buf())
  }()))
}

fn op_global_timer_stop(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(base.sync());
  assert!(data.is_none());
  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  t.cancel();
  ok_future(empty_buf())
}

fn op_global_timer(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(!base.sync());
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_global_timer().unwrap();
  let val = inner.timeout();
  assert!(val >= 0);

  let state = state;
  let mut t = state.global_timer.lock().unwrap();
  let deadline = Instant::now() + Duration::from_millis(val as u64);
  let f = t.new_timeout(deadline);

  Box::new(f.then(move |_| {
    let builder = &mut FlatBufferBuilder::new();
    let inner =
      msg::GlobalTimerRes::create(builder, &msg::GlobalTimerResArgs {});
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::GlobalTimerRes,
        ..Default::default()
      },
    ))
  }))
}

fn op_set_env(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_set_env().unwrap();
  let key = inner.key().unwrap();
  let value = inner.value().unwrap();
  if let Err(e) = state.check_env() {
    return odd_future(e);
  }
  std::env::set_var(key, value);
  ok_future(empty_buf())
}

fn op_env(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  if let Err(e) = state.check_env() {
    return odd_future(e);
  }

  let builder = &mut FlatBufferBuilder::new();
  let vars: Vec<_> = std::env::vars()
    .map(|(key, value)| msg_util::serialize_key_value(builder, &key, &value))
    .collect();
  let tables = builder.create_vector(&vars);
  let inner = msg::EnvironRes::create(
    builder,
    &msg::EnvironResArgs { map: Some(tables) },
  );
  ok_future(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::EnvironRes,
      ..Default::default()
    },
  ))
}

fn op_permissions(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::PermissionsRes::create(
    builder,
    &msg::PermissionsResArgs {
      run: state.permissions.allows_run(),
      read: state.permissions.allows_read(),
      write: state.permissions.allows_write(),
      net: state.permissions.allows_net(),
      env: state.permissions.allows_env(),
      hrtime: state.permissions.allows_hrtime(),
    },
  );
  ok_future(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::PermissionsRes,
      ..Default::default()
    },
  ))
}

fn op_revoke_permission(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_permission_revoke().unwrap();
  let permission = inner.permission().unwrap();
  let result = match permission {
    "run" => state.permissions.revoke_run(),
    "read" => state.permissions.revoke_read(),
    "write" => state.permissions.revoke_write(),
    "net" => state.permissions.revoke_net(),
    "env" => state.permissions.revoke_env(),
    "hrtime" => state.permissions.revoke_hrtime(),
    _ => Ok(()),
  };
  if let Err(e) = result {
    return odd_future(e);
  }
  ok_future(empty_buf())
}

fn op_fetch(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  let inner = base.inner_as_fetch().unwrap();
  let cmd_id = base.cmd_id();

  let header = inner.header().unwrap();
  assert!(header.is_request());
  let url = header.url().unwrap();

  let body = match data {
    None => hyper::Body::empty(),
    Some(buf) => hyper::Body::from(Vec::from(&*buf)),
  };

  let maybe_req = msg_util::deserialize_request(header, body);
  if let Err(e) = maybe_req {
    return odd_future(e);
  }
  let req = maybe_req.unwrap();

  let url_ = match url::Url::parse(url) {
    Err(err) => return odd_future(DenoError::from(err)),
    Ok(v) => v,
  };
  if let Err(e) = state.check_net_url(url_) {
    return odd_future(e);
  }

  let client = http_util::get_client();

  debug!("Before fetch {}", url);
  let future =
    client
      .request(req)
      .map_err(DenoError::from)
      .and_then(move |res| {
        let builder = &mut FlatBufferBuilder::new();
        let header_off = msg_util::serialize_http_response(builder, &res);
        let body = res.into_body();
        let body_resource = resources::add_hyper_body(body);
        let inner = msg::FetchRes::create(
          builder,
          &msg::FetchResArgs {
            header: Some(header_off),
            body_rid: body_resource.rid,
          },
        );

        Ok(serialize_response(
          cmd_id,
          builder,
          msg::BaseArgs {
            inner: Some(inner.as_union_value()),
            inner_type: msg::Any::FetchRes,
            ..Default::default()
          },
        ))
      });
  Box::new(future)
}

// This is just type conversion. Implement From trait?
// See https://github.com/tokio-rs/tokio/blob/ffd73a64e7ec497622b7f939e38017afe7124dc4/tokio-fs/src/lib.rs#L76-L85
fn convert_blocking<F>(f: F) -> Poll<Buf, DenoError>
where
  F: FnOnce() -> DenoResult<Buf>,
{
  use futures::Async::*;
  match tokio_threadpool::blocking(f) {
    Ok(Ready(Ok(v))) => Ok(v.into()),
    Ok(Ready(Err(err))) => Err(err),
    Ok(NotReady) => Ok(NotReady),
    Err(err) => panic!("blocking error {}", err),
  }
}

fn blocking<F>(is_sync: bool, f: F) -> Box<OpWithError>
where
  F: 'static + Send + FnOnce() -> DenoResult<Buf>,
{
  if is_sync {
    Box::new(futures::future::result(f()))
  } else {
    Box::new(tokio_util::poll_fn(move || convert_blocking(f)))
  }
}

fn op_make_temp_dir(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let base = Box::new(*base);
  let inner = base.inner_as_make_temp_dir().unwrap();
  let cmd_id = base.cmd_id();

  // FIXME
  if let Err(e) = state.check_write("make_temp") {
    return odd_future(e);
  }

  let dir = inner.dir().map(PathBuf::from);
  let prefix = inner.prefix().map(String::from);
  let suffix = inner.suffix().map(String::from);

  blocking(base.sync(), move || -> OpResult {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/deno/issues/627.
    // We can't assume that paths are always valid utf8 strings.
    let path = deno_fs::make_temp_dir(
      // Converting Option<String> to Option<&str>
      dir.as_ref().map(|x| &**x),
      prefix.as_ref().map(|x| &**x),
      suffix.as_ref().map(|x| &**x),
    )?;
    let builder = &mut FlatBufferBuilder::new();
    let path_off = builder.create_string(path.to_str().unwrap());
    let inner = msg::MakeTempDirRes::create(
      builder,
      &msg::MakeTempDirResArgs {
        path: Some(path_off),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::MakeTempDirRes,
        ..Default::default()
      },
    ))
  })
}

fn op_mkdir(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_mkdir().unwrap();
  let (path, path_) = match resolve_path(inner.path().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let recursive = inner.recursive();
  let mode = inner.mode();

  if let Err(e) = state.check_write(&path_) {
    return odd_future(e);
  }

  blocking(base.sync(), move || {
    debug!("op_mkdir {}", path_);
    deno_fs::mkdir(&path, mode, recursive)?;
    Ok(empty_buf())
  })
}

fn op_chmod(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_chmod().unwrap();
  let _mode = inner.mode();
  let (path, path_) = match resolve_path(inner.path().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };

  if let Err(e) = state.check_write(&path_) {
    return odd_future(e);
  }

  blocking(base.sync(), move || {
    debug!("op_chmod {}", &path_);
    // Still check file/dir exists on windows
    let _metadata = fs::metadata(&path)?;
    #[cfg(any(unix))]
    {
      let mut permissions = _metadata.permissions();
      permissions.set_mode(_mode);
      fs::set_permissions(&path, permissions)?;
    }
    Ok(empty_buf())
  })
}

fn op_chown(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_chown().unwrap();
  let path = String::from(inner.path().unwrap());
  let uid = inner.uid();
  let gid = inner.gid();

  if let Err(e) = state.check_write(&path) {
    return odd_future(e);
  }

  blocking(base.sync(), move || {
    debug!("op_chown {}", &path);
    match deno_fs::chown(&path, uid, gid) {
      Ok(_) => Ok(empty_buf()),
      Err(e) => Err(e),
    }
  })
}

fn op_open(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_open().unwrap();
  let (filename, filename_) = match resolve_path(inner.filename().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let mode = inner.mode().unwrap();

  let mut open_options = tokio::fs::OpenOptions::new();

  match mode {
    "r" => {
      open_options.read(true);
    }
    "r+" => {
      open_options.read(true).write(true);
    }
    "w" => {
      open_options.create(true).write(true).truncate(true);
    }
    "w+" => {
      open_options
        .read(true)
        .create(true)
        .write(true)
        .truncate(true);
    }
    "a" => {
      open_options.create(true).append(true);
    }
    "a+" => {
      open_options.read(true).create(true).append(true);
    }
    "x" => {
      open_options.create_new(true).write(true);
    }
    "x+" => {
      open_options.create_new(true).read(true).write(true);
    }
    &_ => {
      panic!("Unknown file open mode.");
    }
  }

  match mode {
    "r" => {
      if let Err(e) = state.check_read(&filename_) {
        return odd_future(e);
      }
    }
    "w" | "a" | "x" => {
      if let Err(e) = state.check_write(&filename_) {
        return odd_future(e);
      }
    }
    &_ => {
      if let Err(e) = state.check_read(&filename_) {
        return odd_future(e);
      }
      if let Err(e) = state.check_write(&filename_) {
        return odd_future(e);
      }
    }
  }

  let op = open_options
    .open(filename)
    .map_err(DenoError::from)
    .and_then(move |fs_file| -> OpResult {
      let resource = resources::add_fs_file(fs_file);
      let builder = &mut FlatBufferBuilder::new();
      let inner =
        msg::OpenRes::create(builder, &msg::OpenResArgs { rid: resource.rid });
      Ok(serialize_response(
        cmd_id,
        builder,
        msg::BaseArgs {
          inner: Some(inner.as_union_value()),
          inner_type: msg::Any::OpenRes,
          ..Default::default()
        },
      ))
    });
  Box::new(op)
}

fn op_close(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_close().unwrap();
  let rid = inner.rid();
  match resources::lookup(rid) {
    None => odd_future(errors::bad_resource()),
    Some(resource) => {
      resource.close();
      ok_future(empty_buf())
    }
  }
}

fn op_kill(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_kill().unwrap();
  let pid = inner.pid();
  let signo = inner.signo();
  match kill(pid, signo) {
    Ok(_) => ok_future(empty_buf()),
    Err(e) => odd_future(e),
  }
}

fn op_shutdown(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_shutdown().unwrap();
  let rid = inner.rid();
  let how = inner.how();
  match resources::lookup(rid) {
    None => odd_future(errors::bad_resource()),
    Some(mut resource) => {
      let shutdown_mode = match how {
        0 => Shutdown::Read,
        1 => Shutdown::Write,
        _ => unimplemented!(),
      };
      blocking(base.sync(), move || {
        // Use UFCS for disambiguation
        Resource::shutdown(&mut resource, shutdown_mode)?;
        Ok(empty_buf())
      })
    }
  }
}

fn op_read(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_read().unwrap();
  let rid = inner.rid();

  match resources::lookup(rid) {
    None => odd_future(errors::bad_resource()),
    Some(resource) => {
      let op = tokio::io::read(resource, data.unwrap())
        .map_err(DenoError::from)
        .and_then(move |(_resource, _buf, nread)| {
          let builder = &mut FlatBufferBuilder::new();
          let inner = msg::ReadRes::create(
            builder,
            &msg::ReadResArgs {
              nread: nread as u32,
              eof: nread == 0,
            },
          );
          Ok(serialize_response(
            cmd_id,
            builder,
            msg::BaseArgs {
              inner: Some(inner.as_union_value()),
              inner_type: msg::Any::ReadRes,
              ..Default::default()
            },
          ))
        });
      Box::new(op)
    }
  }
}

fn op_write(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_write().unwrap();
  let rid = inner.rid();

  match resources::lookup(rid) {
    None => odd_future(errors::bad_resource()),
    Some(resource) => {
      let op = tokio_write::write(resource, data.unwrap())
        .map_err(DenoError::from)
        .and_then(move |(_resource, _buf, nwritten)| {
          let builder = &mut FlatBufferBuilder::new();
          let inner = msg::WriteRes::create(
            builder,
            &msg::WriteResArgs {
              nbyte: nwritten as u32,
            },
          );
          Ok(serialize_response(
            cmd_id,
            builder,
            msg::BaseArgs {
              inner: Some(inner.as_union_value()),
              inner_type: msg::Any::WriteRes,
              ..Default::default()
            },
          ))
        });
      Box::new(op)
    }
  }
}

fn op_seek(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let _cmd_id = base.cmd_id();
  let inner = base.inner_as_seek().unwrap();
  let rid = inner.rid();
  let offset = inner.offset();
  let whence = inner.whence();

  match resources::lookup(rid) {
    None => odd_future(errors::bad_resource()),
    Some(resource) => {
      let op = resources::seek(resource, offset, whence)
        .and_then(move |_| Ok(empty_buf()));
      Box::new(op)
    }
  }
}

fn op_remove(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_remove().unwrap();
  let (path, path_) = match resolve_path(inner.path().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let recursive = inner.recursive();

  if let Err(e) = state.check_write(&path_) {
    return odd_future(e);
  }

  blocking(base.sync(), move || {
    debug!("op_remove {}", path.display());
    let metadata = fs::metadata(&path)?;
    if metadata.is_file() {
      fs::remove_file(&path)?;
    } else if recursive {
      remove_dir_all(&path)?;
    } else {
      fs::remove_dir(&path)?;
    }
    Ok(empty_buf())
  })
}

fn op_copy_file(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_copy_file().unwrap();
  let (from, from_) = match resolve_path(inner.from().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let (to, to_) = match resolve_path(inner.to().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };

  if let Err(e) = state.check_read(&from_) {
    return odd_future(e);
  }
  if let Err(e) = state.check_write(&to_) {
    return odd_future(e);
  }

  debug!("op_copy_file {} {}", from.display(), to.display());
  blocking(base.sync(), move || {
    // On *nix, Rust deem non-existent path as invalid input
    // See https://github.com/rust-lang/rust/issues/54800
    // Once the issue is reolved, we should remove this workaround.
    if cfg!(unix) && !from.is_file() {
      return Err(errors::new(
        ErrorKind::NotFound,
        "File not found".to_string(),
      ));
    }

    fs::copy(&from, &to)?;
    Ok(empty_buf())
  })
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
fn get_mode(perm: &fs::Permissions) -> u32 {
  perm.mode()
}

#[cfg(not(any(unix)))]
fn get_mode(_perm: &fs::Permissions) -> u32 {
  0
}

fn op_cwd(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  Box::new(futures::future::result(|| -> OpResult {
    let path = std::env::current_dir()?;
    let builder = &mut FlatBufferBuilder::new();
    let cwd =
      builder.create_string(&path.into_os_string().into_string().unwrap());
    let inner =
      msg::CwdRes::create(builder, &msg::CwdResArgs { cwd: Some(cwd) });
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::CwdRes,
        ..Default::default()
      },
    ))
  }()))
}

fn op_stat(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_stat().unwrap();
  let cmd_id = base.cmd_id();
  let (filename, filename_) = match resolve_path(inner.filename().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let lstat = inner.lstat();

  if let Err(e) = state.check_read(&filename_) {
    return odd_future(e);
  }

  blocking(base.sync(), move || {
    let builder = &mut FlatBufferBuilder::new();
    debug!("op_stat {} {}", filename.display(), lstat);
    let metadata = if lstat {
      fs::symlink_metadata(&filename)?
    } else {
      fs::metadata(&filename)?
    };

    let inner = msg::StatRes::create(
      builder,
      &msg::StatResArgs {
        is_file: metadata.is_file(),
        is_symlink: metadata.file_type().is_symlink(),
        len: metadata.len(),
        modified: to_seconds!(metadata.modified()),
        accessed: to_seconds!(metadata.accessed()),
        created: to_seconds!(metadata.created()),
        mode: get_mode(&metadata.permissions()),
        has_mode: cfg!(target_family = "unix"),
        ..Default::default()
      },
    );

    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::StatRes,
        ..Default::default()
      },
    ))
  })
}

fn op_read_dir(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_read_dir().unwrap();
  let cmd_id = base.cmd_id();
  let (path, path_) = match resolve_path(inner.path().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };

  if let Err(e) = state.check_read(&path_) {
    return odd_future(e);
  }

  blocking(base.sync(), move || -> OpResult {
    debug!("op_read_dir {}", path.display());
    let builder = &mut FlatBufferBuilder::new();
    let entries: Vec<_> = fs::read_dir(path)?
      .map(|entry| {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();
        let file_type = metadata.file_type();
        let name = builder.create_string(entry.file_name().to_str().unwrap());

        msg::StatRes::create(
          builder,
          &msg::StatResArgs {
            is_file: file_type.is_file(),
            is_symlink: file_type.is_symlink(),
            len: metadata.len(),
            modified: to_seconds!(metadata.modified()),
            accessed: to_seconds!(metadata.accessed()),
            created: to_seconds!(metadata.created()),
            name: Some(name),
            mode: get_mode(&metadata.permissions()),
            has_mode: cfg!(target_family = "unix"),
          },
        )
      }).collect();

    let entries = builder.create_vector(&entries);
    let inner = msg::ReadDirRes::create(
      builder,
      &msg::ReadDirResArgs {
        entries: Some(entries),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ReadDirRes,
        ..Default::default()
      },
    ))
  })
}

fn op_rename(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_rename().unwrap();
  let (oldpath, _) = match resolve_path(inner.oldpath().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let (newpath, newpath_) = match resolve_path(inner.newpath().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };

  if let Err(e) = state.check_write(&newpath_) {
    return odd_future(e);
  }
  blocking(base.sync(), move || -> OpResult {
    debug!("op_rename {} {}", oldpath.display(), newpath.display());
    fs::rename(&oldpath, &newpath)?;
    Ok(empty_buf())
  })
}

fn op_link(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_link().unwrap();
  let (oldname, _) = match resolve_path(inner.oldname().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let (newname, newname_) = match resolve_path(inner.newname().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };

  if let Err(e) = state.check_write(&newname_) {
    return odd_future(e);
  }

  blocking(base.sync(), move || -> OpResult {
    debug!("op_link {} {}", oldname.display(), newname.display());
    std::fs::hard_link(&oldname, &newname)?;
    Ok(empty_buf())
  })
}

fn op_symlink(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_symlink().unwrap();
  let (oldname, _) = match resolve_path(inner.oldname().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let (newname, newname_) = match resolve_path(inner.newname().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };

  if let Err(e) = state.check_write(&newname_) {
    return odd_future(e);
  }
  // TODO Use type for Windows.
  if cfg!(windows) {
    return odd_future(errors::new(
      ErrorKind::Other,
      "Not implemented".to_string(),
    ));
  }
  blocking(base.sync(), move || -> OpResult {
    debug!("op_symlink {} {}", oldname.display(), newname.display());
    #[cfg(any(unix))]
    std::os::unix::fs::symlink(&oldname, &newname)?;
    Ok(empty_buf())
  })
}

fn op_read_link(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_readlink().unwrap();
  let cmd_id = base.cmd_id();
  let (name, name_) = match resolve_path(inner.name().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };

  if let Err(e) = state.check_read(&name_) {
    return odd_future(e);
  }

  blocking(base.sync(), move || -> OpResult {
    debug!("op_read_link {}", name.display());
    let path = fs::read_link(&name)?;
    let builder = &mut FlatBufferBuilder::new();
    let path_off = builder.create_string(path.to_str().unwrap());
    let inner = msg::ReadlinkRes::create(
      builder,
      &msg::ReadlinkResArgs {
        path: Some(path_off),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ReadlinkRes,
        ..Default::default()
      },
    ))
  })
}

fn op_repl_start(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_repl_start().unwrap();
  let cmd_id = base.cmd_id();
  let history_file = String::from(inner.history_file().unwrap());

  debug!("op_repl_start {}", history_file);
  let history_path = repl::history_path(&state.dir, &history_file);
  let repl = repl::Repl::new(history_path);
  let resource = resources::add_repl(repl);

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::ReplStartRes::create(
    builder,
    &msg::ReplStartResArgs { rid: resource.rid },
  );
  ok_future(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::ReplStartRes,
      ..Default::default()
    },
  ))
}

fn op_repl_readline(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let inner = base.inner_as_repl_readline().unwrap();
  let cmd_id = base.cmd_id();
  let rid = inner.rid();
  let prompt = inner.prompt().unwrap().to_owned();
  debug!("op_repl_readline {} {}", rid, prompt);

  blocking(base.sync(), move || -> OpResult {
    let repl = resources::get_repl(rid)?;
    let line = repl.lock().unwrap().readline(&prompt)?;

    let builder = &mut FlatBufferBuilder::new();
    let line_off = builder.create_string(&line);
    let inner = msg::ReplReadlineRes::create(
      builder,
      &msg::ReplReadlineResArgs {
        line: Some(line_off),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ReplReadlineRes,
        ..Default::default()
      },
    ))
  })
}

fn op_truncate(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());

  let inner = base.inner_as_truncate().unwrap();
  let (filename, filename_) = match resolve_path(inner.name().unwrap()) {
    Err(err) => return odd_future(err),
    Ok(v) => v,
  };
  let len = inner.len();

  if let Err(e) = state.check_write(&filename_) {
    return odd_future(e);
  }

  blocking(base.sync(), move || {
    debug!("op_truncate {} {}", filename_, len);
    let f = fs::OpenOptions::new().write(true).open(&filename)?;
    f.set_len(u64::from(len))?;
    Ok(empty_buf())
  })
}

fn op_utime(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());

  let inner = base.inner_as_utime().unwrap();
  let filename = String::from(inner.filename().unwrap());
  let atime = inner.atime();
  let mtime = inner.mtime();

  if let Err(e) = state.check_write(&filename) {
    return odd_future(e);
  }

  blocking(base.sync(), move || {
    debug!("op_utimes {} {} {}", filename, atime, mtime);
    utime::set_file_times(filename, atime, mtime)?;
    Ok(empty_buf())
  })
}

fn op_listen(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_listen().unwrap();
  let network = inner.network().unwrap();
  assert_eq!(network, "tcp");
  let address = inner.address().unwrap();

  if let Err(e) = state.check_net(&address) {
    return odd_future(e);
  }

  Box::new(futures::future::result((move || {
    let addr = resolve_addr(address).wait()?;
    let listener = TcpListener::bind(&addr)?;
    let resource = resources::add_tcp_listener(listener);

    let builder = &mut FlatBufferBuilder::new();
    let inner = msg::ListenRes::create(
      builder,
      &msg::ListenResArgs { rid: resource.rid },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ListenRes,
        ..Default::default()
      },
    ))
  })()))
}

fn new_conn(cmd_id: u32, tcp_stream: TcpStream) -> OpResult {
  let tcp_stream_resource = resources::add_tcp_stream(tcp_stream);
  // TODO forward socket_addr to client.

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::NewConn::create(
    builder,
    &msg::NewConnArgs {
      rid: tcp_stream_resource.rid,
      ..Default::default()
    },
  );
  Ok(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::NewConn,
      ..Default::default()
    },
  ))
}

fn op_accept(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_accept().unwrap();
  let server_rid = inner.rid();

  match resources::lookup(server_rid) {
    None => odd_future(errors::bad_resource()),
    Some(server_resource) => {
      let op = tokio_util::accept(server_resource)
        .map_err(DenoError::from)
        .and_then(move |(tcp_stream, _socket_addr)| {
          new_conn(cmd_id, tcp_stream)
        });
      Box::new(op)
    }
  }
}

fn op_dial(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_dial().unwrap();
  let network = inner.network().unwrap();
  assert_eq!(network, "tcp"); // TODO Support others.
  let address = inner.address().unwrap();

  if let Err(e) = state.check_net(&address) {
    return odd_future(e);
  }

  let op =
    resolve_addr(address)
      .map_err(DenoError::from)
      .and_then(move |addr| {
        TcpStream::connect(&addr)
          .map_err(DenoError::from)
          .and_then(move |tcp_stream| new_conn(cmd_id, tcp_stream))
      });
  Box::new(op)
}

fn op_metrics(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::MetricsRes::create(
    builder,
    &msg::MetricsResArgs::from(&state.metrics),
  );
  ok_future(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::MetricsRes,
      ..Default::default()
    },
  ))
}

fn op_resources(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  let builder = &mut FlatBufferBuilder::new();
  let serialized_resources = table_entries();

  let res: Vec<_> = serialized_resources
    .iter()
    .map(|(key, value)| {
      let repr = builder.create_string(value);

      msg::Resource::create(
        builder,
        &msg::ResourceArgs {
          rid: *key,
          repr: Some(repr),
        },
      )
    }).collect();

  let resources = builder.create_vector(&res);
  let inner = msg::ResourcesRes::create(
    builder,
    &msg::ResourcesResArgs {
      resources: Some(resources),
    },
  );

  ok_future(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::ResourcesRes,
      ..Default::default()
    },
  ))
}

fn subprocess_stdio_map(v: msg::ProcessStdio) -> std::process::Stdio {
  match v {
    msg::ProcessStdio::Inherit => std::process::Stdio::inherit(),
    msg::ProcessStdio::Piped => std::process::Stdio::piped(),
    msg::ProcessStdio::Null => std::process::Stdio::null(),
  }
}

fn op_run(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(base.sync());
  let cmd_id = base.cmd_id();

  if let Err(e) = state.check_run() {
    return odd_future(e);
  }

  assert!(data.is_none());
  let inner = base.inner_as_run().unwrap();
  let args = inner.args().unwrap();
  let env = inner.env().unwrap();
  let cwd = inner.cwd();

  let mut c = Command::new(args.get(0));
  (1..args.len()).for_each(|i| {
    let arg = args.get(i);
    c.arg(arg);
  });
  cwd.map(|d| c.current_dir(d));
  (0..env.len()).for_each(|i| {
    let entry = env.get(i);
    c.env(entry.key().unwrap(), entry.value().unwrap());
  });

  c.stdin(subprocess_stdio_map(inner.stdin()));
  c.stdout(subprocess_stdio_map(inner.stdout()));
  c.stderr(subprocess_stdio_map(inner.stderr()));

  // Spawn the command.
  let child = match c.spawn_async() {
    Ok(v) => v,
    Err(err) => {
      return odd_future(err.into());
    }
  };

  let pid = child.id();
  let resources = resources::add_child(child);

  let mut res_args = msg::RunResArgs {
    rid: resources.child_rid,
    pid,
    ..Default::default()
  };

  if let Some(stdin_rid) = resources.stdin_rid {
    res_args.stdin_rid = stdin_rid;
  }
  if let Some(stdout_rid) = resources.stdout_rid {
    res_args.stdout_rid = stdout_rid;
  }
  if let Some(stderr_rid) = resources.stderr_rid {
    res_args.stderr_rid = stderr_rid;
  }

  let builder = &mut FlatBufferBuilder::new();
  let inner = msg::RunRes::create(builder, &res_args);
  ok_future(serialize_response(
    cmd_id,
    builder,
    msg::BaseArgs {
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::RunRes,
      ..Default::default()
    },
  ))
}

fn op_run_status(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_run_status().unwrap();
  let rid = inner.rid();

  if let Err(e) = state.check_run() {
    return odd_future(e);
  }

  let future = match resources::child_status(rid) {
    Err(e) => {
      return odd_future(e);
    }
    Ok(f) => f,
  };

  let future = future.and_then(move |run_status| {
    let code = run_status.code();

    #[cfg(unix)]
    let signal = run_status.signal();
    #[cfg(not(unix))]
    let signal = None;

    code
      .or(signal)
      .expect("Should have either an exit code or a signal.");
    let got_signal = signal.is_some();

    let builder = &mut FlatBufferBuilder::new();
    let inner = msg::RunStatusRes::create(
      builder,
      &msg::RunStatusResArgs {
        got_signal,
        exit_code: code.unwrap_or(-1),
        exit_signal: signal.unwrap_or(-1),
      },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::RunStatusRes,
        ..Default::default()
      },
    ))
  });
  Box::new(future)
}

struct GetMessageFuture {
  pub state: ThreadSafeState,
}

impl Future for GetMessageFuture {
  type Item = Option<Buf>;
  type Error = ();

  fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
    let mut wc = self.state.worker_channels.lock().unwrap();
    wc.1
      .poll()
      .map_err(|err| panic!("worker_channel recv err {:?}", err))
  }
}

/// Get message from host as guest worker
fn op_worker_get_message(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();

  let op = GetMessageFuture {
    state: state.clone(),
  };
  let op = op.map_err(move |_| -> DenoError { unimplemented!() });
  let op = op.and_then(move |maybe_buf| -> DenoResult<Buf> {
    debug!("op_worker_get_message");
    let builder = &mut FlatBufferBuilder::new();

    let data = maybe_buf.as_ref().map(|buf| builder.create_vector(buf));
    let inner = msg::WorkerGetMessageRes::create(
      builder,
      &msg::WorkerGetMessageResArgs { data },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::WorkerGetMessageRes,
        ..Default::default()
      },
    ))
  });
  Box::new(op)
}

/// Post message to host as guest worker
fn op_worker_post_message(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  let cmd_id = base.cmd_id();

  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  let tx = {
    let wc = state.worker_channels.lock().unwrap();
    wc.0.clone()
  };
  let op = tx.send(d);
  let op = op.map_err(|e| errors::new(ErrorKind::Other, e.to_string()));
  let op = op.and_then(move |_| -> DenoResult<Buf> {
    let builder = &mut FlatBufferBuilder::new();

    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        ..Default::default()
      },
    ))
  });
  Box::new(op)
}

/// Create worker as the host
fn op_create_worker(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_create_worker().unwrap();
  let specifier = inner.specifier().unwrap();

  let parent_state = state.clone();

  let child_state = ThreadSafeState::new(
    parent_state.flags.clone(),
    parent_state.argv.clone(),
    op_selector_std,
    parent_state.progress.clone(),
  );
  let rid = child_state.resource.rid;
  let name = format!("USER-WORKER-{}", specifier);

  let mut worker =
    Worker::new(name, startup_data::deno_isolate_init(), child_state);
  js_check(worker.execute("denoMain()"));
  js_check(worker.execute("workerMain()"));

  let op = root_specifier_to_url(specifier)
    .and_then(|specifier_url| {
      Ok(
        worker
          .execute_mod_async(&specifier_url, false)
          .and_then(move |worker| {
            let mut workers_tl = parent_state.workers.lock().unwrap();
            workers_tl.insert(rid, worker.shared());
            let builder = &mut FlatBufferBuilder::new();
            let msg_inner = msg::CreateWorkerRes::create(
              builder,
              &msg::CreateWorkerResArgs { rid },
            );
            Ok(serialize_response(
              cmd_id,
              builder,
              msg::BaseArgs {
                inner: Some(msg_inner.as_union_value()),
                inner_type: msg::Any::CreateWorkerRes,
                ..Default::default()
              },
            ))
          }).map_err(|err| match err {
            (errors::RustOrJsError::Js(_), _worker) => {
              errors::worker_init_failed()
            }
            (errors::RustOrJsError::Rust(err), _worker) => err,
          }),
      )
    }).map_err(DenoError::from);

  Box::new(match op {
    Ok(op) => future::Either::A(op),
    Err(err) => future::Either::B(future::result(Err(err))),
  })
}

/// Return when the worker closes
fn op_host_get_worker_closed(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_host_get_worker_closed().unwrap();
  let rid = inner.rid();
  let state = state.clone();

  let shared_worker_future = {
    let workers_tl = state.workers.lock().unwrap();
    let worker = workers_tl.get(&rid).unwrap();
    worker.clone()
  };

  Box::new(shared_worker_future.then(move |_result| {
    let builder = &mut FlatBufferBuilder::new();

    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        ..Default::default()
      },
    ))
  }))
}

/// Get message from guest worker as host
fn op_host_get_message(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_host_get_message().unwrap();
  let rid = inner.rid();

  let op = resources::get_message_from_worker(rid);
  let op = op.map_err(move |_| -> DenoError { unimplemented!() });
  let op = op.and_then(move |maybe_buf| -> DenoResult<Buf> {
    let builder = &mut FlatBufferBuilder::new();

    let data = maybe_buf.as_ref().map(|buf| builder.create_vector(buf));
    let msg_inner = msg::HostGetMessageRes::create(
      builder,
      &msg::HostGetMessageResArgs { data },
    );
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(msg_inner.as_union_value()),
        inner_type: msg::Any::HostGetMessageRes,
        ..Default::default()
      },
    ))
  });
  Box::new(op)
}

/// Post message to guest worker as host
fn op_host_post_message(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_host_post_message().unwrap();
  let rid = inner.rid();

  let d = Vec::from(data.unwrap().as_ref()).into_boxed_slice();

  let op = resources::post_message_to_worker(rid, d);
  let op = op.map_err(|e| errors::new(ErrorKind::Other, e.to_string()));
  let op = op.and_then(move |_| -> DenoResult<Buf> {
    let builder = &mut FlatBufferBuilder::new();

    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        ..Default::default()
      },
    ))
  });
  Box::new(op)
}

fn op_get_random_values(
  _state: &ThreadSafeState,
  _base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> Box<OpWithError> {
  thread_rng().fill(&mut data.unwrap()[..]);
  Box::new(ok_future(empty_buf()))
}
