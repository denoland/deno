// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::tools::test::TestDescription;
use crate::tools::test::TestEvent;
use crate::tools::test::TestEventSender;
use crate::tools::test::TestLocation;
use crate::tools::test::TestStepDescription;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use deno_core::OpMetrics;
use deno_core::OpState;
use deno_runtime::permissions::create_child_permissions;
use deno_runtime::permissions::ChildPermissionsArg;
use deno_runtime::permissions::PermissionsContainer;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::cell::Ref;
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
    op_dispatch_test_event,
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
    state.put(TestOpSanitizerState::None);
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestInfo<'s> {
  #[serde(rename = "fn")]
  function: serde_v8::Value<'s>,
  name: String,
  #[serde(default)]
  ignore: bool,
  #[serde(default)]
  only: bool,
  location: TestLocation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TestRegisterResult {
  id: usize,
  origin: String,
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[op2]
#[serde]
fn op_register_test<'a>(
  scope: &mut v8::HandleScope<'a>,
  state: &mut OpState,
  #[serde] info: TestInfo<'a>,
) -> Result<TestRegisterResult, AnyError> {
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let description = TestDescription {
    id,
    name: info.name,
    ignore: info.ignore,
    only: info.only,
    origin: origin.clone(),
    location: info.location,
  };
  let function: v8::Local<v8::Function> = info.function.v8_value.try_into()?;
  let function = v8::Global::new(scope, function);
  state
    .borrow_mut::<TestContainer>()
    .0
    .push((description.clone(), function));
  let mut sender = state.borrow::<TestEventSender>().clone();
  sender.send(TestEvent::Register(description)).ok();
  Ok(TestRegisterResult { id, origin })
}

fn deserialize_parent<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
  D: Deserializer<'de>,
{
  #[derive(Deserialize)]
  struct Parent {
    id: usize,
  }
  Ok(Parent::deserialize(deserializer)?.id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestStepInfo {
  name: String,
  location: TestLocation,
  level: usize,
  #[serde(rename = "parent")]
  #[serde(deserialize_with = "deserialize_parent")]
  parent_id: usize,
  root_id: usize,
  root_name: String,
}

#[op2]
#[serde]
fn op_register_test_step(
  state: &mut OpState,
  #[serde] info: TestStepInfo,
) -> Result<TestRegisterResult, AnyError> {
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let description = TestStepDescription {
    id,
    name: info.name,
    origin: origin.clone(),
    location: info.location,
    level: info.level,
    parent_id: info.parent_id,
    root_id: info.root_id,
    root_name: info.root_name,
  };
  let mut sender = state.borrow::<TestEventSender>().clone();
  sender.send(TestEvent::StepRegister(description)).ok();
  Ok(TestRegisterResult { id, origin })
}

#[op2]
fn op_dispatch_test_event(
  state: &mut OpState,
  #[serde] event: TestEvent,
) -> Result<(), AnyError> {
  assert!(
    matches!(event, TestEvent::StepWait(_) | TestEvent::StepResult(..)),
    "Only step wait/result events are expected from JS."
  );
  let mut sender = state.borrow::<TestEventSender>().clone();
  sender.send(event).ok();
  Ok(())
}

enum TestOpSanitizerState {
  None,
  Collecting {
    test_id: u32,
    metrics: Vec<OpMetrics>,
  },
  Finished {
    test_id: u32,
    report: Vec<TestOpSanitizerReport>,
  },
}

fn try_collect_metrics(
  state: &OpState,
  force: bool,
  op_id_host_recv_msg: usize,
  op_id_host_recv_ctrl: usize,
) -> Result<Ref<Vec<OpMetrics>>, bool> {
  let metrics = state.tracker.per_op();
  for op_metric in &*metrics {
    let has_pending_ops = op_metric.ops_dispatched_async
      + op_metric.ops_dispatched_async_unref
      > op_metric.ops_completed_async + op_metric.ops_completed_async_unref;
    if has_pending_ops && !force {
      let host_recv_msg = metrics
        .get(op_id_host_recv_msg)
        .map(|op_metric| {
          op_metric.ops_dispatched_async + op_metric.ops_dispatched_async_unref
            > op_metric.ops_completed_async
              + op_metric.ops_completed_async_unref
        })
        .unwrap_or(false);
      let host_recv_ctrl = metrics
        .get(op_id_host_recv_ctrl)
        .map(|op_metric| {
          op_metric.ops_dispatched_async + op_metric.ops_dispatched_async_unref
            > op_metric.ops_completed_async
              + op_metric.ops_completed_async_unref
        })
        .unwrap_or(false);
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
  let metrics = {
    let metrics = match try_collect_metrics(
      state,
      force,
      op_id_host_recv_msg,
      op_id_host_recv_ctrl,
    ) {
      Ok(metrics) => metrics,
      Err(true) => {
        return Ok(1);
      }
      Err(false) => {
        return Ok(2);
      }
    };
    metrics.clone()
  };
  let op_sanitizer_state = state.borrow_mut::<TestOpSanitizerState>();
  if !matches!(op_sanitizer_state, TestOpSanitizerState::None) {
    return Err(generic_error("Test metrics already being collected"));
  }
  *op_sanitizer_state = TestOpSanitizerState::Collecting {
    test_id: id,
    metrics,
  };
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
  let report = {
    let after_metrics = match try_collect_metrics(
      state,
      force,
      op_id_host_recv_msg,
      op_id_host_recv_ctrl,
    ) {
      Ok(metrics) => metrics,
      Err(true) => {
        return Ok(1);
      }
      Err(false) => {
        return Ok(2);
      }
    };

    let op_sanitizer_state = state.borrow::<TestOpSanitizerState>();
    let before_metrics = match op_sanitizer_state {
      TestOpSanitizerState::Collecting { test_id, metrics }
        if *test_id == id =>
      {
        metrics
      }
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
      let async_pending_before = before.ops_dispatched_async
        + before.ops_dispatched_async_unref
        - before.ops_completed_async
        - before.ops_completed_async_unref;
      let async_pending_after = after.ops_dispatched_async
        + after.ops_dispatched_async_unref
        - after.ops_completed_async
        - after.ops_completed_async_unref;
      let diff = async_pending_after as i64 - async_pending_before as i64;
      if diff != 0 {
        report.push(TestOpSanitizerReport { id, diff });
      }
    }

    report
  };

  let op_sanitizer_state = state.borrow_mut::<TestOpSanitizerState>();

  if report.is_empty() {
    *op_sanitizer_state = TestOpSanitizerState::None;
    Ok(0)
  } else {
    *op_sanitizer_state = TestOpSanitizerState::Finished {
      test_id: id,
      report,
    };
    Ok(3)
  }
}

#[op2]
#[serde]
fn op_test_op_sanitizer_report(
  state: &mut OpState,
  #[smi] id: u32,
) -> Result<Vec<TestOpSanitizerReport>, AnyError> {
  let op_sanitizer_state = state.borrow_mut::<TestOpSanitizerState>();
  match std::mem::replace(op_sanitizer_state, TestOpSanitizerState::None) {
    TestOpSanitizerState::Finished { test_id, report } if test_id == id => {
      Ok(report)
    }
    _ => Err(generic_error(format!("No report prepared for {id}"))),
  }
}
