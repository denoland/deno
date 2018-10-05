// Copyright 2018 the Deno authors. All rights reserved. MIT license.

use errors;
use errors::permission_denied;
use errors::DenoError;
use errors::DenoResult;
use fs as deno_fs;
use isolate::Buf;
use isolate::Isolate;
use isolate::IsolateState;
use isolate::Op;
use msg;
use resources;
use resources::Resource;
use tokio_util;

use flatbuffers::FlatBufferBuilder;
use futures;
use futures::future::poll_fn;
use futures::Poll;
use hyper;
use hyper::rt::{Future, Stream};
use hyper::Client;
use remove_dir_all::remove_dir_all;
use std;
use std::fs;
use std::net::{Shutdown, SocketAddr};
#[cfg(any(unix))]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use std::time::{Duration, Instant};
use tokio;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_io;
use tokio_threadpool;

type OpResult = DenoResult<Buf>;

// TODO Ideally we wouldn't have to box the Op being returned.
// The box is just to make it easier to get a prototype refactor working.
type OpCreator =
  fn(state: Arc<IsolateState>, base: &msg::Base, data: &'static mut [u8])
    -> Box<Op>;

// Hopefully Rust optimizes this away.
fn empty_buf() -> Buf {
  Box::new([])
}

/// Processes raw messages from JavaScript.
/// This functions invoked every time libdeno.send() is called.
/// control corresponds to the first argument of libdeno.send().
/// data corresponds to the second argument of libdeno.send().
pub fn dispatch(
  isolate: &mut Isolate,
  control: &[u8],
  data: &'static mut [u8],
) -> (bool, Box<Op>) {
  let base = msg::get_root_as_base(control);
  let is_sync = base.sync();
  let inner_type = base.inner_type();
  let cmd_id = base.cmd_id();

  let op: Box<Op> = if inner_type == msg::Any::SetTimeout {
    // SetTimeout is an exceptional op: the global timeout field is part of the
    // Isolate state (not the IsolateState state) and it must be updated on the
    // main thread.
    assert_eq!(is_sync, true);
    op_set_timeout(isolate, &base, data)
  } else {
    // Handle regular ops.
    let op_creator: OpCreator = match inner_type {
      msg::Any::Start => op_start,
      msg::Any::CodeFetch => op_code_fetch,
      msg::Any::CodeCache => op_code_cache,
      msg::Any::Environ => op_env,
      msg::Any::FetchReq => op_fetch_req,
      msg::Any::MakeTempDir => op_make_temp_dir,
      msg::Any::Mkdir => op_mkdir,
      msg::Any::Open => op_open,
      msg::Any::Read => op_read,
      msg::Any::Write => op_write,
      msg::Any::Close => op_close,
      msg::Any::Shutdown => op_shutdown,
      msg::Any::Remove => op_remove,
      msg::Any::ReadFile => op_read_file,
      msg::Any::ReadDir => op_read_dir,
      msg::Any::Rename => op_rename,
      msg::Any::Readlink => op_read_link,
      msg::Any::Symlink => op_symlink,
      msg::Any::SetEnv => op_set_env,
      msg::Any::Stat => op_stat,
      msg::Any::Truncate => op_truncate,
      msg::Any::WriteFile => op_write_file,
      msg::Any::Exit => op_exit,
      msg::Any::CopyFile => op_copy_file,
      msg::Any::Listen => op_listen,
      msg::Any::Accept => op_accept,
      msg::Any::Dial => op_dial,
      _ => panic!(format!(
        "Unhandled message {}",
        msg::enum_name_any(inner_type)
      )),
    };
    op_creator(isolate.state.clone(), &base, data)
  };

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
    msg::enum_name_any(inner_type),
    base.sync()
  );
  return (base.sync(), boxed_op);
}

fn op_exit(
  _config: Arc<IsolateState>,
  base: &msg::Base,
  _data: &'static mut [u8],
) -> Box<Op> {
  let inner = base.inner_as_exit().unwrap();
  std::process::exit(inner.code())
}

fn op_start(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let mut builder = FlatBufferBuilder::new();

  let argv = state.argv.iter().map(|s| s.as_str()).collect::<Vec<_>>();
  let argv_off = builder.create_vector_of_strings(argv.as_slice());

  let cwd_path = std::env::current_dir().unwrap();
  let cwd_off =
    builder.create_string(deno_fs::normalize_path(cwd_path.as_ref()).as_ref());

  let inner = msg::StartRes::create(
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
      inner_type: msg::Any::StartRes,
      inner: Some(inner.as_union_value()),
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
fn op_code_fetch(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_code_fetch().unwrap();
  let cmd_id = base.cmd_id();
  let module_specifier = inner.module_specifier().unwrap();
  let containing_file = inner.containing_file().unwrap();

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
    let inner = msg::CodeFetchRes::create(builder, &msg_args);
    Ok(serialize_response(
      cmd_id,
      builder,
      msg::BaseArgs {
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::CodeFetchRes,
        ..Default::default()
      },
    ))
  }()))
}

