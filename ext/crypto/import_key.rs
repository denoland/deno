use deno_core::error::AnyError;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use rsa::pkcs1::UIntBytes;
use serde::Deserialize;
use serde::Serialize;
use spki::der::Decodable;
use spki::der::Encodable;

use crate::shared::*;
use crate::OaepPrivateKeyParameters;
use crate::PssPrivateKeyParameters;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyData {
  Spki(ZeroCopyBuf),
  Pkcs8(ZeroCopyBuf),
  Raw(ZeroCopyBuf),
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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "algorithm")]
pub enum ImportKeyOptions {
  #[serde(rename = "RSASSA-PKCS1-v1_5")]
  RsassaPkcs1v15 { hash: ShaHash },
  #[serde(rename = "RSA-PSS")]
  RsaPss { hash: ShaHash },
  #[serde(rename = "RSA-OAEP")]
  RsaOaep { hash: ShaHash },
  #[serde(rename = "ECDSA", rename_all = "camelCase")]
  Ecdsa { named_curve: EcNamedCurve },
  #[serde(rename = "ECDH", rename_all = "camelCase")]
  Ecdh { named_curve: EcNamedCurve },
  #[serde(rename = "AES", rename_all = "camelCase")]
  Aes {},
  #[serde(rename = "HMAC", rename_all = "camelCase")]
  Hmac {},
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ImportKeyResult {
  #[serde(rename_all = "camelCase")]
  Rsa {
    raw_data: RawKeyData,
    modulus_length: usize,
    public_exponent: ZeroCopyBuf,
  },
  #[serde(rename_all = "camelCase")]
  Ec { raw_data: RawKeyData },
  #[serde(rename_all = "camelCase")]
  Aes { raw_data: RawKeyData },
  #[serde(rename_all = "camelCase")]
  Hmac { raw_data: RawKeyData },
}

pub fn op_crypto_import_key(
  _state: &mut OpState,
  opts: ImportKeyOptions,
  key_data: KeyData,
) -> Result<ImportKeyResult, AnyError> {
  match opts {
    ImportKeyOptions::RsassaPkcs1v15 { hash } => {
      import_key_rsassa(key_data, hash)
    }
    ImportKeyOptions::RsaPss { hash } => import_key_rsapss(key_data, hash),
    ImportKeyOptions::RsaOaep { hash } => import_key_rsaoaep(key_data, hash),
    ImportKeyOptions::Ecdsa { named_curve }
    | ImportKeyOptions::Ecdh { named_curve } => {
      import_key_ec(key_data, named_curve)
    }
    ImportKeyOptions::Aes {} => import_key_aes(key_data),
    ImportKeyOptions::Hmac {} => import_key_hmac(key_data),
  }
}

macro_rules! jwt_b64_int_or_err {
  ($name:ident, $b64:expr, $err:expr) => {
    let bytes = base64::decode_config($b64, base64::URL_SAFE)
      .map_err(|_| data_error($err))?;
    let $name = UIntBytes::new(&bytes).map_err(|_| data_error($err))?;
  };
}

fn import_key_rsa_jwk(
  key_data: KeyData,
) -> Result<ImportKeyResult, deno_core::anyhow::Error> {
  match key_data {
    KeyData::JwkPublicRsa { n, e } => {
      jwt_b64_int_or_err!(modulus, &n, "invalid modulus");
      jwt_b64_int_or_err!(public_exponent, &e, "invalid public exponent");

      let public_key = rsa::pkcs1::RsaPublicKey {
        modulus,
        public_exponent,
      };

      let data = public_key
        .to_vec()
        .map_err(|_| data_error("invalid rsa public key"))?;
      let public_exponent =
        public_key.public_exponent.as_bytes().to_vec().into();
      let modulus_length = public_key.modulus.as_bytes().len() * 8;

      Ok(ImportKeyResult::Rsa {
        raw_data: RawKeyData::Public(data.into()),
        modulus_length,
        public_exponent,
      })
    }
    KeyData::JwkPrivateRsa {
      n,
      e,
      d,
      p,
      q,
      dp,
      dq,
      qi,
    } => {
      jwt_b64_int_or_err!(modulus, &n, "invalid modulus");
      jwt_b64_int_or_err!(public_exponent, &e, "invalid public exponent");
      jwt_b64_int_or_err!(private_exponent, &d, "invalid private exponent");
      jwt_b64_int_or_err!(prime1, &p, "invalid first prime factor");
      jwt_b64_int_or_err!(prime2, &q, "invalid second prime factor");
      jwt_b64_int_or_err!(exponent1, &dp, "invalid first CRT exponent");
      jwt_b64_int_or_err!(exponent2, &dq, "invalid second CRT exponent");
      jwt_b64_int_or_err!(coefficient, &qi, "invalid CRT coefficient");

      let private_key = rsa::pkcs1::RsaPrivateKey {
        version: rsa::pkcs1::Version::TwoPrime,
        modulus,
        public_exponent,
        private_exponent,
        prime1,
        prime2,
        exponent1,
        exponent2,
        coefficient,
      };

      let data = private_key
        .to_vec()
        .map_err(|_| data_error("invalid rsa private key"))?;

      let public_exponent =
        private_key.public_exponent.as_bytes().to_vec().into();
      let modulus_length = private_key.modulus.as_bytes().len() * 8;

      Ok(ImportKeyResult::Rsa {
        raw_data: RawKeyData::Private(data.into()),
        modulus_length,
        public_exponent,
      })
    }
    _ => unreachable!(),
  }
}

fn import_key_rsassa(
  key_data: KeyData,
  hash: ShaHash,
) -> Result<ImportKeyResult, deno_core::anyhow::Error> {
  match key_data {
    KeyData::Spki(data) => {
      // 2-3.
      let pk_info = spki::SubjectPublicKeyInfo::from_der(&data)
        .map_err(|e| data_error(e.to_string()))?;

      // 4-5.
      let alg = pk_info.algorithm.oid;

      // 6.
      let pk_hash = match alg {
        // rsaEncryption
        RSA_ENCRYPTION_OID => None,
        // sha1WithRSAEncryption
        SHA1_RSA_ENCRYPTION_OID => Some(ShaHash::Sha1),
        // sha256WithRSAEncryption
        SHA256_RSA_ENCRYPTION_OID => Some(ShaHash::Sha256),
        // sha384WithRSAEncryption
        SHA384_RSA_ENCRYPTION_OID => Some(ShaHash::Sha384),
        // sha512WithRSAEncryption
        SHA512_RSA_ENCRYPTION_OID => Some(ShaHash::Sha512),
        _ => return Err(data_error("unsupported algorithm")),
      };

      // 7.
      if let Some(pk_hash) = pk_hash {
        if pk_hash != hash {
          return Err(data_error("hash mismatch"));
        }
      }

      // 8-9.
      let public_key =
        rsa::pkcs1::RsaPublicKey::from_der(pk_info.subject_public_key)
          .map_err(|e| data_error(e.to_string()))?;

      let bytes_consumed = public_key
        .encoded_len()
        .map_err(|e| data_error(e.to_string()))?;

      if bytes_consumed
        != spki::der::Length::new(pk_info.subject_public_key.len() as u16)
      {
        return Err(data_error("public key is invalid (too long)"));
      }

      let data = pk_info.subject_public_key.to_vec().into();
      let public_exponent =
        public_key.public_exponent.as_bytes().to_vec().into();
      let modulus_length = public_key.modulus.as_bytes().len() * 8;

      Ok(ImportKeyResult::Rsa {
        raw_data: RawKeyData::Public(data),
        modulus_length,
        public_exponent,
      })
    }
    KeyData::Pkcs8(data) => {
      // 2-3.
      let pk_info = rsa::pkcs8::PrivateKeyInfo::from_der(&data)
        .map_err(|e| data_error(e.to_string()))?;

      // 4-5.
      let alg = pk_info.algorithm.oid;

      // 6.
      let pk_hash = match alg {
        // rsaEncryption
        RSA_ENCRYPTION_OID => None,
        // sha1WithRSAEncryption
        SHA1_RSA_ENCRYPTION_OID => Some(ShaHash::Sha1),
        // sha256WithRSAEncryption
        SHA256_RSA_ENCRYPTION_OID => Some(ShaHash::Sha256),
        // sha384WithRSAEncryption
        SHA384_RSA_ENCRYPTION_OID => Some(ShaHash::Sha384),
        // sha512WithRSAEncryption
        SHA512_RSA_ENCRYPTION_OID => Some(ShaHash::Sha512),
        _ => return Err(data_error("unsupported algorithm")),
      };

      // 7.
      if let Some(pk_hash) = pk_hash {
        if pk_hash != hash {
          return Err(data_error("hash mismatch"));
        }
      }

      // 8-9.
      let private_key =
        rsa::pkcs1::RsaPrivateKey::from_der(pk_info.private_key)
          .map_err(|e| data_error(e.to_string()))?;

      let bytes_consumed = private_key
        .encoded_len()
        .map_err(|e| data_error(e.to_string()))?;

      if bytes_consumed
        != spki::der::Length::new(pk_info.private_key.len() as u16)
      {
        return Err(data_error("private key is invalid (too long)"));
      }

      let data = pk_info.private_key.to_vec().into();
      let public_exponent =
        private_key.public_exponent.as_bytes().to_vec().into();
      let modulus_length = private_key.modulus.as_bytes().len() * 8;

      Ok(ImportKeyResult::Rsa {
        raw_data: RawKeyData::Private(data),
        modulus_length,
        public_exponent,
      })
    }
    KeyData::JwkPublicRsa { .. } | KeyData::JwkPrivateRsa { .. } => {
      import_key_rsa_jwk(key_data)
    }
    _ => Err(unsupported_format()),
  }
}

fn import_key_rsapss(
  key_data: KeyData,
  hash: ShaHash,
) -> Result<ImportKeyResult, deno_core::anyhow::Error> {
  match key_data {
    KeyData::Spki(data) => {
      // 2-3.
      let pk_info = spki::SubjectPublicKeyInfo::from_der(&data)
        .map_err(|e| data_error(e.to_string()))?;

      // 4-5.
      let alg = pk_info.algorithm.oid;

      // 6.
      let pk_hash = match alg {
        // rsaEncryption
        RSA_ENCRYPTION_OID => None,
        // id-RSASSA-PSS
        RSASSA_PSS_OID => {
          let params = PssPrivateKeyParameters::try_from(
            pk_info
              .algorithm
              .parameters
              .ok_or_else(|| data_error("malformed parameters"))?,
          )
          .map_err(|_| data_error("malformed parameters"))?;

          let hash_alg = params.hash_algorithm;
          let hash = match hash_alg.oid {
            // id-sha1
            ID_SHA1_OID => Some(ShaHash::Sha1),
            // id-sha256
            ID_SHA256_OID => Some(ShaHash::Sha256),
            // id-sha384
            ID_SHA384_OID => Some(ShaHash::Sha384),
            // id-sha256
            ID_SHA512_OID => Some(ShaHash::Sha512),
            _ => return Err(data_error("unsupported hash algorithm")),
          };

          if params.mask_gen_algorithm.oid != ID_MFG1 {
            return Err(not_supported_error("unsupported hash algorithm"));
          }

          // TODO(lucacasonato):
          // If the parameters field of the maskGenAlgorithm field of params
          // is not an instance of the HashAlgorithm ASN.1 type that is
          // identical in content to the hashAlgorithm field of params,
          // throw a NotSupportedError.

          hash
        }
        _ => return Err(data_error("unsupported algorithm")),
      };

      // 7.
      if let Some(pk_hash) = pk_hash {
        if pk_hash != hash {
          return Err(data_error("hash mismatch"));
        }
      }

      // 8-9.
      let public_key =
        rsa::pkcs1::RsaPublicKey::from_der(pk_info.subject_public_key)
          .map_err(|e| data_error(e.to_string()))?;

      let bytes_consumed = public_key
        .encoded_len()
        .map_err(|e| data_error(e.to_string()))?;

      if bytes_consumed
        != spki::der::Length::new(pk_info.subject_public_key.len() as u16)
      {
        return Err(data_error("public key is invalid (too long)"));
      }

      let data = pk_info.subject_public_key.to_vec().into();
      let public_exponent =
        public_key.public_exponent.as_bytes().to_vec().into();
      let modulus_length = public_key.modulus.as_bytes().len() * 8;

      Ok(ImportKeyResult::Rsa {
        raw_data: RawKeyData::Public(data),
        modulus_length,
        public_exponent,
      })
    }
    KeyData::Pkcs8(data) => {
      // 2-3.
      let pk_info = rsa::pkcs8::PrivateKeyInfo::from_der(&data)
        .map_err(|e| data_error(e.to_string()))?;

      // 4-5.
      let alg = pk_info.algorithm.oid;

      // 6.
      // 6.
      let pk_hash = match alg {
        // rsaEncryption
        RSA_ENCRYPTION_OID => None,
        // id-RSASSA-PSS
        RSASSA_PSS_OID => {
          let params = PssPrivateKeyParameters::try_from(
            pk_info
              .algorithm
              .parameters
              .ok_or_else(|| not_supported_error("malformed parameters"))?,
          )
          .map_err(|_| not_supported_error("malformed parameters"))?;

          let hash_alg = params.hash_algorithm;
          let hash = match hash_alg.oid {
            // id-sha1
            ID_SHA1_OID => Some(ShaHash::Sha1),
            // id-sha256
            ID_SHA256_OID => Some(ShaHash::Sha256),
            // id-sha384
            ID_SHA384_OID => Some(ShaHash::Sha384),
            // id-sha256
            ID_SHA512_OID => Some(ShaHash::Sha512),
            _ => return Err(data_error("unsupported hash algorithm")),
          };

          if params.mask_gen_algorithm.oid != ID_MFG1 {
            return Err(not_supported_error("unsupported mask gen algorithm"));
          }

          // TODO(lucacasonato):
          // If the parameters field of the maskGenAlgorithm field of params
          // is not an instance of the HashAlgorithm ASN.1 type that is
          // identical in content to the hashAlgorithm field of params,
          // throw a NotSupportedError.

          hash
        }
        _ => return Err(data_error("unsupported algorithm")),
      };

      // 7.
      if let Some(pk_hash) = pk_hash {
        if pk_hash != hash {
          return Err(data_error("hash mismatch"));
        }
      }

      // 8-9.
      let private_key =
        rsa::pkcs1::RsaPrivateKey::from_der(pk_info.private_key)
          .map_err(|e| data_error(e.to_string()))?;

      let bytes_consumed = private_key
        .encoded_len()
        .map_err(|e| data_error(e.to_string()))?;

      if bytes_consumed
        != spki::der::Length::new(pk_info.private_key.len() as u16)
      {
        return Err(data_error("private key is invalid (too long)"));
      }

      let data = pk_info.private_key.to_vec().into();
      let public_exponent =
        private_key.public_exponent.as_bytes().to_vec().into();
      let modulus_length = private_key.modulus.as_bytes().len() * 8;

      Ok(ImportKeyResult::Rsa {
        raw_data: RawKeyData::Private(data),
        modulus_length,
        public_exponent,
      })
    }
    KeyData::JwkPublicRsa { .. } | KeyData::JwkPrivateRsa { .. } => {
      import_key_rsa_jwk(key_data)
    }
    _ => Err(unsupported_format()),
  }
}

fn import_key_rsaoaep(
  key_data: KeyData,
  hash: ShaHash,
) -> Result<ImportKeyResult, deno_core::anyhow::Error> {
  match key_data {
    KeyData::Spki(data) => {
      // 2-3.
      let pk_info = spki::SubjectPublicKeyInfo::from_der(&data)
        .map_err(|e| data_error(e.to_string()))?;

      // 4-5.
      let alg = pk_info.algorithm.oid;

      // 6.
      let pk_hash = match alg {
        // rsaEncryption
        RSA_ENCRYPTION_OID => None,
        // id-RSAES-OAEP
        RSAES_OAEP_OID => {
          let params = OaepPrivateKeyParameters::try_from(
            pk_info
              .algorithm
              .parameters
              .ok_or_else(|| data_error("malformed parameters"))?,
          )
          .map_err(|_| data_error("malformed parameters"))?;

          let hash_alg = params.hash_algorithm;
          let hash = match hash_alg.oid {
            // id-sha1
            ID_SHA1_OID => Some(ShaHash::Sha1),
            // id-sha256
            ID_SHA256_OID => Some(ShaHash::Sha256),
            // id-sha384
            ID_SHA384_OID => Some(ShaHash::Sha384),
            // id-sha256
            ID_SHA512_OID => Some(ShaHash::Sha512),
            _ => return Err(data_error("unsupported hash algorithm")),
          };

          if params.mask_gen_algorithm.oid != ID_MFG1 {
            return Err(not_supported_error("unsupported hash algorithm"));
          }

          // TODO(lucacasonato):
          // If the parameters field of the maskGenAlgorithm field of params
          // is not an instance of the HashAlgorithm ASN.1 type that is
          // identical in content to the hashAlgorithm field of params,
          // throw a NotSupportedError.

          hash
        }
        _ => return Err(data_error("unsupported algorithm")),
      };

      // 7.
      if let Some(pk_hash) = pk_hash {
        if pk_hash != hash {
          return Err(data_error("hash mismatch"));
        }
      }

      // 8-9.
      let public_key =
        rsa::pkcs1::RsaPublicKey::from_der(pk_info.subject_public_key)
          .map_err(|e| data_error(e.to_string()))?;

      let bytes_consumed = public_key
        .encoded_len()
        .map_err(|e| data_error(e.to_string()))?;

      if bytes_consumed
        != spki::der::Length::new(pk_info.subject_public_key.len() as u16)
      {
        return Err(data_error("public key is invalid (too long)"));
      }

      let data = pk_info.subject_public_key.to_vec().into();
      let public_exponent =
        public_key.public_exponent.as_bytes().to_vec().into();
      let modulus_length = public_key.modulus.as_bytes().len() * 8;

      Ok(ImportKeyResult::Rsa {
        raw_data: RawKeyData::Public(data),
        modulus_length,
        public_exponent,
      })
    }
    KeyData::Pkcs8(data) => {
      // 2-3.
      let pk_info = rsa::pkcs8::PrivateKeyInfo::from_der(&data)
        .map_err(|e| data_error(e.to_string()))?;

      // 4-5.
      let alg = pk_info.algorithm.oid;

      // 6.
      // 6.
      let pk_hash = match alg {
        // rsaEncryption
        RSA_ENCRYPTION_OID => None,
        // id-RSAES-OAEP
        RSAES_OAEP_OID => {
          let params = OaepPrivateKeyParameters::try_from(
            pk_info
              .algorithm
              .parameters
              .ok_or_else(|| not_supported_error("malformed parameters"))?,
          )
          .map_err(|_| not_supported_error("malformed parameters"))?;

          let hash_alg = params.hash_algorithm;
          let hash = match hash_alg.oid {
            // id-sha1
            ID_SHA1_OID => Some(ShaHash::Sha1),
            // id-sha256
            ID_SHA256_OID => Some(ShaHash::Sha256),
            // id-sha384
            ID_SHA384_OID => Some(ShaHash::Sha384),
            // id-sha256
            ID_SHA512_OID => Some(ShaHash::Sha512),
            _ => return Err(data_error("unsupported hash algorithm")),
          };

          if params.mask_gen_algorithm.oid != ID_MFG1 {
            return Err(not_supported_error("unsupported mask gen algorithm"));
          }

          // TODO(lucacasonato):
          // If the parameters field of the maskGenAlgorithm field of params
          // is not an instance of the HashAlgorithm ASN.1 type that is
          // identical in content to the hashAlgorithm field of params,
          // throw a NotSupportedError.

          hash
        }
        _ => return Err(data_error("unsupported algorithm")),
      };

      // 7.
      if let Some(pk_hash) = pk_hash {
        if pk_hash != hash {
          return Err(data_error("hash mismatch"));
        }
      }

      // 8-9.
      let private_key =
        rsa::pkcs1::RsaPrivateKey::from_der(pk_info.private_key)
          .map_err(|e| data_error(e.to_string()))?;

      let bytes_consumed = private_key
        .encoded_len()
        .map_err(|e| data_error(e.to_string()))?;

      if bytes_consumed
        != spki::der::Length::new(pk_info.private_key.len() as u16)
      {
        return Err(data_error("private key is invalid (too long)"));
      }

      let data = pk_info.private_key.to_vec().into();
      let public_exponent =
        private_key.public_exponent.as_bytes().to_vec().into();
      let modulus_length = private_key.modulus.as_bytes().len() * 8;

      Ok(ImportKeyResult::Rsa {
        raw_data: RawKeyData::Private(data),
        modulus_length,
        public_exponent,
      })
    }
    KeyData::JwkPublicRsa { .. } | KeyData::JwkPrivateRsa { .. } => {
      import_key_rsa_jwk(key_data)
    }
    _ => Err(unsupported_format()),
  }
}

fn import_key_ec(
  key_data: KeyData,
  named_curve: EcNamedCurve,
) -> Result<ImportKeyResult, AnyError> {
  Ok(match key_data {
    KeyData::Raw(data) => {
      // The point is parsed and validated, ultimately the original data is
      // returned though.
      match named_curve {
        EcNamedCurve::P256 => {
          // 1-2.
          let point = p256::EncodedPoint::from_bytes(&data)
            .map_err(|_| data_error("invalid P-256 eliptic curve point"))?;
          // 3.
          if point.is_identity() {
            return Err(data_error("invalid P-256 eliptic curve point"));
          }
        }
        EcNamedCurve::P384 => {
          // 1-2.
          let point = p384::EncodedPoint::from_bytes(&data)
            .map_err(|_| data_error("invalid P-384 eliptic curve point"))?;
          // 3.
          if point.is_identity() {
            return Err(data_error("invalid P-384 eliptic curve point"));
          }
        }
      };
      ImportKeyResult::Ec {
        raw_data: RawKeyData::Public(data),
      }
    }
    _ => return Err(unsupported_format()),
  })
}

fn import_key_aes(key_data: KeyData) -> Result<ImportKeyResult, AnyError> {
  Ok(match key_data {
    KeyData::JwkSecret { k } => {
      let data = base64::decode_config(k, base64::URL_SAFE)
        .map_err(|_| data_error("invalid key data"))?;
      ImportKeyResult::Hmac {
        raw_data: RawKeyData::Secret(data.into()),
      }
    }
    _ => return Err(unsupported_format()),
  })
}

fn import_key_hmac(key_data: KeyData) -> Result<ImportKeyResult, AnyError> {
  Ok(match key_data {
    KeyData::JwkSecret { k } => {
      let data = base64::decode_config(k, base64::URL_SAFE)
        .map_err(|_| data_error("invalid key data"))?;
      ImportKeyResult::Hmac {
        raw_data: RawKeyData::Secret(data.into()),
      }
    }
    _ => return Err(unsupported_format()),
  })
}
