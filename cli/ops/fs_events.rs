// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ErrBox;
use deno_core::ZeroCopyBuf;
use futures::future::poll_fn;
use futures::future::FutureExt;
use notify::event::Event as NotifyEvent;
use notify::Error as NotifyError;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use serde::Serialize;
use std::convert::From;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_fs_events_open", s.stateful_json_op2(op_fs_events_open));
  i.register_op("op_fs_events_poll", s.stateful_json_op2(op_fs_events_poll));
}

struct FsEventsResource {
  #[allow(unused)]
  watcher: RecommendedWatcher,
  receiver: mpsc::Receiver<Result<FsEvent, ErrBox>>,
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

impl From<NotifyEvent> for FsEvent {
  fn from(e: NotifyEvent) -> Self {
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

pub fn op_fs_events_open(
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  #[derive(Deserialize)]
  struct OpenArgs {
    recursive: bool,
    paths: Vec<String>,
  }
  let args: OpenArgs = serde_json::from_value(args)?;
  let (sender, receiver) = mpsc::channel::<Result<FsEvent, ErrBox>>(16);
  let sender = std::sync::Mutex::new(sender);
  let mut watcher: RecommendedWatcher =
    Watcher::new_immediate(move |res: Result<NotifyEvent, NotifyError>| {
      let res2 = res.map(FsEvent::from).map_err(ErrBox::from);
      let mut sender = sender.lock().unwrap();
      // Ignore result, if send failed it means that watcher was already closed,
      // but not all messages have been flushed.
      let _ = sender.try_send(res2);
    })
    .map_err(ErrBox::from)?;
  let recursive_mode = if args.recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };
  for path in &args.paths {
    state.check_read(&PathBuf::from(path))?;
    watcher.watch(path, recursive_mode).map_err(ErrBox::from)?;
  }
  let resource = FsEventsResource { watcher, receiver };
  let mut resource_table = isolate_state.resource_table.borrow_mut();
  let rid = resource_table.add("fsEvents", Box::new(resource));
  Ok(JsonOp::Sync(json!(rid)))
}

pub fn op_fs_events_poll(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  #[derive(Deserialize)]
  struct PollArgs {
    rid: u32,
  }
  let PollArgs { rid } = serde_json::from_value(args)?;
  let resource_table = isolate_state.resource_table.clone();
  let f = poll_fn(move |cx| {
    let mut resource_table = resource_table.borrow_mut();
    let watcher = resource_table
      .get_mut::<FsEventsResource>(rid)
      .ok_or_else(OpError::bad_resource_id)?;
    watcher
      .receiver
      .poll_recv(cx)
      .map(|maybe_result| match maybe_result {
        Some(Ok(value)) => Ok(json!({ "value": value, "done": false })),
        Some(Err(err)) => Err(OpError::from(err)),
        None => Ok(json!({ "done": true })),
      })
  });
  Ok(JsonOp::Async(f.boxed_local()))
}
