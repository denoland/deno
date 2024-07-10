// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Error;
use deno_core::error::range_error;
use deno_core::op2;

use std::borrow::Cow;

// map_domain, to_ascii and to_unicode are based on the punycode implementation in node.js
// https://github.com/nodejs/node/blob/73025c4dec042e344eeea7912ed39f7b7c4a3991/lib/punycode.js

const PUNY_PREFIX: &str = "xn--";

fn invalid_input_err() -> Error {
  range_error("Invalid input")
}

fn not_basic_err() -> Error {
  range_error("Illegal input >= 0x80 (not a basic code point)")
}

/// map a domain by mapping each label with the given function
fn map_domain<E>(
  domain: &str,
  f: impl Fn(&str) -> Result<Cow<'_, str>, E>,
) -> Result<String, E> {
  let mut result = String::with_capacity(domain.len());
  let mut domain = domain;

  // if it's an email, leave the local part as is
  let mut parts = domain.split('@');
  if let (Some(local), Some(remaining)) = (parts.next(), parts.next()) {
    result.push_str(local);
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
  if input.is_ascii() {
    return Ok(input.into());
  }

  let mut result = String::with_capacity(input.len()); // at least as long as input

  let rest = map_domain(input, |label| {
    if label.is_ascii() {
      Ok(label.into())
    } else {
      idna::punycode::encode_str(label)
        .map(|encoded| [PUNY_PREFIX, &encoded].join("").into()) // add the prefix
        .ok_or_else(|| {
          Error::msg("Input would take more than 63 characters to encode") // only error possible per the docs
        })
    }
  })?;

  result.push_str(&rest);
  Ok(result)
}

/// Maps an ascii domain to unicode by punycode decoding each label
///
/// Note this is not IDNA2003 or IDNA2008 compliant, rather it matches node.js's punycode implementation
fn to_unicode(input: &str) -> Result<String, Error> {
  map_domain(input, |s| {
    if let Some(puny) = s.strip_prefix(PUNY_PREFIX) {
      // it's a punycode encoded label
      Ok(
        idna::punycode::decode_to_string(&puny.to_lowercase())
          .ok_or_else(invalid_input_err)?
          .into(),
      )
    } else {
      Ok(s.into())
    }
  })
}

/// Converts a domain to unicode with behavior that is
/// compatible with the `punycode` module in node.js
#[op2]
#[string]
pub fn op_node_idna_punycode_to_ascii(
  #[string] domain: String,
) -> Result<String, Error> {
  to_ascii(&domain)
}

/// Converts a domain to ASCII with behavior that is
/// compatible with the `punycode` module in node.js
#[op2]
#[string]
pub fn op_node_idna_punycode_to_unicode(
  #[string] domain: String,
) -> Result<String, Error> {
  to_unicode(&domain)
}

/// Converts a domain to ASCII as per the IDNA spec
/// (specifically UTS #46)
#[op2]
#[string]
pub fn op_node_idna_domain_to_ascii(
  #[string] domain: String,
) -> Result<String, Error> {
  idna::domain_to_ascii(&domain).map_err(|e| e.into())
}

/// Converts a domain to Unicode as per the IDNA spec
/// (specifically UTS #46)
#[op2]
#[string]
pub fn op_node_idna_domain_to_unicode(#[string] domain: String) -> String {
  idna::domain_to_unicode(&domain).0
}

#[op2]
#[string]
pub fn op_node_idna_punycode_decode(
  #[string] domain: String,
) -> Result<String, Error> {
  if domain.is_empty() {
    return Ok(domain);
  }

  // all code points before the last delimiter must be basic
  // see https://github.com/nodejs/node/blob/73025c4dec042e344eeea7912ed39f7b7c4a3991/lib/punycode.js#L215-L227
  let last_dash = domain.len()
    - 1
    - domain
      .bytes()
      .rev()
      .position(|b| b == b'-')
      .unwrap_or(domain.len() - 1);

  if !domain[..last_dash].is_ascii() {
    return Err(not_basic_err());
  }

  idna::punycode::decode_to_string(&domain)
    .ok_or_else(|| deno_core::error::range_error("Invalid input"))
}

#[op2]
#[string]
pub fn op_node_idna_punycode_encode(#[string] domain: String) -> String {
  idna::punycode::encode_str(&domain).unwrap_or_default()
}
