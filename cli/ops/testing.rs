// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use crate::tools::test::TestContainer;
use crate::tools::test::TestDescription;
use crate::tools::test::TestEvent;
use crate::tools::test::TestEventSender;
use crate::tools::test::TestFailure;
use crate::tools::test::TestLocation;
use crate::tools::test::TestStepDescription;
use crate::tools::test::TestStepResult;

use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::deno_permissions::ChildPermissionsArg;
use deno_runtime::deno_permissions::PermissionsContainer;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use uuid::Uuid;

deno_core::extension!(deno_test,
  ops = [
    op_pledge_test_permissions,
    op_restore_test_permissions,
    op_register_test,
    op_register_test_step,
    op_test_get_origin,
    op_test_event_step_wait,
    op_test_event_step_result_ok,
    op_test_event_step_result_ignored,
    op_test_event_step_result_failed,
  ],
  options = {
    sender: TestEventSender,
  },
  state = |state, options| {
    state.put(options.sender);
    state.put(TestContainer::default());
  },
);

#[derive(Clone)]
struct PermissionsHolder(Uuid, PermissionsContainer);

#[op2]
#[serde]
pub fn op_pledge_test_permissions(
  state: &mut OpState,
  #[serde] args: ChildPermissionsArg,
) -> Result<Uuid, AnyError> {
  let token = Uuid::new_v4();
  let parent_permissions = state.borrow_mut::<PermissionsContainer>();
  let worker_permissions = parent_permissions.create_child_permissions(args)?;
  let parent_permissions = parent_permissions.clone();

  if state.try_take::<PermissionsHolder>().is_some() {
    panic!("pledge test permissions called before restoring previous pledge");
  }
  state.put::<PermissionsHolder>(PermissionsHolder(token, parent_permissions));

  // NOTE: This call overrides current permission set for the worker
  state.put::<PermissionsContainer>(worker_permissions);

  Ok(token)
}

#[op2]
pub fn op_restore_test_permissions(
  state: &mut OpState,
  #[serde] token: Uuid,
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

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[allow(clippy::too_many_arguments)]
#[op2]
fn op_register_test(
  state: &mut OpState,
  #[global] function: v8::Global<v8::Function>,
  #[string] name: String,
  ignore: bool,
  only: bool,
  sanitize_ops: bool,
  sanitize_resources: bool,
  #[string] file_name: String,
  #[smi] line_number: u32,
  #[smi] column_number: u32,
  #[buffer] ret_buf: &mut [u8],
) -> Result<(), AnyError> {
  if ret_buf.len() != 4 {
    return Err(type_error(format!(
      "Invalid ret_buf length: {}",
      ret_buf.len()
    )));
  }
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let description = TestDescription {
    id,
    name,
    ignore,
    only,
    sanitize_ops,
    sanitize_resources,
    origin: origin.clone(),
    location: TestLocation {
      file_name,
      line_number,
      column_number,
    },
  };
  let container = state.borrow_mut::<TestContainer>();
  container.register(description, function);
  ret_buf.copy_from_slice(&(id as u32).to_le_bytes());
  Ok(())
}

#[op2]
#[string]
fn op_test_get_origin(state: &mut OpState) -> String {
  state.borrow::<ModuleSpecifier>().to_string()
}

#[op2(fast)]
#[smi]
#[allow(clippy::too_many_arguments)]
fn op_register_test_step(
  state: &mut OpState,
  #[string] name: String,
  #[string] file_name: String,
  #[smi] line_number: u32,
  #[smi] column_number: u32,
  #[smi] level: usize,
  #[smi] parent_id: usize,
  #[smi] root_id: usize,
  #[string] root_name: String,
) -> Result<usize, AnyError> {
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let description = TestStepDescription {
    id,
    name,
    origin: origin.clone(),
    location: TestLocation {
      file_name,
      line_number,
      column_number,
    },
    level,
    parent_id,
    root_id,
    root_name,
  };
  let sender = state.borrow_mut::<TestEventSender>();
  sender.send(TestEvent::StepRegister(description)).ok();
  Ok(id)
}

#[op2(fast)]
fn op_test_event_step_wait(state: &mut OpState, #[smi] id: usize) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender.send(TestEvent::StepWait(id)).ok();
}

#[op2(fast)]
fn op_test_event_step_result_ok(
  state: &mut OpState,
  #[smi] id: usize,
  #[smi] duration: u64,
) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender
    .send(TestEvent::StepResult(id, TestStepResult::Ok, duration))
    .ok();
}

#[op2(fast)]
fn op_test_event_step_result_ignored(
  state: &mut OpState,
  #[smi] id: usize,
  #[smi] duration: u64,
) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender
    .send(TestEvent::StepResult(id, TestStepResult::Ignored, duration))
    .ok();
}

#[op2]
fn op_test_event_step_result_failed(
  state: &mut OpState,
  #[smi] id: usize,
  #[serde] failure: TestFailure,
  #[smi] duration: u64,
) {
  let sender = state.borrow_mut::<TestEventSender>();
  sender
    .send(TestEvent::StepResult(
      id,
      TestStepResult::Failed(failure),
      duration,
    ))
    .ok();
}
