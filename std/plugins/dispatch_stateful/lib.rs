// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use deno_core::*;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;

#[derive(Default)]
pub struct PluginState {
  resource_table: Arc<RwLock<ResourceTable>>,
}

impl Clone for PluginState {
  fn clone(&self) -> Self {
    PluginState {
      resource_table: self.resource_table.clone(),
    }
  }
}

type StatefulOpFn = dyn Fn(&PluginState, &[u8], Option<PinnedBuf>) -> CoreOp
  + Send
  + Sync
  + 'static;

impl PluginState {
  pub fn lock_resources_mut(&self) -> RwLockWriteGuard<ResourceTable> {
    self.resource_table.write().unwrap()
  }

  pub fn lock_resources(&self) -> RwLockReadGuard<ResourceTable> {
    self.resource_table.read().unwrap()
  }

  pub fn stateful_op(
    &self,
    d: Box<StatefulOpFn>,
  ) -> Box<dyn Fn(&[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static>
  {
    let state = self.clone();

    Box::new(
      move |control: &[u8], zero_copy: Option<PinnedBuf>| -> CoreOp {
        d(&state, control, zero_copy)
      },
    )
  }
}
