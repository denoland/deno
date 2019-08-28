// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{wrap_json_op, Deserialize, JsonOp};
use crate::deno_error;
use crate::fs as deno_fs;
use crate::resources;
use crate::state::DenoOpDispatcher;
use crate::state::ThreadSafeState;
use deno::*;
use futures::Future;
use std;
use std::convert::From;
use tokio;

// Open

pub struct OpOpen;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenArgs {
  promise_id: Option<u64>,
  filename: String,
  mode: String,
}

impl DenoOpDispatcher for OpOpen {
  fn dispatch(
    &self,
    state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
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
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "open";
}

// Close

pub struct OpClose;

#[derive(Deserialize)]
struct CloseArgs {
  rid: i32,
}

impl DenoOpDispatcher for OpClose {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
        let args: CloseArgs = serde_json::from_value(args)?;

        match resources::lookup(args.rid as u32) {
          None => Err(deno_error::bad_resource()),
          Some(resource) => {
            resource.close();
            Ok(JsonOp::Sync(json!({})))
          }
        }
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "close";
}

// Seek

pub struct OpSeek;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeekArgs {
  promise_id: Option<u64>,
  rid: i32,
  offset: i32,
  whence: i32,
}

impl DenoOpDispatcher for OpSeek {
  fn dispatch(
    &self,
    _state: &ThreadSafeState,
    control: &[u8],
    buf: Option<PinnedBuf>,
  ) -> CoreOp {
    wrap_json_op(
      move |args, _zero_copy| {
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
      },
      control,
      buf,
    )
  }

  const NAME: &'static str = "seek";
}
