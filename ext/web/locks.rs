// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;

use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::oneshot;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum LockError {
  #[class(type)]
  #[error("Invalid lock mode")]
  InvalidLockMode,
  #[class(type)]
  #[error("Lock manager not available")]
  LockManagerNotAvailable,
  #[class(inherit)]
  #[error(transparent)]
  Resource(#[from] deno_core::error::ResourceError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LockMode {
  Exclusive,
  Shared,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockInfo {
  pub name: String,
  pub mode: LockMode,
  pub client_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockManagerSnapshot {
  pub held: Vec<LockInfo>,
  pub pending: Vec<LockInfo>,
}

pub struct LockRequest {
  pub name: String,
  pub mode: LockMode,
  pub if_available: bool,
  pub steal: bool,
  pub client_id: String,
  pub sender: oneshot::Sender<Option<ResourceId>>,
}

pub struct LockResource {
  pub name: String,
  pub mode: LockMode,
  pub client_id: String,
}

impl Resource for LockResource {
  fn name(&self) -> Cow<str> {
    "lock".into()
  }

  fn close(self: Rc<Self>) {
    // Lock is automatically released when the resource is closed
  }
}

#[derive(Default)]
pub struct WebLockManager {
  /// Held locks: lock_name -> (mode, client_id, resource_id)
  held_locks: HashMap<String, Vec<(LockMode, String, ResourceId)>>,
  /// Pending lock requests
  pending_requests: VecDeque<LockRequest>,
}

impl WebLockManager {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn try_grant_lock(
    &mut self,
    name: String,
    mode: LockMode,
    if_available: bool,
    steal: bool,
    client_id: String,
  ) -> Option<LockResource> {
    // Check if we can grant the lock immediately
    if self.can_grant_lock(&name, &mode, if_available, steal) {
      if steal {
        self.steal_locks(&name, &client_id);
      }

      Some(LockResource {
        name,
        mode,
        client_id,
      })
    } else if if_available {
      None
    } else {
      // Queue the request
      let (sender, _receiver) = oneshot::channel();
      let request = LockRequest {
        name,
        mode,
        if_available,
        steal,
        client_id,
        sender,
      };

      self.pending_requests.push_back(request);
      None
    }
  }

  pub fn add_held_lock(
    &mut self,
    name: String,
    mode: LockMode,
    client_id: String,
    resource_id: ResourceId,
  ) {
    self.held_locks.entry(name).or_default().push((
      mode,
      client_id,
      resource_id,
    ));
  }

  pub fn release_lock(&mut self, resource_id: ResourceId) {
    // Find and remove the lock from held_locks
    for (_, locks) in self.held_locks.iter_mut() {
      if let Some(pos) = locks.iter().position(|(_, _, id)| *id == resource_id)
      {
        locks.remove(pos);
        break;
      }
    }

    // Clean up empty entries
    self.held_locks.retain(|_, locks| !locks.is_empty());

    // Process pending requests
    self.process_pending_requests();
  }

  pub fn query(&self) -> LockManagerSnapshot {
    let mut held = Vec::new();
    let mut pending = Vec::new();

    // Collect held locks
    for (name, locks) in &self.held_locks {
      for (mode, client_id, _) in locks {
        held.push(LockInfo {
          name: name.clone(),
          mode: mode.clone(),
          client_id: client_id.clone(),
        });
      }
    }

    // Collect pending requests
    for request in &self.pending_requests {
      pending.push(LockInfo {
        name: request.name.clone(),
        mode: request.mode.clone(),
        client_id: request.client_id.clone(),
      });
    }

    LockManagerSnapshot { held, pending }
  }

  pub fn can_grant_lock(
    &self,
    name: &str,
    mode: &LockMode,
    _if_available: bool,
    steal: bool,
  ) -> bool {
    if steal {
      return true;
    }

    let Some(held) = self.held_locks.get(name) else {
      return true;
    };

    match mode {
      LockMode::Exclusive => held.is_empty(),
      LockMode::Shared => held
        .iter()
        .all(|(held_mode, _, _)| matches!(held_mode, LockMode::Shared)),
    }
  }

  fn steal_locks(&mut self, name: &str, client_id: &str) {
    if let Some(locks) = self.held_locks.get_mut(name) {
      locks.retain(|(_, held_client_id, _)| held_client_id == client_id);
    }
  }

  fn process_pending_requests(&mut self) {
    // This would be more complex in a real implementation
    // For now, we just keep the queue
  }
}

#[op2(async)]
#[serde]
pub async fn op_lock_request(
  state: Rc<RefCell<OpState>>,
  #[string] name: String,
  #[string] mode: String,
  #[string] client_id: String,
  if_available: bool,
  steal: bool,
) -> Result<Option<ResourceId>, LockError> {
  let mode = match mode.as_str() {
    "exclusive" => LockMode::Exclusive,
    "shared" => LockMode::Shared,
    _ => return Err(LockError::InvalidLockMode),
  };

  let mut state_borrow = state.borrow_mut();

  let lock_resource = {
    let lock_manager = state_borrow
      .try_borrow_mut::<WebLockManager>()
      .ok_or(LockError::LockManagerNotAvailable)?;

    lock_manager.try_grant_lock(
      name.clone(),
      mode.clone(),
      if_available,
      steal,
      client_id.clone(),
    )
  };

  if let Some(lock_resource) = lock_resource {
    let resource_id = state_borrow.resource_table.add(lock_resource);

    // Update the lock manager with the resource ID
    let lock_manager = state_borrow
      .try_borrow_mut::<WebLockManager>()
      .ok_or(LockError::LockManagerNotAvailable)?;

    lock_manager.add_held_lock(name, mode, client_id, resource_id);

    Ok(Some(resource_id))
  } else {
    Ok(None)
  }
}

#[op2]
#[serde]
pub fn op_lock_release(
  state: &mut OpState,
  #[smi] resource_id: ResourceId,
) -> Result<(), LockError> {
  // Remove the resource
  let resource = state.resource_table.take::<LockResource>(resource_id)?;
  resource.close();

  // Update lock manager
  let lock_manager = state
    .try_borrow_mut::<WebLockManager>()
    .ok_or(LockError::LockManagerNotAvailable)?;

  lock_manager.release_lock(resource_id);

  Ok(())
}

#[op2]
#[serde]
pub fn op_lock_query(
  state: &mut OpState,
) -> Result<LockManagerSnapshot, LockError> {
  let lock_manager = state
    .try_borrow::<WebLockManager>()
    .ok_or(LockError::LockManagerNotAvailable)?;

  Ok(lock_manager.query())
}
