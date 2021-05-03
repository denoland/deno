// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
use crate::permissions::Permissions;
use deno_core::error::AnyError;
use deno_core::op_sync;
use deno_core::Extension;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use deno_core::ZeroCopyBuf;
use dlopen::symbor::Library;
use log::debug;
use std::borrow::Cow;
use std::path::PathBuf;
use std::rc::Rc;

pub type InitFn = fn() -> Extension;

pub fn init() -> Extension {
  Extension::builder()
    .ops(vec![("op_open_plugin", op_sync(op_open_plugin))])
    .build()
}

pub fn op_open_plugin(
  state: &mut OpState,
  filename: String,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<ResourceId, AnyError> {
  let filename = PathBuf::from(&filename);

  super::check_unstable(state, "Deno.openPlugin");
  let permissions = state.borrow_mut::<Permissions>();
  permissions.plugin.check()?;

  debug!("Loading Plugin: {:#?}", filename);
  let plugin_lib = Library::open(filename).map(Rc::new)?;
  let plugin_resource = PluginResource::new(&plugin_lib);

  let rid;
  let init;
  {
    rid = state.resource_table.add(plugin_resource);
    init = *unsafe {
      state
        .resource_table
        .get::<PluginResource>(rid)
        .unwrap()
        .lib
        .symbol::<InitFn>("init")
        .unwrap()
    };
  }

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

struct PluginResource {
  lib: Rc<Library>,
}

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
    Self { lib: lib.clone() }
  }
}
