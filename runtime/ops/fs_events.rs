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
use deno_node::ops::fs::NodeFsError;
use deno_node::ops::fs::NodeFsErrorContext;
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
  ops = [
    op_fs_events_open,
    op_fs_events_poll,
    op_node_fs_watch_open,
    op_node_fs_watch_poll
  ],
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

/// The shared guts of a watch registration: the event channel plus the
/// bookkeeping needed to clean up the shared watcher on drop. Wrapped by both
/// [`FsEventsResource`] (`Deno.watchFs`) and [`NodeFsWatcherResource`]
/// (`node:fs.watch`).
struct WatchHandle {
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
  /// a `flag: "rescan"` event. Shares the `Arc` held by our `WatchSender`.
  overflowed: Arc<AtomicBool>,
}

struct FsEventsResource {
  handle: WatchHandle,
}

impl Resource for FsEventsResource {
  fn name(&self) -> Cow<'_, str> {
    "fsEvents".into()
  }

  fn close(self: Rc<Self>) {
    self.handle.cancel.cancel();
  }
}

impl Drop for WatchHandle {
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
  /// Paths whose events should be filtered out (the `ignore` option).
  ignore: Vec<PathBuf>,
  /// Pre-canonicalized versions of `ignore`, mirroring `canonical_paths`.
  canonical_ignore: Vec<Option<PathBuf>>,
  sender: mpsc::Sender<Result<FsEvent, NotifyError>>,
  /// See [`WatchHandle::overflowed`].
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

fn make_watch_sender(
  id: u64,
  paths: Vec<PathBuf>,
  ignore: Vec<PathBuf>,
  sender: mpsc::Sender<Result<FsEvent, NotifyError>>,
  overflowed: Arc<AtomicBool>,
) -> WatchSender {
  let canonical_paths = paths.iter().map(|p| canonicalize_path(p)).collect();
  let canonical_ignore = ignore.iter().map(|p| canonicalize_path(p)).collect();
  WatchSender {
    id,
    paths,
    canonical_paths,
    ignore,
    canonical_ignore,
    sender,
    overflowed,
  }
}

fn ensure_watcher(
  state: &mut OpState,
) -> Result<Arc<WatcherInner>, NotifyError> {
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
            // Skip events whose paths all fall under one of the ignored
            // paths (the `ignore` option). Removed files are checked too so
            // deletions inside an ignored directory don't leak through
            // (canonicalize/is_same_file fail for paths that no longer
            // exist). The `is_empty` guard keeps this off the event-callback
            // hot path for the common case where `ignore` is unused.
            //
            // `all` (not `any`) means an event is dropped only when every
            // path is ignored, so a rename out of an ignored dir into a
            // watched one (paths = [from, to] with `from` ignored and `to`
            // watched) is still delivered. The reverse case (rename into an
            // ignored dir) still delivers an event containing the ignored
            // `to` path; that is intentional.
            if !ws.ignore.is_empty()
              && !event.paths.is_empty()
              && event.paths.iter().all(|event_path| {
                event_matches_watched_paths(
                  event_path,
                  &ws.ignore,
                  &ws.canonical_ignore,
                ) || removed_event_matches_watched_paths(
                  event_path,
                  &ws.ignore,
                  &ws.canonical_ignore,
                )
              })
            {
              continue;
            }
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
  )?;

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

/// Register `resolved_paths` with the shared watcher and return the channel
/// handle. Shared by `op_fs_events_open` and `op_node_fs_watch_open`.
fn open_watch_handle(
  state: &mut OpState,
  resolved_paths: Vec<PathBuf>,
  ignore_paths: Vec<PathBuf>,
  recursive_mode: RecursiveMode,
) -> Result<WatchHandle, NotifyError> {
  let (sender, receiver) =
    mpsc::channel::<Result<FsEvent, NotifyError>>(FS_EVENT_QUEUE_CAPACITY);
  let overflowed = Arc::new(AtomicBool::new(false));

  let inner = ensure_watcher(state)?;

  let id = inner.next_id.fetch_add(1, Ordering::Relaxed);

  inner.senders.lock().push(make_watch_sender(
    id,
    resolved_paths.clone(),
    ignore_paths,
    sender,
    overflowed.clone(),
  ));

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
        return Err(e);
      }
      *count += 1;
      watched.push((path.clone(), recursive_mode));
    }
  }

  Ok(WatchHandle {
    receiver: AsyncRefCell::new(receiver),
    cancel: Default::default(),
    inner,
    id,
    watched,
    overflowed,
  })
}

