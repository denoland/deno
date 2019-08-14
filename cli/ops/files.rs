// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::deno_error;
use crate::fs as deno_fs;
use crate::msg;
use crate::ops::empty_buf;
use crate::ops::ok_buf;
use crate::ops::serialize_response;
use crate::ops::CliOpResult;
use crate::resources;
use crate::state::ThreadSafeState;
use deno::*;
use flatbuffers::FlatBufferBuilder;
use futures::Future;
use std;
use std::convert::From;
use tokio;

pub fn op_open(
  state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let cmd_id = base.cmd_id();
  let inner = base.inner_as_open().unwrap();
  let (filename, filename_) =
    deno_fs::resolve_from_cwd(inner.filename().unwrap())?;
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

  let op = open_options.open(filename).map_err(ErrBox::from).and_then(
    move |fs_file| {
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
    },
  );
  if base.sync() {
    let buf = op.wait()?;
    Ok(Op::Sync(buf))
  } else {
    Ok(Op::Async(Box::new(op)))
  }
}

pub fn op_close(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_close().unwrap();
  let rid = inner.rid();
  match resources::lookup(rid) {
    None => Err(deno_error::bad_resource()),
    Some(resource) => {
      resource.close();
      ok_buf(empty_buf())
    }
  }
}

pub fn op_seek(
  _state: &ThreadSafeState,
  base: &msg::Base<'_>,
  data: Option<PinnedBuf>,
) -> CliOpResult {
  assert!(data.is_none());
  let inner = base.inner_as_seek().unwrap();
  let rid = inner.rid();
  let offset = inner.offset();
  let whence = inner.whence();

  match resources::lookup(rid) {
    None => Err(deno_error::bad_resource()),
    Some(resource) => {
      let op = resources::seek(resource, offset, whence)
        .and_then(move |_| Ok(empty_buf()));
      if base.sync() {
        let buf = op.wait()?;
        Ok(Op::Sync(buf))
      } else {
        Ok(Op::Async(Box::new(op)))
      }
    }
  }
}
