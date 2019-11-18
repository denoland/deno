use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::fs as deno_fs;
use crate::ops::json_op;
use crate::state::ThreadSafeState;
use deno::*;
use dlopen::symbor::Library;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::sync::Arc;
use std::sync::Mutex;

pub fn init(
  i: &mut Isolate,
  s: &ThreadSafeState,
  isolate_arc: Arc<Mutex<deno::Isolate>>,
) {
  let isolate_arc_ = isolate_arc.clone();
  i.register_op(
    "open_native_plugin",
    s.core_op(json_op(s.stateful_op(move |state, args, zero_copy| {
      op_open_native_plugin(&isolate_arc_, state, args, zero_copy)
    }))),
  );
}

fn open_plugin<P: AsRef<OsStr>>(lib_path: P) -> Result<Library, ErrBox> {
  debug!("Loading Native Plugin: {:#?}", lib_path.as_ref());

  Library::open(lib_path).map_err(ErrBox::from)
}

struct NativePluginResource {
  lib: Library,
  ops: Vec<(String, OpId)>,
}

impl Resource for NativePluginResource {}

struct InitContext {
  isolate: Arc<Mutex<deno::Isolate>>,
  plugin_rid: ResourceId,
  ops: Vec<(String, OpId)>,
}

impl PluginInitContext for InitContext {
  fn register_op(
    &mut self,
    name: &str,
    op: Box<dyn Fn(&[u8], Option<PinnedBuf>) -> CoreOp + Send + Sync + 'static>,
  ) -> OpId {
    let mut i = self.isolate.lock().unwrap();
    let opid = i.register_op(&format!("{}_{}", self.plugin_rid, name), op);
    self.ops.push((name.to_string(), opid));
    opid
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenNativePluginArgs {
  filename: String,
}

pub fn op_open_native_plugin(
  isolate: &Arc<Mutex<Isolate>>,
  state: &ThreadSafeState,
  args: Value,
  _zero_copy: Option<PinnedBuf>,
) -> Result<JsonOp, ErrBox> {
  let args: OpenNativePluginArgs = serde_json::from_value(args)?;
  let (filename, filename_) = deno_fs::resolve_from_cwd(&args.filename)?;

  state.check_native(&filename_)?;

  let lib = open_plugin(filename)?;
  let plugin_resource = NativePluginResource {
    lib,
    ops: Vec::new(),
  };
  let mut table = state.lock_resource_table();
  let rid = table.add("native_plugin", Box::new(plugin_resource));
  let plugin_resource = table.get_mut::<NativePluginResource>(rid).unwrap();

  let init_fn = *unsafe {
    plugin_resource
      .lib
      .symbol::<PluginInitFn>("native_plugin_init")
  }?;
  let mut init_context = InitContext {
    isolate: isolate.clone(),
    plugin_rid: rid,
    ops: Vec::new(),
  };
  init_fn(&mut init_context);
  plugin_resource.ops.append(&mut init_context.ops);
  let ops: HashMap<String, OpId> = plugin_resource
    .ops
    .iter()
    .map(|record| (record.0.clone(), record.1))
    .collect();

  Ok(JsonOp::Sync(json!({ "rid": rid, "ops": ops })))
}
