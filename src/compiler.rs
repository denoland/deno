#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use isolate::Buf;
use isolate::IsolateState;
use isolate::WorkerChannels;
use libdeno;
use msg;
use resources;
use resources::add_worker;
use resources::Resource;
use resources::ResourceId;
use workers::spawn_worker;
use workers::Worker;

use futures::Future;
use serde_json;
use std::cell::Cell;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;

lazy_static! {
  static ref c_rid: Mutex<Option<ResourceId>> = Mutex::new(None);
}

// This corresponds to JS ModuleMetaData.
// TODO Rename one or the other so they correspond.
#[derive(Debug)]
pub struct CodeFetchOutput {
  pub module_name: String,
  pub filename: String,
  pub media_type: msg::MediaType,
  pub source_code: String,
  pub maybe_output_code: Option<String>,
  pub maybe_source_map: Option<String>,
}

impl CodeFetchOutput {
  pub fn js_source<'a>(&'a self) -> &'a String {
    match self.maybe_output_code {
      None => &self.source_code,
      Some(ref output_code) => output_code,
    }
  }
}

impl CodeFetchOutput {
  // TODO Use serde_derive? Use flatbuffers?
  fn from_json(json_str: &str) -> Option<Self> {
    match serde_json::from_str::<serde_json::Value>(json_str) {
      Ok(serde_json::Value::Object(map)) => {
        let module_name = match map["moduleId"].as_str() {
          None => return None,
          Some(s) => s.to_string(),
        };

        let filename = match map["fileName"].as_str() {
          None => return None,
          Some(s) => s.to_string(),
        };

        let source_code = match map["sourceCode"].as_str() {
          None => return None,
          Some(s) => s.to_string(),
        };

        let maybe_output_code =
          map["outputCode"].as_str().map(|s| s.to_string());

        let maybe_source_map = map["sourceMap"].as_str().map(|s| s.to_string());

        Some(CodeFetchOutput {
          module_name,
          filename,
          media_type: msg::MediaType::JavaScript, // TODO
          source_code,
          maybe_output_code,
          maybe_source_map,
        })
      }
      _ => None,
    }
  }
}

fn lazy_start(parent_state: &Arc<IsolateState>) -> Resource {
  let mut cell = c_rid.lock().unwrap();
  let rid = cell.get_or_insert_with(|| {
    let (_t, c) =
      spawn_worker(parent_state.clone(), "compilerMain()".to_string());
    let resource = add_worker(c);
    resource.rid
  });
  Resource { rid: *rid }
}

fn stop() {
  /*
  if let Some(rid) = c_rid.lock().unwrap() {
    println!("kill compiler");
  }
  */
}

fn req(specifier: &str, referrer: &str) -> Buf {
  String::from(format!(
    r#"{{"specifier": "{}", "referrer": "{}"}}"#,
    specifier, referrer
  )).into_boxed_str()
  .into_boxed_bytes()
}

pub fn compile_sync(
  parent_state: &Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
) -> Option<CodeFetchOutput> {
  let req_msg = req(specifier, referrer);

  let compiler = lazy_start(parent_state);

  let send_future = resources::worker_post_message(compiler.rid, req_msg);
  send_future.wait().unwrap();

  let recv_future = resources::worker_recv_message(compiler.rid);
  let res_msg = recv_future.wait().unwrap().unwrap();

  let res_json = std::str::from_utf8(&res_msg).unwrap();
  CodeFetchOutput::from_json(res_json)
}

#[cfg(test)]
mod tests {
  use super::*;
  use futures::Sink;

  #[test]
  fn test_compile_sync() {
    let cwd = std::env::current_dir().unwrap();
    let cwd_string = cwd.to_str().unwrap().to_owned();

    let specifier = "./tests/002_hello.ts";
    let referrer = cwd_string + "/";

    let cfo = compile_sync(&IsolateState::mock(), specifier, &referrer);
    println!("compile_sync  {:?}", cfo);

    stop();
  }

  #[test]
  fn code_fetch_output_from_json() {
    let json = r#"{
      "moduleId":"/Users/rld/src/deno/tests/002_hello.ts",
      "fileName":"/Users/rld/src/deno/tests/002_hello.ts",
      "mediaType":1,
      "sourceCode":"console.log(\"Hello World\");\n",
      "outputCode":"yyy",
      "sourceMap":"xxx",
      "scriptVersion":"1"
    }"#;
    let actual = CodeFetchOutput::from_json(json).unwrap();
    assert_eq!(actual.filename, "/Users/rld/src/deno/tests/002_hello.ts");
    assert_eq!(actual.module_name, "/Users/rld/src/deno/tests/002_hello.ts");
    assert_eq!(actual.source_code, "console.log(\"Hello World\");\n");
    assert_eq!(actual.maybe_output_code, Some("yyy".to_string()));
    assert_eq!(actual.maybe_source_map, Some("xxx".to_string()));
  }
}
