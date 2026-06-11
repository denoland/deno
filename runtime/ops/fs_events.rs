// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::From;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use deno_core::AsyncRefCell;
use deno_core::CancelFuture;
use deno_core::CancelHandle;
use deno_core::OpState;
use deno_core::RcRef;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ToV8;
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
use notify::event::AccessKind;
use notify::event::AccessMode;
use notify::event::Event as NotifyEvent;
use notify::event::ModifyKind;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

deno_core::extension!(
  deno_fs_events,
  ops = [op_fs_events_open, op_fs_events_poll],
);

/// Capacity of the queue between the notify backend thread and the JS-side
/// consumer. The watcher callback must not block, so when this queue is full
/// further events are dropped; the drop is recorded and surfaced to the
/// consumer as a `flag: "rescan"` event (see [`op_fs_events_poll`]) instead
/// of being silently lost (see denoland/deno#11373). The capacity is sized so
/// that ordinary bursts (e.g. unpacking an archive into a watched directory)
/// fit without dropping; tokio's bounded channel allocates lazily, so an
/// idle watcher does not pay for it.
const FS_EVENT_QUEUE_CAPACITY: usize = 1024;

struct FsEventsResource {
  receiver: AsyncRefCell<mpsc::Receiver<Result<FsEvent, NotifyError>>>,
  cancel: CancelHandle,
  /// Shared backend used to clean up our watch on drop.
  inner: Arc<WatcherInner>,
  /// Identifies our `WatchSender` entry in [`WatcherInner::senders`].
  id: u64,
  /// The (path, recursive_mode) pairs this resource registered. Tracked so the
  /// shared watcher unwatches them when the last interested resource closes.
  watched: Vec<(PathBuf, RecursiveMode)>,
  /// Set by the watcher callback when the event queue overflowed and events
  /// were dropped; drained by `op_fs_events_poll`, which surfaces the loss as
  /// a `flag: "rescan"` event.
  overflowed: Arc<AtomicBool>,
}

impl Resource for FsEventsResource {
  fn name(&self) -> Cow<'_, str> {
    "fsEvents".into()
  }

  fn close(self: Rc<Self>) {
    self.cancel.cancel();
  }
}

