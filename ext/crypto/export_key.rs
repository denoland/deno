use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use rsa::pkcs1::UIntBytes;
use serde::Deserialize;
use serde::Serialize;
use spki::der::asn1;
use spki::der::Decodable;
use spki::der::Encodable;

use crate::shared::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportKeyOptions {
  format: ExportKeyFormat,
  #[serde(flatten)]
  algorithm: ExportKeyAlgorithm,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportKeyFormat {
  Pkcs8,
  Spki,
  JwkPublic,
  JwkPrivate,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "algorithm")]
pub enum ExportKeyAlgorithm {
  #[serde(rename = "RSASSA-PKCS1-v1_5")]
  RsassaPkcs1v15 {},
  #[serde(rename = "RSA-PSS")]
  RsaPss {},
  #[serde(rename = "RSA-OAEP")]
  RsaOaep {},
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ExportKeyResult {
  Pkcs8(ZeroCopyBuf),
  Spki(ZeroCopyBuf),
  JwkPublicRsa {
    n: String,
    e: String,
  },
  JwkPrivateRsa {
    n: String,
    e: String,
    d: String,
    p: String,
    q: String,
    dp: String,
    dq: String,
    qi: String,
  },
}

pub fn op_crypto_export_key(
  _state: &mut OpState,
  opts: ExportKeyOptions,
  key_data: RawKeyData,
) -> Result<ExportKeyResult, AnyError> {
  match opts.algorithm {
    ExportKeyAlgorithm::RsassaPkcs1v15 {}
    | ExportKeyAlgorithm::RsaPss {}
    | ExportKeyAlgorithm::RsaOaep {} => export_key_rsa(opts.format, key_data),
  }
}

fn uint_to_b64(bytes: UIntBytes) -> String {
  base64::encode_config(bytes.as_bytes(), base64::URL_SAFE_NO_PAD)
}

fn export_key_rsa(
  format: ExportKeyFormat,
  key_data: RawKeyData,
) -> Result<ExportKeyResult, deno_core::anyhow::Error> {
  match format {
    ExportKeyFormat::Spki => {
      let subject_public_key = &key_data.as_rsa_public_key()?;

      // the SPKI structure
      let key_info = spki::SubjectPublicKeyInfo {
        algorithm: spki::AlgorithmIdentifier {
          // rsaEncryption(1)
          oid: spki::ObjectIdentifier::new("1.2.840.113549.1.1.1"),
          // parameters field should not be ommited (None).
          // It MUST have ASN.1 type NULL.
          parameters: Some(asn1::Any::from(asn1::Null)),
        },
        subject_public_key,
      };

      // Infallible because we know the public key is valid.
      let spki_der = key_info.to_vec().unwrap();
      Ok(ExportKeyResult::Spki(spki_der.into()))
    }
    ExportKeyFormat::Pkcs8 => {
      let private_key = key_data.as_rsa_private_key()?;

      // the PKCS#8 v1 structure
      // PrivateKeyInfo ::= SEQUENCE {
      //   version                   Version,
      //   privateKeyAlgorithm       PrivateKeyAlgorithmIdentifier,
      //   privateKey                PrivateKey,
      //   attributes           [0]  IMPLICIT Attributes OPTIONAL }

      // version is 0 when publickey is None

      let pk_info = rsa::pkcs8::PrivateKeyInfo {
        attributes: None,
        public_key: None,
        algorithm: rsa::pkcs8::AlgorithmIdentifier {
          // rsaEncryption(1)
          oid: rsa::pkcs8::ObjectIdentifier::new("1.2.840.113549.1.1.1"),
          // parameters field should not be ommited (None).
          // It MUST have ASN.1 type NULL as per defined in RFC 3279 Section 2.3.1
          parameters: Some(asn1::Any::from(asn1::Null)),
        },
        private_key,
      };

      // Infallible because we know the private key is valid.
      let pkcs8_der = pk_info.to_vec().unwrap();

      Ok(ExportKeyResult::Pkcs8(pkcs8_der.into()))
    }
    ExportKeyFormat::JwkPublic => {
      let public_key = key_data.as_rsa_public_key()?;
      let public_key = rsa::pkcs1::RsaPublicKey::from_der(&public_key)
        .map_err(|_| {
          custom_error(
            "DOMExceptionOperationError",
            "failed to decode public key",
          )
        })?;

      Ok(ExportKeyResult::JwkPublicRsa {
        n: uint_to_b64(public_key.modulus),
        e: uint_to_b64(public_key.public_exponent),
      })
    }
    ExportKeyFormat::JwkPrivate => {
      let private_key = key_data.as_rsa_private_key()?;
      let private_key = rsa::pkcs1::RsaPrivateKey::from_der(private_key)
        .map_err(|_| {
          custom_error(
            "DOMExceptionOperationError",
            "failed to decode private key",
          )
        })?;

      Ok(ExportKeyResult::JwkPrivateRsa {
        n: uint_to_b64(private_key.modulus),
        e: uint_to_b64(private_key.public_exponent),
        d: uint_to_b64(private_key.private_exponent),
        p: uint_to_b64(private_key.prime1),
        q: uint_to_b64(private_key.prime2),
        dp: uint_to_b64(private_key.exponent1),
        dq: uint_to_b64(private_key.exponent2),
        qi: uint_to_b64(private_key.coefficient),
      })
    }
  }
}
