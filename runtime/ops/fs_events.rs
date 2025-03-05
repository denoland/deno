// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_error::builtin_classes::GENERIC_ERROR;
use deno_error::JsErrorClass;
use deno_permissions::PermissionsContainer;
use notify::event::Event as NotifyEvent;
use notify::event::ModifyKind;
use notify::Error as NotifyError;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use serde::Serialize;
use tokio::sync::mpsc;

deno_core::extension!(
  deno_fs_events,
  ops = [op_fs_events_open, op_fs_events_poll],
);

struct FsEventsResource {
  receiver: AsyncRefCell<mpsc::Receiver<Result<FsEvent, NotifyError>>>,
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
#[derive(Serialize, Debug, Clone)]
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

type WatchSender = (Vec<String>, mpsc::Sender<Result<FsEvent, NotifyError>>);

struct WatcherState {
  senders: Arc<Mutex<Vec<WatchSender>>>,
  watcher: RecommendedWatcher,
}

fn starts_with_canonicalized(path: &Path, prefix: &str) -> bool {
  #[allow(clippy::disallowed_methods)]
  let path = path.canonicalize().ok();
  #[allow(clippy::disallowed_methods)]
  let prefix = std::fs::canonicalize(prefix).ok();
  match (path, prefix) {
    (Some(path), Some(prefix)) => path.starts_with(prefix),
    _ => false,
  }
}

fn is_file_removed(event_path: &PathBuf) -> bool {
  let exists_path = std::fs::exists(event_path);
  match exists_path {
    Ok(res) => !res,
    Err(_) => false,
  }
}

deno_error::js_error_wrapper!(NotifyError, JsNotifyError, |err| {
  match &err.kind {
    notify::ErrorKind::Generic(_) => GENERIC_ERROR.into(),
    notify::ErrorKind::Io(e) => e.get_class(),
    notify::ErrorKind::PathNotFound => "NotFound".into(),
    notify::ErrorKind::WatchNotFound => "NotFound".into(),
    notify::ErrorKind::InvalidConfig(_) => "InvalidData".into(),
    notify::ErrorKind::MaxFilesWatch => GENERIC_ERROR.into(),
  }
});

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FsEventsError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  Notify(JsNotifyError),
  #[class(inherit)]
  #[error(transparent)]
  Canceled(#[from] deno_core::Canceled),
}

fn start_watcher(
  state: &mut OpState,
  paths: Vec<String>,
  sender: mpsc::Sender<Result<FsEvent, NotifyError>>,
) -> Result<(), FsEventsError> {
  if let Some(watcher) = state.try_borrow_mut::<WatcherState>() {
    watcher.senders.lock().push((paths, sender));
    return Ok(());
  }

  let senders = Arc::new(Mutex::new(vec![(paths, sender)]));

  let sender_clone = senders.clone();
  let watcher: RecommendedWatcher = Watcher::new(
    move |res: Result<NotifyEvent, NotifyError>| {
      let res2 = res
        .map(FsEvent::from)
        .map_err(|e| FsEventsError::Notify(JsNotifyError(e)));
      for (paths, sender) in sender_clone.lock().iter() {
        // Ignore result, if send failed it means that watcher was already closed,
        // but not all messages have been flushed.

        // Only send the event if the path matches one of the paths that the user is watching
        if let Ok(event) = &res2 {
          if paths.iter().any(|path| {
            event.paths.iter().any(|event_path| {
              same_file::is_same_file(event_path, path).unwrap_or(false)
                || starts_with_canonicalized(event_path, path)
            })
          }) {
            let _ = sender.try_send(Ok(event.clone()));
          } else if event.paths.iter().any(is_file_removed) {
            let remove_event = FsEvent {
              kind: "remove",
              paths: event.paths.clone(),
              flag: None,
            };
            let _ = sender.try_send(Ok(remove_event));
          }
        }
      }
    },
    Default::default(),
  )
  .map_err(|e| FsEventsError::Notify(JsNotifyError(e)))?;

  state.put::<WatcherState>(WatcherState { watcher, senders });

  Ok(())
}

#[op2(stack_trace)]
#[smi]
fn op_fs_events_open(
  state: &mut OpState,
  recursive: bool,
  #[serde] paths: Vec<String>,
) -> Result<ResourceId, FsEventsError> {
  let (sender, receiver) = mpsc::channel::<Result<FsEvent, NotifyError>>(16);

  start_watcher(state, paths.clone(), sender)?;

  let recursive_mode = if recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };
  for path in &paths {
    let path = state
      .borrow_mut::<PermissionsContainer>()
      .check_read(path, "Deno.watchFs()")?;

    let watcher = state.borrow_mut::<WatcherState>();
    watcher
      .watcher
      .watch(&path, recursive_mode)
      .map_err(|e| FsEventsError::Notify(JsNotifyError(e)))?;
  }
  let resource = FsEventsResource {
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
) -> Result<Option<FsEvent>, FsEventsError> {
  let resource = state.borrow().resource_table.get::<FsEventsResource>(rid)?;
  let mut receiver = RcRef::map(&resource, |r| &r.receiver).borrow_mut().await;
  let cancel = RcRef::map(resource, |r| &r.cancel);
  let maybe_result = receiver.recv().or_cancel(cancel).await?;
  match maybe_result {
    Some(Ok(value)) => Ok(Some(value)),
    Some(Err(err)) => Err(FsEventsError::Notify(JsNotifyError(err))),
    None => Ok(None),
  }
}
