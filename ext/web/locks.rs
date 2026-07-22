// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use deno_core::FromV8;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ToV8;
use deno_core::op2;
use tokio::sync::oneshot;

#[derive(ToV8, FromV8, Debug, PartialEq, Clone, Copy)]
enum LockMode {
  Shared,
  Exclusive,
}

struct HeldLock {
  name: String,
  mode: LockMode,
  id: u64,
  client_id: String,
  // Fires when the lock is stolen, so the holder's `request()` promise can
  // reject with an AbortError. `None` until the holder starts awaiting it.
  broken_tx: Option<oneshot::Sender<()>>,
}

struct PendingRequest {
  name: String,
  mode: LockMode,
  id: u64,
  client_id: String,
  tx: oneshot::Sender<bool>,
}

struct LockState {
  held: Vec<HeldLock>,
  queues: HashMap<String, VecDeque<PendingRequest>>,
  counter: u64,
}

static LOCK_STATE: LazyLock<Mutex<LockState>> = LazyLock::new(|| {
  Mutex::new(LockState {
    held: vec![],
    queues: HashMap::new(),
    counter: 0,
  })
});

static CLIENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

struct LockClientId(String);

fn get_client_id(state: &mut OpState) -> String {
  if let Some(id) = state.try_borrow::<LockClientId>() {
    return id.0.clone();
  }
  let id = CLIENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
  let client_id = format!("{id}");
  state.put(LockClientId(client_id.clone()));
  client_id
}

fn grantable(held: &[HeldLock], name: &str, mode: LockMode) -> bool {
  match mode {
    LockMode::Exclusive => !held.iter().any(|h| h.name == name),
    LockMode::Shared => !held
      .iter()
      .any(|h| h.name == name && h.mode == LockMode::Exclusive),
  }
}

fn process_queue(state: &mut LockState, name: &str) {
  let queue = match state.queues.get_mut(name) {
    Some(q) => q,
    None => return,
  };

  while let Some(front) = queue.front() {
    if !grantable(&state.held, name, front.mode) {
      break;
    }
    let request = queue.pop_front().unwrap();
    if request.tx.send(true).is_ok() {
      state.held.push(HeldLock {
        name: request.name,
        mode: request.mode,
        id: request.id,
        client_id: request.client_id,
        broken_tx: None,
      });
    }
    // If send fails (receiver dropped / cancelled), skip this request
  }
}

fn release_lock(state: &mut LockState, id: u64) {
  if let Some(pos) = state.held.iter().position(|h| h.id == id) {
    let lock = state.held.remove(pos);
    process_queue(state, &lock.name);
  }
}

fn cancel_request(state: &mut LockState, id: u64) {
  for queue in state.queues.values_mut() {
    if let Some(pos) = queue.iter().position(|r| r.id == id) {
      let req = queue.remove(pos).unwrap();
      let _ = req.tx.send(false);
      return;
    }
  }
}

// Resource for a held lock — releases the lock on drop
struct HeldLockResource {
  id: u64,
  // Resolves when the lock is stolen (see `op_lock_manager_await_steal`).
  broken_rx: RefCell<Option<oneshot::Receiver<()>>>,
}

// Sets up the steal-notification channel for a freshly granted lock: the
// sender is stored on the held lock in the global state and the receiver is
// stored on the resource handed back to the holder.
fn make_held_lock_resource(state: &mut LockState, id: u64) -> HeldLockResource {
  let (tx, rx) = oneshot::channel();
  if let Some(held) = state.held.iter_mut().find(|h| h.id == id) {
    held.broken_tx = Some(tx);
  }
  HeldLockResource {
    id,
    broken_rx: RefCell::new(Some(rx)),
  }
}

impl Drop for HeldLockResource {
  fn drop(&mut self) {
    let mut state = LOCK_STATE.lock().unwrap();
    release_lock(&mut state, self.id);
  }
}

impl Resource for HeldLockResource {
  fn name(&self) -> Cow<'_, str> {
    "webLock".into()
  }
}

// Resource for a pending lock request — cancels the request on drop
struct PendingLockResource {
  rx: RefCell<Option<oneshot::Receiver<bool>>>,
  id: u64,
}

impl Drop for PendingLockResource {
  fn drop(&mut self) {
    let mut state = LOCK_STATE.lock().unwrap();
    cancel_request(&mut state, self.id);
  }
}

impl Resource for PendingLockResource {
  fn name(&self) -> Cow<'_, str> {
    "pendingWebLock".into()
  }
}

/// Result from op_lock_manager_request.
/// status: 0 = granted, 1 = pending, 2 = not available (ifAvailable)
#[derive(ToV8)]
struct LockRequestResult {
  status: u8,
  rid: ResourceId,
}

