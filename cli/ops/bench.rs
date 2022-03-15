use crate::tools::bench::BenchEvent;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::create_child_permissions;
use deno_runtime::permissions::ChildPermissionsArg;
use deno_runtime::permissions::Permissions;
use std::time;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

pub fn init(sender: UnboundedSender<BenchEvent>) -> Extension {
  Extension::builder()
    .ops(vec![
      op_pledge_test_permissions::decl(),
      op_restore_test_permissions::decl(),
      op_get_bench_origin::decl(),
      op_dispatch_bench_event::decl(),
      op_bench_now::decl(),
    ])
    .state(move |state| {
      state.put(sender.clone());
      Ok(())
    })
    .build()
}

#[derive(Clone)]
struct PermissionsHolder(Uuid, Permissions);

#[op]
pub fn op_pledge_test_permissions(
  state: &mut OpState,
  args: ChildPermissionsArg,
) -> Result<Uuid, AnyError> {
  let token = Uuid::new_v4();
  let parent_permissions = state.borrow_mut::<Permissions>();
  let worker_permissions = create_child_permissions(parent_permissions, args)?;
  let parent_permissions = parent_permissions.clone();

  state.put::<PermissionsHolder>(PermissionsHolder(token, parent_permissions));

  // NOTE: This call overrides current permission set for the worker
  state.put::<Permissions>(worker_permissions);

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
    state.put::<Permissions>(permissions);
    Ok(())
  } else {
    Err(generic_error("no permissions to restore"))
  }
}

#[op]
fn op_get_bench_origin(state: &mut OpState) -> Result<String, AnyError> {
  Ok(state.borrow::<ModuleSpecifier>().to_string())
}

#[op]
fn op_dispatch_bench_event(
  state: &mut OpState,
  event: BenchEvent,
) -> Result<(), AnyError> {
  let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
  sender.send(event).ok();

  Ok(())
}

#[op]
fn op_bench_now(state: &mut OpState) -> Result<u64, AnyError> {
  let ns = state.borrow::<time::Instant>().elapsed().as_nanos();
  let ns_u64 = u64::try_from(ns)?;
  Ok(ns_u64)
}