#[op2(stack_trace)]
#[smi]
fn op_fs_events_open(
  state: &mut OpState,
  recursive: bool,
  #[scoped] ignore: Vec<String>,
  #[scoped] paths: Vec<String>,
) -> Result<ResourceId, FsEventsError> {
  let mut resolved_paths = Vec::with_capacity(paths.len());
  let mut ignore_paths = Vec::with_capacity(ignore.len());
  {
    let permissions_container = state.borrow_mut::<PermissionsContainer>();
    for path in ignore {
      let checked = permissions_container
        .check_open(
          Cow::Owned(PathBuf::from(path)),
          deno_permissions::OpenAccessKind::ReadNoFollow,
          Some("Deno.watchFs()"),
        )?
        .into_owned_path();
      ignore_paths.push(normalize_watch_path(checked));
    }
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

  let recursive_mode = if recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };

  let handle =
    open_watch_handle(state, resolved_paths, ignore_paths, recursive_mode)
      .map_err(|e| FsEventsError::Notify(JsNotifyError(e)))?;
  let rid = state.resource_table.add(FsEventsResource { handle });
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
  let mut receiver = RcRef::map(&resource, |r| &r.handle.receiver)
    .borrow_mut()
    .await;

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
  if resource.handle.overflowed.swap(false, Ordering::Relaxed) {
    loop {
      match receiver.try_recv() {
        Ok(Ok(_)) => continue,
        Ok(Err(err)) => {
          resource.handle.overflowed.store(true, Ordering::Relaxed);
          return Err(FsEventsError::Notify(JsNotifyError(err)));
        }
        Err(_) => break,
      }
    }
    let paths = resource
      .handle
      .watched
      .iter()
      .map(|(p, _)| p.clone())
      .collect();
    return Ok(Some(FsEvent {
      kind: "any",
      paths,
      flag: Some("rescan"),
    }));
  }

  let cancel = RcRef::map(resource, |r| &r.handle.cancel);
  let maybe_result = receiver.recv().or_cancel(cancel).await?;
  match maybe_result {
    Some(Ok(value)) => Ok(Some(value)),
    Some(Err(err)) => Err(FsEventsError::Notify(JsNotifyError(err))),
    None => Ok(None),
  }
}

// --- node:fs.watch ---
//
// node-flavored watcher ops sharing the same notify backend as
// `Deno.watchFs`. They live here (not in ext/node) because they reuse the
// private shared-watcher machinery above; errors are built fully node-formed
// via deno_node's error helpers so the polyfill needs no translation layer.

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum NodeFsEventsError {
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
  #[class(inherit)]
  #[error(transparent)]
  Node(#[from] NodeFsError),
}

// Maps a notify error to node's uv-style watch error (syscall "watch" + the
// user-supplied path). PathNotFound/WatchNotFound surface as ENOENT like
// node's failed `handle.start`.
fn node_notify_err(e: NotifyError, path: &str) -> NodeFsError {
  let ctx = NodeFsErrorContext::new_syscall_path("watch", path);
  match e.kind {
    notify::ErrorKind::PathNotFound | notify::ErrorKind::WatchNotFound => {
      NodeFsError::from_code("ENOENT", ctx)
    }
    notify::ErrorKind::Io(io) => match io.raw_os_error() {
      Some(errno) => NodeFsError::new(errno, ctx),
      None => {
        NodeFsError::from_code("UNKNOWN", ctx.with_message(io.to_string()))
      }
    },
    notify::ErrorKind::MaxFilesWatch => NodeFsError::from_code("ENOSPC", ctx),
    notify::ErrorKind::Generic(msg) => {
      NodeFsError::from_code("UNKNOWN", ctx.with_message(msg))
    }
    notify::ErrorKind::InvalidConfig(_) => {
      NodeFsError::from_code("EINVAL", ctx)
    }
  }
}

struct NodeFsWatcherResource {
  handle: WatchHandle,
  recursive: bool,
  /// Symlink-resolved watch root, used to relativize recursive event paths
  /// (notify reports real paths, e.g. /private/var for /var on macOS).
  resolved_path: PathBuf,
  /// The user-supplied path, for error messages.
  path: String,
}

impl Resource for NodeFsWatcherResource {
  fn name(&self) -> Cow<'_, str> {
    "nodeFsWatcher".into()
  }

  fn close(self: Rc<Self>) {
    self.handle.cancel.cancel();
  }
}

#[allow(
  clippy::disallowed_methods,
  reason = "always using real fs with watcher"
)]
fn lstat_real(path: &Path) -> Result<(), std::io::Error> {
  std::fs::symlink_metadata(path).map(|_| ())
}

