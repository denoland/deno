// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//! https://url.spec.whatwg.org/#idna

use deno_core::ErrBox;
use deno_core::ZeroCopyBuf;
use idna::domain_to_ascii;
use idna::domain_to_ascii_strict;
use serde_derive::Deserialize;
use serde_json::Value;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_json_sync(rt, "op_domain_to_ascii", op_domain_to_ascii);
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DomainToAscii {
  domain: String,
  be_strict: bool,
}

fn op_domain_to_ascii(
  _state: &mut deno_core::OpState,
  args: Value,
  _zero_copy: &mut [ZeroCopyBuf],
) -> Result<Value, ErrBox> {
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
  .map(|domain| json!(domain))
}
