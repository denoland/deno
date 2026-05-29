// Copyright 2018-2026 the Deno authors. MIT license.

use aws_lc_rs::kem;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use rsa::pkcs8;
use serde::Deserialize;
use serde::Serialize;
use spki::der::Decode;
use spki::der::Encode;
use spki::der::asn1::BitString;
use spki::der::asn1::OctetString;

// FIPS 203 OIDs (NIST 2.16.840.1.101.3.4.4.{1,2,3}).
pub const ML_KEM_512_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.4.1");
pub const ML_KEM_768_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.4.2");
pub const ML_KEM_1024_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.4.3");

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum MlKemError {
  #[class("DOMExceptionOperationError")]
  #[error("ML-KEM operation failed")]
  OperationFailed,
  #[class("DOMExceptionDataError")]
  #[error("invalid ML-KEM key data")]
  InvalidKeyData,
  #[class("DOMExceptionNotSupportedError")]
  #[error("unsupported ML-KEM variant")]
  Unsupported,
}

#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Debug)]
pub enum MlKemVariant {
  #[serde(rename = "ML-KEM-512")]
  MlKem512,
  #[serde(rename = "ML-KEM-768")]
  MlKem768,
  #[serde(rename = "ML-KEM-1024")]
  MlKem1024,
}

impl MlKemVariant {
  fn algorithm(self) -> &'static kem::Algorithm<kem::AlgorithmId> {
    match self {
      MlKemVariant::MlKem512 => &kem::ML_KEM_512,
      MlKemVariant::MlKem768 => &kem::ML_KEM_768,
      MlKemVariant::MlKem1024 => &kem::ML_KEM_1024,
    }
  }

  pub fn oid(self) -> const_oid::ObjectIdentifier {
    match self {
      MlKemVariant::MlKem512 => ML_KEM_512_OID,
      MlKemVariant::MlKem768 => ML_KEM_768_OID,
      MlKemVariant::MlKem1024 => ML_KEM_1024_OID,
    }
  }

  pub fn from_oid(oid: &const_oid::ObjectIdentifier) -> Option<Self> {
    if *oid == ML_KEM_512_OID {
      Some(MlKemVariant::MlKem512)
    } else if *oid == ML_KEM_768_OID {
      Some(MlKemVariant::MlKem768)
    } else if *oid == ML_KEM_1024_OID {
      Some(MlKemVariant::MlKem1024)
    } else {
      None
    }
  }

  pub fn private_key_size(self) -> usize {
    match self {
      MlKemVariant::MlKem512 => 1632,
      MlKemVariant::MlKem768 => 2400,
      MlKemVariant::MlKem1024 => 3168,
    }
  }

  pub fn public_key_size(self) -> usize {
    match self {
      MlKemVariant::MlKem512 => 800,
      MlKemVariant::MlKem768 => 1184,
      MlKemVariant::MlKem1024 => 1568,
    }
  }
}

#[derive(deno_core::ToV8)]
pub struct MlKemKeyPair {
  pub private_key: Uint8Array,
  pub public_key: Uint8Array,
}

#[derive(deno_core::ToV8)]
pub struct MlKemEncapsulationOutput {
  pub ciphertext: Uint8Array,
  pub shared_secret: Uint8Array,
}

/// Generate an ML-KEM key pair. Returns the expanded (FIPS 203) form of the
/// decapsulation key together with the encapsulation key.
#[op2]
pub fn op_crypto_ml_kem_generate_key(
  #[serde] variant: MlKemVariant,
) -> Result<MlKemKeyPair, MlKemError> {
  let alg = variant.algorithm();
  let dk = kem::DecapsulationKey::generate(alg)
    .map_err(|_| MlKemError::OperationFailed)?;
  let ek = dk
    .encapsulation_key()
    .map_err(|_| MlKemError::OperationFailed)?;

  let dk_bytes = dk
    .key_bytes()
    .map_err(|_| MlKemError::OperationFailed)?
    .as_ref()
    .to_vec();
  let ek_bytes = ek
    .key_bytes()
    .map_err(|_| MlKemError::OperationFailed)?
    .as_ref()
    .to_vec();

  Ok(MlKemKeyPair {
    private_key: dk_bytes.into(),
    public_key: ek_bytes.into(),
  })
}

