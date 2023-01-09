// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::tools::test::FailFastTracker;
use crate::tools::test::TestDescription;
use crate::tools::test::TestEvent;
use crate::tools::test::TestEventSender;
use crate::tools::test::TestFilter;
use crate::tools::test::TestLocation;
use crate::tools::test::TestResult;
use crate::tools::test::TestStepDescription;

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
use serde::Deserializer;
use serde::Serialize;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use uuid::Uuid;

pub fn init(
  sender: TestEventSender,
  fail_fast_tracker: FailFastTracker,
  filter: TestFilter,
) -> Extension {
  Extension::builder("deno_test")
    .ops(vec![
      op_pledge_test_permissions::decl(),
      op_restore_test_permissions::decl(),
      op_get_test_origin::decl(),
      op_register_test::decl(),
      op_register_test_step::decl(),
      op_dispatch_test_event::decl(),
      op_tests_should_stop::decl(),
    ])
    .state(move |state| {
      state.put(sender.clone());
      state.put(fail_fast_tracker.clone());
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
fn op_get_test_origin(state: &mut OpState) -> Result<String, AnyError> {
  Ok(state.borrow::<ModuleSpecifier>().to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestInfo {
  name: String,
  origin: String,
  location: TestLocation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TestRegisterResult {
  id: usize,
  filtered_out: bool,
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[op]
fn op_register_test(
  state: &mut OpState,
  info: TestInfo,
) -> Result<TestRegisterResult, AnyError> {
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let filter = state.borrow::<TestFilter>().clone();
  let filtered_out = !filter.includes(&info.name);
  let description = TestDescription {
    id,
    name: info.name,
    origin: info.origin,
    location: info.location,
  };
  let mut sender = state.borrow::<TestEventSender>().clone();
  sender.send(TestEvent::Register(description)).ok();
  Ok(TestRegisterResult { id, filtered_out })
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
  origin: String,
  location: TestLocation,
  level: usize,
  #[serde(rename = "parent")]
  #[serde(deserialize_with = "deserialize_parent")]
  parent_id: usize,
  root_id: usize,
  root_name: String,
}

#[op]
fn op_register_test_step(
  state: &mut OpState,
  info: TestStepInfo,
) -> Result<TestRegisterResult, AnyError> {
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let description = TestStepDescription {
    id,
    name: info.name,
    origin: info.origin,
    location: info.location,
    level: info.level,
    parent_id: info.parent_id,
    root_id: info.root_id,
    root_name: info.root_name,
  };
  let mut sender = state.borrow::<TestEventSender>().clone();
  sender.send(TestEvent::StepRegister(description)).ok();
  Ok(TestRegisterResult {
    id,
    filtered_out: false,
  })
}

#[op]
fn op_dispatch_test_event(
  state: &mut OpState,
  event: TestEvent,
) -> Result<(), AnyError> {
  if matches!(
    event,
    TestEvent::Result(_, TestResult::Cancelled | TestResult::Failed(_), _)
  ) {
    state.borrow::<FailFastTracker>().add_failure();
  }
  let mut sender = state.borrow::<TestEventSender>().clone();
  sender.send(event).ok();
  Ok(())
}

#[op]
fn op_tests_should_stop(state: &mut OpState) -> bool {
  state.borrow::<FailFastTracker>().should_stop()
}
