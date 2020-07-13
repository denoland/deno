// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Some deserializer fields are only used on Unix and Windows build fails without it
use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::OpError;
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::CoreIsolateState;
use deno_core::ZeroCopyBuf;
use futures::future::FutureExt;
use reqwest::Client;

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op(
    "op_create_http_client",
    s.stateful_json_op2(op_create_http_client),
  );
  i.register_op(
    "op_do_http_request",
    s.stateful_json_op2(op_do_http_request),
  );
}

fn op_create_http_client(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  _args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let mut resource_table = isolate_state.resource_table.borrow_mut();

  let rid: u32 =
    resource_table.add("httpClient", Box::new(reqwest::Client::new()));

  Ok(JsonOp::Sync(json!(rid)))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DoHttpRequestArgs {
  rid: i32,
}

fn op_do_http_request(
  isolate_state: &mut CoreIsolateState,
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: DoHttpRequestArgs = serde_json::from_value(args)?;
  let rid: u32 = args.rid as u32;

  let resource_table = isolate_state.resource_table.clone();

  let future = async move {
    let mut resource_table = resource_table.borrow_mut();

    let client = resource_table
      .get_mut::<Client>(rid)
      .ok_or_else(OpError::bad_resource_id)?;

    let res = client
      // todo(rudoi): implement url argument
      .get("https://httpbin.org/get")
      .send()
      .await?
      .json()
      .await?;
    Ok(res)
  };

  Ok(JsonOp::Async(future.boxed_local()))
}
