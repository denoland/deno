// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! https://url.spec.whatwg.org/#idna

use super::dispatch_json::{Deserialize, JsonOp, Value};
use crate::state::State;
use deno_core::CoreIsolate;
use deno_core::ErrBox;
use deno_core::ZeroCopyBuf;
use idna::{domain_to_ascii, domain_to_ascii_strict};
use std::rc::Rc;

pub fn init(i: &mut CoreIsolate, s: &Rc<State>) {
  i.register_op("op_domain_to_ascii", s.stateful_json_op(op_domain_to_ascii));
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DomainToAscii {
  domain: String,
  be_strict: bool,
}

fn op_domain_to_ascii(
  _state: &Rc<State>,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<JsonOp, ErrBox> {
  let args: DomainToAscii = serde_json::from_value(args)?;
  if args.be_strict {
    domain_to_ascii_strict(args.domain.as_str())
  } else {
    domain_to_ascii(args.domain.as_str())
  }
  .map_err(|err| {
    let message = format!("Invalid IDNA encoded domain name: {:?}", err);
    ErrBox::new("URIError", message)
  })
  .map(|domain| JsonOp::Sync(json!(domain)))
}
