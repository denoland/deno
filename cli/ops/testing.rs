// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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
use deno_core::OpMetricsSummary;
use deno_core::OpMetricsSummaryTracker;
use deno_core::OpState;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::permissions::create_child_permissions;
use deno_runtime::permissions::ChildPermissionsArg;
use deno_runtime::permissions::PermissionsContainer;
use serde::Serialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use uuid::Uuid;

#[derive(Default)]
pub(crate) struct TestContainer(
  pub Vec<(TestDescription, v8::Global<v8::Function>)>,
);

deno_core::extension!(deno_test,
  ops = [
    op_pledge_test_permissions,
    op_restore_test_permissions,
    op_register_test,
    op_register_test_step,
    op_test_event_step_wait,
    op_test_event_step_result_ok,
    op_test_event_step_result_ignored,
    op_test_event_step_result_failed,
    op_test_op_sanitizer_collect,
    op_test_op_sanitizer_finish,
    op_test_op_sanitizer_report,
  ],
  options = {
    sender: TestEventSender,
  },
  state = |state, options| {
    state.put(options.sender);
    state.put(TestContainer::default());
    state.put(TestOpSanitizers::default());
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TestRegisterResult {
  id: usize,
  origin: String,
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[allow(clippy::too_many_arguments)]
#[op2]
#[string]
fn op_register_test(
  state: &mut OpState,
  #[global] function: v8::Global<v8::Function>,
  #[string] name: String,
  ignore: bool,
  only: bool,
  #[string] file_name: String,
  #[smi] line_number: u32,
  #[smi] column_number: u32,
  #[buffer] ret_buf: &mut [u8],
) -> Result<String, AnyError> {
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
    origin: origin.clone(),
    location: TestLocation {
      file_name,
      line_number,
      column_number,
    },
  };
  state
    .borrow_mut::<TestContainer>()
    .0
    .push((description.clone(), function));
  let sender = state.borrow_mut::<TestEventSender>();
  sender.send(TestEvent::Register(description)).ok();
  ret_buf.copy_from_slice(&(id as u32).to_le_bytes());
  Ok(origin)
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

#[derive(Default)]
struct TestOpSanitizers(HashMap<u32, TestOpSanitizerState>);

enum TestOpSanitizerState {
  Collecting { metrics: Vec<OpMetricsSummary> },
  Finished { report: Vec<TestOpSanitizerReport> },
}

fn try_collect_metrics(
  metrics: &OpMetricsSummaryTracker,
  force: bool,
  op_id_host_recv_msg: usize,
  op_id_host_recv_ctrl: usize,
) -> Result<std::cell::Ref<Vec<OpMetricsSummary>>, bool> {
  let metrics = metrics.per_op();
  let host_recv_msg = metrics
    .get(op_id_host_recv_msg)
    .map(OpMetricsSummary::has_outstanding_ops)
    .unwrap_or(false);
  let host_recv_ctrl = metrics
    .get(op_id_host_recv_ctrl)
    .map(OpMetricsSummary::has_outstanding_ops)
    .unwrap_or(false);

  for op_metric in metrics.iter() {
    if op_metric.has_outstanding_ops() && !force {
      return Err(host_recv_msg || host_recv_ctrl);
    }
  }
  Ok(metrics)
}

#[op2(fast)]
#[smi]
// Returns:
// 0 - success
// 1 - for more accurate results, spin event loop and call again with force=true
// 2 - for more accurate results, delay(1ms) and call again with force=true
fn op_test_op_sanitizer_collect(
  state: &mut OpState,
  #[smi] id: u32,
  force: bool,
  #[smi] op_id_host_recv_msg: usize,
  #[smi] op_id_host_recv_ctrl: usize,
) -> Result<u8, AnyError> {
  let metrics = state.borrow::<Rc<OpMetricsSummaryTracker>>();
  let metrics = match try_collect_metrics(
    metrics,
    force,
    op_id_host_recv_msg,
    op_id_host_recv_ctrl,
  ) {
    Ok(metrics) => metrics,
    Err(false) => {
      return Ok(1);
    }
    Err(true) => {
      return Ok(2);
    }
  }
  .clone();

  let op_sanitizers = state.borrow_mut::<TestOpSanitizers>();
  match op_sanitizers.0.entry(id) {
    Entry::Vacant(entry) => {
      entry.insert(TestOpSanitizerState::Collecting { metrics });
    }
    Entry::Occupied(_) => {
      return Err(generic_error(format!(
        "Test metrics already being collected for test id {id}",
      )));
    }
  }
  Ok(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TestOpSanitizerReport {
  id: usize,
  diff: i64,
}

#[op2(fast)]
#[smi]
// Returns:
// 0 - sanitizer finished with no pending ops
// 1 - for more accurate results, spin event loop and call again with force=true
// 2 - for more accurate results, delay(1ms) and call again with force=true
// 3 - sanitizer finished with pending ops, collect the report with op_test_op_sanitizer_report
fn op_test_op_sanitizer_finish(
  state: &mut OpState,
  #[smi] id: u32,
  force: bool,
  #[smi] op_id_host_recv_msg: usize,
  #[smi] op_id_host_recv_ctrl: usize,
) -> Result<u8, AnyError> {
  // Drop `fetch` connection pool at the end of a test
  state.try_take::<reqwest::Client>();
  let metrics = state.borrow::<Rc<OpMetricsSummaryTracker>>();

  // Generate a report of pending ops
  let report = {
    let after_metrics = match try_collect_metrics(
      metrics,
      force,
      op_id_host_recv_msg,
      op_id_host_recv_ctrl,
    ) {
      Ok(metrics) => metrics,
      Err(false) => {
        return Ok(1);
      }
      Err(true) => {
        return Ok(2);
      }
    };

    let op_sanitizers = state.borrow::<TestOpSanitizers>();
    let before_metrics = match op_sanitizers.0.get(&id) {
      Some(TestOpSanitizerState::Collecting { metrics }) => metrics,
      _ => {
        return Err(generic_error(format!(
          "Metrics not collected before for test id {id}",
        )));
      }
    };
    let mut report = vec![];

    for (id, (before, after)) in
      before_metrics.iter().zip(after_metrics.iter()).enumerate()
    {
      let async_pending_before =
        before.ops_dispatched_async - before.ops_completed_async;
      let async_pending_after =
        after.ops_dispatched_async - after.ops_completed_async;
      let diff = async_pending_after as i64 - async_pending_before as i64;
      if diff != 0 {
        report.push(TestOpSanitizerReport { id, diff });
      }
    }

    report
  };

  let op_sanitizers = state.borrow_mut::<TestOpSanitizers>();

  if report.is_empty() {
    op_sanitizers
      .0
      .remove(&id)
      .expect("TestOpSanitizerState::Collecting");
    Ok(0)
  } else {
    op_sanitizers
      .0
      .insert(id, TestOpSanitizerState::Finished { report })
      .expect("TestOpSanitizerState::Collecting");
    Ok(3)
  }
}

#[op2]
#[serde]
fn op_test_op_sanitizer_report(
  state: &mut OpState,
  #[smi] id: u32,
) -> Result<Vec<TestOpSanitizerReport>, AnyError> {
  let op_sanitizers = state.borrow_mut::<TestOpSanitizers>();
  match op_sanitizers.0.remove(&id) {
    Some(TestOpSanitizerState::Finished { report }) => Ok(report),
    _ => Err(generic_error(format!(
      "Metrics not finished collecting for test id {id}",
    ))),
  }
}
