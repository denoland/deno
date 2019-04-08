// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::cli_behavior::CliBehavior;
use crate::isolate_state::*;
use crate::js_errors;
use crate::js_errors::JSErrorColor;
use crate::msg;
use crate::resources;
use crate::resources::ResourceId;
use crate::startup_data;
use crate::tokio_util;
use crate::worker::Worker;
use deno::js_check;
use deno::Buf;
use deno::JSError;
use futures::future::*;
use futures::sync::oneshot;
use futures::Future;
use futures::Stream;
use serde_json;
use std::collections::HashMap;
use std::str;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::runtime::Runtime;

type CmdId = u32;
type ResponseSenderTable = HashMap<CmdId, oneshot::Sender<Buf>>;

lazy_static! {
  static ref C_NEXT_CMD_ID: AtomicUsize = AtomicUsize::new(1);
  // Map of response senders
  static ref C_RES_SENDER_TABLE: Mutex<ResponseSenderTable> = Mutex::new(ResponseSenderTable::new());
  // Shared worker resources so we can spawn
  static ref C_RID: Mutex<Option<ResourceId>> = Mutex::new(None);
  // tokio runtime specifically for spawning logic that is dependent on
  // completetion of the compiler worker future
  static ref C_RUNTIME: Mutex<Runtime> = Mutex::new(Runtime::new().unwrap());
}

// This corresponds to JS ModuleMetaData.
// TODO Rename one or the other so they correspond.
#[derive(Debug, Clone)]
pub struct ModuleMetaData {
  pub module_name: String,
  pub module_redirect_source_name: Option<String>, // source of redirect
  pub filename: String,
  pub media_type: msg::MediaType,
  pub source_code: Vec<u8>,
  pub maybe_output_code_filename: Option<String>,
  pub maybe_output_code: Option<Vec<u8>>,
  pub maybe_source_map_filename: Option<String>,
  pub maybe_source_map: Option<Vec<u8>>,
}

