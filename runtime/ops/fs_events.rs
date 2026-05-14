// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::From;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use deno_core::parking_lot::Mutex;
use deno_error::JsErrorClass;
use deno_error::builtin_classes::GENERIC_ERROR;
use deno_permissions::PermissionsContainer;
use notify::Error as NotifyError;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify::Watcher;
use notify::event::Event as NotifyEvent;
use notify::event::ModifyKind;
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
  fn name(&self) -> Cow<'_, str> {
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

struct WatchSender {
  /// Original paths as provided by the caller.
  paths: Vec<PathBuf>,
  /// Pre-canonicalized versions of `paths`, computed once at watch time
  /// to avoid repeated syscalls in the event callback hot path.
  /// Entries are `None` if canonicalization failed for that path.
  canonical_paths: Vec<Option<PathBuf>>,
  sender: mpsc::Sender<Result<FsEvent, NotifyError>>,
}

struct WatcherState {
  senders: Arc<Mutex<Vec<WatchSender>>>,
  watcher: RecommendedWatcher,
}

#[allow(
  clippy::disallowed_methods,
  reason = "always using real fs with watcher"
)]
fn canonicalize_path(path: &Path) -> Option<PathBuf> {
  path.canonicalize().ok()
}

/// Check if `event_path` (or its canonicalized form) matches one of the
/// watched paths. The watched paths are pre-canonicalized to avoid
/// repeated syscalls in the hot path.
fn event_matches_watched_paths(
  event_path: &Path,
  paths: &[PathBuf],
  canonical_paths: &[Option<PathBuf>],
) -> bool {
  // Canonicalize the event path at most once per call.
  let canonical_event_path = canonicalize_path(event_path);
  for (path, canonical_path) in paths.iter().zip(canonical_paths.iter()) {
    if same_file::is_same_file(event_path, path).unwrap_or(false) {
      return true;
    }
    if matches!(
      (&canonical_event_path, canonical_path),
      (Some(ce), Some(cp)) if ce.starts_with(cp)
    ) {
      return true;
    }
  }
  false
}

/// Check if `event_path` refers to a file that has been removed and
/// that file is within one of the watched paths. This is needed because
/// `event_matches_watched_paths` will fail for removed files (canonicalize
/// and is_same_file don't work on non-existent paths). On macOS with
/// FSEvents, remove events may arrive as generic events for a path that
/// no longer exists.
fn removed_event_matches_watched_paths(
  event_path: &Path,
  paths: &[PathBuf],
  canonical_paths: &[Option<PathBuf>],
) -> bool {
  if !is_file_removed(event_path) {
    return false;
  }
  let canonical_parent = event_path.parent().and_then(canonicalize_path);
  for (path, canonical_path) in paths.iter().zip(canonical_paths.iter()) {
    // Direct path comparison: the file is gone so is_same_file won't work,
    // but the event path may match the watched path or its canonical form
    // exactly (e.g. when watching a single file that gets deleted).
    if event_path == path {
      return true;
    }
    if canonical_path
      .as_ref()
      .is_some_and(|cp| event_path == cp.as_path())
    {
      return true;
    }
    // Check if the removed file's parent is within a watched directory.
    if matches!(
      (&canonical_parent, canonical_path),
      (Some(cp_event), Some(cp_watched)) if cp_event.starts_with(cp_watched)
    ) {
      return true;
    }
  }
  false
}

fn is_file_removed(event_path: &Path) -> bool {
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

fn make_watch_sender(
  paths: Vec<PathBuf>,
  sender: mpsc::Sender<Result<FsEvent, NotifyError>>,
) -> WatchSender {
  let canonical_paths = paths.iter().map(|p| canonicalize_path(p)).collect();
  WatchSender {
    paths,
    canonical_paths,
    sender,
  }
}

fn start_watcher(
  state: &mut OpState,
  paths: Vec<PathBuf>,
  sender: mpsc::Sender<Result<FsEvent, NotifyError>>,
) -> Result<(), FsEventsError> {
  if let Some(watcher) = state.try_borrow_mut::<WatcherState>() {
    watcher
      .senders
      .lock()
      .push(make_watch_sender(paths, sender));
    return Ok(());
  }

  let senders = Arc::new(Mutex::new(vec![make_watch_sender(paths, sender)]));

  let sender_clone = senders.clone();
  let watcher: RecommendedWatcher = Watcher::new(
    move |res: Result<NotifyEvent, NotifyError>| {
      let res2 = res
        .map(FsEvent::from)
        .map_err(|e| FsEventsError::Notify(JsNotifyError(e)));
      for ws in sender_clone.lock().iter() {
        // Ignore result, if send failed it means that watcher was already closed,
        // but not all messages have been flushed.

        // Only send the event if the path matches one of the paths
        // that the user is watching.
        if let Ok(event) = &res2 {
          if event.paths.iter().any(|event_path| {
            event_matches_watched_paths(
              event_path,
              &ws.paths,
              &ws.canonical_paths,
            )
          }) {
            let _ = ws.sender.try_send(Ok(event.clone()));
          } else if event.paths.iter().any(|event_path| {
            removed_event_matches_watched_paths(
              event_path,
              &ws.paths,
              &ws.canonical_paths,
            )
          }) {
            let remove_event = FsEvent {
              kind: "remove",
              paths: event.paths.clone(),
              flag: None,
            };
            let _ = ws.sender.try_send(Ok(remove_event));
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

/// Make `path` absolute and collapse `.` / `..` segments so that paths
/// reported back in `FsEvent` don't carry the leftover relative bits notify
/// pastes onto its event paths (see denoland/deno#32000). Symlinks are
/// intentionally not resolved here so user-visible event paths still reflect
/// the path the caller passed in.
fn normalize_watch_path(path: PathBuf) -> PathBuf {
  if path.is_absolute() {
    return deno_path_util::normalize_path(Cow::Owned(path)).into_owned();
  }
  #[allow(
    clippy::disallowed_methods,
    reason = "fs watcher needs the real cwd to absolutize the watch path"
  )]
  let cwd = std::env::current_dir();
  match cwd {
    Ok(cwd) => {
      deno_path_util::normalize_path(Cow::Owned(cwd.join(&path))).into_owned()
    }
    Err(_) => path,
  }
}

#[op2(stack_trace)]
#[smi]
fn op_fs_events_open(
  state: &mut OpState,
  recursive: bool,
  #[scoped] paths: Vec<String>,
) -> Result<ResourceId, FsEventsError> {
  let mut resolved_paths = Vec::with_capacity(paths.len());
  {
    let permissions_container = state.borrow_mut::<PermissionsContainer>();
    for path in paths {
      let checked = permissions_container
        .check_open(
          Cow::Owned(PathBuf::from(path)),
          deno_permissions::OpenAccessKind::ReadNoFollow,
          Some("Deno.watchFs()"),
        )?
        .into_owned_path();
      resolved_paths.push(normalize_watch_path(checked));
    }
  }

  let (sender, receiver) = mpsc::channel::<Result<FsEvent, NotifyError>>(16);

  start_watcher(state, resolved_paths.clone(), sender)?;

  let recursive_mode = if recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };
  for path in &resolved_paths {
    let watcher = state.borrow_mut::<WatcherState>();
    watcher
      .watcher
      .watch(path, recursive_mode)
      .map_err(|e| FsEventsError::Notify(JsNotifyError(e)))?;
  }
  let resource = FsEventsResource {
    receiver: AsyncRefCell::new(receiver),
    cancel: Default::default(),
  };
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

#[op2]
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