/// Encapsulate to an ML-KEM encapsulation key, returning ciphertext and
/// shared secret.
#[op2]
pub fn op_crypto_ml_kem_encapsulate(
  #[serde] variant: MlKemVariant,
  #[buffer] public_key: &[u8],
) -> Result<MlKemEncapsulationOutput, MlKemError> {
  let alg = variant.algorithm();
  let ek = kem::EncapsulationKey::new(alg, public_key)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  let (ciphertext, shared_secret) =
    ek.encapsulate().map_err(|_| MlKemError::OperationFailed)?;
  Ok(MlKemEncapsulationOutput {
    ciphertext: ciphertext.as_ref().to_vec().into(),
    shared_secret: shared_secret.as_ref().to_vec().into(),
  })
}

/// Decapsulate an ML-KEM ciphertext, returning the shared secret.
#[op2]
pub fn op_crypto_ml_kem_decapsulate(
  #[serde] variant: MlKemVariant,
  #[buffer] private_key: &[u8],
  #[buffer] ciphertext: &[u8],
) -> Result<Uint8Array, MlKemError> {
  let alg = variant.algorithm();
  let dk = kem::DecapsulationKey::new(alg, private_key)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  let ct = kem::Ciphertext::from(ciphertext);
  let shared_secret = dk
    .decapsulate(ct)
    .map_err(|_| MlKemError::OperationFailed)?;
  Ok(shared_secret.as_ref().to_vec().into())
}

/// Import a SubjectPublicKeyInfo (SPKI) encoded ML-KEM encapsulation key.
/// The OID inside the SPKI determines the variant; the JS layer is expected
/// to validate it matches the requested algorithm.
#[op2]
pub fn op_crypto_ml_kem_import_spki(
  #[buffer] data: &[u8],
) -> Result<MlKemSpkiImport, MlKemError> {
  let info = spki::SubjectPublicKeyInfoRef::try_from(data)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  let variant = MlKemVariant::from_oid(&info.algorithm.oid)
    .ok_or(MlKemError::InvalidKeyData)?;
  if info.algorithm.parameters.is_some() {
    return Err(MlKemError::InvalidKeyData);
  }
  let public_key = info
    .subject_public_key
    .as_bytes()
    .ok_or(MlKemError::InvalidKeyData)?;
  if public_key.len() != variant.public_key_size() {
    return Err(MlKemError::InvalidKeyData);
  }
  // Validate by constructing an EncapsulationKey.
  kem::EncapsulationKey::new(variant.algorithm(), public_key)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  Ok(MlKemSpkiImport {
    variant,
    public_key: public_key.to_vec().into(),
  })
}

#[derive(deno_core::ToV8)]
pub struct MlKemSpkiImport {
  #[to_v8(serde)]
  pub variant: MlKemVariant,
  pub public_key: Uint8Array,
}

/// Import a PKCS#8 encoded ML-KEM decapsulation key. Supports the "expanded"
/// (raw-private) form encoded inside the PrivateKeyInfo OCTET STRING.
#[op2]
pub fn op_crypto_ml_kem_import_pkcs8(
  #[buffer] data: &[u8],
) -> Result<MlKemPkcs8Import, MlKemError> {
  let info = pkcs8::PrivateKeyInfo::from_der(data)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  let variant = MlKemVariant::from_oid(&info.algorithm.oid)
    .ok_or(MlKemError::InvalidKeyData)?;
  if info.algorithm.parameters.is_some() {
    return Err(MlKemError::InvalidKeyData);
  }
  let private_key = info.private_key;
  if private_key.len() != variant.private_key_size() {
    return Err(MlKemError::InvalidKeyData);
  }
  // Validate by constructing the DecapsulationKey.
  kem::DecapsulationKey::new(variant.algorithm(), private_key)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  Ok(MlKemPkcs8Import {
    variant,
    private_key: private_key.to_vec().into(),
  })
}

#[derive(deno_core::ToV8)]
pub struct MlKemPkcs8Import {
  #[to_v8(serde)]
  pub variant: MlKemVariant,
  pub private_key: Uint8Array,
}

