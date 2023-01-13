use crate::tools::bench::BenchDescription;
use crate::tools::bench::BenchEvent;
use crate::tools::test::TestFilter;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::create_child_permissions;
use deno_runtime::permissions::ChildPermissionsArg;
use deno_runtime::permissions::PermissionsContainer;
use serde::Deserialize;
use serde::Serialize;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

pub fn init(
  sender: UnboundedSender<BenchEvent>,
  filter: TestFilter,
) -> Extension {
  Extension::builder("deno_bench")
    .ops(vec![
      op_pledge_test_permissions::decl(),
      op_restore_test_permissions::decl(),
      op_get_bench_origin::decl(),
      op_register_bench::decl(),
      op_dispatch_bench_event::decl(),
      op_bench_now::decl(),
    ])
    .state(move |state| {
      state.put(sender.clone());
      state.put(filter.clone());
      Ok(())
    })
    .build()
}

#[derive(Clone)]
struct PermissionsHolder(Uuid, PermissionsContainer);

#[op]
pub fn op_pledge_test_permissions(
  state: &mut OpState,
  args: ChildPermissionsArg,
) -> Result<Uuid, AnyError> {
  let token = Uuid::new_v4();
  let parent_permissions = state.borrow_mut::<PermissionsContainer>();
  let worker_permissions = {
    let mut parent_permissions = parent_permissions.0.lock();
    let perms = create_child_permissions(&mut parent_permissions, args)?;
    PermissionsContainer::new(perms)
  };
  let parent_permissions = parent_permissions.clone();

  if state.try_take::<PermissionsHolder>().is_some() {
    panic!("pledge test permissions called before restoring previous pledge");
  }

  state.put::<PermissionsHolder>(PermissionsHolder(token, parent_permissions));

  // NOTE: This call overrides current permission set for the worker
  state.put::<PermissionsContainer>(worker_permissions);

  Ok(token)
}

#[op]
pub fn op_restore_test_permissions(
  state: &mut OpState,
  token: Uuid,
) -> Result<(), AnyError> {
  if let Some(permissions_holder) = state.try_take::<PermissionsHolder>() {
    if token != permissions_holder.0 {
      panic!("restore test permissions token does not match the stored token");
    }

    let permissions = permissions_holder.1;
    state.put::<PermissionsContainer>(permissions);
    Ok(())
  } else {
    Err(generic_error("no permissions to restore"))
  }
}

#[op]
fn op_get_bench_origin(state: &mut OpState) -> String {
  state.borrow::<ModuleSpecifier>().to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchInfo {
  name: String,
  origin: String,
  baseline: bool,
  group: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchRegisterResult {
  id: usize,
  filtered_out: bool,
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[op]
fn op_register_bench(
  state: &mut OpState,
  info: BenchInfo,
) -> Result<BenchRegisterResult, AnyError> {
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let filter = state.borrow::<TestFilter>().clone();
  let filtered_out = !filter.includes(&info.name);
  let description = BenchDescription {
    id,
    name: info.name,
    origin: info.origin,
    baseline: info.baseline,
    group: info.group,
  };
  let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
  sender.send(BenchEvent::Register(description)).ok();
  Ok(BenchRegisterResult { id, filtered_out })
}

#[op]
fn op_dispatch_bench_event(state: &mut OpState, event: BenchEvent) {
  let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
  sender.send(event).ok();
}

#[op]
fn op_bench_now(state: &mut OpState) -> Result<u64, AnyError> {
  let ns = state.borrow::<time::Instant>().elapsed().as_nanos();
  let ns_u64 = u64::try_from(ns)?;
  Ok(ns_u64)
}
