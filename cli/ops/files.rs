// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::deno_error::DenoError;
use crate::deno_error::ErrorKind;
use crate::fs as deno_fs;
use crate::ops::json_op;
use crate::resources;
use crate::resources::CliResource;
use crate::state::ThreadSafeState;
use deno::*;
use futures::Future;
use futures::Poll;
use std;
use std::convert::From;
use std::io::SeekFrom;
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
  let op = open_options.open(filename).map_err(ErrBox::from).and_then(
    move |fs_file| {
      let rid = resources::add_fs_file(fs_file);
      futures::future::ok(json!(rid))
    },
  );

  if is_sync {
    let buf = op.wait()?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(Box::new(op)))
  }
}

#[derive(Deserialize)]
struct CloseArgs {
  rid: i32,
}

fn op_close(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CloseArgs = serde_json::from_value(args)?;

  let mut table = resources::lock_resource_table();
  table.close(args.rid as u32).ok_or_else(bad_resource)?;
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Debug)]
pub struct SeekFuture {
  seek_from: SeekFrom,
  rid: ResourceId,
}

impl Future for SeekFuture {
  type Item = u64;
  type Error = ErrBox;

  fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
    let mut table = resources::lock_resource_table();
    let resource = table
      .get_mut::<CliResource>(self.rid)
      .ok_or_else(bad_resource)?;

    let tokio_file = match resource {
      CliResource::FsFile(ref mut file) => file,
      _ => return Err(bad_resource()),
    };

    tokio_file.poll_seek(self.seek_from).map_err(ErrBox::from)
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
  _state: &ThreadSafeState,
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

  let fut = SeekFuture { seek_from, rid };

  let op = fut.and_then(move |_| futures::future::ok(json!({})));
  if args.promise_id.is_none() {
    let buf = op.wait()?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(Box::new(op)))
  }
}
