use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::fs as deno_fs;
use crate::op_error::OpError;
use crate::ops::json_op;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use dlopen::symbor::Library;
use std::ffi::OsStr;
use std::path::Path;

pub type PluginInitFn = fn(isolate: &mut CoreIsolate);

pub fn init(i: &mut CoreIsolate, s: &State) {
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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenPluginArgs {
  filename: String,
}

pub fn op_open_plugin(
  isolate: &mut CoreIsolate,
  state: &State,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<JsonOp, OpError> {
  state.check_unstable("Deno.openPlugin");
  let args: OpenPluginArgs = serde_json::from_value(args).unwrap();
  let filename = deno_fs::resolve_from_cwd(Path::new(&args.filename))?;

  state.check_plugin(&filename)?;

  let lib = open_plugin(filename).unwrap();
  let plugin_resource = PluginResource { lib };

  let mut resource_table = isolate.resource_table.borrow_mut();
  let rid = resource_table.add("plugin", Box::new(plugin_resource));
  let plugin_resource = resource_table.get::<PluginResource>(rid).unwrap();

  let deno_plugin_init = *unsafe {
    plugin_resource
      .lib
      .symbol::<PluginInitFn>("deno_plugin_init")
  }
  .unwrap();
  drop(resource_table);

  deno_plugin_init(isolate);

  Ok(JsonOp::Sync(json!(rid)))
}