#[op2(fast, stack_trace)]
#[smi]
fn op_node_fs_watch_open(
  state: &mut OpState,
  #[string] path: String,
  recursive: bool,
) -> Result<ResourceId, NodeFsEventsError> {
  let checked = state
    .borrow_mut::<PermissionsContainer>()
    .check_open(
      Cow::Owned(PathBuf::from(&path)),
      deno_permissions::OpenAccessKind::ReadNoFollow,
      Some("node:fs.watch"),
    )?
    .into_owned_path();

  // Pre-validate path existence so missing-path failures surface as uv ENOENT
  // consistently across platforms: notify's Windows backend (`add_watch` in
  // src/windows.rs) returns a Generic error rather than a typed not-found when
  // the path doesn't exist.
  if let Err(e) = lstat_real(&checked) {
    let ctx = NodeFsErrorContext::new_syscall_path("watch", &path);
    return Err(
      match e.raw_os_error() {
        Some(errno) => NodeFsError::new(errno, ctx),
        None => NodeFsError::from_code("ENOENT", ctx),
      }
      .into(),
    );
  }

  let normalized = normalize_watch_path(checked);
  let resolved_path =
    canonicalize_path(&normalized).unwrap_or_else(|| normalized.clone());

  let recursive_mode = if recursive {
    RecursiveMode::Recursive
  } else {
    RecursiveMode::NonRecursive
  };

  let handle =
    open_watch_handle(state, vec![normalized], vec![], recursive_mode)
      .map_err(|e| node_notify_err(e, &path))?;
  Ok(state.resource_table.add(NodeFsWatcherResource {
    handle,
    recursive,
    resolved_path,
    path,
  }))
}

// node maps create/remove/rename events to "rename" and everything else to
// "change" (see the FSWatcher rename/change split in lib/internal/fs/watchers.js).
fn node_event_type(kind: &str) -> &'static str {
  match kind {
    "create" | "remove" | "rename" => "rename",
    _ => "change",
  }
}

// Strips Windows verbatim (`\\?\`) / UNC prefixes so a canonicalized base
// (`\\?\D:\x`, from `Path::canonicalize`) compares equal to notify's plain
// event paths (`D:\x\y`). No-op on non-Windows.
fn strip_verbatim_prefix(p: &Path) -> Cow<'_, Path> {
  #[cfg(windows)]
  if let Some(s) = p.as_os_str().to_str() {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
      return Cow::Owned(PathBuf::from(format!(r"\\{rest}")));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
      return Cow::Owned(PathBuf::from(rest));
    }
  }
  Cow::Borrowed(p)
}

// Relative path from `base` to `to`, or `None` if `to` is not under `base`
// (after normalizing away Windows verbatim prefixes).
fn relative_under(base: &Path, to: &Path) -> Option<String> {
  let base = strip_verbatim_prefix(base);
  let to = strip_verbatim_prefix(to);
  to.strip_prefix(base.as_ref())
    .ok()
    .map(|p| p.to_string_lossy().into_owned())
}

/// `[eventType, filename]` for an fs.watch "change" emission, or `null` once
/// the watcher closes (ends the JS poll loop).
struct NodeWatchEvent(Option<(&'static str, String)>);

impl<'a> ToV8<'a> for NodeWatchEvent {
  type Error = std::convert::Infallible;
  fn to_v8(
    self,
    scope: &mut deno_core::v8::PinScope<'a, '_>,
  ) -> Result<deno_core::v8::Local<'a, deno_core::v8::Value>, Self::Error> {
    use deno_core::v8;
    Ok(match self.0 {
      Some((event_type, filename)) => {
        let event_type: v8::Local<v8::Value> =
          v8::String::new(scope, event_type)
            .map(|s| s.into())
            .unwrap_or_else(|| v8::undefined(scope).into());
        let filename: v8::Local<v8::Value> = v8::String::new(scope, &filename)
          .map(|s| s.into())
          .unwrap_or_else(|| v8::undefined(scope).into());
        v8::Array::new_with_elements(scope, &[event_type, filename]).into()
      }
      None => v8::null(scope).into(),
    })
  }
}

#[op2]
async fn op_node_fs_watch_poll(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<NodeWatchEvent, NodeFsEventsError> {
  let resource = state
    .borrow()
    .resource_table
    .get::<NodeFsWatcherResource>(rid)?;
  let mut receiver = RcRef::map(&resource, |r| &r.handle.receiver)
    .borrow_mut()
    .await;
  loop {
    let cancel = RcRef::map(&resource, |r| &r.handle.cancel);
    let maybe_result = match receiver.recv().or_cancel(cancel).await {
      Ok(v) => v,
      // Closed by FSWatcher.close().
      Err(_canceled) => return Ok(NodeWatchEvent(None)),
    };
    match maybe_result {
      None => return Ok(NodeWatchEvent(None)),
      Some(Err(e)) => {
        return Err(node_notify_err(e, &resource.path).into());
      }
      Some(Ok(event)) => {
        let Some(event_path) = event.paths.first() else {
          continue;
        };
        // node reports the path relative to the watch root for recursive
        // watches, just the basename for non-recursive ones. notify reports
        // canonical paths on macOS (relativize against the symlink-resolved
        // root) but the original watched path on Windows (relativize against
        // that); try both, then fall back to the basename rather than emitting
        // a `..`-walk garbage path.
        let filename = if resource.recursive {
          relative_under(&resource.resolved_path, event_path)
            .or_else(|| {
              relative_under(
                &normalize_watch_path(PathBuf::from(&resource.path)),
                event_path,
              )
            })
            .unwrap_or_else(|| {
              event_path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default()
            })
        } else {
          event_path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default()
        };
        return Ok(NodeWatchEvent(Some((
          node_event_type(event.kind),
          filename,
        ))));
      }
    }
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
