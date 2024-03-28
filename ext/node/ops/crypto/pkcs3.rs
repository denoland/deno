// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
//
// PKCS #3: Diffie-Hellman Key Agreement Standard

use spki::der;
use spki::der::asn1;
use spki::der::Reader;

// The parameters fields associated with OID dhKeyAgreement
//
// DHParameter ::= SEQUENCE {
//  prime INTEGER, -- p
//  base INTEGER, -- g
//  privateValueLength INTEGER OPTIONAL }
pub struct DhParameter {
  pub prime: asn1::Int,
  pub base: asn1::Int,
  pub private_value_length: Option<asn1::Int>,
}

impl<'a> TryFrom<asn1::AnyRef<'a>> for DhParameter {
  type Error = der::Error;

  fn try_from(any: asn1::AnyRef<'a>) -> der::Result<DhParameter> {
    any.sequence(|decoder| {
      let prime = decoder.decode()?;
      let base = decoder.decode()?;
      let private_value_length = decoder.decode()?;
      Ok(DhParameter {
        prime,
        base,
        private_value_length,
      })
    })
  }
}
