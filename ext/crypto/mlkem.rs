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
use spki::der::TagMode;
use spki::der::TagNumber;
use spki::der::asn1::BitString;
use spki::der::asn1::ContextSpecific;
use spki::der::asn1::OctetStringRef;

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

  /// Offset of the embedded encapsulation key inside the FIPS 203 §7.1
  /// expanded decapsulation key (`dk_PKE || ek || H(ek) || z`), where
  /// `dk_PKE` is `K * 384` bytes.
  fn ek_offset(self) -> usize {
    let k: usize = match self {
      MlKemVariant::MlKem512 => 2,
      MlKemVariant::MlKem768 => 3,
      MlKemVariant::MlKem1024 => 4,
    };
    k * 384
  }

  /// Extract the encapsulation (public) key bytes embedded in an expanded
  /// decapsulation key.
  fn public_from_expanded(
    self,
    expanded: &[u8],
  ) -> Result<Vec<u8>, MlKemError> {
    if expanded.len() != self.private_key_size() {
      return Err(MlKemError::InvalidKeyData);
    }
    let offset = self.ek_offset();
    let pub_len = self.public_key_size();
    let public_key = &expanded[offset..offset + pub_len];
    // Validate by constructing an EncapsulationKey.
    kem::EncapsulationKey::new(self.algorithm(), public_key)
      .map_err(|_| MlKemError::InvalidKeyData)?;
    Ok(public_key.to_vec())
  }

  /// Expand a 64-byte FIPS 203 seed (`d || z`) into the expanded
  /// decapsulation key and its encapsulation key, using the pure-Rust
  /// `fips203` implementation (aws-lc-rs does not expose seed-based key
  /// derivation). The returned expanded bytes are the FIPS 203 §7.1
  /// standard encoding and interoperate with the aws-lc-rs `kem` operations.
  ///
  /// Returns `(expanded_decapsulation_key, encapsulation_key)`.
  fn expand_seed(self, seed: &[u8]) -> Result<(Vec<u8>, Vec<u8>), MlKemError> {
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

/// Import a PKCS#8 encoded ML-KEM decapsulation key.
///
/// Per the WICG Modern Algorithms spec and the
/// `ML-KEM-PrivateKey` CHOICE of `draft-ietf-lamps-kyber-certificates`, the
/// `privateKey` field is one of:
///   - `seed [0] OCTET STRING (SIZE(64))` — the required form, also emitted by
///     [`op_crypto_ml_kem_export_pkcs8`] and OpenSSL 3.5.
///   - `both SEQUENCE { OCTET STRING seed, OCTET STRING expandedKey }` —
///     optional; the seed must expand to exactly `expandedKey`
///     (`DataError` on mismatch).
///   - `expandedKey OCTET STRING` — explicitly rejected with
///     `NotSupportedError`.
#[op2]
pub fn op_crypto_ml_kem_import_pkcs8(
  #[buffer] data: &[u8],
) -> Result<MlKemPkcs8Import, MlKemError> {
  import_pkcs8(data)
}

fn import_pkcs8(data: &[u8]) -> Result<MlKemPkcs8Import, MlKemError> {
  let info = pkcs8::PrivateKeyInfo::from_der(data)
    .map_err(|_| MlKemError::InvalidKeyData)?;
  let variant = MlKemVariant::from_oid(&info.algorithm.oid)
    .ok_or(MlKemError::InvalidKeyData)?;
  if info.algorithm.parameters.is_some() {
    return Err(MlKemError::InvalidKeyData);
  }

  let seed = match decode_pkcs8_inner(info.private_key) {
    Some(Pkcs8Inner::Seed(seed)) => seed,
    Some(Pkcs8Inner::Both { seed, expanded }) => {
      // Consistency check: the seed must expand to exactly `expandedKey`.
      let (derived, _) = variant.expand_seed(&seed)?;
      if derived != expanded {
        return Err(MlKemError::InvalidKeyData);
      }
      seed
    }
    // The expanded-key-only form is explicitly unsupported.
    Some(Pkcs8Inner::Expanded) => {
      return Err(MlKemError::UnsupportedPkcs8Format);
    }
    None => return Err(MlKemError::InvalidKeyData),
  };

  let (private_key, public_key) = variant.expand_seed(&seed)?;
  Ok(MlKemPkcs8Import {
    variant,
    seed: seed.into(),
    private_key: private_key.into(),
    public_key: public_key.into(),
  })
}

/// The recognised shapes of the `ML-KEM-PrivateKey` CHOICE inside a PKCS#8
/// `privateKey` OCTET STRING.
enum Pkcs8Inner {
  /// `seed [0] IMPLICIT OCTET STRING (SIZE(64))`.
  Seed(Vec<u8>),
  /// `both SEQUENCE { OCTET STRING seed, OCTET STRING expandedKey }`.
  Both { seed: Vec<u8>, expanded: Vec<u8> },
  /// `expandedKey OCTET STRING`.
  Expanded,
}

/// Classify the DER inside the PKCS#8 `privateKey` OCTET STRING. Returns `None`
/// for anything that is not a well-formed `ML-KEM-PrivateKey` CHOICE.
fn decode_pkcs8_inner(inner: &[u8]) -> Option<Pkcs8Inner> {
  match inner.first()? {
    // `[0] IMPLICIT OCTET STRING` (primitive context tag) holding the seed.
    0x80 => {
      let (seed, rest) = parse_tlv(inner, 0x80)?;
      if !rest.is_empty() || seed.len() != ML_KEM_SEED_SIZE {
        return None;
      }
      Some(Pkcs8Inner::Seed(seed.to_vec()))
    }
    // `SEQUENCE { OCTET STRING seed, OCTET STRING expandedKey }`.
    0x30 => {
      let (body, rest) = parse_tlv(inner, 0x30)?;
      if !rest.is_empty() {
        return None;
      }
      let (seed, after_seed) = parse_tlv(body, 0x04)?;
      if seed.len() != ML_KEM_SEED_SIZE {
        return None;
      }
      let (expanded, after_expanded) = parse_tlv(after_seed, 0x04)?;
      if !after_expanded.is_empty() {
        return None;
      }
      Some(Pkcs8Inner::Both {
        seed: seed.to_vec(),
        expanded: expanded.to_vec(),
      })
    }
    // A bare `OCTET STRING` is the expanded-key-only form.
    0x04 => {
      let (_, rest) = parse_tlv(inner, 0x04)?;
      if !rest.is_empty() {
        return None;
      }
      Some(Pkcs8Inner::Expanded)
    }
    _ => None,
  }
}

/// Parse a single DER tag-length-value where the first byte must equal `tag`.
/// Returns `(value, rest)` where `rest` is the bytes following the value.
fn parse_tlv(buf: &[u8], tag: u8) -> Option<(&[u8], &[u8])> {
  let (first, rest) = buf.split_first()?;
  if *first != tag {
    return None;
  }
  let (len_byte, rest) = rest.split_first()?;
  let (len, body) = if *len_byte & 0x80 == 0 {
    (*len_byte as usize, rest)
  } else {
    let n = (*len_byte & 0x7f) as usize;
    if n == 0 || n > 4 || rest.len() < n {
      return None;
    }
    let (len_bytes, after) = rest.split_at(n);
    let mut len = 0usize;
    for b in len_bytes {
      len = (len << 8) | (*b as usize);
    }
    (len, after)
  };
  if body.len() < len {
    return None;
  }
  Some(body.split_at(len))
}

#[derive(deno_core::ToV8)]
pub struct MlKemPkcs8Import {
  #[to_v8(serde)]
  pub variant: MlKemVariant,
  pub seed: Uint8Array,
  pub private_key: Uint8Array,
  pub public_key: Uint8Array,
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

/// Export the decapsulation key as PKCS#8 PrivateKeyInfo in the standard
/// seed-only form (`[0] IMPLICIT OCTET STRING` holding the 64-byte `d || z`
/// seed), per [draft-ietf-lamps-kyber-certificates]. Requires the seed, so a
/// key imported from the legacy expanded form (which has no recoverable seed)
/// cannot be re-exported as PKCS#8.
#[op2]
pub fn op_crypto_ml_kem_export_pkcs8(
  #[serde] variant: MlKemVariant,
  #[buffer] seed: &[u8],
) -> Result<Uint8Array, MlKemError> {
  Ok(encode_pkcs8_seed(variant, seed)?.into())
}

/// Encode a 64-byte seed as a PKCS#8 PrivateKeyInfo whose `privateKey` is the
/// seed-only form `[0] IMPLICIT OCTET STRING (SIZE(64))`.
fn encode_pkcs8_seed(
  variant: MlKemVariant,
  seed: &[u8],
) -> Result<Vec<u8>, MlKemError> {
  if seed.len() != ML_KEM_SEED_SIZE {
    return Err(MlKemError::InvalidKeyData);
  }
  // Validate the seed expands to a real key before emitting it.
  variant.expand_seed(seed)?;

  let seed_octet =
    OctetStringRef::new(seed).map_err(|_| MlKemError::OperationFailed)?;
  let inner = ContextSpecific {
    tag_number: TagNumber::N0,
    tag_mode: TagMode::Implicit,
    value: seed_octet,
  }
  .to_der()
  .map_err(|_| MlKemError::OperationFailed)?;

  let info = pkcs8::PrivateKeyInfo {
    algorithm: pkcs8::AlgorithmIdentifierRef {
      oid: variant.oid(),
      parameters: None,
    },
    private_key: &inner,
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
#[op2]
pub fn op_crypto_ml_kem_get_public_key(
  #[serde] variant: MlKemVariant,
  #[cppgc] key: &CryptoKeyHandle,
) -> Result<Uint8Array, MlKemError> {
  let private_key = key.data().expanded_private_key();
  let public_key = variant.public_from_expanded(private_key)?;
  Ok(public_key.into())
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

#[cfg(test)]
mod tests {
  use super::*;

  // The expanded decapsulation key and encapsulation key produced by the
  // `fips203` crate (from a seed) must be byte-compatible with the aws-lc-rs
  // `kem` operations, since both implement the FIPS 203 encodings. This is
  // load-bearing: generated/seed-imported keys are derived with `fips203` but
  // encapsulated/decapsulated with aws-lc-rs.
  #[test]
  fn seed_expansion_interops_with_aws_lc() {
    for variant in [
      MlKemVariant::MlKem512,
      MlKemVariant::MlKem768,
      MlKemVariant::MlKem1024,
    ] {
      let seed = [7u8; ML_KEM_SEED_SIZE];
      let (expanded, public) = variant.expand_seed(&seed).unwrap();
      assert_eq!(expanded.len(), variant.private_key_size());
      assert_eq!(public.len(), variant.public_key_size());

      // The embedded encapsulation key matches the standalone one.
      assert_eq!(variant.public_from_expanded(&expanded).unwrap(), public);

      // aws-lc-rs accepts both and a round-trip shared secret matches.
      let ek =
        kem::EncapsulationKey::new(variant.algorithm(), &public).unwrap();
      let dk =
        kem::DecapsulationKey::new(variant.algorithm(), &expanded).unwrap();
      let (ct, ss_enc) = ek.encapsulate().unwrap();
      let ss_dec = dk.decapsulate(kem::Ciphertext::from(ct.as_ref())).unwrap();
      assert_eq!(ss_enc.as_ref(), ss_dec.as_ref());
    }
  }

  // PKCS#8 export uses the seed form and round-trips through import.
  #[test]
  fn pkcs8_seed_roundtrip() {
    let variant = MlKemVariant::MlKem768;
    let seed = [3u8; ML_KEM_SEED_SIZE];
    let der = encode_pkcs8_seed(variant, &seed).unwrap();
    // The OID identifies the variant and the seed round-trips.
    let info = pkcs8::PrivateKeyInfo::from_der(&der).unwrap();
    assert_eq!(MlKemVariant::from_oid(&info.algorithm.oid), Some(variant));
    match decode_pkcs8_inner(info.private_key).unwrap() {
      Pkcs8Inner::Seed(decoded) => assert_eq!(decoded, seed),
      _ => panic!("expected seed form"),
    }
  }

  fn octet_string_der(bytes: &[u8]) -> Vec<u8> {
    OctetStringRef::new(bytes).unwrap().to_der().unwrap()
  }

  fn der_sequence(content: &[u8]) -> Vec<u8> {
    let mut out = vec![0x30u8];
    let len = content.len();
    if len < 0x80 {
      out.push(len as u8);
    } else {
      let bytes = len.to_be_bytes();
      let start = bytes.iter().position(|&b| b != 0).unwrap();
      let used = &bytes[start..];
      out.push(0x80 | used.len() as u8);
      out.extend_from_slice(used);
    }
    out.extend_from_slice(content);
    out
  }

  // The `both` form must classify as Both and the seed must expand to exactly
  // the embedded expanded key (the consistency check the importer enforces).
  #[test]
  fn pkcs8_both_form() {
    let variant = MlKemVariant::MlKem512;
    let seed = vec![5u8; ML_KEM_SEED_SIZE];
    let (expanded, _) = variant.expand_seed(&seed).unwrap();

    let mut content = octet_string_der(&seed);
    content.extend_from_slice(&octet_string_der(&expanded));
    let inner = der_sequence(&content);

    match decode_pkcs8_inner(&inner).unwrap() {
      Pkcs8Inner::Both {
        seed: s,
        expanded: e,
      } => {
        assert_eq!(s, seed);
        assert_eq!(e, expanded);
        // Consistency holds for a genuine pair.
        assert_eq!(variant.expand_seed(&s).unwrap().0, e);
      }
      _ => panic!("expected both form"),
    }
  }

  // The expanded-key-only form must be classified as Expanded so the importer
  // can reject it with NotSupportedError.
  #[test]
  fn pkcs8_expanded_only_classified() {
    let variant = MlKemVariant::MlKem512;
    let seed = vec![9u8; ML_KEM_SEED_SIZE];
    let (expanded, _) = variant.expand_seed(&seed).unwrap();
    let inner = octet_string_der(&expanded);
    assert!(matches!(
      decode_pkcs8_inner(&inner),
      Some(Pkcs8Inner::Expanded)
    ));
  }

  // op_crypto_ml_kem_import_pkcs8 rejects the expanded-only form with
  // NotSupportedError and a `both`-form mismatch with DataError.
  #[test]
  fn pkcs8_import_rejects_per_spec() {
    let variant = MlKemVariant::MlKem512;
    let seed = vec![1u8; ML_KEM_SEED_SIZE];
    let (expanded, _) = variant.expand_seed(&seed).unwrap();

    let wrap = |inner: &[u8]| -> Vec<u8> {
      let info = pkcs8::PrivateKeyInfo {
        algorithm: pkcs8::AlgorithmIdentifierRef {
          oid: variant.oid(),
          parameters: None,
        },
        private_key: inner,
        public_key: None,
      };
      let mut buf = Vec::new();
      info.encode_to_vec(&mut buf).unwrap();
      buf
    };

    // expanded-only -> NotSupportedError.
    let expanded_only = wrap(&octet_string_der(&expanded));
    assert!(matches!(
      import_pkcs8(&expanded_only),
      Err(MlKemError::UnsupportedPkcs8Format),
    ));

    // both form with a tampered expanded key -> DataError.
    let mut tampered = expanded.clone();
    tampered[0] ^= 0xff;
    let mut content = octet_string_der(&seed);
    content.extend_from_slice(&octet_string_der(&tampered));
    let both_bad = wrap(&der_sequence(&content));
    assert!(matches!(
      import_pkcs8(&both_bad),
      Err(MlKemError::InvalidKeyData),
    ));

    // valid seed form -> Ok with the seed preserved.
    let good = encode_pkcs8_seed(variant, &seed).unwrap();
    let imported = import_pkcs8(&good).unwrap();
    assert_eq!(imported.variant, variant);
  }
}