// https://github.com/denoland/isolate/blob/golang/os.go#L156-L169
fn op_code_cache(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_code_cache().unwrap();
  let filename = inner.filename().unwrap();
  let source_code = inner.source_code().unwrap();
  let output_code = inner.output_code().unwrap();
  Box::new(futures::future::result(|| -> OpResult {
    state.dir.code_cache(filename, source_code, output_code)?;
    Ok(empty_buf())
  }()))
}

fn op_set_timeout(
  isolate: &mut Isolate,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_set_timeout().unwrap();
  let val = inner.timeout() as i64;
  isolate.timeout_due = if val >= 0 {
    Some(Instant::now() + Duration::from_millis(val as u64))
  } else {
    None
  };
  ok_future(empty_buf())
}

fn op_set_env(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_set_env().unwrap();
  let key = inner.key().unwrap();
  let value = inner.value().unwrap();

  if !state.flags.allow_env {
    return odd_future(permission_denied());
  }

  std::env::set_var(key, value);
  ok_future(empty_buf())
}

fn op_env(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
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
  let inner = msg::EnvironRes::create(
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
      inner: Some(inner.as_union_value()),
      inner_type: msg::Any::EnvironRes,
      ..Default::default()
    },
  ))
}

fn op_fetch_req(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_fetch_req().unwrap();
  let cmd_id = base.cmd_id();
  let id = inner.id();
  let url = inner.url().unwrap();

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

      let inner = msg::FetchRes::create(
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
          inner: Some(inner.as_union_value()),
          inner_type: msg::Any::FetchRes,
          ..Default::default()
        },
      ))
    },
  );
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
    Err(_) => panic!("blocking error"),
  }
}

// TODO Do not use macro for the blocking function.. We should instead be able
// to do this with a normal function, but there seems to some type system
// issues. The type of this function should be something like this:
//   fn blocking<F>(is_sync: bool, f: F) -> Box<Op>
//   where F: FnOnce() -> DenoResult<Buf>
macro_rules! blocking {
  ($is_sync:expr,$fn:expr) => {
    if $is_sync {
      // If synchronous, execute the function immediately on the main thread.
      Box::new(futures::future::result($fn()))
    } else {
      // Otherwise dispatch to thread pool.
      Box::new(poll_fn(move || convert_blocking($fn)))
    }
  };
}

fn op_make_temp_dir(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let base = Box::new(*base);
  let inner = base.inner_as_make_temp_dir().unwrap();
  let cmd_id = base.cmd_id();

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }

  let dir = inner.dir().map(PathBuf::from);
  let prefix = inner.prefix().map(String::from);
  let suffix = inner.suffix().map(String::from);

  blocking!(base.sync(), || -> OpResult {
    // TODO(piscisaureus): use byte vector for paths, not a string.
    // See https://github.com/denoland/isolate/issues/627.
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
        ..Default::default()
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
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_mkdir().unwrap();
  let mode = inner.mode();
  let path = String::from(inner.path().unwrap());

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }

  blocking!(base.sync(), || {
    debug!("op_mkdir {}", path);
    deno_fs::mkdir(Path::new(&path), mode)?;
    Ok(empty_buf())
  })
}

