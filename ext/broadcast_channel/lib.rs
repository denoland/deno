// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

mod in_memory_broadcast_channel;

pub use in_memory_broadcast_channel::InMemoryBroadcastChannel;

use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_async;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[async_trait]
pub trait BroadcastChannel: Clone {
  type Resource: Resource;

  fn subscribe(&self) -> Result<Self::Resource, AnyError>;

  fn unsubscribe(&self, resource: &Self::Resource) -> Result<(), AnyError>;

  async fn send(
    &self,
    resource: &Self::Resource,
    name: String,
    data: Vec<u8>,
  ) -> Result<(), AnyError>;

  async fn recv(
    &self,
    resource: &Self::Resource,
  ) -> Result<Option<Message>, AnyError>;
}

pub type Message = (String, Vec<u8>);

struct Unstable(bool); // --unstable

pub fn op_broadcast_subscribe<BC: BroadcastChannel + 'static>(
  state: &mut OpState,
  _args: (),
  _buf: (),
) -> Result<ResourceId, AnyError> {
  let unstable = state.borrow::<Unstable>().0;

  if !unstable {
    eprintln!(
      "Unstable API 'BroadcastChannel'. The --unstable flag must be provided.",
    );
    std::process::exit(70);
  }

  let bc = state.borrow::<BC>();
  let resource = bc.subscribe()?;
  Ok(state.resource_table.add(resource))
}

pub fn op_broadcast_unsubscribe<BC: BroadcastChannel + 'static>(
  state: &mut OpState,
  rid: ResourceId,
  _buf: (),
) -> Result<(), AnyError> {
  let resource = state.resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow::<BC>();
  bc.unsubscribe(&resource)
}

pub async fn op_broadcast_send<BC: BroadcastChannel + 'static>(
  state: Rc<RefCell<OpState>>,
  (rid, name): (ResourceId, String),
  buf: ZeroCopyBuf,
) -> Result<(), AnyError> {
  let resource = state.borrow().resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow().borrow::<BC>().clone();
  bc.send(&resource, name, buf.to_vec()).await
}

pub async fn op_broadcast_recv<BC: BroadcastChannel + 'static>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  _buf: (),
) -> Result<Option<Message>, AnyError> {
  let resource = state.borrow().resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow().borrow::<BC>().clone();
  bc.recv(&resource).await
}

pub fn init<BC: BroadcastChannel + 'static>(
  bc: BC,
  unstable: bool,
) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/broadcast_channel",
      "01_broadcast_channel.js",
    ))
    .ops(vec![
      (
        "op_broadcast_subscribe",
        op_sync(op_broadcast_subscribe::<BC>),
      ),
      (
        "op_broadcast_unsubscribe",
        op_sync(op_broadcast_unsubscribe::<BC>),
      ),
      ("op_broadcast_send", op_async(op_broadcast_send::<BC>)),
      ("op_broadcast_recv", op_async(op_broadcast_recv::<BC>)),
    ])
    .state(move |state| {
      state.put(bc.clone());
      state.put(Unstable(unstable));
      Ok(())
    })
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("lib.deno_broadcast_channel.d.ts")
}
