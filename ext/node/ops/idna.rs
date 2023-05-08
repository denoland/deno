// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::op;

#[op]
pub fn op_node_idna_domain_to_ascii(
  domain: String,
) -> Result<String, AnyError> {
  Ok(idna::domain_to_ascii(&domain)?)
}

#[op]
pub fn op_node_idna_domain_to_unicode(domain: String) -> String {
  idna::domain_to_unicode(&domain).0
}

#[op]
pub fn op_node_idna_punycode_decode(domain: String) -> String {
  idna::punycode::decode_to_string(&domain).unwrap_or_default()
}

#[op]
pub fn op_node_idna_punycode_encode(domain: String) -> String {
  idna::punycode::encode_str(&domain).unwrap_or_default()
}
