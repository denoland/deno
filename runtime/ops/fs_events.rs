// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::parking_lot::Mutex;
use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;

use deno_core::op2;

use deno_permissions::PermissionsContainer;
use notify::event::Event as NotifyEvent;
use notify::event::ModifyKind;
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

deno_core::extension!(
  deno_fs_events,
  ops = [op_fs_events_open, op_fs_events_poll],
);

struct FsEventsResource {
  #[allow(unused)]
  watcher: RecommendedWatcher,
  receiver: AsyncRefCell<mpsc::Receiver<Result<FsEvent, AnyError>>>,
  cancel: CancelHandle,
}

impl Resource for FsEventsResource {
  fn name(&self) -> Cow<str> {
    "fsEvents".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
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
  kind: &'static str,
  paths: Vec<PathBuf>,
  flag: Option<&'static str>,
}

impl From<NotifyEvent> for FsEvent {
  fn from(e: NotifyEvent) -> Self {
    let kind = match e.kind {
      EventKind::Any => "any",
      EventKind::Access(_) => "access",
      EventKind::Create(_) => "create",
      EventKind::Modify(modify_kind) => match modify_kind {
        ModifyKind::Name(_) => "rename",
        ModifyKind::Any
        | ModifyKind::Data(_)
        | ModifyKind::Metadata(_)
        | ModifyKind::Other => "modify",
      },
      EventKind::Remove(_) => "remove",
      EventKind::Other => "other",
    };
    let flag = e.flag().map(|f| match f {
      notify::event::Flag::Rescan => "rescan",
    });
    FsEvent {
      kind,
      paths: e.paths,
      flag,
    }
  }
}

#[derive(Deserialize)]
pub struct OpenArgs {
  recursive: bool,
  paths: Vec<String>,
}

#[op2]
#[smi]
fn op_fs_events_open(
  state: &mut OpState,
  #[serde] args: OpenArgs,
) -> Result<ResourceId, AnyError> {
  let (sender, receiver) = mpsc::channel::<Result<FsEvent, AnyError>>(16);
  let sender = Mutex::new(sender);
  let mut watcher: RecommendedWatcher = Watcher::new(
    move |res: Result<NotifyEvent, NotifyError>| {
      let res2 = res.map(FsEvent::from).map_err(AnyError::from);
      let sender = sender.lock();
      // Ignore result, if send failed it means that watcher was already closed,
      // but not all messages have been flushed.
      let _ = sender.try_send(res2);
    },
    Default::default(),
  )?;
  let recursive_mode = if args.recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };
  for path in &args.paths {
    let path = state
      .borrow_mut::<PermissionsContainer>()
      .check_read(path, "Deno.watchFs()")?;
    watcher.watch(&path, recursive_mode)?;
  }
  let resource = FsEventsResource {
    watcher,
    receiver: AsyncRefCell::new(receiver),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

#[op2(async)]
#[serde]
async fn op_fs_events_poll(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<FsEvent>, AnyError> {
  let resource = state.borrow().resource_table.get::<FsEventsResource>(rid)?;
  let mut receiver = RcRef::map(&resource, |r| &r.receiver).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let maybe_result = receiver.recv().or_cancel(cancel).await?;
  match maybe_result {
    Some(Ok(value)) => Ok(Some(value)),
    Some(Err(err)) => Err(err),
    None => Ok(None),
  }
}
