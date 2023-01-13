// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

mod in_memory_broadcast_channel;

pub use in_memory_broadcast_channel::InMemoryBroadcastChannel;
pub use in_memory_broadcast_channel::InMemoryBroadcastChannelResource;

use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op;
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

#[op]
pub fn op_broadcast_subscribe<BC>(
  state: &mut OpState,
) -> Result<ResourceId, AnyError>
where
  BC: BroadcastChannel + 'static,
{
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

#[op]
pub fn op_broadcast_unsubscribe<BC>(
  state: &mut OpState,
  rid: ResourceId,
) -> Result<(), AnyError>
where
  BC: BroadcastChannel + 'static,
{
  let resource = state.resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow::<BC>();
  bc.unsubscribe(&resource)
}

#[op]
pub async fn op_broadcast_send<BC>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
  name: String,
  buf: ZeroCopyBuf,
) -> Result<(), AnyError>
where
  BC: BroadcastChannel + 'static,
{
  let resource = state.borrow().resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow().borrow::<BC>().clone();
  bc.send(&resource, name, buf.to_vec()).await
}

#[op]
pub async fn op_broadcast_recv<BC>(
  state: Rc<RefCell<OpState>>,
  rid: ResourceId,
) -> Result<Option<Message>, AnyError>
where
  BC: BroadcastChannel + 'static,
{
  let resource = state.borrow().resource_table.get::<BC::Resource>(rid)?;
  let bc = state.borrow().borrow::<BC>().clone();
  bc.recv(&resource).await
}

pub fn init<BC: BroadcastChannel + 'static>(
  bc: BC,
  unstable: bool,
) -> Extension {
  Extension::builder(env!("CARGO_PKG_NAME"))
    .dependencies(vec!["deno_webidl", "deno_web"])
    .js(include_js_files!(
      prefix "deno:ext/broadcast_channel",
      "01_broadcast_channel.js",
    ))
    .ops(vec![
      op_broadcast_subscribe::decl::<BC>(),
      op_broadcast_unsubscribe::decl::<BC>(),
      op_broadcast_send::decl::<BC>(),
      op_broadcast_recv::decl::<BC>(),
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
