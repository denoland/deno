// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use super::io::{FileMetadata, StreamResource};
use crate::fs as deno_fs;
use crate::op_error::OpError;
use crate::state::State;
use deno_core::*;
use futures::future::FutureExt;
use std;
use std::convert::From;
use std::convert::TryInto;
use std::fs;
use std::io::SeekFrom;
use std::path::Path;
use tokio::fs as tokio_fs;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op("op_open", s.stateful_json_op(op_open));
  i.register_op("op_close", s.stateful_json_op(op_close));
  i.register_op("op_seek", s.stateful_json_op(op_seek));
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenArgs {
  promise_id: Option<u64>,
  path: String,
  options: Option<OpenOptions>,
  mode: Option<String>,
  perm: Option<u32>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
struct OpenOptions {
  read: bool,
  write: bool,
  create: bool,
  truncate: bool,
  append: bool,
  create_new: bool,
}

fn op_open(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: OpenArgs = serde_json::from_value(args)?;
  let path = deno_fs::resolve_from_cwd(Path::new(&args.path))?;
  let state_ = state.clone();
  let gave_perm = args.perm.is_some();

  let mut open_options = if let Some(_perm) = args.perm {
    #[allow(unused_mut)]
    let mut std_options = fs::OpenOptions::new();
    // perm only used if creating the file on Unix
    // if not specified, defaults to 0o666
    #[cfg(unix)]
    std_options.mode(_perm & 0o777);
    tokio_fs::OpenOptions::from(std_options)
  } else {
    tokio_fs::OpenOptions::new()
  };

  if let Some(options) = args.options {
    if options.read {
      state.check_read(&path)?;
    }

    if options.write || options.append {
      state.check_write(&path)?;
    }

    if gave_perm && !(options.create || options.create_new) {
      return Err(OpError::type_error(
        "specified perm without allowing file creation".to_string(),
      ));
    }

    open_options
      .read(options.read)
      .create(options.create)
      .write(options.write)
      .truncate(options.truncate)
      .append(options.append)
      .create_new(options.create_new);
  } else if let Some(mode) = args.mode {
    let mode = mode.as_ref();
    match mode {
      "r" => {
        state.check_read(&path)?;
        if gave_perm {
          return Err(OpError::type_error(
            "specified perm for read-only open".to_string(),
          ));
        }
      }
      "w" | "a" | "x" => {
        state.check_write(&path)?;
      }
      &_ => {
        state.check_read(&path)?;
        state.check_write(&path)?;
      }
    };

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
        // TODO: this should be type error
        return Err(OpError::other("Unknown open mode.".to_string()));
      }
    }
  } else {
    return Err(OpError::other(
      "Open requires either mode or options.".to_string(),
    ));
  };

  let is_sync = args.promise_id.is_none();

  let fut = async move {
    let fs_file = open_options.open(path).await?;
    let mut state = state_.borrow_mut();
    let rid = state.resource_table.add(
      "fsFile",
      Box::new(StreamResource::FsFile(fs_file, FileMetadata::default())),
    );
    Ok(json!(rid))
  };

  if is_sync {
    let buf = futures::executor::block_on(fut)?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}

#[derive(Deserialize)]
struct CloseArgs {
  rid: i32,
}

fn op_close(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: CloseArgs = serde_json::from_value(args)?;

  let mut state = state.borrow_mut();
  state
    .resource_table
    .close(args.rid as u32)
    .ok_or_else(OpError::bad_resource)?;
  Ok(JsonOp::Sync(json!({})))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeekArgs {
  promise_id: Option<u64>,
  rid: i32,
  offset: i64,
  whence: i32,
}

fn op_seek(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: SeekArgs = serde_json::from_value(args)?;
  let rid = args.rid as u32;
  let offset = args.offset;
  let whence = args.whence;
  // Translate seek mode to Rust repr.
  let seek_from = match whence {
    0 => {
      // require offset to be 63 bit unsigned
      let offset: u64 = offset.try_into()?;
      SeekFrom::Start(offset)
    }
    1 => SeekFrom::Current(offset),
    2 => SeekFrom::End(offset),
    _ => {
      return Err(OpError::type_error(format!(
        "Invalid seek mode: {}",
        whence
      )));
    }
  };

  let state = state.borrow();
  let resource = state
    .resource_table
    .get::<StreamResource>(rid)
    .ok_or_else(OpError::bad_resource)?;

  let tokio_file = match resource {
    StreamResource::FsFile(ref file, _) => file,
    _ => return Err(OpError::bad_resource()),
  };
  let mut file = futures::executor::block_on(tokio_file.try_clone())?;

  let fut = async move {
    debug!("op_seek {} {} {}", rid, offset, whence);
    let pos = file.seek(seek_from).await?;
    Ok(json!(pos))
  };

  if args.promise_id.is_none() {
    let buf = futures::executor::block_on(fut)?;
    Ok(JsonOp::Sync(buf))
  } else {
    Ok(JsonOp::Async(fut.boxed_local()))
  }
}
