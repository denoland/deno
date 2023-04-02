// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use crate::tools::test::TestDescription;
use crate::tools::test::TestEvent;
use crate::tools::test::TestEventSender;
use crate::tools::test::TestLocation;
use crate::tools::test::TestStepDescription;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::serde_v8;
use deno_core::v8;
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
  ],
  options = {
    sender: TestEventSender,
  },
  state = |state, options| {
    state.put(options.sender);
    state.put(TestContainer::default());
  },
  customizer = |ext: &mut deno_core::ExtensionBuilder| {
    ext.force_op_registration();
  },
);

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestInfo<'s> {
  #[serde(rename = "fn")]
  function: serde_v8::Value<'s>,
  name: String,
  ignore: bool,
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

#[op(v8)]
fn op_register_test<'a>(
  scope: &mut v8::HandleScope<'a>,
  state: &mut OpState,
  info: TestInfo<'a>,
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

#[op]
fn op_register_test_step(
  state: &mut OpState,
  info: TestStepInfo,
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

#[op]
fn op_dispatch_test_event(
  state: &mut OpState,
  event: TestEvent,
) -> Result<(), AnyError> {
  assert!(
    matches!(event, TestEvent::StepWait(_) | TestEvent::StepResult(..)),
    "Only step wait/result events are expected from JS."
  );
  let mut sender = state.borrow::<TestEventSender>().clone();
  sender.send(event).ok();
  Ok(())
}