impl Drop for FsEventsResource {
  fn drop(&mut self) {
    // Remove this resource's sender from the shared dispatch list so the
    // watcher callback stops trying to deliver events to a dead channel.
    self.inner.senders.lock().retain(|ws| ws.id != self.id);

    // Reference-count the underlying watches: only unwatch when no other
    // resource still depends on this path. Without this, calling
    // `Deno.watchFs(path)` repeatedly leaks watches in the shared
    // `RecommendedWatcher` — on Windows each leaked watch registers a
    // separate `ReadDirectoryChangesW` request, so the next watcher created
    // for the same path receives every event N times.
    let mut watched_paths = self.inner.watched_paths.lock();
    let mut watcher = self.inner.watcher.lock();
    for (path, mode) in &self.watched {
      let key = (path.clone(), *mode);
      let Some(count) = watched_paths.get_mut(&key) else {
        continue;
      };
      *count = count.saturating_sub(1);
      if *count == 0 {
        watched_paths.remove(&key);
        // Best-effort: ignore errors (e.g. the watcher already lost track of
        // the path because the path was deleted).
        let _ = watcher.unwatch(path);
      }
    }
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
#[derive(ToV8, Debug, Clone)]
struct FsEvent {
  kind: &'static str,
  #[to_v8(serde)]
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

fn is_ignored_notify_event(event: &NotifyEvent) -> bool {
  // notify 8 added inotify OPEN events to the default watch mask. Deno did not
  // expose these with notify 6, and forwarding them can self-amplify because
  // our path filtering performs filesystem reads.
  matches!(
    event.kind,
    EventKind::Access(AccessKind::Open(AccessMode::Any))
  )
}

struct WatchSender {
  /// Unique identifier so the matching entry can be removed when the
  /// owning [`FsEventsResource`] is dropped.
  id: u64,
  /// Original paths as provided by the caller.
  paths: Vec<PathBuf>,
  /// Pre-canonicalized versions of `paths`, computed once at watch time
  /// to avoid repeated syscalls in the event callback hot path.
  /// Entries are `None` if canonicalization failed for that path.
  canonical_paths: Vec<Option<PathBuf>>,
  sender: mpsc::Sender<Result<FsEvent, NotifyError>>,
  /// See [`FsEventsResource::overflowed`].
  overflowed: Arc<AtomicBool>,
}

impl WatchSender {
  /// Deliver an event or error without blocking the notify backend thread.
  /// If the queue is full the message is dropped and the overflow is
  /// recorded so the consumer learns about the loss via a `"rescan"` event.
  /// A closed channel just means the watcher resource is gone; that is not
  /// a loss.
  fn send_or_record_overflow(&self, msg: Result<FsEvent, NotifyError>) {
    if let Err(TrySendError::Full(_)) = self.sender.try_send(msg) {
      self.overflowed.store(true, Ordering::Relaxed);
    }
  }
}

/// `notify::Error` is not `Clone` (it can wrap an `std::io::Error`), but the
/// shared watcher callback has to deliver an error to every interested
/// resource, so rebuild an equivalent error for each receiver. For I/O errors
/// the original `ErrorKind` and message are preserved.
fn clone_notify_error(err: &NotifyError) -> NotifyError {
  let kind = match &err.kind {
    notify::ErrorKind::Generic(msg) => notify::ErrorKind::Generic(msg.clone()),
    notify::ErrorKind::Io(io_err) => notify::ErrorKind::Io(
      std::io::Error::new(io_err.kind(), io_err.to_string()),
    ),
    notify::ErrorKind::PathNotFound => notify::ErrorKind::PathNotFound,
    notify::ErrorKind::WatchNotFound => notify::ErrorKind::WatchNotFound,
    notify::ErrorKind::InvalidConfig(config) => {
      notify::ErrorKind::InvalidConfig(*config)
    }
    notify::ErrorKind::MaxFilesWatch => notify::ErrorKind::MaxFilesWatch,
  };
  NotifyError {
    kind,
    paths: err.paths.clone(),
  }
}

struct WatcherInner {
  /// Per-watcher dispatch list. The shared notify callback iterates this
  /// list on every event. Wrapped in `Arc<Mutex<...>>` so the watcher
  /// callback (which lives on the notify backend thread) can hold a
  /// reference without keeping the rest of [`WatcherInner`] alive.
  senders: Arc<Mutex<Vec<WatchSender>>>,
  /// The shared `RecommendedWatcher` instance backing every
  /// `Deno.watchFs(...)` call in this OpState.
  watcher: Mutex<RecommendedWatcher>,
  /// Reference count per (path, recursive_mode) registered with `watcher`.
  /// When a count drops to zero, the path is unwatched.
  watched_paths: Mutex<HashMap<(PathBuf, RecursiveMode), usize>>,
  /// Monotonic counter for assigning `WatchSender::id`.
  next_id: AtomicU64,
}

struct WatcherState {
  inner: Arc<WatcherInner>,
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
  id: u64,
  paths: Vec<PathBuf>,
  sender: mpsc::Sender<Result<FsEvent, NotifyError>>,
  overflowed: Arc<AtomicBool>,
) -> WatchSender {
  let canonical_paths = paths.iter().map(|p| canonicalize_path(p)).collect();
  WatchSender {
    id,
    paths,
    canonical_paths,
    sender,
    overflowed,
  }
}

fn ensure_watcher(
  state: &mut OpState,
) -> Result<Arc<WatcherInner>, FsEventsError> {
  if let Some(ws) = state.try_borrow::<WatcherState>() {
    return Ok(ws.inner.clone());
  }

  let senders: Arc<Mutex<Vec<WatchSender>>> = Arc::new(Mutex::new(Vec::new()));
  let sender_clone = senders.clone();
  let watcher: RecommendedWatcher = Watcher::new(
    move |res: Result<NotifyEvent, NotifyError>| {
      let res2 = match res {
        Ok(event) if is_ignored_notify_event(&event) => return,
        Ok(event) => Ok(FsEvent::from(event)),
        Err(e) => Err(e),
      };
      for ws in sender_clone.lock().iter() {
        match &res2 {
          // Only send the event if the path matches one of the paths
          // that the user is watching.
          Ok(event) => {
            if event.paths.iter().any(|event_path| {
              event_matches_watched_paths(
                event_path,
                &ws.paths,
                &ws.canonical_paths,
              )
            }) {
              ws.send_or_record_overflow(Ok(event.clone()));
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
              ws.send_or_record_overflow(Ok(remove_event));
            }
          }
          // Watcher errors are not reliably path-scoped (their `paths` are
          // often empty), so deliver them to every watcher rather than
          // dropping them on the floor.
          Err(err) => {
            ws.send_or_record_overflow(Err(clone_notify_error(err)));
          }
        }
      }
    },
    Default::default(),
  )
  .map_err(|e| FsEventsError::Notify(JsNotifyError(e)))?;

  let inner = Arc::new(WatcherInner {
    senders,
    watcher: Mutex::new(watcher),
    watched_paths: Mutex::new(HashMap::new()),
    next_id: AtomicU64::new(0),
  });

  state.put::<WatcherState>(WatcherState {
    inner: inner.clone(),
  });

  Ok(inner)
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

  let (sender, receiver) =
    mpsc::channel::<Result<FsEvent, NotifyError>>(FS_EVENT_QUEUE_CAPACITY);
  let overflowed = Arc::new(AtomicBool::new(false));

  let inner = ensure_watcher(state)?;

  let id = inner.next_id.fetch_add(1, Ordering::Relaxed);

  inner.senders.lock().push(make_watch_sender(
    id,
    resolved_paths.clone(),
    sender,
    overflowed.clone(),
  ));

  let recursive_mode = if recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };

  // Register each path with the shared watcher exactly once per
  // (path, mode) pair. Subsequent resources requesting the same
  // (path, mode) bump the refcount but skip the `watch` syscall.
  // This is the core of the duplicate-event fix on Windows (see
  // denoland/deno#27742): otherwise repeated `watch` calls register
  // duplicate ReadDirectoryChangesW operations whose callbacks all
  // fire on every change.
  let mut watched = Vec::with_capacity(resolved_paths.len());
  {
    let mut watched_paths = inner.watched_paths.lock();
    let mut watcher = inner.watcher.lock();
    for path in &resolved_paths {
      let key = (path.clone(), recursive_mode);
      let count = watched_paths.entry(key.clone()).or_insert(0);
      if *count == 0
        && let Err(e) = watcher.watch(path, recursive_mode)
      {
        // Roll back any partial state we accumulated for this call so
        // a failed open doesn't leave dangling refcounts/senders.
        watched_paths.remove(&key);
        drop(watcher);
        drop(watched_paths);
        rollback_partial_open(&inner, id, &watched);
        return Err(FsEventsError::Notify(JsNotifyError(e)));
      }
      *count += 1;
      watched.push((path.clone(), recursive_mode));
    }
  }

  let resource = FsEventsResource {
    receiver: AsyncRefCell::new(receiver),
    cancel: Default::default(),
    inner,
    id,
    watched,
    overflowed,
  };
  let rid = state.resource_table.add(resource);
  Ok(rid)
}

/// Undo the `senders` push and any `watch` calls we performed before
/// hitting an error in `op_fs_events_open`. Mirrors the cleanup that
/// would run via [`FsEventsResource`]'s `Drop`, but is needed because
/// the resource itself was never constructed.
fn rollback_partial_open(
  inner: &Arc<WatcherInner>,
  id: u64,
  watched: &[(PathBuf, RecursiveMode)],
) {
  inner.senders.lock().retain(|ws| ws.id != id);

  let mut watched_paths = inner.watched_paths.lock();
  let mut watcher = inner.watcher.lock();
  for (path, mode) in watched {
    let key = (path.clone(), *mode);
    if let Some(count) = watched_paths.get_mut(&key) {
      *count = count.saturating_sub(1);
      if *count == 0 {
        watched_paths.remove(&key);
        let _ = watcher.unwatch(path);
      }
    }
  }
}

#[op2]
async fn op_fs_events_poll(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<Option<FsEvent>, FsEventsError> {
  let resource = state.borrow().resource_table.get::<FsEventsResource>(rid)?;
  let mut receiver = RcRef::map(&resource, |r| &r.receiver).borrow_mut().await;

  // If the event queue overflowed since the last poll, events were dropped:
  // tell the consumer via a `"rescan"`-flagged event rather than losing them
  // silently (denoland/deno#11373). Everything still queued predates the
  // rescan and is superseded by it, so drain and discard those stale events
  // rather than delivering them after the rescan. Queued watcher errors must
  // not be lost though: deliver such an error now and re-arm the flag so the
  // next poll still reports the rescan. There is no missed-wakeup hazard
  // here: the flag can only be set while the queue is full, so a consumer
  // blocked in `recv()` below always has queued events to wake it, and the
  // flag is checked again on the next poll.
  if resource.overflowed.swap(false, Ordering::Relaxed) {
    loop {
      match receiver.try_recv() {
        Ok(Ok(_)) => continue,
        Ok(Err(err)) => {
          resource.overflowed.store(true, Ordering::Relaxed);
          return Err(FsEventsError::Notify(JsNotifyError(err)));
        }
        Err(_) => break,
      }
    }
    let paths = resource.watched.iter().map(|(p, _)| p.clone()).collect();
    return Ok(Some(FsEvent {
      kind: "any",
      paths,
      flag: Some("rescan"),
    }));
  }

  let cancel = RcRef::map(resource, |r| &r.cancel);
  let maybe_result = receiver.recv().or_cancel(cancel).await?;
  match maybe_result {
    Some(Ok(value)) => Ok(Some(value)),
    Some(Err(err)) => Err(FsEventsError::Notify(JsNotifyError(err))),
    None => Ok(None),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn notify_event(kind: EventKind) -> NotifyEvent {
    NotifyEvent {
      kind,
      paths: vec![PathBuf::from("file.txt")],
      attrs: Default::default(),
    }
  }

  #[test]
  fn ignores_notify_open_any_events() {
    let event =
      notify_event(EventKind::Access(AccessKind::Open(AccessMode::Any)));

    assert!(is_ignored_notify_event(&event));
  }

  #[test]
  fn preserves_notify_close_write_as_access() {
    let event =
      notify_event(EventKind::Access(AccessKind::Close(AccessMode::Write)));

    assert!(!is_ignored_notify_event(&event));
    assert_eq!(FsEvent::from(event).kind, "access");
  }

  #[test]
  fn clone_notify_error_preserves_kind_and_paths() {
    let err = NotifyError {
      kind: notify::ErrorKind::Io(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "denied",
      )),
      paths: vec![PathBuf::from("watched/dir")],
    };

    let cloned = clone_notify_error(&err);

    assert_eq!(cloned.paths, err.paths);
    match cloned.kind {
      notify::ErrorKind::Io(io_err) => {
        assert_eq!(io_err.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(io_err.to_string(), "denied");
      }
      kind => panic!("expected Io error, got {kind:?}"),
    }
  }
}
