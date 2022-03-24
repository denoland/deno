use deno_core::error::AnyError;

use elliptic_curve::AlgorithmParameters;

use elliptic_curve::pkcs8;
use elliptic_curve::pkcs8::der;
use elliptic_curve::pkcs8::der::asn1::*;
use elliptic_curve::pkcs8::der::Decodable as Pkcs8Decodable;
use elliptic_curve::pkcs8::der::Encodable;
use elliptic_curve::pkcs8::der::TagNumber;
use elliptic_curve::pkcs8::AlgorithmIdentifier;
use elliptic_curve::pkcs8::ObjectIdentifier;
use elliptic_curve::pkcs8::PrivateKeyDocument;
use elliptic_curve::pkcs8::PrivateKeyInfo;
use elliptic_curve::zeroize::Zeroizing;

use crate::shared::*;

const VERSION: u8 = 1;

const PUBLIC_KEY_TAG: TagNumber = TagNumber::new(1);

pub struct ECPrivateKey<'a, C: elliptic_curve::Curve> {
  pub algorithm: AlgorithmIdentifier<'a>,

  pub private_d: elliptic_curve::FieldBytes<C>,

  pub encoded_point: &'a [u8],
}

impl<'a, C> ECPrivateKey<'a, C>
where
  C: elliptic_curve::Curve + AlgorithmParameters,
{
  /// Create a new ECPrivateKey from a serialized private scalar and encoded public key
  pub fn from_private_and_public_bytes(
    private_d: elliptic_curve::FieldBytes<C>,
    encoded_point: &'a [u8],
  ) -> Self {
    Self {
      private_d,
      encoded_point,
      algorithm: C::algorithm_identifier(),
    }
  }

  pub fn named_curve_oid(&self) -> Result<ObjectIdentifier, AnyError> {
    let parameters = self
      .algorithm
      .parameters
      .ok_or_else(|| data_error("malformed parameters"))?;

    Ok(parameters.oid().unwrap())
  }

  fn internal_to_pkcs8_der(&self) -> der::Result<Vec<u8>> {
    // Shamelessly copied from pkcs8 crate and modified so as
    // to not require Arithmetic trait currently missing from p384
    let secret_key_field = OctetString::new(&self.private_d)?;
    let public_key_bytes = &self.encoded_point;
    let public_key_field = ContextSpecific {
      tag_number: PUBLIC_KEY_TAG,
      value: BitString::new(public_key_bytes)?.into(),
    };

    let der_message_fields: &[&dyn Encodable] =
      &[&VERSION, &secret_key_field, &public_key_field];

    let encoded_len =
      der::message::encoded_len(der_message_fields)?.try_into()?;
    let mut der_message = Zeroizing::new(vec![0u8; encoded_len]);
    let mut encoder = der::Encoder::new(&mut der_message);
    encoder.message(der_message_fields)?;
    encoder.finish()?;

    Ok(der_message.to_vec())
  }

  pub fn to_pkcs8_der(&self) -> Result<PrivateKeyDocument, AnyError> {
    let pkcs8_der = self
      .internal_to_pkcs8_der()
      .map_err(|_| data_error("expected valid PKCS#8 data"))?;

    let pki =
      pkcs8::PrivateKeyInfo::new(C::algorithm_identifier(), pkcs8_der.as_ref());

    Ok(pki.to_der())
  }
}

impl<'a, C: elliptic_curve::Curve> TryFrom<&'a [u8]> for ECPrivateKey<'a, C> {
  type Error = AnyError;

  fn try_from(bytes: &'a [u8]) -> Result<ECPrivateKey<C>, AnyError> {
    let pk_info = PrivateKeyInfo::from_der(bytes)
      .map_err(|_| data_error("expected valid PKCS#8 data"))?;

    Self::try_from(pk_info)
  }
}

impl<'a, C: elliptic_curve::Curve> TryFrom<PrivateKeyInfo<'a>>
  for ECPrivateKey<'a, C>
{
  type Error = AnyError;

  fn try_from(
    pk_info: PrivateKeyInfo<'a>,
  ) -> Result<ECPrivateKey<'a, C>, AnyError> {
    let any = der::asn1::Any::from_der(pk_info.private_key).map_err(|_| {
      data_error("expected valid PrivateKeyInfo private_key der")
    })?;

    if pk_info.algorithm.oid != elliptic_curve::ALGORITHM_OID {
      return Err(data_error("unsupported algorithm"));
    }

    any
      .sequence(|decoder| {
        // ver
        if decoder.uint8()? != VERSION {
          return Err(der::Tag::Integer.value_error());
        }

        // private_key
        let priv_key = decoder.octet_string()?.as_bytes();
        let mut private_d = elliptic_curve::FieldBytes::<C>::default();
        if priv_key.len() != private_d.len() {
          return Err(der::Tag::Sequence.value_error());
        };
        private_d.copy_from_slice(priv_key);

        let public_key = decoder
          .context_specific(PUBLIC_KEY_TAG)?
          .ok_or_else(|| {
            der::Tag::ContextSpecific(PUBLIC_KEY_TAG).value_error()
          })?
          .bit_string()?;

        Ok(Self {
          private_d,
          encoded_point: public_key.as_bytes(),
          algorithm: pk_info.algorithm,
        })
      })
      .map_err(|_| data_error("expected valid PrivateKeyInfo private_key der"))
  }
}
