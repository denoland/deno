// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::StreamResource;
use crate::deno_error::bad_resource;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::fs as deno_fs;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;
use futures::future::FutureExt;
use futures::future::TryFutureExt;
use std;
use std::convert::From;
use std::future::Future;
use std::io::SeekFrom;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio;

pub fn init(i: &mut Isolate, s: &ThreadSafeState) {
  i.register_op("open", s.core_op(json_op(s.stateful_op(op_open))));
  i.register_op("close", s.core_op(json_op(s.stateful_op(op_close))));
  i.register_op("seek", s.core_op(json_op(s.stateful_op(op_seek))));
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenArgs {
  promise_id: Option<u64>,
  filename: String,
  mode: String,
}

fn op_open(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: OpenArgs = serde_json::from_value(args)?;
  let (filename, filename_) = deno_fs::resolve_from_cwd(&args.filename)?;
  let mode = args.mode.as_ref();
  let state_ = state.clone();
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
      state.check_read(&filename_)?;
    }
    "w" | "a" | "x" => {
      state.check_write(&filename_)?;
    }
    &_ => {
      state.check_read(&filename_)?;
      state.check_write(&filename_)?;
    }
  }

  let is_sync = args.promise_id.is_none();
  let op = futures::compat::Compat01As03::new(tokio::prelude::Future::map_err(
    open_options.open(filename),
    ErrBox::from,
  ))
  .and_then(move |fs_file| {
    let mut table = state_.lock_resource_table();
    let rid = table.add("fsFile", Box::new(StreamResource::FsFile(fs_file)));
    futures::future::ok(json!(rid))
  });

  if is_sync {
    let buf = futures::executor::block_on(op)?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(op.boxed()))
  }
}

#[derive(Deserialize)]
struct CloseArgs {
  rid: i32,
}

fn op_close(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CloseArgs = serde_json::from_value(args)?;

  let mut table = state.lock_resource_table();
  table.close(args.rid as u32).ok_or_else(bad_resource)?;
  Ok(JsonOp::Sync(json!({})))
}

pub struct SeekFuture {
  seek_from: SeekFrom,
  rid: ResourceId,
  state: ThreadSafeState,
}

impl Future for SeekFuture {
  type Output = Result<u64, ErrBox>;

  fn poll(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Self::Output> {
    let inner = self.get_mut();
    let mut table = inner.state.lock_resource_table();
    let resource = table
      .get_mut::<StreamResource>(inner.rid)
      .ok_or_else(bad_resource)?;

    let tokio_file = match resource {
      StreamResource::FsFile(ref mut file) => file,
      _ => return Poll::Ready(Err(bad_resource())),
    };

    use tokio::prelude::Async::*;

    match tokio_file.poll_seek(inner.seek_from).map_err(ErrBox::from) {
      Ok(Ready(v)) => Poll::Ready(Ok(v)),
      Err(err) => Poll::Ready(Err(err)),
      Ok(NotReady) => Poll::Pending,
    }
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeekArgs {
  promise_id: Option<u64>,
  rid: i32,
  offset: i32,
  whence: i32,
}

fn op_seek(
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SeekArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let offset = args.offset;
  let whence = args.whence as u32;
  // Translate seek mode to Rust repr.
  let seek_from = match whence {
    0 => SeekFrom::Start(offset as u64),
    1 => SeekFrom::Current(i64::from(offset)),
    2 => SeekFrom::End(i64::from(offset)),
    _ => {
      return Err(ErrBox::from(DenoError::new(
        ErrorKind::InvalidSeekMode,
        format!("Invalid seek mode: {}", whence),
      )));
    }
  };

  let fut = SeekFuture {
    state: state.clone(),
    seek_from,
    rid,
  };

  let op = fut.and_then(move |_| futures::future::ok(json!({})));
  if args.promise_id.is_none() {
    let buf = futures::executor::block_on(op)?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(op.boxed()))
  }
}
