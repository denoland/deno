// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#![allow(warnings)]
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::ops::json_op;
use crate::state::State;
use core::future::Future;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;
use deno_core::*;
use futures::future::poll_fn;
use futures::future::FutureExt;
use futures::stream::StreamExt;
use notify;
use notify::event::Event;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use serde::Serialize;
use std::convert::From;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op(
    "fs_watch_open",
    s.core_op(json_op(s.stateful_op(op_fs_watch_open))),
  );
  i.register_op(
    "fs_watch_poll",
    s.core_op(json_op(s.stateful_op(op_fs_watch_poll))),
  );
}

struct FsWatcher {
  #[allow(unused)]
  watcher: RecommendedWatcher,
  receiver: mpsc::Receiver<Result<FsEvent, ErrBox>>,
}

#[derive(Deserialize)]
struct FsWatchOpenArgs {
  recursive: bool,
  paths: Vec<String>,
}

/// Represents a file system event.
///
/// We do not use the event directly from the notify crate. We flatten
/// the structure into this simpler structure. We want to only make it more
/// complex as needed.
///
/// Feel free to expand this struct as long as you can add tests to demonstrate
/// the complexity.
#[derive(Serialize, Debug)]
struct FsEvent {
  kind: String,
  paths: Vec<PathBuf>,
}

impl From<Event> for FsEvent {
  fn from(e: Event) -> Self {
    let kind = match e.kind {
      EventKind::Any => "any",
      EventKind::Access(_) => "access",
      EventKind::Create(_) => "create",
      EventKind::Modify(_) => "modify",
      EventKind::Remove(_) => "remove",
      EventKind::Other => todo!(), // What's this for? Leaving it out for now.
    }
    .to_string();
    FsEvent {
      kind,
      paths: e.paths,
    }
  }
}

pub fn op_fs_watch_open(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: FsWatchOpenArgs = serde_json::from_value(args)?;
  let (sender, receiver) = mpsc::channel::<Result<FsEvent, ErrBox>>(16);
  let sender = std::sync::Mutex::new(sender);
  let mut watcher: RecommendedWatcher =
    Watcher::new_immediate(move |res: Result<Event, notify::Error>| {
      let res2 = res.map(FsEvent::from).map_err(ErrBox::from);
      let mut sender = sender.lock().unwrap();
      futures::executor::block_on(sender.send(res2));
    })?;
  let recursive_mode = if args.recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };
  for path in &args.paths {
    state.check_read(&PathBuf::from(path))?;
    watcher.watch(path, recursive_mode)?;
  }
  let watcher_resource = FsWatcher { watcher, receiver };
  let table = &mut state.borrow_mut().resource_table;
  let rid = table.add("fsWatcher", Box::new(watcher_resource));
  Ok(JsonOp::Sync(json!(rid)))
}

#[derive(Deserialize)]
struct PollArgs {
  rid: u32,
}

pub fn op_fs_watch_poll(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let PollArgs { rid } = serde_json::from_value(args)?;
  let state = state.clone();
  let f = poll_fn(move |cx| {
    let resource_table = &mut state.borrow_mut().resource_table;
    let watcher = resource_table
      .get_mut::<FsWatcher>(rid)
      .ok_or_else(bad_resource)?;
    watcher.receiver.poll_recv(cx).map(|maybe_result| {
      Ok(if let Some(result) = maybe_result {
        let value = result?;
        json!({ "value": value, "done": false })
      } else {
        json!({ "done": true })
      })
    })
  });
  Ok(JsonOp::Async(f.boxed_local()))
}
