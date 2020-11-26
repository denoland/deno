// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::permissions::Permissions;
use deno_core::error::bad_resource_id;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::AsyncMutFuture;
use deno_core::AsyncRefCell;
use deno_core::BufVec;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use notify::event::Event as NotifyEvent;
use notify::Error as NotifyError;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::path::PathBuf;
use std::rc::Rc;
use tokio::sync::mpsc;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_fs_events_open", op_fs_events_open);
  super::reg_json_async(rt, "op_fs_events_poll", op_fs_events_poll);
}

struct FsEventsResource {
  #[allow(unused)]
  watcher: RecommendedWatcher,
  receiver: AsyncRefCell<mpsc::Receiver<Result<FsEvent, AnyError>>>,
}

impl Resource for FsEventsResource {
  fn name(&self) -> Cow<str> {
    "fsEvents".into()
  }
}

impl FsEventsResource {
  fn recv_borrow_mut(
    self: Rc<Self>,
  ) -> AsyncMutFuture<mpsc::Receiver<Result<FsEvent, AnyError>>> {
    RcRef::map(self, |r| &r.receiver).borrow_mut()
  }
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

fn op_fs_events_open(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  #[derive(Deserialize)]
  struct OpenArgs {
    recursive: bool,
    paths: Vec<String>,
  }
  let args: OpenArgs = serde_json::from_value(args)?;
  let (sender, receiver) = mpsc::channel::<Result<FsEvent, AnyError>>(16);
  let sender = std::sync::Mutex::new(sender);
  let mut watcher: RecommendedWatcher =
    Watcher::new_immediate(move |res: Result<NotifyEvent, NotifyError>| {
      let res2 = res.map(FsEvent::from).map_err(AnyError::from);
      let mut sender = sender.lock().unwrap();
      // Ignore result, if send failed it means that watcher was already closed,
      // but not all messages have been flushed.
      let _ = sender.try_send(res2);
    })?;
  let recursive_mode = if args.recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };
  for path in &args.paths {
    state
      .borrow::<Permissions>()
      .check_read(&PathBuf::from(path))?;
    watcher.watch(path, recursive_mode)?;
  }
  let resource = FsEventsResource {
    watcher,
    receiver: AsyncRefCell::new(receiver),
  };
  let rid = state.resource_table_2.add(resource);
  Ok(json!(rid))
}

async fn op_fs_events_poll(
  state: Rc<RefCell<OpState>>,
  args: Value,
  _zero_copy: BufVec,
) -> Result<Value, AnyError> {
  #[derive(Deserialize)]
  struct PollArgs {
    rid: u32,
  }
  let PollArgs { rid } = serde_json::from_value(args)?;

  let resource = state
    .borrow()
    .resource_table_2
    .get::<FsEventsResource>(rid)
    .ok_or_else(bad_resource_id)?;
  let mut receiver = resource.recv_borrow_mut().await;
  let maybe_result = receiver.recv().await;
  match maybe_result {
    Some(Ok(value)) => Ok(json!({ "value": value, "done": false })),
    Some(Err(err)) => Err(err),
    None => Ok(json!({ "done": true })),
  }
}
