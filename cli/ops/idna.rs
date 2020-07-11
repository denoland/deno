// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! https://url.spec.whatwg.org/#idna

use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::op_error::{ErrorKind, OpError};
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ZeroCopyBuf;
use idna::{domain_to_ascii, domain_to_ascii_strict};

pub fn init(i: &mut CoreIsolate, s: &State) {
  i.register_op("op_domain_to_ascii", s.stateful_json_op(op_domain_to_ascii));
}

fn invalid_domain_error() -> OpError {
  OpError {
    kind: ErrorKind::TypeError,
    msg: "Invalid domain.".to_string(),
  }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DomainToAscii {
  domain: String,
  be_strict: bool,
}

fn op_domain_to_ascii(
  _state: &State,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, OpError> {
  let args: DomainToAscii = serde_json::from_value(args)?;
  let domain = if args.be_strict {
    domain_to_ascii_strict(args.domain.as_str())
      .map_err(|_| invalid_domain_error())?
  } else {
    domain_to_ascii(args.domain.as_str()).map_err(|_| invalid_domain_error())?
  };
  Ok(JsonOp::Sync(json!(domain)))
}
