// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::cli::Buf;
use crate::isolate_init;
use crate::isolate_state::IsolateState;
use crate::msg;
use crate::permissions::DenoPermissions;
use crate::resources;
use crate::resources::Resource;
use crate::resources::ResourceId;
use crate::workers;

use futures::Future;
use serde_json;
use std::str;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::Mutex;

lazy_static! {
  static ref C_RID: Mutex<Option<ResourceId>> = Mutex::new(None);
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

fn lazy_start(parent_state: &Arc<IsolateState>) -> Resource {
  let mut cell = C_RID.lock().unwrap();
  let isolate_init = isolate_init::compiler_isolate_init();
  let permissions = DenoPermissions {
    allow_read: AtomicBool::new(true),
    allow_write: AtomicBool::new(true),
    allow_env: AtomicBool::new(false),
    allow_net: AtomicBool::new(true),
    allow_run: AtomicBool::new(false),
  };
  let rid = cell.get_or_insert_with(|| {
    let resource = workers::spawn(
      isolate_init,
      parent_state.clone(),
      "compilerMain()".to_string(),
      permissions,
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
  parent_state: &Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
  module_meta_data: &ModuleMetaData,
) -> ModuleMetaData {
  let req_msg = req(specifier, referrer);

  let compiler = lazy_start(parent_state);

  let send_future = resources::worker_post_message(compiler.rid, req_msg);
  send_future.wait().unwrap();

  let recv_future = resources::worker_recv_message(compiler.rid);
  let res_msg = recv_future.wait().unwrap().unwrap();

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

    out = compile_sync(&IsolateState::mock(), specifier, &referrer, &mut out);
    assert!(
      out
        .maybe_output_code
        .unwrap()
        .starts_with("console.log(\"Hello World\");".as_bytes())
    );
  }
}
