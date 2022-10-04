use crate::shared::*;
use const_oid::AssociatedOid;
use const_oid::ObjectIdentifier;
use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::op;
use deno_core::ZeroCopyBuf;
use elliptic_curve::sec1::ToEncodedPoint;
use p256::pkcs8::DecodePrivateKey;
use rsa::pkcs1::UIntRef;
use serde::Deserialize;
use serde::Serialize;
use spki::der::asn1;
use spki::der::Decode;
use spki::der::Encode;
use spki::AlgorithmIdentifier;

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
  Raw,
  Pkcs8,
  Spki,
  JwkPublic,
  JwkPrivate,
  JwkSecret,
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
  #[serde(rename = "ECDSA", rename_all = "camelCase")]
  Ecdsa { named_curve: EcNamedCurve },
  #[serde(rename = "ECDH", rename_all = "camelCase")]
  Ecdh { named_curve: EcNamedCurve },
  #[serde(rename = "AES")]
  Aes {},
  #[serde(rename = "HMAC")]
  Hmac {},
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ExportKeyResult {
  Raw(ZeroCopyBuf),
  Pkcs8(ZeroCopyBuf),
  Spki(ZeroCopyBuf),
  JwkSecret {
    k: String,
  },
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
  JwkPublicEc {
    x: String,
    y: String,
  },
  JwkPrivateEc {
    x: String,
    y: String,
    d: String,
  },
}

#[op]
pub fn op_crypto_export_key(
  opts: ExportKeyOptions,
  key_data: RawKeyData,
) -> Result<ExportKeyResult, AnyError> {
  match opts.algorithm {
    ExportKeyAlgorithm::RsassaPkcs1v15 {}
    | ExportKeyAlgorithm::RsaPss {}
    | ExportKeyAlgorithm::RsaOaep {} => export_key_rsa(opts.format, key_data),
    ExportKeyAlgorithm::Ecdh { named_curve }
    | ExportKeyAlgorithm::Ecdsa { named_curve } => {
      export_key_ec(opts.format, key_data, opts.algorithm, named_curve)
    }
    ExportKeyAlgorithm::Aes {} | ExportKeyAlgorithm::Hmac {} => {
      export_key_symmetric(opts.format, key_data)
    }
  }
}

fn uint_to_b64(bytes: UIntRef) -> String {
  base64::encode_config(bytes.as_bytes(), base64::URL_SAFE_NO_PAD)
}

fn bytes_to_b64(bytes: &[u8]) -> String {
  base64::encode_config(bytes, base64::URL_SAFE_NO_PAD)
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
          oid: const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1"),
          // parameters field should not be ommited (None).
          // It MUST have ASN.1 type NULL.
          parameters: Some(asn1::AnyRef::from(asn1::Null)),
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
        public_key: None,
        algorithm: rsa::pkcs8::AlgorithmIdentifier {
          // rsaEncryption(1)
          oid: rsa::pkcs8::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1"),
          // parameters field should not be ommited (None).
          // It MUST have ASN.1 type NULL as per defined in RFC 3279 Section 2.3.1
          parameters: Some(asn1::AnyRef::from(asn1::Null)),
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
    _ => Err(unsupported_format()),
  }
}

fn export_key_symmetric(
  format: ExportKeyFormat,
  key_data: RawKeyData,
) -> Result<ExportKeyResult, deno_core::anyhow::Error> {
  match format {
    ExportKeyFormat::JwkSecret => {
      let bytes = key_data.as_secret_key()?;

      Ok(ExportKeyResult::JwkSecret {
        k: bytes_to_b64(bytes),
      })
    }
    _ => Err(unsupported_format()),
  }
}

