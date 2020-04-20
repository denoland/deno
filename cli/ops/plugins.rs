use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::fs as deno_fs;
use crate::op_error::OpError;
use crate::ops::json_op;
use crate::state::State;
use deno_core::Isolate;
use deno_core::OpDispatcher;
use deno_core::OpId;
use deno_core::PluginInitContext;
use deno_core::PluginInitFn;
use deno_core::ZeroCopyBuf;
use dlopen::symbor::Library;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;

pub fn init(i: &mut Isolate, s: &State) {
  i.register_op(
    "op_open_plugin",
    s.core_op(json_op(s.stateful_op2(op_open_plugin))),
  );
}

fn open_plugin<P: AsRef<OsStr>>(lib_path: P) -> Result<Library, OpError> {
  debug!("Loading Plugin: {:#?}", lib_path.as_ref());

  Library::open(lib_path).map_err(OpError::from)
}

struct PluginResource {
  lib: Library,
  ops: HashMap<String, OpId>,
}

struct InitContext {
  ops: HashMap<String, Box<OpDispatcher>>,
}

impl PluginInitContext for InitContext {
  fn register_op(&mut self, name: &str, op: Box<OpDispatcher>) {
    let existing = self.ops.insert(name.to_string(), op);
    assert!(
      existing.is_none(),
      format!("Op already registered: {}", name)
    );
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenPluginArgs {
  filename: String,
}

pub fn op_open_plugin(
  isolate: &mut deno_core::Isolate,
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  let args: OpenPluginArgs = serde_json::from_value(args)?;
  let filename = deno_fs::resolve_from_cwd(Path::new(&args.filename))?;

  state.check_plugin(&filename)?;

  let lib = open_plugin(filename)?;
  let plugin_resource = PluginResource {
    lib,
    ops: HashMap::new(),
  };
  let mut state_ = state.borrow_mut();
  let rid = state_
    .resource_table
    .add("plugin", Box::new(plugin_resource));
  let plugin_resource = state_
    .resource_table
    .get_mut::<PluginResource>(rid)
    .unwrap();

  let init_fn = *unsafe {
    plugin_resource
      .lib
      .symbol::<PluginInitFn>("deno_plugin_init")
  }?;
  let mut init_context = InitContext {
    ops: HashMap::new(),
  };
  init_fn(&mut init_context);
  for op in init_context.ops {
    // Register each plugin op in the `OpRegistry` with the name
    // formated like this `plugin_{plugin_rid}_{name}`.
    // The inclusion of prefix and rid is designed to avoid any
    // op name collision beyond the bound of a single loaded
    // plugin instance.
    let op_id = isolate
      .register_op(&format!("plugin_{}_{}", rid, op.0), state.core_op(op.1));
    plugin_resource.ops.insert(op.0, op_id);
  }

  Ok(JsonOp::Sync(
    json!({ "rid": rid, "ops": plugin_resource.ops }),
  ))
}
