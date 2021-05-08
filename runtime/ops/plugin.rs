// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use crate::permissions::Permissions;
use deno_core::error::AnyError;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use dlopen::symbor::Library;
use log::debug;
use std::borrow::Cow;
use std::mem;
use std::path::PathBuf;
use std::rc::Rc;

/// A default `init` function for plugins which mimics the way the internal
/// extensions are initalized. Plugins currently do not support all extension
/// features and are most likely not going to in the future. Currently only
/// `init_state` and `init_ops` are supported while `init_middleware` and `init_js`
/// are not. Currently the `PluginResource` does not support being closed due to
/// certain risks in unloading the dynamic library without unloading dependent
/// functions and resources.
pub type InitFn = fn() -> Extension;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![("op_open_plugin", op_sync(op_open_plugin))])
    .build()
}

pub fn op_open_plugin(
  state: &mut OpState,
  filename: String,
  _: (),
) -> Result<ResourceId, AnyError> {
  let filename = PathBuf::from(&filename);

  super::check_unstable(state, "Deno.openPlugin");
  let permissions = state.borrow_mut::<Permissions>();
  permissions.plugin.check()?;

  debug!("Loading Plugin: {:#?}", filename);
  let plugin_lib = Library::open(filename).map(Rc::new)?;
  let plugin_resource = PluginResource::new(&plugin_lib);

  // Forgets the plugin_lib value to prevent segfaults when the process exits
  mem::forget(plugin_lib);

  let init = *unsafe { plugin_resource.0.symbol::<InitFn>("init") }?;
  let rid = state.resource_table.add(plugin_resource);
  let mut extension = init();

  if !extension.init_js().is_empty() {
    panic!("Plugins do not support loading js");
  }

  if extension.init_middleware().is_some() {
    panic!("Plugins do not support middleware");
  }

  extension.init_state(state)?;
  let ops = extension.init_ops().unwrap_or_default();
  for (name, opfn) in ops {
    state.op_table.register_op(name, opfn);
  }

  Ok(rid)
}

struct PluginResource(Rc<Library>);

impl Resource for PluginResource {
  fn name(&self) -> Cow<str> {
    "plugin".into()
  }

  fn close(self: Rc<Self>) {
    unimplemented!();
  }
}

impl PluginResource {
  fn new(lib: &Rc<Library>) -> Self {
    Self(lib.clone())
  }
}