impl ModuleMetaData {
  pub fn has_output_code_and_source_map(&self) -> bool {
    self.maybe_output_code.is_some() && self.maybe_source_map.is_some()
  }

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

fn new_cmd_id() -> CmdId {
  let next_rid = C_NEXT_CMD_ID.fetch_add(1, Ordering::SeqCst);
  next_rid as CmdId
}

fn parse_cmd_id(res_json: &str) -> CmdId {
  match serde_json::from_str::<serde_json::Value>(res_json) {
    Ok(serde_json::Value::Object(map)) => match map["cmdId"].as_u64() {
      Some(cmd_id) => cmd_id as CmdId,
      _ => panic!("Error decoding compiler response: expected cmdId"),
    },
    _ => panic!("Error decoding compiler response"),
  }
}

fn lazy_start(parent_state: Arc<IsolateState>) -> ResourceId {
  let mut cell = C_RID.lock().unwrap();
  cell
    .get_or_insert_with(|| {
      let child_state = Arc::new(IsolateState::new(
        parent_state.flags.clone(),
        parent_state.argv.clone(),
      ));
      let rid = child_state.resource.rid;
      let resource = child_state.resource.clone();
      let behavior = CliBehavior::new(child_state);

      let mut worker = Worker::new(
        "TS".to_string(),
        startup_data::compiler_isolate_init(),
        behavior,
      );

      js_check(worker.execute("denoMain()"));
      js_check(worker.execute("workerMain()"));
      js_check(worker.execute("compilerMain()"));

      let mut runtime = C_RUNTIME.lock().unwrap();
      runtime.spawn(lazy(move || {
        worker.then(move |result| -> Result<(), ()> {
          // Close resource so the future created by
          // handle_worker_message_stream exits
          resource.close();
          debug!("Compiler worker exited!");
          if let Err(e) = result {
            eprintln!("{}", JSErrorColor(&e).to_string());
          }
          std::process::exit(1);
        })
      }));
      runtime.spawn(lazy(move || {
        debug!("Start worker stream handler!");
        let worker_stream = resources::get_message_stream_from_worker(rid);
        worker_stream
          .for_each(|msg: Buf| {
            // All worker responses are handled here first before being sent via
            // their respective sender. This system can be compared to the
            // promise system used on the js side. This provides a way to
            // resolve many futures via the same channel.
            let res_json = std::str::from_utf8(&msg).unwrap();
            debug!("Got message from worker: {}", res_json);
            // Get the intended receiver's cmd_id from the message.
            let cmd_id = parse_cmd_id(res_json);
            let mut table = C_RES_SENDER_TABLE.lock().unwrap();
            debug!("Cmd id for get message handler: {}", cmd_id);
            // Get the corresponding response sender from the table and
            // send a response.
            let response_sender = table.remove(&(cmd_id as CmdId)).unwrap();
            response_sender.send(msg).unwrap();
            Ok(())
          }).map_err(|_| ())
      }));
      rid
    }).clone()
}

fn req(specifier: &str, referrer: &str, cmd_id: u32) -> Buf {
  json!({
    "specifier": specifier,
    "referrer": referrer,
    "cmdId": cmd_id,
  }).to_string()
  .into_boxed_str()
  .into_boxed_bytes()
}

pub fn compile_async(
  parent_state: Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
  module_meta_data: &ModuleMetaData,
) -> impl Future<Item = ModuleMetaData, Error = JSError> {
  debug!(
    "Running rust part of compile_sync. specifier: {}, referrer: {}",
    &specifier, &referrer
  );
  let cmd_id = new_cmd_id();

  let req_msg = req(&specifier, &referrer, cmd_id);
  let module_meta_data_ = module_meta_data.clone();

  let compiler_rid = lazy_start(parent_state.clone());

  let (local_sender, local_receiver) =
    oneshot::channel::<Result<ModuleMetaData, Option<JSError>>>();

  let (response_sender, response_receiver) = oneshot::channel::<Buf>();

  // Scoping to auto dispose of locks when done using them
  {
    let mut table = C_RES_SENDER_TABLE.lock().unwrap();
    debug!("Cmd id for response sender insert: {}", cmd_id);
    // Place our response sender in the table so we can find it later.
    table.insert(cmd_id, response_sender);

    let mut runtime = C_RUNTIME.lock().unwrap();
    runtime.spawn(lazy(move || {
      resources::post_message_to_worker(compiler_rid, req_msg)
        .then(move |_| {
          debug!("Sent message to worker");
          response_receiver.map_err(|_| None)
        }).and_then(move |res_msg| {
          debug!("Received message from worker");
          let res_json = std::str::from_utf8(res_msg.as_ref()).unwrap();
          let res = serde_json::from_str::<serde_json::Value>(res_json)
            .expect("Error decoding compiler response");
          let res_data = res["data"].as_object().expect(
            "Error decoding compiler response: expected object field 'data'",
          );
          match res["success"].as_bool() {
            Some(true) => Ok(ModuleMetaData {
              maybe_output_code: res_data["outputCode"]
                .as_str()
                .map(|s| s.as_bytes().to_owned()),
              maybe_source_map: res_data["sourceMap"]
                .as_str()
                .map(|s| s.as_bytes().to_owned()),
              ..module_meta_data_
            }),
            Some(false) => {
              let js_error = JSError::from_json_value(
                serde_json::Value::Object(res_data.clone()),
              ).expect(
                "Error decoding compiler response: failed to parse error",
              );
              Err(Some(js_errors::apply_source_map(
                &js_error,
                &parent_state.dir,
              )))
            }
            _ => panic!(
              "Error decoding compiler response: expected bool field 'success'"
            ),
          }
        }).then(move |result| {
          local_sender.send(result).expect("Oneshot send() failed");
          Ok(())
        })
    }));
  }

  local_receiver
    .map_err(|e| {
      panic!(
        "Local channel canceled before compile request could be completed: {}",
        e
      )
    }).and_then(move |result| match result {
      Ok(v) => futures::future::result(Ok(v)),
      Err(Some(err)) => futures::future::result(Err(err)),
      Err(None) => panic!("Failed to communicate with the compiler worker."),
    })
}

pub fn compile_sync(
  parent_state: Arc<IsolateState>,
  specifier: &str,
  referrer: &str,
  module_meta_data: &ModuleMetaData,
) -> Result<ModuleMetaData, JSError> {
  tokio_util::block_on(compile_async(
    parent_state,
    specifier,
    referrer,
    module_meta_data,
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_compile_sync() {
    tokio_util::init(|| {
      let cwd = std::env::current_dir().unwrap();
      let cwd_string = cwd.to_str().unwrap().to_owned();

      let specifier = "./tests/002_hello.ts";
      let referrer = cwd_string + "/";

      let mut out = ModuleMetaData {
        module_name: "xxx".to_owned(),
        module_redirect_source_name: None,
        filename: "/tests/002_hello.ts".to_owned(),
        media_type: msg::MediaType::TypeScript,
        source_code: include_bytes!("../tests/002_hello.ts").to_vec(),
        maybe_output_code_filename: None,
        maybe_output_code: None,
        maybe_source_map_filename: None,
        maybe_source_map: None,
      };

      out = compile_sync(
        Arc::new(IsolateState::mock()),
        specifier,
        &referrer,
        &out,
      ).unwrap();
      assert!(
        out
          .maybe_output_code
          .unwrap()
          .starts_with("console.log(\"Hello World\");".as_bytes())
      );
    })
  }

  #[test]
  fn test_parse_cmd_id() {
    let cmd_id = new_cmd_id();

    let msg = req("Hello", "World", cmd_id);

    let res_json = std::str::from_utf8(&msg).unwrap();

    assert_eq!(parse_cmd_id(res_json), cmd_id);
  }
}
