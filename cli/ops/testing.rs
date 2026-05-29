// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use deno_error::JsErrorBox;
use deno_runtime::deno_permissions::ChildPermissionsArg;
use deno_runtime::deno_permissions::PermissionsContainer;
use uuid::Uuid;

use crate::tools::test::TestContainer;
use crate::tools::test::TestDescription;
use crate::tools::test::TestEvent;
use crate::tools::test::TestEventSender;
use crate::tools::test::TestFailure;
use crate::tools::test::TestLocation;
use crate::tools::test::TestStepDescription;
use crate::tools::test::TestStepResult;

deno_core::extension!(deno_test,
  ops = [
    op_pledge_test_permissions,
    op_restore_test_permissions,
    op_register_test,
    op_register_test_step,
    op_register_test_hook,
    op_test_get_origin,
    op_test_event_step_wait,
    op_test_event_step_result_ok,
    op_test_event_step_result_ignored,
    op_test_event_step_result_failed,
    op_test_event_exit,
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

#[op2(stack_trace)]
#[serde]
pub fn op_pledge_test_permissions(
  state: &mut OpState,
  #[serde] args: ChildPermissionsArg,
) -> Result<Uuid, deno_runtime::deno_permissions::ChildPermissionError> {
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
) -> Result<(), JsErrorBox> {
  match state.try_take::<PermissionsHolder>() {
    Some(permissions_holder) => {
      if token != permissions_holder.0 {
        panic!(
          "restore test permissions token does not match the stored token"
        );
      }

      let permissions = permissions_holder.1;
      state.put::<PermissionsContainer>(permissions);
      Ok(())
    }
    _ => Err(JsErrorBox::generic("no permissions to restore")),
  }
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[allow(clippy::too_many_arguments, reason = "op")]
#[op2]
fn op_register_test(
  state: &mut OpState,
  #[scoped] function: v8::Global<v8::Function>,
  #[string] name: String,
  ignore: bool,
  only: bool,
  sanitize_ops: bool,
  sanitize_resources: bool,
  #[string] file_name: String,
  #[smi] line_number: u32,
  #[smi] column_number: u32,
  #[buffer] ret_buf: &mut [u8],
  sanitize_only: bool,
  #[smi] timeout_ms: u32,
) -> Result<(), JsErrorBox> {
  if ret_buf.len() != 4 {
    return Err(JsErrorBox::type_error(format!(
      "Invalid ret_buf length: {}",
      ret_buf.len()
    )));
  }
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let timeout_ms = if timeout_ms == 0 {
    None
  } else {
    Some(timeout_ms)
  };
  let description = TestDescription {
    id,
    name,
    ignore,
    only,
    sanitize_only,
    sanitize_ops,
    sanitize_resources,
    origin: origin.clone(),
    location: TestLocation {
      file_name,
      line_number,
      column_number,
    },
    timeout_ms,
  };
  state
    .borrow_mut::<TestContainer>()
    .register(description, function)?;
  ret_buf.copy_from_slice(&(id as u32).to_le_bytes());
  Ok(())
}

#[op2]
fn op_register_test_hook(
  state: &mut OpState,
  #[string] hook_type: String,
  #[scoped] function: v8::Global<v8::Function>,
) -> Result<(), JsErrorBox> {
  let container = state.borrow_mut::<TestContainer>();
  container.register_hook(hook_type, function);
  Ok(())
}

#[op2]
#[string]
fn op_test_get_origin(state: &mut OpState) -> String {
  state.borrow::<ModuleSpecifier>().to_string()
}

#[op2(fast)]
#[smi]
#[allow(clippy::too_many_arguments, reason = "op")]
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
) -> usize {
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
  id
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

/// Called when a test calls `Deno.exit()` while the exit sanitizer is disabled
/// (`sanitizeExit: false`). Rather than letting the process terminate
/// immediately - which can drop buffered test output - we hand the exit code
/// off to the reporter (running on a separate thread) so it can print a
/// message, flush all output, and then exit the process with the given code.
///
/// This op does not return: once the `Exit` event has been queued, we park the
/// worker so it doesn't keep executing user code (which would otherwise resume
/// after `Deno.exit()` returned) while we wait for the process to be
/// terminated.
#[op2(fast)]
fn op_test_event_exit(state: &mut OpState, #[smi] exit_code: i32) {
  let sender = state.borrow_mut::<TestEventSender>();
  // `TestEvent::Exit` requires stdio sync, so sending it first drains any
  // pending stdout/stderr output to the reporter.
  if sender.send(TestEvent::Exit(exit_code)).is_ok() {
    loop {
      std::thread::park();
    }
  }

  // The channel is closed (the receiver has already finished), so there's
  // nobody left to print a message or flush - just exit directly.
  #[allow(
    clippy::disallowed_methods,
    reason = "a test called Deno.exit() with the exit sanitizer disabled"
  )]
  std::process::exit(exit_code);
}
