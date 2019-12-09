use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::fs as deno_fs;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;
use dlopen::symbor::Library;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::Arc;

pub fn init(i: &mut Isolate, s: &ThreadSafeState, r: Arc<deno::OpRegistry>) {
  let r_ = r.clone();
  i.register_op(
    "open_plugin",
    s.core_op(json_op(s.stateful_op(move |state, args, zero_copy| {
      op_open_plugin(&r_, state, args, zero_copy)
    }))),
  );
}

fn open_plugin<P: AsRef<OsStr>>(lib_path: P) -> Result<Library, ErrBox> {
  debug!("Loading Plugin: {:#?}", lib_path.as_ref());

  Library::open(lib_path).map_err(ErrBox::from)
}

struct PluginResource {
  lib: Library,
  ops: HashMap<String, OpId>,
}

impl Resource for PluginResource {}

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
  registry: &Arc<deno::OpRegistry>,
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: OpenPluginArgs = serde_json::from_value(args)?;
  let (filename, filename_) = deno_fs::resolve_from_cwd(&args.filename)?;

  state.check_plugin(&filename_)?;

  let lib = open_plugin(filename)?;
  let plugin_resource = PluginResource {
    lib,
    ops: HashMap::new(),
  };
  let mut table = state.lock_resource_table();
  let rid = table.add("plugin", Box::new(plugin_resource));
  let plugin_resource = table.get_mut::<PluginResource>(rid).unwrap();

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
    let op_id = registry
      .register(&format!("plugin_{}_{}", rid, op.0), state.core_op(op.1));
    plugin_resource.ops.insert(op.0, op_id);
  }

  Ok(JsonOp::Sync(
    json!({ "rid": rid, "ops": plugin_resource.ops }),
  ))
}
