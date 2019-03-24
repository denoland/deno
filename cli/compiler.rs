// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::isolate_state::*;
use crate::msg;
use crate::ops;
use crate::resources;
use crate::resources::Resource;
use crate::resources::ResourceId;
use crate::startup_data;
use crate::workers;
use crate::workers::WorkerBehavior;
use deno_core::deno_buf;
use deno_core::Behavior;
use deno_core::Buf;
use deno_core::Op;
use deno_core::StartupData;
use futures::Future;
use serde_json;
use std::str;
use std::sync::Arc;
use std::sync::Mutex;

lazy_static! {
  static ref C_RID: Mutex<Option<ResourceId>> = Mutex::new(None);
}

pub struct CompilerBehavior {
  pub state: Arc<IsolateState>,
}

impl CompilerBehavior {
  pub fn new(state: Arc<IsolateState>) -> Self {
    Self { state }
  }
}

impl IsolateStateContainer for CompilerBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl IsolateStateContainer for &CompilerBehavior {
  fn state(&self) -> Arc<IsolateState> {
    self.state.clone()
  }
}

impl Behavior for CompilerBehavior {
  fn startup_data(&mut self) -> Option<StartupData> {
    Some(startup_data::compiler_isolate_init())
  }

  fn dispatch(
    &mut self,
    control: &[u8],
    zero_copy: deno_buf,
  ) -> (bool, Box<Op>) {
    ops::dispatch_all(self, control, zero_copy, ops::op_selector_compiler)
  }
}

impl WorkerBehavior for CompilerBehavior {
  fn set_internal_channels(&mut self, worker_channels: WorkerChannels) {
    self.state = Arc::new(IsolateState::new(
      self.state.flags.clone(),
      self.state.argv.clone(),
      Some(worker_channels),
    ));
  }
}

// This corresponds to JS ModuleMetaData.
// TODO Rename one or the other so they correspond.
#[derive(Debug)]
pub struct ModuleMetaData {
  pub module_name: String,
  pub filename: String,
  pub media_type: msg::MediaType,
  pub source_code: Vec<u8>,
  pub maybe_output_code_filename: Option<String>,
  pub maybe_output_code: Option<Vec<u8>>,
  pub maybe_source_map_filename: Option<String>,
  pub maybe_source_map: Option<Vec<u8>>,
}

impl ModuleMetaData {
  pub fn js_source(&self) -> String {
    if self.media_type == msg::MediaType::Json {
      return format!(
        "export default {};",
        str::from_utf8(&self.source_code).unwrap()
      );
    }
    match self.maybe_output_code {
      None => str::from_utf8(&self.source_code).unwrap().to_string(),
      Some(ref output_code) => str::from_utf8(output_code).unwrap().to_string(),
    }
  }
}

fn lazy_start(parent_state: Arc<IsolateState>) -> Resource {
  let mut cell = C_RID.lock().unwrap();
  let rid = cell.get_or_insert_with(|| {
    let resource = workers::spawn(
      CompilerBehavior::new(Arc::new(IsolateState::new(
        parent_state.flags.clone(),
        parent_state.argv.clone(),
        None,
      ))),
      "compilerMain()".to_string(),
    );
    resource.rid
  });
  Resource { rid: *rid }
}

fn req(specifier: &str, referrer: &str) -> Buf {
  json!({
    "specifier": specifier,
    "referrer": referrer,
  }).to_string()
  .into_boxed_str()
  .into_boxed_bytes()
}

pub fn compile_sync(
  parent_state: Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
  module_meta_data: &ModuleMetaData,
) -> ModuleMetaData {
  let req_msg = req(specifier, referrer);

  let compiler = lazy_start(parent_state);

  let send_future = resources::worker_post_message(compiler.rid, req_msg);
  send_future.wait().unwrap();

  let recv_future = resources::worker_recv_message(compiler.rid);
  let result = recv_future.wait().unwrap();
  assert!(result.is_some());
  let res_msg = result.unwrap();

  let res_json = std::str::from_utf8(&res_msg).unwrap();
  match serde_json::from_str::<serde_json::Value>(res_json) {
    Ok(serde_json::Value::Object(map)) => ModuleMetaData {
      module_name: module_meta_data.module_name.clone(),
      filename: module_meta_data.filename.clone(),
      media_type: module_meta_data.media_type,
      source_code: module_meta_data.source_code.clone(),
      maybe_output_code: match map["outputCode"].as_str() {
        Some(str) => Some(str.as_bytes().to_owned()),
        _ => None,
      },
      maybe_output_code_filename: None,
      maybe_source_map: match map["sourceMap"].as_str() {
        Some(str) => Some(str.as_bytes().to_owned()),
        _ => None,
      },
      maybe_source_map_filename: None,
    },
    _ => panic!("error decoding compiler response"),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_compile_sync() {
    let cwd = std::env::current_dir().unwrap();
    let cwd_string = cwd.to_str().unwrap().to_owned();

    let specifier = "./tests/002_hello.ts";
    let referrer = cwd_string + "/";

    let mut out = ModuleMetaData {
      module_name: "xxx".to_owned(),
      filename: "/tests/002_hello.ts".to_owned(),
      media_type: msg::MediaType::TypeScript,
      source_code: "console.log(\"Hello World\");".as_bytes().to_owned(),
      maybe_output_code_filename: None,
      maybe_output_code: None,
      maybe_source_map_filename: None,
      maybe_source_map: None,
    };

    out = compile_sync(
      Arc::new(IsolateState::mock()),
      specifier,
      &referrer,
      &mut out,
    );
    assert!(
      out
        .maybe_output_code
        .unwrap()
        .starts_with("console.log(\"Hello World\");".as_bytes())
    );
  }
}