/// Export the encapsulation key as SubjectPublicKeyInfo (SPKI).
#[op2]
pub fn op_crypto_ml_kem_export_spki(
  #[serde] variant: MlKemVariant,
  #[buffer] public_key: &[u8],
) -> Result<Uint8Array, MlKemError> {
  if public_key.len() != variant.public_key_size() {
    return Err(MlKemError::InvalidKeyData);
  }
  let info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierOwned {
      oid: variant.oid(),
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(public_key)
      .map_err(|_| MlKemError::OperationFailed)?,
  };
  Ok(
    info
      .to_der()
      .map_err(|_| MlKemError::OperationFailed)?
      .into(),
  )
}

/// Export the decapsulation key (expanded form) as PKCS#8 PrivateKeyInfo.
#[op2]
pub fn op_crypto_ml_kem_export_pkcs8(
  #[serde] variant: MlKemVariant,
  #[buffer] private_key: &[u8],
) -> Result<Uint8Array, MlKemError> {
  if private_key.len() != variant.private_key_size() {
    return Err(MlKemError::InvalidKeyData);
  }
  // Validate the bytes look like a real decapsulation key.
  kem::DecapsulationKey::new(variant.algorithm(), private_key)
    .map_err(|_| MlKemError::InvalidKeyData)?;

  let octet_string =
    OctetString::new(private_key).map_err(|_| MlKemError::OperationFailed)?;
  let info = pkcs8::PrivateKeyInfo {
    algorithm: pkcs8::AlgorithmIdentifierRef {
      oid: variant.oid(),
      parameters: None,
    },
    private_key: octet_string.as_bytes(),
    public_key: None,
  };
  let mut buf = Vec::new();
  info
    .encode_to_vec(&mut buf)
    .map_err(|_| MlKemError::OperationFailed)?;
  Ok(buf.into())
}

/// Derive the encapsulation key (public key) bytes from a decapsulation key.
/// Used by `getPublicKey()` on ML-KEM keys.
///
/// The encapsulation key is embedded in the FIPS 203 §7.1 expanded
/// decapsulation key encoding:
///   dk' = dk_PKE || ek_PKE || H(ek) || z
/// where `dk_PKE` is K*384 bytes and `ek_PKE` is K*384 + 32 bytes.
/// aws-lc-rs `DecapsulationKey::new` / `encapsulation_key` round-trips do
/// not retain the EncapsulationKey, so we extract the public key bytes
/// directly from the serialized form.
#[op2]
pub fn op_crypto_ml_kem_get_public_key(
  #[serde] variant: MlKemVariant,
  #[buffer] private_key: &[u8],
) -> Result<Uint8Array, MlKemError> {
  if private_key.len() != variant.private_key_size() {
    return Err(MlKemError::InvalidKeyData);
  }
  let k: usize = match variant {
    MlKemVariant::MlKem512 => 2,
    MlKemVariant::MlKem768 => 3,
    MlKemVariant::MlKem1024 => 4,
  };
  let offset = k * 384;
  let pub_len = variant.public_key_size();
  let public_key = &private_key[offset..offset + pub_len];
  // Validate by constructing an EncapsulationKey.
  kem::EncapsulationKey::new(variant.algorithm(), public_key)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  Ok(public_key.to_vec().into())
}

/// Validate an ML-KEM private key has the right size for the variant and is a
/// well-formed FIPS 203 expanded decapsulation key.
#[op2]
pub fn op_crypto_ml_kem_validate_private_key(
  #[serde] variant: MlKemVariant,
  #[buffer] private_key: &[u8],
) -> bool {
  if private_key.len() != variant.private_key_size() {
    return false;
  }
  kem::DecapsulationKey::new(variant.algorithm(), private_key).is_ok()
}

/// Validate an ML-KEM public key has the right size for the variant.
#[op2]
pub fn op_crypto_ml_kem_validate_public_key(
  #[serde] variant: MlKemVariant,
  #[buffer] public_key: &[u8],
) -> bool {
  if public_key.len() != variant.public_key_size() {
    return false;
  }
  kem::EncapsulationKey::new(variant.algorithm(), public_key).is_ok()
}
