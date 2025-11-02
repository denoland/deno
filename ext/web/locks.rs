// Copyright 2018-2025 the Deno authors. MIT license.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::LazyLock;

use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::op2;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

#[derive(
  serde::Serialize, serde::Deserialize, Debug, PartialEq, Clone, Copy,
)]
#[serde(rename_all = "camelCase")]
enum LockMode {
  Shared,
  Exclusive,
}

struct LockRequest {
  name: String,
  mode: LockMode,
  id: u64,
  client_id: String,
  tx: oneshot::Sender<u64>,
}

#[derive(Clone, serde::Serialize)]
struct Lock {
  name: String,
  mode: LockMode,
  id: u64,
  client_id: String,
}

enum LockTask {
  Request {
    name: String,
    mode: LockMode,
    if_available: bool,
    steal: bool,
    tx: oneshot::Sender<u64>,
    client_id: String,
  },
  Release {
    id: u64,
    tx: oneshot::Sender<()>,
  },
  Query {
    tx: oneshot::Sender<Query>,
  },
}

static LOCKS: LazyLock<mpsc::UnboundedSender<LockTask>> = LazyLock::new(|| {
  let (tx, mut rx) = mpsc::unbounded_channel();

  std::thread::spawn(|| {
    let rt = tokio::runtime::Builder::new_current_thread()
      .build()
      .unwrap();

    rt.block_on(async move {
      let mut held: Vec<Lock> = vec![];
      let mut queue_map = HashMap::new();
      let mut counter = 0;

      let grantable = |held: &Vec<Lock>, name: &str, mode| {
        if mode == LockMode::Exclusive && held.iter().any(|h| h.name == name) {
          return false;
        }
        if mode == LockMode::Shared
          && held
            .iter()
            .any(|h| h.name == name && h.mode != LockMode::Shared)
        {
          return false;
        }

        true
      };

      let process = |held: &mut Vec<Lock>,
                     queue: &mut VecDeque<LockRequest>| {
        while let Some(request) = queue.pop_front() {
          if !grantable(held, &request.name, request.mode) {
            queue.push_front(request);
            break;
          }

          if request.tx.send(request.id).is_ok() {
            held.push(Lock {
              name: request.name,
              mode: request.mode,
              id: request.id,
              client_id: request.client_id,
            });
          }
        }
      };

      while let Some(task) = rx.recv().await {
        match task {
          LockTask::Request {
            name,
            mode,
            if_available,
            steal,
            client_id,
            tx,
          } => {
            let queue = match queue_map.entry(name.clone()) {
              std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
              std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(VecDeque::new())
              }
            };

            counter += 1;
            let id = counter;

            if steal {
              let mut i = 0;
              while i < held.len() {
                if held[i].name == name {
                  held.remove(i);
                  // TODO: resolve p1?
                } else {
                  i += 1;
                }
              }
              queue.push_front(LockRequest {
                name,
                mode,
                id,
                client_id,
                tx,
              });
            } else {
              if if_available && !grantable(&held, &name, mode) {
                continue;
              }
              queue.push_back(LockRequest {
                name,
                mode,
                id,
                client_id,
                tx,
              });
            }

            process(&mut held, queue);
          }
          LockTask::Release { id, tx } => {
            if let Some(index) = held.iter().position(|l| l.id == id) {
              let lock = held.remove(index);
              let _ = tx.send(());
              if let Some(queue) = queue_map.get_mut(&lock.name) {
                process(&mut held, queue);
              }
            } else {
              let _ = tx.send(());
            }
          }
          LockTask::Query { tx } => {
            let mut query = Query {
              held: held
                .iter()
                .map(|h| QueryLock {
                  name: h.name.clone(),
                  mode: h.mode,
                  id: h.id,
                  client_id: h.client_id.clone(),
                })
                .collect(),
              pending: vec![],
            };
            for queue in queue_map.values() {
              for p in queue {
                query.pending.push(QueryLock {
                  name: p.name.clone(),
                  mode: p.mode,
                  id: p.id,
                  client_id: p.client_id.clone(),
                });
              }
            }
            query.held.sort_by_key(|h| h.id);
            query.pending.sort_by_key(|h| h.id);
            let _ = tx.send(query);
          }
        }
      }
    });
  });

  tx
});

struct LockResource(u64);

impl Resource for LockResource {}

#[op2(async)]
#[smi]
pub async fn op_lock_manager_request(
  state: Rc<RefCell<OpState>>,
  #[string] name: String,
  #[serde] mode: LockMode,
  if_available: bool,
  steal: bool,
) -> Option<ResourceId> {
  let client_id = std::thread::current().name()?.to_owned();

  let (tx, rx) = oneshot::channel();
  LOCKS
    .send(LockTask::Request {
      name,
      mode,
      if_available,
      steal,
      client_id,
      tx,
    })
    .unwrap();

  match rx.await {
    Ok(id) => {
      if id == 0 {
        None
      } else {
        let rid = state.borrow_mut().resource_table.add(LockResource(id));
        Some(rid)
      }
    }
    Err(_) => None,
  }
}

#[op2(async)]
pub async fn op_lock_manager_release(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) {
  let Ok(resource) =
    state.borrow_mut().resource_table.take::<LockResource>(rid)
  else {
    return;
  };
  let (tx, rx) = oneshot::channel();
  LOCKS
    .send(LockTask::Release { id: resource.0, tx })
    .unwrap();
  let _ = rx.await;
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct QueryLock {
  name: String,
  mode: LockMode,
  client_id: String,
  #[serde(skip)]
  id: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Query {
  held: Vec<QueryLock>,
  pending: Vec<QueryLock>,
}

#[op2(async)]
#[serde]
pub async fn op_lock_manager_query() -> Query {
  let (tx, rx) = oneshot::channel();
  LOCKS.send(LockTask::Query { tx }).unwrap();
  rx.await.unwrap()
}