fn op_open(
  _state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_open().unwrap();
  let filename = PathBuf::from(inner.filename().unwrap());
  // TODO let perm = inner.perm();

  let op = tokio::fs::File::open(filename)
    .map_err(|err| DenoError::from(err))
    .and_then(move |fs_file| -> OpResult {
      let resource = resources::add_fs_file(fs_file);
      let builder = &mut FlatBufferBuilder::new();
      let inner = msg::OpenRes::create(
        builder,
        &msg::OpenResArgs {
          rid: resource.rid,
          ..Default::default()
        },
      );
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
  _state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_close().unwrap();
  let rid = inner.rid();
  match resources::lookup(rid) {
    None => odd_future(errors::bad_resource()),
    Some(mut resource) => {
      resource.close();
      ok_future(empty_buf())
    }
  }
}

fn op_shutdown(
  _state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_shutdown().unwrap();
  let rid = inner.rid();
  let how = inner.how();
  match resources::lookup(rid) {
    None => odd_future(errors::new(
      errors::ErrorKind::BadFileDescriptor,
      String::from("Bad File Descriptor"),
    )),
    Some(mut resource) => {
      let shutdown_mode = match how {
        0 => Shutdown::Read,
        1 => Shutdown::Write,
        _ => unimplemented!(),
      };
      blocking!(base.sync(), || {
        // Use UFCS for disambiguation
        Resource::shutdown(&mut resource, shutdown_mode)?;
        Ok(empty_buf())
      })
    }
  }
}

fn op_read(
  _state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_read().unwrap();
  let rid = inner.rid();

  match resources::lookup(rid) {
    None => odd_future(errors::bad_resource()),
    Some(resource) => {
      let op = tokio_io::io::read(resource, data)
        .map_err(|err| DenoError::from(err))
        .and_then(move |(_resource, _buf, nread)| {
          let builder = &mut FlatBufferBuilder::new();
          let inner = msg::ReadRes::create(
            builder,
            &msg::ReadResArgs {
              nread: nread as u32,
              eof: nread == 0,
              ..Default::default()
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
  _state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_write().unwrap();
  let rid = inner.rid();

  match resources::lookup(rid) {
    None => odd_future(errors::bad_resource()),
    Some(resource) => {
      let len = data.len();
      let op = tokio_io::io::write_all(resource, data)
        .map_err(|err| DenoError::from(err))
        .and_then(move |(_resource, _buf)| {
          let builder = &mut FlatBufferBuilder::new();
          let inner = msg::WriteRes::create(
            builder,
            &msg::WriteResArgs {
              nbyte: len as u32,
              ..Default::default()
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

fn op_remove(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_remove().unwrap();
  let path = PathBuf::from(inner.path().unwrap());
  let recursive = inner.recursive();
  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }
  blocking!(base.sync(), || {
    debug!("op_remove {}", path.display());
    let metadata = fs::metadata(&path)?;
    if metadata.is_file() {
      fs::remove_file(&path)?;
    } else {
      if recursive {
        remove_dir_all(&path)?;
      } else {
        fs::remove_dir(&path)?;
      }
    }
    Ok(empty_buf())
  })
}

// Prototype https://github.com/denoland/isolate/blob/golang/os.go#L171-L184
fn op_read_file(
  _config: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_read_file().unwrap();
  let cmd_id = base.cmd_id();
  let filename = PathBuf::from(inner.filename().unwrap());
  debug!("op_read_file {}", filename.display());
  blocking!(base.sync(), || {
    let vec = fs::read(&filename)?;
    // Build the response message. memcpy data into inner.
    // TODO(ry) zero-copy.
    let builder = &mut FlatBufferBuilder::new();
    let data_off = builder.create_vector(vec.as_slice());
    let inner = msg::ReadFileRes::create(
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
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ReadFileRes,
        ..Default::default()
      },
    ))
  })
}

fn op_copy_file(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_copy_file().unwrap();
  let from = PathBuf::from(inner.from().unwrap());
  let to = PathBuf::from(inner.to().unwrap());

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }

  debug!("op_copy_file {} {}", from.display(), to.display());
  blocking!(base.sync(), || {
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
fn get_mode(perm: fs::Permissions) -> u32 {
  perm.mode()
}

#[cfg(not(any(unix)))]
fn get_mode(_perm: fs::Permissions) -> u32 {
  0
}

fn op_stat(
  _config: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_stat().unwrap();
  let cmd_id = base.cmd_id();
  let filename = PathBuf::from(inner.filename().unwrap());
  let lstat = inner.lstat();

  blocking!(base.sync(), || {
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
        mode: get_mode(metadata.permissions()),
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
  _state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_read_dir().unwrap();
  let cmd_id = base.cmd_id();
  let path = String::from(inner.path().unwrap());

  blocking!(base.sync(), || -> OpResult {
    debug!("op_read_dir {}", path);
    let builder = &mut FlatBufferBuilder::new();
    let entries: Vec<_> = fs::read_dir(Path::new(&path))?
      .map(|entry| {
        let entry = entry.unwrap();
        let metadata = entry.metadata().unwrap();
        let file_type = metadata.file_type();
        let name = builder.create_string(entry.file_name().to_str().unwrap());
        let path = builder.create_string(entry.path().to_str().unwrap());

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
            path: Some(path),
            ..Default::default()
          },
        )
      }).collect();

    let entries = builder.create_vector(&entries);
    let inner = msg::ReadDirRes::create(
      builder,
      &msg::ReadDirResArgs {
        entries: Some(entries),
        ..Default::default()
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

fn op_write_file(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  let inner = base.inner_as_write_file().unwrap();

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }

  let filename = String::from(inner.filename().unwrap());
  let perm = inner.perm();

  blocking!(base.sync(), || -> OpResult {
    debug!("op_write_file {} {}", filename, data.len());
    deno_fs::write_file(Path::new(&filename), data, perm)?;
    Ok(empty_buf())
  })
}

fn op_rename(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }
  let inner = base.inner_as_rename().unwrap();
  let oldpath = PathBuf::from(inner.oldpath().unwrap());
  let newpath = PathBuf::from(inner.newpath().unwrap());
  blocking!(base.sync(), || -> OpResult {
    debug!("op_rename {} {}", oldpath.display(), newpath.display());
    fs::rename(&oldpath, &newpath)?;
    Ok(empty_buf())
  })
}

fn op_symlink(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }
  // TODO Use type for Windows.
  if cfg!(windows) {
    panic!("symlink for windows is not yet implemented")
  }

  let inner = base.inner_as_symlink().unwrap();
  let oldname = PathBuf::from(inner.oldname().unwrap());
  let newname = PathBuf::from(inner.newname().unwrap());
  blocking!(base.sync(), || -> OpResult {
    debug!("op_symlink {} {}", oldname.display(), newname.display());
    #[cfg(any(unix))]
    std::os::unix::fs::symlink(&oldname, &newname)?;
    Ok(empty_buf())
  })
}

fn op_read_link(
  _state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  let inner = base.inner_as_readlink().unwrap();
  let cmd_id = base.cmd_id();
  let name = PathBuf::from(inner.name().unwrap());

  blocking!(base.sync(), || -> OpResult {
    debug!("op_read_link {}", name.display());
    let path = fs::read_link(&name)?;
    let builder = &mut FlatBufferBuilder::new();
    let path_off = builder.create_string(path.to_str().unwrap());
    let inner = msg::ReadlinkRes::create(
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
        inner: Some(inner.as_union_value()),
        inner_type: msg::Any::ReadlinkRes,
        ..Default::default()
      },
    ))
  })
}

fn op_truncate(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);

  if !state.flags.allow_write {
    return odd_future(permission_denied());
  }

  let inner = base.inner_as_truncate().unwrap();
  let filename = String::from(inner.name().unwrap());
  let len = inner.len();
  blocking!(base.sync(), || {
    debug!("op_truncate {} {}", filename, len);
    let f = fs::OpenOptions::new().write(true).open(&filename)?;
    f.set_len(len as u64)?;
    Ok(empty_buf())
  })
}

fn op_listen(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  if !state.flags.allow_net {
    return odd_future(permission_denied());
  }

  let cmd_id = base.cmd_id();
  let inner = base.inner_as_listen().unwrap();
  let network = inner.network().unwrap();
  assert_eq!(network, "tcp");
  let address = inner.address().unwrap();

  Box::new(futures::future::result((move || {
    // TODO properly parse addr
    let addr = SocketAddr::from_str(address).unwrap();

    let listener = TcpListener::bind(&addr)?;
    let resource = resources::add_tcp_listener(listener);

    let builder = &mut FlatBufferBuilder::new();
    let inner = msg::ListenRes::create(
      builder,
      &msg::ListenResArgs {
        rid: resource.rid,
        ..Default::default()
      },
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
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  if !state.flags.allow_net {
    return odd_future(permission_denied());
  }

  let cmd_id = base.cmd_id();
  let inner = base.inner_as_accept().unwrap();
  let server_rid = inner.rid();

  match resources::lookup(server_rid) {
    None => odd_future(errors::bad_resource()),
    Some(server_resource) => {
      let op = tokio_util::accept(server_resource)
        .map_err(|err| DenoError::from(err))
        .and_then(move |(tcp_stream, _socket_addr)| {
          new_conn(cmd_id, tcp_stream)
        });
      Box::new(op)
    }
  }
}

fn op_dial(
  state: Arc<IsolateState>,
  base: &msg::Base,
  data: &'static mut [u8],
) -> Box<Op> {
  assert_eq!(data.len(), 0);
  if !state.flags.allow_net {
    return odd_future(permission_denied());
  }

  let cmd_id = base.cmd_id();
  let inner = base.inner_as_dial().unwrap();
  let network = inner.network().unwrap();
  assert_eq!(network, "tcp");
  let address = inner.address().unwrap();

  // TODO properly parse addr
  let addr = SocketAddr::from_str(address).unwrap();

  let op = TcpStream::connect(&addr)
    .map_err(|err| err.into())
    .and_then(move |tcp_stream| new_conn(cmd_id, tcp_stream));
  Box::new(op)
}
