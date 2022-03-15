use crate::tools::bench::BenchEvent;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op_sync;
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
      (
        "op_pledge_test_permissions",
        op_sync(op_pledge_test_permissions),
      ),
      (
        "op_restore_test_permissions",
        op_sync(op_restore_test_permissions),
      ),
      ("op_get_bench_origin", op_sync(op_get_bench_origin)),
      ("op_dispatch_bench_event", op_sync(op_dispatch_bench_event)),
      ("op_bench_now", op_sync(op_bench_now)),
    ])
    .state(move |state| {
      state.put(sender.clone());
      Ok(())
    })
    .build()
}

#[derive(Clone)]
struct PermissionsHolder(Uuid, Permissions);

pub fn op_pledge_test_permissions(
  state: &mut OpState,
  args: ChildPermissionsArg,
  _: (),
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

pub fn op_restore_test_permissions(
  state: &mut OpState,
  token: Uuid,
  _: (),
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

fn op_get_bench_origin(
  state: &mut OpState,
  _: (),
  _: (),
) -> Result<String, AnyError> {
  Ok(state.borrow::<ModuleSpecifier>().to_string())
}

fn op_dispatch_bench_event(
  state: &mut OpState,
  event: BenchEvent,
  _: (),
) -> Result<(), AnyError> {
  let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
  sender.send(event).ok();

  Ok(())
}

fn op_bench_now(state: &mut OpState, _: (), _: ()) -> Result<u64, AnyError> {
  let ns = state.borrow::<time::Instant>().elapsed().as_nanos();
  let ns_u64 = u64::try_from(ns)?;
  Ok(ns_u64)
}
