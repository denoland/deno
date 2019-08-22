// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_flatbuffers::serialize_response;
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::utils::*;
use crate::deno_error;
use crate::fs as deno_fs;
use crate::msg;
use crate::resources;
use crate::state::ThreadSafeState;
use crate::tokio_write;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use futures::Future;
use std;
use std::convert::From;
use tokio;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenArgs {
  promise_id: Option<u64>,
  filename: String,
  mode: String,
}

pub fn op_open(
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
      let resource = resources::add_fs_file(fs_file);
      futures::future::ok(json!(resource.rid))
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

pub fn op_close(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: CloseArgs = serde_json::from_value(args)?;

  match resources::lookup(args.rid as u32) {
    None => Err(deno_error::bad_resource()),
    Some(resource) => {
      resource.close();
      Ok(JsonOp::Sync(json!({})))
    }
  }
}

pub fn op_read(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_read().unwrap();
  let rid = inner.rid();

  match resources::lookup(rid) {
    None => Err(deno_error::bad_resource()),
    Some(resource) => {
      let op = tokio::io::read(resource, data.unwrap())
        .map_err(ErrBox::from)
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
      if base.sync() {
        let buf = op.wait()?;
        Ok(Op::Sync(buf))
      } else {
        Ok(Op::Async(Box::new(op)))
      }
    }
  }
}

pub fn op_write(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_write().unwrap();
  let rid = inner.rid();

  match resources::lookup(rid) {
    None => Err(deno_error::bad_resource()),
    Some(resource) => {
      let op = tokio_write::write(resource, data.unwrap())
        .map_err(ErrBox::from)
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
      if base.sync() {
        let buf = op.wait()?;
        Ok(Op::Sync(buf))
      } else {
        Ok(Op::Async(Box::new(op)))
      }
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

pub fn op_seek(
  _state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: SeekArgs = serde_json::from_value(args)?;

  match resources::lookup(args.rid as u32) {
    None => Err(deno_error::bad_resource()),
    Some(resource) => {
      let op = resources::seek(resource, args.offset, args.whence as u32)
        .and_then(move |_| futures::future::ok(json!({})));
      if args.promise_id.is_none() {
        let buf = op.wait()?;
        Ok(JsonOp::Sync(buf))
      } else {
        Ok(JsonOp::Async(Box::new(op)))
      }
    }
  }
}