fn export_key_ec(
  format: ExportKeyFormat,
  key_data: RawKeyData,
  algorithm: ExportKeyAlgorithm,
  named_curve: EcNamedCurve,
) -> Result<ExportKeyResult, deno_core::anyhow::Error> {
  match format {
    ExportKeyFormat::Raw => {
      let subject_public_key = match named_curve {
        EcNamedCurve::P256 => {
          let point = key_data.as_ec_public_key_p256()?;

          point.as_ref().to_vec()
        }
        EcNamedCurve::P384 => {
          let point = key_data.as_ec_public_key_p384()?;

          point.as_ref().to_vec()
        }
        EcNamedCurve::P521 => {
          return Err(data_error("Unsupported named curve"))
        }
      };
      Ok(ExportKeyResult::Raw(subject_public_key.into()))
    }
    ExportKeyFormat::Spki => {
      let subject_public_key = match named_curve {
        EcNamedCurve::P256 => {
          let point = key_data.as_ec_public_key_p256()?;

          point.as_ref().to_vec()
        }
        EcNamedCurve::P384 => {
          let point = key_data.as_ec_public_key_p384()?;

          point.as_ref().to_vec()
        }
        EcNamedCurve::P521 => {
          return Err(data_error("Unsupported named curve"))
        }
      };

      let alg_id = match named_curve {
        EcNamedCurve::P256 => AlgorithmIdentifier {
          oid: elliptic_curve::ALGORITHM_OID,
          parameters: Some((&p256::NistP256::OID).into()),
        },
        EcNamedCurve::P384 => AlgorithmIdentifier {
          oid: elliptic_curve::ALGORITHM_OID,
          parameters: Some((&p384::NistP384::OID).into()),
        },
        EcNamedCurve::P521 => {
          return Err(data_error("Unsupported named curve"))
        }
      };

      let alg_id = match algorithm {
        ExportKeyAlgorithm::Ecdh { .. } => AlgorithmIdentifier {
          oid: ObjectIdentifier::new_unwrap("1.3.132.1.12"),
          parameters: alg_id.parameters,
        },
        _ => alg_id,
      };

      // the SPKI structure
      let key_info = spki::SubjectPublicKeyInfo {
        algorithm: alg_id,
        subject_public_key: &subject_public_key,
      };

      let spki_der = key_info.to_vec().unwrap();

      Ok(ExportKeyResult::Spki(spki_der.into()))
    }
    ExportKeyFormat::Pkcs8 => {
      // private_key is a PKCS#8 DER-encoded private key
      let private_key = key_data.as_ec_private_key()?;

      Ok(ExportKeyResult::Pkcs8(private_key.to_vec().into()))
    }
    ExportKeyFormat::JwkPublic => match named_curve {
      EcNamedCurve::P256 => {
        let point = key_data.as_ec_public_key_p256()?;
        let coords = point.coordinates();

        if let p256::elliptic_curve::sec1::Coordinates::Uncompressed { x, y } =
          coords
        {
          Ok(ExportKeyResult::JwkPublicEc {
            x: bytes_to_b64(x),
            y: bytes_to_b64(y),
          })
        } else {
          Err(custom_error(
            "DOMExceptionOperationError",
            "failed to decode public key",
          ))
        }
      }
      EcNamedCurve::P384 => {
        let point = key_data.as_ec_public_key_p384()?;
        let coords = point.coordinates();

        if let p384::elliptic_curve::sec1::Coordinates::Uncompressed { x, y } =
          coords
        {
          Ok(ExportKeyResult::JwkPublicEc {
            x: bytes_to_b64(x),
            y: bytes_to_b64(y),
          })
        } else {
          Err(custom_error(
            "DOMExceptionOperationError",
            "failed to decode public key",
          ))
        }
      }
      EcNamedCurve::P521 => Err(data_error("Unsupported named curve")),
    },
    ExportKeyFormat::JwkPrivate => {
      let private_key = key_data.as_ec_private_key()?;

      match named_curve {
        EcNamedCurve::P256 => {
          let ec_key =
            p256::SecretKey::from_pkcs8_der(private_key).map_err(|_| {
              custom_error(
                "DOMExceptionOperationError",
                "failed to decode private key",
              )
            })?;

          let point = ec_key.public_key().to_encoded_point(false);
          if let elliptic_curve::sec1::Coordinates::Uncompressed { x, y } =
            point.coordinates()
          {
            Ok(ExportKeyResult::JwkPrivateEc {
              x: bytes_to_b64(x),
              y: bytes_to_b64(y),
              d: bytes_to_b64(&ec_key.to_be_bytes()),
            })
          } else {
            Err(data_error("expected valid public EC key"))
          }
        }

        EcNamedCurve::P384 => {
          let ec_key =
            p384::SecretKey::from_pkcs8_der(private_key).map_err(|_| {
              custom_error(
                "DOMExceptionOperationError",
                "failed to decode private key",
              )
            })?;

          let point = ec_key.public_key().to_encoded_point(false);
          if let elliptic_curve::sec1::Coordinates::Uncompressed { x, y } =
            point.coordinates()
          {
            Ok(ExportKeyResult::JwkPrivateEc {
              x: bytes_to_b64(x),
              y: bytes_to_b64(y),
              d: bytes_to_b64(&ec_key.to_be_bytes()),
            })
          } else {
            Err(data_error("expected valid public EC key"))
          }
        }
        _ => Err(not_supported_error("Unsupported namedCurve")),
      }
    }
    ExportKeyFormat::JwkSecret => Err(unsupported_format()),
  }
}