/// Synchronous op: registers a lock request.
/// Returns immediately with either a granted lock, a pending handle, or
/// a not-available indicator.
#[op2]
pub fn op_lock_manager_request(
  state: &mut OpState,
  #[string] name: String,
  #[scoped] mode: LockMode,
  if_available: bool,
  steal: bool,
) -> LockRequestResult {
  let client_id = get_client_id(state);
  let mut ls = LOCK_STATE.lock().unwrap();

  ls.counter += 1;
  let id = ls.counter;

  if steal {
    // Notify the current holders that their lock has been broken, then remove
    // all held locks for this name. The holders' `request()` promises reject
    // with an AbortError. Pending requests are left untouched: the stealing
    // request jumps to the front of the queue (below) and is granted ahead of
    // them, but they remain queued and are granted once the steal is released.
    for held in ls.held.iter_mut().filter(|h| h.name == name) {
      if let Some(tx) = held.broken_tx.take() {
        let _ = tx.send(());
      }
    }
    ls.held.retain(|h| h.name != name);
  } else if if_available && !grantable(&ls.held, &name, mode) {
    return LockRequestResult { status: 2, rid: 0 };
  }

  let (tx, mut rx) = oneshot::channel();
  let name_clone = name.clone();
  let queue = ls.queues.entry(name).or_default();

  if steal {
    queue.push_front(PendingRequest {
      name: name_clone.clone(),
      mode,
      id,
      client_id,
      tx,
    });
  } else {
    queue.push_back(PendingRequest {
      name: name_clone.clone(),
      mode,
      id,
      client_id,
      tx,
    });
  }

  process_queue(&mut ls, &name_clone);

  // Check if granted immediately (process_queue sends synchronously
  // through the oneshot before we check)
  match rx.try_recv() {
    Ok(true) => {
      let resource = make_held_lock_resource(&mut ls, id);
      drop(ls);
      let rid = state.resource_table.add(resource);
      LockRequestResult { status: 0, rid }
    }
    _ => {
      drop(ls);
      let rid = state.resource_table.add(PendingLockResource {
        rx: RefCell::new(Some(rx)),
        id,
      });
      LockRequestResult { status: 1, rid }
    }
  }
}

/// Async op: waits for a pending lock request to be granted.
/// Returns the held lock resource ID, or null if the request was
/// cancelled/stolen.
#[op2]
#[smi]
pub async fn op_lock_manager_await_lock(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Option<ResourceId> {
  let (rx, lock_id) = {
    let state = state.borrow();
    let pending = state.resource_table.get::<PendingLockResource>(rid).ok()?;
    let rx = pending.rx.borrow_mut().take()?;
    (rx, pending.id)
  };

  let granted = rx.await.unwrap_or(false);

  // Clean up the pending resource (Drop will no-op since request
  // is already resolved or cancelled in global state)
  let _ = state
    .borrow_mut()
    .resource_table
    .take::<PendingLockResource>(rid);

  if granted {
    let resource = {
      let mut ls = LOCK_STATE.lock().unwrap();
      make_held_lock_resource(&mut ls, lock_id)
    };
    let held_rid = state.borrow_mut().resource_table.add(resource);
    Some(held_rid)
  } else {
    None
  }
}

/// Async op: resolves while a held lock is alive. Returns `true` if the lock
/// was stolen (the holder's `request()` promise must reject with AbortError),
/// or `false` when the lock is released normally. Keeping this op pending also
/// keeps the event loop alive for as long as the lock is held.
#[op2]
pub async fn op_lock_manager_await_steal(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> bool {
  let rx = {
    let state = state.borrow();
    let Ok(held) = state.resource_table.get::<HeldLockResource>(rid) else {
      return false;
    };
    let Some(rx) = held.broken_rx.borrow_mut().take() else {
      return false;
    };
    rx
  };
  // `Ok(())` means the lock was stolen; `Err(_)` means the sender was dropped
  // because the lock was released normally.
  rx.await.is_ok()
}

/// Cancels a pending lock request (used by AbortSignal).
#[op2(fast)]
pub fn op_lock_manager_cancel(state: &mut OpState, #[smi] rid: ResourceId) {
  if let Ok(pending) = state.resource_table.get::<PendingLockResource>(rid) {
    let id = pending.id;
    drop(pending);
    let mut ls = LOCK_STATE.lock().unwrap();
    cancel_request(&mut ls, id);
  }
}

/// Releases a held lock.
#[op2(fast)]
pub fn op_lock_manager_release(state: &mut OpState, #[smi] rid: ResourceId) {
  // Taking the resource drops it, which triggers release_lock via Drop
  let _ = state.resource_table.take::<HeldLockResource>(rid);
}

#[derive(ToV8)]
struct QueryLock {
  name: String,
  mode: LockMode,
  client_id: String,
}

#[derive(ToV8)]
struct Query {
  held: Vec<QueryLock>,
  pending: Vec<QueryLock>,
}

#[op2]
pub fn op_lock_manager_query() -> Query {
  let ls = LOCK_STATE.lock().unwrap();
  let held: Vec<QueryLock> = ls
    .held
    .iter()
    .map(|h| QueryLock {
      name: h.name.clone(),
      mode: h.mode,
      client_id: h.client_id.clone(),
    })
    .collect();
  let mut pending: Vec<QueryLock> = vec![];
  for queue in ls.queues.values() {
    for p in queue {
      pending.push(QueryLock {
        name: p.name.clone(),
        mode: p.mode,
        client_id: p.client_id.clone(),
      });
    }
  }
  Query { held, pending }
}
