// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use crate::metrics::metrics_op;
use crate::permissions::Permissions;
use deno_core::error::AnyError;
use deno_core::futures::prelude::*;
use deno_core::plugin_api;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::BufVec;
use deno_core::JsRuntime;
use deno_core::Op;
use deno_core::OpAsyncFuture;
use deno_core::OpId;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ZeroCopyBuf;
use dlopen::symbor::Library;
use serde::Deserialize;
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

pub fn init(rt: &mut JsRuntime) {
  super::reg_json_sync(rt, "op_open_plugin", op_open_plugin);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenPluginArgs {
  filename: String,
}

pub fn op_open_plugin(
  state: &mut OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, AnyError> {
  let args: OpenPluginArgs = serde_json::from_value(args)?;
  let filename = PathBuf::from(&args.filename);

  super::check_unstable(state, "Deno.openPlugin");
  let permissions = state.borrow::<Permissions>();
  permissions.check_plugin(&filename)?;

  debug!("Loading Plugin: {:#?}", filename);
  let plugin_lib = Library::open(filename).map(Rc::new)?;
  let plugin_resource = PluginResource::new(&plugin_lib);

  let rid;
  let deno_plugin_init;
  {
    rid = state.resource_table.add(plugin_resource);
    deno_plugin_init = *unsafe {
      state
        .resource_table
        .get::<PluginResource>(rid)
        .unwrap()
        .lib
        .symbol::<plugin_api::InitFn>("deno_plugin_init")
        .unwrap()
    };
  }

  let mut interface = PluginInterface::new(state, &plugin_lib);
  deno_plugin_init(&mut interface);

  Ok(json!(rid))
}

struct PluginResource {
  lib: Rc<Library>,
}

impl Resource for PluginResource {
  fn name(&self) -> Cow<str> {
    "plugin".into()
  }
}

impl PluginResource {
  fn new(lib: &Rc<Library>) -> Self {
    Self { lib: lib.clone() }
  }
}

struct PluginInterface<'a> {
  state: &'a mut OpState,
  plugin_lib: &'a Rc<Library>,
}

impl<'a> PluginInterface<'a> {
  fn new(state: &'a mut OpState, plugin_lib: &'a Rc<Library>) -> Self {
    Self { state, plugin_lib }
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
    let plugin_op_fn = move |state_rc: Rc<RefCell<OpState>>,
                             mut zero_copy: BufVec| {
      let mut state = state_rc.borrow_mut();
      let mut interface = PluginInterface::new(&mut state, &plugin_lib);
      let op = dispatch_op_fn(&mut interface, &mut zero_copy);
      match op {
        sync_op @ Op::Sync(..) => sync_op,
        Op::Async(fut) => Op::Async(PluginOpAsyncFuture::new(&plugin_lib, fut)),
        Op::AsyncUnref(fut) => {
          Op::AsyncUnref(PluginOpAsyncFuture::new(&plugin_lib, fut))
        }
        _ => unreachable!(),
      }
    };
    self
      .state
      .op_table
      .register_op(name, metrics_op(name.to_string(), Box::new(plugin_op_fn)))
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
