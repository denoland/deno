// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Error;
use deno_core::op2;

use std::borrow::Cow;
use std::convert::Infallible;

// map_domain, to_ascii and to_unicode are based on the punycode implementation in node.js
// https://github.com/nodejs/node/blob/73025c4dec042e344eeea7912ed39f7b7c4a3991/lib/punycode.js

fn map_domain<E>(domain: &str, f: impl Fn(&str) -> Result<Cow<'_, str>, E>) -> Result<String, E> {
  let mut result = String::with_capacity(domain.len());
  let mut domain = domain;

  // if it's an email, map the local part separately
  let mut parts = domain.split('@'); 
  if let (Some(local), Some(remaining)) = (parts.next(), parts.next()) {
      result.push_str(&f(local)?);
      result.push('@');
      domain = remaining;
  }
  
  // split into labels and map each one
  for (i, label) in domain.split('.').enumerate() {
      if i > 0 {
          result.push('.');
      }
      result.push_str(&f(label)?);
  }
  Ok(result)
}


/// Maps a unicode domain to ascii by punycode encoding each label
/// 
/// Note this is not IDNA2003 or IDNA2008 compliant, rather it matches node.js's punycode implementation
fn to_ascii(input: &str) -> Result<String, Error> {
  let mut result = String::with_capacity(4 + 2*input.len());
  result.push_str("xn--");
  let rest = map_domain(input, |label| {
      let chars = label.chars().collect::<Vec<_>>();
      if label.is_ascii() {
          Ok(label.into())
      } else {
        idna::punycode::encode(&chars).map(Cow::Owned).ok_or_else(|| {
            Error::msg("Input would take more than 63 characters to encode")
        })
      }
  })?;
  result.push_str(&rest);
  Ok(result)
}

/// Maps an ascii domain to unicode by punycode decoding each label
/// 
/// Note this is not IDNA2003 or IDNA2008 compliant, rather it matches node.js's punycode implementation
fn to_unicode(input: &str) -> String {
  map_domain::<Infallible>(input, |s| {
      if let Some(puny) = s.strip_prefix("xn--") {
          Ok(idna::punycode::decode_to_string(puny)
              .unwrap_or_default()
              .into())
      } else {
          Ok(s.into())
      }
  }).expect("infallible")
}

#[op2]
#[string]
pub fn op_node_idna_domain_to_ascii(
  #[string] domain: String,
) -> Result<String, Error> {
  to_ascii(&domain)
}

#[op2]
#[string]
pub fn op_node_idna_domain_to_unicode(#[string] domain: String) -> String {
  to_unicode(&domain)
}

#[op2]
#[string]
pub fn op_node_idna_punycode_decode(#[string] domain: String) -> String {
  idna::punycode::decode_to_string(&domain).unwrap_or_default()
}

#[op2]
#[string]
pub fn op_node_idna_punycode_encode(#[string] domain: String) -> String {
  idna::punycode::encode_str(&domain).unwrap_or_default()
}
