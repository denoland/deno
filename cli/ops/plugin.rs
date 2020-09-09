// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::state::State;
use deno_core::BufVec;
use deno_core::ErrBox;
use deno_core::Op;
use deno_core::OpAsyncFuture;
use deno_core::OpId;
use deno_core::OpRegistry;
use deno_core::ZeroCopyBuf;
use deno_core::{plugin_api, OpTable};
use dlopen::symbor::Library;
use futures::prelude::*;
use serde_derive::Deserialize;
use serde_json::Value;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;
use std::{cell::RefCell, path::PathBuf};

pub fn init(s: &Rc<State>) {
  s.register_op_json_sync("op_open_plugin", op_open_plugin);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenPluginArgs {
  filename: String,
}

pub fn op_open_plugin(
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
  state.check_unstable("Deno.openPlugin");

  let args: OpenPluginArgs = serde_json::from_value(args).unwrap();
  let filename = PathBuf::from(&args.filename);

  state.check_plugin(&filename)?;

  debug!("Loading Plugin: {:#?}", filename);
  let plugin_lib = Library::open(filename).map(Rc::new)?;
  let plugin_resource = PluginResource::new(&plugin_lib, &state.op_table);

  let rid;
  let deno_plugin_init;
  {
    let mut resource_table = state.resource_table.borrow_mut();
    rid = resource_table.add("plugin", Box::new(plugin_resource));
    deno_plugin_init = *unsafe {
      resource_table
        .get::<PluginResource>(rid)
        .unwrap()
        .lib
        .symbol::<plugin_api::InitFn>("deno_plugin_init")
        .unwrap()
    };
  }

  let mut interface = PluginInterface::new(state, &plugin_lib, rid);
  deno_plugin_init(&mut interface);

  Ok(json!(rid))
}

struct PluginResource {
  lib: Rc<Library>,
  op_table: Rc<RefCell<OpTable<State>>>,
  ops: Vec<String>,
}

impl PluginResource {
  fn new(lib: &Rc<Library>, op_table: &Rc<RefCell<OpTable<State>>>) -> Self {
    Self {
      lib: lib.clone(),
      op_table: op_table.clone(),
      ops: Vec::new(),
    }
  }
}

impl Drop for PluginResource {
  fn drop(&mut self) {
    let mut table_lock = self.op_table.borrow_mut();
    for op in &self.ops {
      table_lock.unregister_op(&op);
    }
  }
}

struct PluginInterface<'a> {
  state: &'a State,
  plugin_lib: &'a Rc<Library>,
  rid: u32,
}

impl<'a> PluginInterface<'a> {
  fn new(state: &'a State, plugin_lib: &'a Rc<Library>, rid: u32) -> Self {
    Self {
      state,
      plugin_lib,
      rid,
    }
  }
}

impl<'a> plugin_api::Interface for PluginInterface<'a> {
  /// Does the same as `core::Isolate::register_op()`, but additionally makes
  /// the registered op dispatcher, as well as the op futures created by it,
  /// keep reference to the plugin `Library` object, so that the plugin doesn't
  /// get unloaded before all its op registrations and the futures created by
  /// them are dropped.
  fn register_op(
    &mut self,
    name: &str,
    dispatch_op_fn: Box<plugin_api::DispatchOpFn>,
  ) -> OpId {
    let plugin_lib = self.plugin_lib.clone();
    let plugin_rid = self.rid;
    let op_rid = self.state.register_op(
      name,
      move |state: Rc<State>, mut zero_copy: BufVec| {
        let mut interface =
          PluginInterface::new(&state, &plugin_lib, plugin_rid);
        let op = dispatch_op_fn(&mut interface, &mut zero_copy);
        match op {
          sync_op @ Op::Sync(..) => sync_op,
          Op::Async(fut) => {
            Op::Async(PluginOpAsyncFuture::new(&plugin_lib, fut))
          }
          Op::AsyncUnref(fut) => {
            Op::AsyncUnref(PluginOpAsyncFuture::new(&plugin_lib, fut))
          }
          _ => unreachable!(),
        }
      },
    );
    let mut resource_table = self.state.resource_table.borrow_mut();
    let plugin_resource =
      resource_table.get_mut::<PluginResource>(self.rid).unwrap();
    plugin_resource.ops.push(name.to_string());
    op_rid
  }
}

struct PluginOpAsyncFuture {
  fut: Option<OpAsyncFuture>,
  _plugin_lib: Rc<Library>,
}

impl PluginOpAsyncFuture {
  fn new(plugin_lib: &Rc<Library>, fut: OpAsyncFuture) -> Pin<Box<Self>> {
    let wrapped_fut = Self {
      fut: Some(fut),
      _plugin_lib: plugin_lib.clone(),
    };
    Box::pin(wrapped_fut)
  }
}

impl Future for PluginOpAsyncFuture {
  type Output = <OpAsyncFuture as Future>::Output;
  fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
    self.fut.as_mut().unwrap().poll_unpin(ctx)
  }
}

impl Drop for PluginOpAsyncFuture {
  fn drop(&mut self) {
    self.fut.take();
  }
}
