// Copyright 2018-2026 the Deno authors. MIT license.

use aws_lc_rs::kem;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use fips203::traits::KeyGen;
use fips203::traits::SerDes;
use rsa::pkcs8;
use serde::Deserialize;
use serde::Serialize;
use spki::der::Decode;
use spki::der::Encode;
use spki::der::asn1::BitString;
use spki::der::asn1::OctetString;

use crate::key_store::CryptoKeyHandle;

// FIPS 203 OIDs (NIST 2.16.840.1.101.3.4.4.{1,2,3}).
pub const ML_KEM_512_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.4.1");
pub const ML_KEM_768_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.4.2");
pub const ML_KEM_1024_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.4.3");

/// FIPS 203 ML-KEM seed (`d || z`) length in bytes.
const ML_KEM_SEED_SIZE: usize = 64;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum MlKemError {
  #[class("DOMExceptionOperationError")]
  #[error("ML-KEM operation failed")]
  OperationFailed,
  #[class("DOMExceptionDataError")]
  #[error("invalid ML-KEM key data")]
  InvalidKeyData,
  #[class("DOMExceptionNotSupportedError")]
  #[error("unsupported ML-KEM PKCS#8 private key format")]
  UnsupportedPkcs8Format,
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
  pub fn algorithm(self) -> &'static kem::Algorithm<kem::AlgorithmId> {
    match self {
      MlKemVariant::MlKem512 => &kem::ML_KEM_512,
      MlKemVariant::MlKem768 => &kem::ML_KEM_768,
      MlKemVariant::MlKem1024 => &kem::ML_KEM_1024,
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    match name {
      "ML-KEM-512" => Some(MlKemVariant::MlKem512),
      "ML-KEM-768" => Some(MlKemVariant::MlKem768),
      "ML-KEM-1024" => Some(MlKemVariant::MlKem1024),
      _ => None,
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

  /// Expand a 64-byte FIPS 203 seed (`d || z`) into the expanded
  /// decapsulation key and its encapsulation key, using the pure-Rust
  /// `fips203` implementation (aws-lc-rs does not expose seed-based key
  /// derivation). The returned expanded bytes are the FIPS 203 §7.1
  /// standard encoding and interoperate with the aws-lc-rs `kem` operations.
  ///
  /// Returns `(expanded_decapsulation_key, encapsulation_key)`.
  pub fn expand_seed(
    self,
    seed: &[u8],
  ) -> Result<(Vec<u8>, Vec<u8>), MlKemError> {
    if seed.len() != ML_KEM_SEED_SIZE {
      return Err(MlKemError::InvalidKeyData);
    }
    let d: [u8; 32] = seed[..32]
      .try_into()
      .map_err(|_| MlKemError::InvalidKeyData)?;
    let z: [u8; 32] = seed[32..]
      .try_into()
      .map_err(|_| MlKemError::InvalidKeyData)?;
    let (private_key, public_key) = match self {
      MlKemVariant::MlKem512 => {
        let (ek, dk) = fips203::ml_kem_512::KG::keygen_from_seed(d, z);
        (dk.into_bytes().to_vec(), ek.into_bytes().to_vec())
      }
      MlKemVariant::MlKem768 => {
        let (ek, dk) = fips203::ml_kem_768::KG::keygen_from_seed(d, z);
        (dk.into_bytes().to_vec(), ek.into_bytes().to_vec())
      }
      MlKemVariant::MlKem1024 => {
        let (ek, dk) = fips203::ml_kem_1024::KG::keygen_from_seed(d, z);
        (dk.into_bytes().to_vec(), ek.into_bytes().to_vec())
      }
    };
    Ok((private_key, public_key))
  }
}

#[derive(deno_core::ToV8)]
pub struct MlKemEncapsulationOutput {
  pub ciphertext: Uint8Array,
  pub shared_secret: Uint8Array,
}

/// Derived from a FIPS 203 seed: the expanded decapsulation key together with
/// the encapsulation key.
#[derive(deno_core::ToV8)]
pub struct MlKemSeedKeys {
  pub private_key: Uint8Array,
  pub public_key: Uint8Array,
}

/// Derive the expanded ML-KEM decapsulation key and its encapsulation key from
/// a 64-byte FIPS 203 seed (`d || z`). Used by `generateKey`, `importKey`
/// (`raw-seed`/`jwk`) and seed-form PKCS#8 import.
#[op2]
pub fn op_crypto_ml_kem_from_seed(
  #[serde] variant: MlKemVariant,
  #[buffer] seed: &[u8],
) -> Result<MlKemSeedKeys, MlKemError> {
  let (private_key, public_key) = variant.expand_seed(seed)?;
  Ok(MlKemSeedKeys {
    private_key: private_key.into(),
    public_key: public_key.into(),
  })
}

/// Derive the expanded decapsulation key and encapsulation key from a 64-byte
/// FIPS 203 seed. Returns `(private_key, public_key)`.
pub fn ml_kem_from_seed(
  variant: MlKemVariant,
  seed: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), MlKemError> {
  variant.expand_seed(seed)
}

/// Encapsulate to an ML-KEM encapsulation key, returning ciphertext and
/// shared secret.
#[op2]
pub fn op_crypto_ml_kem_encapsulate(
  #[serde] variant: MlKemVariant,
  #[cppgc] key: &CryptoKeyHandle,
) -> Result<MlKemEncapsulationOutput, MlKemError> {
  let public_key = key.data().bytes();
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
  #[cppgc] key: &CryptoKeyHandle,
  #[buffer] ciphertext: &[u8],
) -> Result<Uint8Array, MlKemError> {
  let private_key = key.data().expanded_private_key();
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
/// Returns `(variant, public_key_bytes)`.
pub fn ml_kem_import_spki(
  data: &[u8],
) -> Result<(MlKemVariant, Vec<u8>), MlKemError> {
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
  Ok((variant, public_key.to_vec()))
}

/// Import a seed-form PKCS#8 encoded ML-KEM decapsulation key.
///
/// Per the WICG modern-algorithms spec only the seed form is supported: the
/// PrivateKeyInfo `privateKey` OCTET STRING must wrap a `seed [0] OCTET STRING
/// (SIZE (64))` (context-tag `0x80`). The legacy expanded-key form (a bare
/// OCTET STRING of the expanded decapsulation key) is rejected with a
/// `NotSupportedError`. Returns `(variant, seed_bytes)`.
pub fn ml_kem_import_pkcs8_seed(
  data: &[u8],
) -> Result<(MlKemVariant, Vec<u8>), MlKemError> {
  let info = pkcs8::PrivateKeyInfo::from_der(data)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  let variant = MlKemVariant::from_oid(&info.algorithm.oid)
    .ok_or(MlKemError::InvalidKeyData)?;
  if info.algorithm.parameters.is_some() {
    return Err(MlKemError::InvalidKeyData);
  }
  let body = info.private_key;
  // Expect `seed [0] OCTET STRING`: context-specific primitive tag 0x80,
  // followed by a single length byte (64 < 0x80) and the 64-byte seed.
  if body.len() == 2 + ML_KEM_SEED_SIZE
    && body[0] == 0x80
    && body[1] as usize == ML_KEM_SEED_SIZE
  {
    let seed = body[2..].to_vec();
    // Validate by expanding.
    variant.expand_seed(&seed)?;
    return Ok((variant, seed));
  }
  // Any other (e.g. expanded-key) form is not supported.
  Err(MlKemError::UnsupportedPkcs8Format)
}

/// Export the encapsulation key as SubjectPublicKeyInfo (SPKI).
/// Core of [`op_crypto_ml_kem_export_spki`].
pub fn ml_kem_export_spki(
  variant: MlKemVariant,
  public_key: &[u8],
) -> Result<Vec<u8>, MlKemError> {
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
  info.to_der().map_err(|_| MlKemError::OperationFailed)
}

/// Export the seed as a seed-form PKCS#8 PrivateKeyInfo.
///
/// Per the IETF ML-KEM private-key draft the `ML-KEM-PrivateKey` is a CHOICE
/// whose seed alternative is `seed [0] OCTET STRING (SIZE (64))`. The
/// PrivateKeyInfo `privateKey` OCTET STRING therefore wraps a single
/// context-tagged-`[0]` primitive carrying the 64-byte seed. This produces the
/// compact (~86-byte) form the WICG modern-algorithms spec mandates, rather
/// than the much larger expanded-key form older releases emitted.
/// Core of [`op_crypto_ml_kem_export_pkcs8`].
pub fn ml_kem_export_pkcs8_seed(
  variant: MlKemVariant,
  seed: &[u8],
) -> Result<Vec<u8>, MlKemError> {
  if seed.len() != ML_KEM_SEED_SIZE {
    return Err(MlKemError::InvalidKeyData);
  }
  // `seed [0] OCTET STRING` -> context-specific primitive tag 0x80.
  let mut choice = Vec::with_capacity(2 + seed.len());
  choice.push(0x80);
  choice.push(seed.len() as u8);
  choice.extend_from_slice(seed);

  let octet_string =
    OctetString::new(choice).map_err(|_| MlKemError::OperationFailed)?;
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
  Ok(buf)
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
/// Core of [`op_crypto_ml_kem_get_public_key`].
pub fn ml_kem_get_public_key(
  variant: MlKemVariant,
  private_key: &[u8],
) -> Result<Vec<u8>, MlKemError> {
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
  Ok(public_key.to_vec())
}

#[op2]
pub fn op_crypto_ml_kem_get_public_key(
  #[serde] variant: MlKemVariant,
  #[buffer] private_key: &[u8],
) -> Result<Uint8Array, MlKemError> {
  Ok(ml_kem_get_public_key(variant, private_key)?.into())
}

/// Validate an ML-KEM private key has the right size for the variant and is a
/// well-formed FIPS 203 expanded decapsulation key.
pub fn ml_kem_validate_private_key(
  variant: MlKemVariant,
  private_key: &[u8],
) -> bool {
  if private_key.len() != variant.private_key_size() {
    return false;
  }
  kem::DecapsulationKey::new(variant.algorithm(), private_key).is_ok()
}

/// Validate an ML-KEM public key has the right size for the variant.
pub fn ml_kem_validate_public_key(
  variant: MlKemVariant,
  public_key: &[u8],
) -> bool {
  if public_key.len() != variant.public_key_size() {
    return false;
  }
  kem::EncapsulationKey::new(variant.algorithm(), public_key).is_ok()
}
