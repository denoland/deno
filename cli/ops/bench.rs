// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time;

use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::serde_v8;
use deno_core::v8;
use deno_core::ModuleSpecifier;
use deno_core::OpState;
use deno_runtime::permissions::create_child_permissions;
use deno_runtime::permissions::ChildPermissionsArg;
use deno_runtime::permissions::PermissionsContainer;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::tools::bench::BenchDescription;
use crate::tools::bench::BenchEvent;

#[derive(Default)]
pub(crate) struct BenchContainer(
  pub Vec<(BenchDescription, v8::Global<v8::Function>)>,
);

deno_core::extension!(deno_bench,
  ops = [
    op_pledge_test_permissions,
    op_restore_test_permissions,
    op_register_bench,
    op_dispatch_bench_event,
    op_bench_now,
  ],
  options = {
    sender: UnboundedSender<BenchEvent>,
  },
  state = |state, options| {
    state.put(options.sender);
    state.put(BenchContainer::default());
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
struct BenchInfo<'s> {
  #[serde(rename = "fn")]
  function: serde_v8::Value<'s>,
  name: String,
  baseline: bool,
  group: Option<String>,
  ignore: bool,
  only: bool,
  #[serde(default)]
  warmup: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchRegisterResult {
  id: usize,
  origin: String,
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[op2]
#[serde]
fn op_register_bench<'a>(
  scope: &mut v8::HandleScope<'a>,
  state: &mut OpState,
  #[serde] info: BenchInfo<'a>,
) -> Result<BenchRegisterResult, AnyError> {
  let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
  let origin = state.borrow::<ModuleSpecifier>().to_string();
  let description = BenchDescription {
    id,
    name: info.name,
    origin: origin.clone(),
    baseline: info.baseline,
    group: info.group,
    ignore: info.ignore,
    only: info.only,
    warmup: info.warmup,
  };
  let function: v8::Local<v8::Function> = info.function.v8_value.try_into()?;
  let function = v8::Global::new(scope, function);
  state
    .borrow_mut::<BenchContainer>()
    .0
    .push((description.clone(), function));
  let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
  sender.send(BenchEvent::Register(description)).ok();
  Ok(BenchRegisterResult { id, origin })
}

#[op2]
fn op_dispatch_bench_event(state: &mut OpState, #[serde] event: BenchEvent) {
  assert!(
    matches!(event, BenchEvent::Output(_)),
    "Only output events are expected from JS."
  );
  let sender = state.borrow::<UnboundedSender<BenchEvent>>().clone();
  sender.send(event).ok();
}

#[op2(fast)]
#[number]
fn op_bench_now(state: &mut OpState) -> Result<u64, AnyError> {
  let ns = state.borrow::<time::Instant>().elapsed().as_nanos();
  let ns_u64 = u64::try_from(ns)?;
  Ok(ns_u64)
}
