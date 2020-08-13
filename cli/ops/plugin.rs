// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
use crate::op_error::OpError;
use crate::ops::dispatch_json::Deserialize;
use crate::ops::dispatch_json::JsonOp;
use crate::ops::dispatch_json::Value;
use crate::ops::json_op;
use crate::state::State;
use deno_core::plugin_api;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::Op;
use deno_core::OpAsyncFuture;
use deno_core::OpId;
use deno_core::ZeroCopyBuf;
use dlopen::symbor::Library;
use futures::prelude::*;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op(
    "op_open_plugin",
    s.core_op(json_op(s.stateful_op2(op_open_plugin))),
  );
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenPluginArgs {
  filename: String,
}

pub fn op_open_plugin(
  isolate_state: &mut CoreIsolateState,
  state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.openPlugin");
  let args: OpenPluginArgs = serde_json::from_value(args).unwrap();
  let filename = PathBuf::from(&args.filename);

  state.check_plugin(&filename)?;

  debug!("Loading Plugin: {:#?}", filename);
  let plugin_lib = Library::open(filename)
    .map(Rc::new)
    .map_err(OpError::from)?;
  let plugin_resource = PluginResource::new(&plugin_lib);

  let mut resource_table = isolate_state.resource_table.borrow_mut();
  let rid = resource_table.add("plugin", Box::new(plugin_resource));
  let plugin_resource = resource_table.get::<PluginResource>(rid).unwrap();

  let deno_plugin_init = *unsafe {
    plugin_resource
      .lib
      .symbol::<plugin_api::InitFn>("deno_plugin_init")
  }
  .unwrap();
  drop(resource_table);

  let mut interface = PluginInterface::new(isolate_state, &plugin_lib);
  deno_plugin_init(&mut interface);

  Ok(JsonOp::Sync(json!(rid)))
}

struct PluginResource {
  lib: Rc<Library>,
}

impl PluginResource {
  fn new(lib: &Rc<Library>) -> Self {
    Self { lib: lib.clone() }
  }
}

struct PluginInterface<'a> {
  isolate_state: &'a mut CoreIsolateState,
  plugin_lib: &'a Rc<Library>,
}

impl<'a> PluginInterface<'a> {
  fn new(
    isolate_state: &'a mut CoreIsolateState,
    plugin_lib: &'a Rc<Library>,
  ) -> Self {
    Self {
      isolate_state,
      plugin_lib,
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
    dispatch_op_fn: plugin_api::DispatchOpFn,
  ) -> OpId {
    let plugin_lib = self.plugin_lib.clone();
    self.isolate_state.op_registry.register(
      name,
      move |isolate_state, zero_copy| {
        let mut interface = PluginInterface::new(isolate_state, &plugin_lib);
        let op = dispatch_op_fn(&mut interface, zero_copy);
        match op {
          sync_op @ Op::Sync(..) => sync_op,
          Op::Async(fut) => {
            Op::Async(PluginOpAsyncFuture::new(&plugin_lib, fut))
          }
          Op::AsyncUnref(fut) => {
            Op::AsyncUnref(PluginOpAsyncFuture::new(&plugin_lib, fut))
          }
        }
      },
    )
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
