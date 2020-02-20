// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Some deserializer fields are only used on Unix and Windows build fails without it
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::deno_error::bad_resource;
use crate::ops::json_op;
use crate::state::State;
use deno_core::*;
use futures::channel::mpsc;
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
use std::sync::mpsc::channel as sync_channel;
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex as AsyncMutex;

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

type FsWatcherReceiver = mpsc::Receiver<Result<DenoFsEvent, ErrBox>>;

struct FsWatcher {
  #[allow(unused)]
  watcher: RecommendedWatcher,
  receiver: Arc<AsyncMutex<FsWatcherReceiver>>,
}

#[derive(Deserialize)]
struct FsWatchOpenArgs {
  recursive: bool,
  paths: Vec<String>,
}

/// We do not send the event from the notify crate directly to JS. We flatten
/// the structure into this one, which is simpler.
/// We want to only make it more complex as needed, and then with tests.
#[derive(Serialize, Debug)]
struct DenoFsEvent {
  kind: String,
  paths: Vec<PathBuf>,
}

impl From<Event> for DenoFsEvent {
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
    DenoFsEvent {
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
  let (mut tx, rx) = mpsc::channel::<Result<DenoFsEvent, ErrBox>>(100);
  let (sync_tx, sync_rx) = sync_channel::<Result<DenoFsEvent, ErrBox>>();

  // TODO(bartlomieju): this is bad, but `Watcher::new_immediate` takes `Fn` and
  // not `FnMut` so now way to use async channel there
  thread::spawn(move || {
    for msg in sync_rx {
      tx.try_send(msg).expect("Failed to pump message");
    }
  });

  let mut watcher: RecommendedWatcher =
    Watcher::new_immediate(move |res: Result<Event, notify::Error>| {
      println!("got fs event {:?}", res);
      let res2 = res.map(DenoFsEvent::from).map_err(ErrBox::from);
      sync_tx.send(res2).unwrap()
    })?;

  let recursive_mode: RecursiveMode = if args.recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };
  for path in &args.paths {
    state.check_read(&PathBuf::from(path))?;
    watcher.watch(path, recursive_mode)?;
  }

  let watcher_resource = FsWatcher {
    watcher,
    receiver: Arc::new(AsyncMutex::new(rx)),
  };
  let table = &mut state.borrow_mut().resource_table;
  let rid = table.add("fsWatcher", Box::new(watcher_resource));
  Ok(JsonOp::Sync(json!(rid)))
}

#[derive(Deserialize)]
struct FsWatchPollArgs {
  rid: i32,
}

pub fn op_fs_watch_poll(
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: FsWatchPollArgs = serde_json::from_value(args)?;
  let receiver_mutex = {
    let table = &mut state.borrow_mut().resource_table;
    let resource = table
      .get_mut::<FsWatcher>(args.rid as u32)
      .ok_or_else(bad_resource)?;
    resource.receiver.clone()
  };

  let f = async move {
    println!("watcher poll");
    let mut receiver = receiver_mutex.lock().await;
    println!("watcher after lock");
    if let Some(result) = receiver.next().await {
      let e: DenoFsEvent = result?;
      println!("got deno fs event {:?}", e);
      let json_value = serde_json::value::to_value(e).unwrap();
      Ok(json_value)
    } else {
      Ok(json!({})) // When closed
    }
  };

  Ok(JsonOp::Async(f.boxed()))
}
