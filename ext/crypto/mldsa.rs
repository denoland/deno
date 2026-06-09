// Copyright 2018-2026 the Deno authors. MIT license.

use aws_lc_rs::signature::KeyPair;
use aws_lc_rs::signature::UnparsedPublicKey;
use aws_lc_rs::unstable::signature::ML_DSA_44;
use aws_lc_rs::unstable::signature::ML_DSA_44_SIGNING;
use aws_lc_rs::unstable::signature::ML_DSA_65;
use aws_lc_rs::unstable::signature::ML_DSA_65_SIGNING;
use aws_lc_rs::unstable::signature::ML_DSA_87;
use aws_lc_rs::unstable::signature::ML_DSA_87_SIGNING;
use aws_lc_rs::unstable::signature::PqdsaKeyPair;
use aws_lc_rs::unstable::signature::PqdsaSigningAlgorithm;
use aws_lc_rs::unstable::signature::PqdsaVerificationAlgorithm;
use deno_core::ToJsBuffer;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use serde::Serialize;
use spki::der::Encode;
use spki::der::asn1::BitString;

use crate::key_store::CryptoKeyHandle;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum MlDsaError {
  #[class("DOMExceptionDataError")]
  #[error("Invalid key data")]
  InvalidKeyData,
  #[class("DOMExceptionOperationError")]
  #[error("Failed to export key")]
  FailedExport,
  #[class("DOMExceptionOperationError")]
  #[error("Signing failed")]
  SigningFailed,
  #[class("DOMExceptionNotSupportedError")]
  #[error("Non-empty context is not supported")]
  ContextNotSupported,
  #[class("DOMExceptionNotSupportedError")]
  #[error("unsupported ML-DSA PKCS#8 private key format")]
  UnsupportedPkcs8Format,
  #[class("DOMExceptionDataError")]
  #[error("Unknown ML-DSA variant")]
  UnknownVariant,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] spki::der::Error),
}

// ML-DSA OIDs (NIST CSOR), 2.16.840.1.101.3.4.3.{17,18,19}.
const ML_DSA_44_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.3.17");
const ML_DSA_65_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.3.18");
const ML_DSA_87_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.3.19");

#[derive(Clone, Copy)]
struct MlDsaParams {
  signing: &'static PqdsaSigningAlgorithm,
  verifying: &'static PqdsaVerificationAlgorithm,
  oid: const_oid::ObjectIdentifier,
  pub_key_len: usize,
  #[allow(
    dead_code,
    reason = "kept for symmetry/documentation; sizes come \
    from FIPS 204 Table 2 and aren't checked at this layer"
  )]
  priv_key_len: usize,
  sig_len: usize,
}

const ML_DSA_44_PARAMS: MlDsaParams = MlDsaParams {
  signing: &ML_DSA_44_SIGNING,
  verifying: &ML_DSA_44,
  oid: ML_DSA_44_OID,
  pub_key_len: 1312,
  priv_key_len: 2560,
  sig_len: 2420,
};

const ML_DSA_65_PARAMS: MlDsaParams = MlDsaParams {
  signing: &ML_DSA_65_SIGNING,
  verifying: &ML_DSA_65,
  oid: ML_DSA_65_OID,
  pub_key_len: 1952,
  priv_key_len: 4032,
  sig_len: 3309,
};

const ML_DSA_87_PARAMS: MlDsaParams = MlDsaParams {
  signing: &ML_DSA_87_SIGNING,
  verifying: &ML_DSA_87,
  oid: ML_DSA_87_OID,
  pub_key_len: 2592,
  priv_key_len: 4896,
  sig_len: 4627,
};

fn params(name: &str) -> Result<MlDsaParams, MlDsaError> {
  match name {
    "ML-DSA-44" => Ok(ML_DSA_44_PARAMS),
    "ML-DSA-65" => Ok(ML_DSA_65_PARAMS),
    "ML-DSA-87" => Ok(ML_DSA_87_PARAMS),
    _ => Err(MlDsaError::UnknownVariant),
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MlDsaKeys {
  private_key: ToJsBuffer,
  public_key: ToJsBuffer,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MlDsaImportedKeys {
  private_key: ToJsBuffer,
  public_key: ToJsBuffer,
  seed: Option<ToJsBuffer>,
}

#[op2]
#[serde]
pub fn op_crypto_mldsa_from_seed(
  #[string] name: &str,
  #[buffer] seed: &[u8],
) -> Result<MlDsaKeys, MlDsaError> {
  let p = params(name)?;
  let key_pair = PqdsaKeyPair::from_seed(p.signing, seed)
    .map_err(|_| MlDsaError::InvalidKeyData)?;
  let private_key = key_pair
    .private_key()
    .as_raw_bytes_vec()
    .map_err(|_| MlDsaError::FailedExport)?;
  let public_key = key_pair.public_key().as_ref().to_vec();
  Ok(MlDsaKeys {
    private_key: private_key.into(),
    public_key: public_key.into(),
  })
}

#[op2]
#[serde]
pub fn op_crypto_mldsa_from_pkcs8(
  #[string] name: &str,
  #[buffer] pkcs8: &[u8],
) -> Result<MlDsaImportedKeys, MlDsaError> {
  let p = params(name)?;

  // Classify the inner `ML-DSA-PrivateKey` CHOICE before handing the DER to
  // aws-lc, so we can return the spec-mandated error types:
  //   - expanded-key-only         -> NotSupportedError
  //   - both, seed mismatch        -> DataError
  //   - seed-only / consistent both -> ok
  match classify_pkcs8_inner(pkcs8) {
    // The expanded-key-only form is explicitly unsupported.
    Some(Pkcs8Inner::Expanded) => {
      return Err(MlDsaError::UnsupportedPkcs8Format);
    }
    // For the `both` form, the seed must regenerate exactly the expanded key.
    Some(Pkcs8Inner::Both { seed, expanded }) => {
      let derived = PqdsaKeyPair::from_seed(p.signing, &seed)
        .ok()
        .and_then(|kp| kp.private_key().as_raw_bytes_vec().ok());
      match derived {
        Some(d) if d == expanded => {}
        _ => return Err(MlDsaError::InvalidKeyData),
      }
    }
    // Seed-only (or an inner shape we don't classify) falls through to the
    // aws-lc parse below, which is authoritative for validity.
    Some(Pkcs8Inner::Seed(_)) | None => {}
  }

  let key_pair = PqdsaKeyPair::from_pkcs8(p.signing, pkcs8)
    .map_err(|_| MlDsaError::InvalidKeyData)?;
  let private_key = key_pair
    .private_key()
    .as_raw_bytes_vec()
    .map_err(|_| MlDsaError::FailedExport)?;
  let public_key = key_pair.public_key().as_ref().to_vec();
  // Best-effort: extract the seed from the inner OCTET STRING when the
  // PKCS#8 uses the Case 1 (`[0] OCTET STRING { seed }`) encoding from
  // draft-ietf-lamps-dilithium-certificates (the form aws-lc itself
  // emits). Case 2 (expanded only) leaves seed = None.
  let seed = extract_seed_from_pkcs8(pkcs8).map(Into::into);
  Ok(MlDsaImportedKeys {
    private_key: private_key.into(),
    public_key: public_key.into(),
    seed,
  })
}

/// The recognised shapes of the `ML-DSA-PrivateKey` CHOICE inside a PKCS#8
/// `privateKey` OCTET STRING (draft-ietf-lamps-dilithium-certificates).
enum Pkcs8Inner {
  /// `seed [0] IMPLICIT OCTET STRING (SIZE(32))` -> tag `0x80` (or `0xA0`). The seed is
  /// re-extracted by `extract_seed_from_pkcs8` after aws-lc validates the key,
  /// so it is not read here; carried for parity/documentation.
  #[allow(dead_code, reason = "seed re-extracted post-validation")]
  Seed(Vec<u8>),
  /// `both SEQUENCE { OCTET STRING seed, OCTET STRING expandedKey }`.
  Both { seed: Vec<u8>, expanded: Vec<u8> },
  /// `expandedKey OCTET STRING` -> tag `0x04`.
  Expanded,
}

/// Classify the DER inside the PKCS#8 `privateKey` OCTET STRING. Returns `None`
/// for anything that is not a well-formed `ML-DSA-PrivateKey` CHOICE; the
/// caller then defers to aws-lc for authoritative validation.
fn classify_pkcs8_inner(pkcs8: &[u8]) -> Option<Pkcs8Inner> {
  let inner = pkcs8_inner_private_key(pkcs8)?;
  match inner.first().copied()? {
    // `seed [0] IMPLICIT OCTET STRING (SIZE(32))`. aws-lc emits the primitive
    // context tag `0x80` (like ML-KEM); tolerate the constructed `0xA0` too.
    tag @ (0x80 | 0xA0) => {
      let body = parse_tag_and_length(inner, tag)?;
      if body.len() != 32 {
        return None;
      }
      Some(Pkcs8Inner::Seed(body.to_vec()))
    }
    // `SEQUENCE { OCTET STRING seed, OCTET STRING expandedKey }`.
    0x30 => {
      let seq_body = parse_tag_and_length(inner, 0x30)?;
      let (seed, after_seed) = parse_tlv(seq_body, 0x04)?;
      if seed.len() != 32 {
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
    0x04 => Some(Pkcs8Inner::Expanded),
    _ => None,
  }
}

/// Returns `Some(seed)` if the PKCS#8 v1 ML-DSA encoding contains a
/// 32-byte seed, otherwise `None`. Used to recover the seed for
/// round-tripping; signing and verifying still work without it.
fn extract_seed_from_pkcs8(pkcs8: &[u8]) -> Option<Vec<u8>> {
  let inner = pkcs8_inner_private_key(pkcs8)?;
  // Case 1: `seed [0] IMPLICIT OCTET STRING` -> primitive tag 0x80 (the form
  // aws-lc emits, like ML-KEM); tolerate the constructed 0xA0 too.
  if let Some(tag @ (0x80 | 0xA0)) = inner.first().copied() {
    let body = parse_tag_and_length(inner, tag)?;
    if body.len() == 32 {
      return Some(body.to_vec());
    }
  }
  // Case 3: `SEQUENCE { OCTET STRING seed, OCTET STRING expanded }` ->
  // tag 0x30. Take the first OCTET STRING.
  if inner.first().copied() == Some(0x30) {
    let seq_body = parse_tag_and_length(inner, 0x30)?;
    let seed_body = parse_tag_and_length(seq_body, 0x04)?;
    if seed_body.len() == 32 {
      return Some(seed_body.to_vec());
    }
  }
  None
}

/// Parses a DER tag-length-value where `tag` is the expected first byte
/// and returns the value slice on success.
fn parse_tag_and_length(buf: &[u8], tag: u8) -> Option<&[u8]> {
  Some(parse_tlv(buf, tag)?.0)
}

/// Parses a single DER tag-length-value where the first byte must equal `tag`.
/// Returns `(value, rest)` where `rest` is the bytes following the value.
fn parse_tlv(buf: &[u8], tag: u8) -> Option<(&[u8], &[u8])> {
  let (first, rest) = buf.split_first()?;
  if *first != tag {
    return None;
  }
  let (len_byte, rest) = rest.split_first()?;
  let (len, body_start) = if *len_byte & 0x80 == 0 {
    (*len_byte as usize, rest)
  } else {
    let n = (*len_byte & 0x7F) as usize;
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
  if body_start.len() < len {
    return None;
  }
  Some((&body_start[..len], &body_start[len..]))
}

/// Returns the bytes of the `privateKey` OCTET STRING inside a PKCS#8
/// `PrivateKeyInfo`, without pulling in a typed PKCS#8 decoder. Parsing is
/// kept self-contained on the file's own DER helpers (matching how the SPKI
/// paths use `spki::der`) and is best-effort: anything that is not a
/// well-formed `PrivateKeyInfo` returns `None` and the caller defers to aws-lc.
///
/// `PrivateKeyInfo ::= SEQUENCE { version INTEGER, privateKeyAlgorithm
/// AlgorithmIdentifier, privateKey OCTET STRING, attributes [0] OPTIONAL }`.
fn pkcs8_inner_private_key(pkcs8: &[u8]) -> Option<&[u8]> {
  let (seq, _) = parse_tlv(pkcs8, 0x30)?;
  let (_version, after_version) = parse_tlv(seq, 0x02)?;
  let (_algorithm, after_algorithm) = parse_tlv(after_version, 0x30)?;
  let (private_key, _) = parse_tlv(after_algorithm, 0x04)?;
  Some(private_key)
}

#[op2]
pub fn op_crypto_mldsa_from_spki(
  #[string] name: &str,
  #[buffer] spki: &[u8],
) -> Result<Uint8Array, MlDsaError> {
  let p = params(name)?;
  let pk_info = spki::SubjectPublicKeyInfoRef::try_from(spki)
    .map_err(|_| MlDsaError::InvalidKeyData)?;
  if pk_info.algorithm.oid != p.oid {
    return Err(MlDsaError::InvalidKeyData);
  }
  if pk_info.algorithm.parameters.is_some() {
    return Err(MlDsaError::InvalidKeyData);
  }
  let raw = pk_info.subject_public_key.raw_bytes();
  if raw.len() != p.pub_key_len {
    return Err(MlDsaError::InvalidKeyData);
  }
  Ok(raw.to_vec().into())
}

/// Validates a raw ML-DSA public key against the variant's expected length
/// (FIPS 204 Table 2) and returns a fresh copy. Keeping the length here makes
/// the Rust `params` table the single source of truth for key sizes.
#[op2]
pub fn op_crypto_mldsa_from_raw_public(
  #[string] name: &str,
  #[buffer] public_key_bytes: &[u8],
) -> Result<Uint8Array, MlDsaError> {
  let p = params(name)?;
  if public_key_bytes.len() != p.pub_key_len {
    return Err(MlDsaError::InvalidKeyData);
  }
  Ok(public_key_bytes.to_vec().into())
}

/// PKCS#8 v1 export for ML-DSA encodes the seed-only form
/// (`[0] (CONTEXT_SPECIFIC) OCTET STRING seed`), per the
/// `draft-ietf-lamps-dilithium-certificates` proposal that aws-lc
/// implements. The seed is therefore required; a key whose seed has
/// been discarded (e.g. one imported from a `raw-private` expanded key)
/// cannot be re-exported as PKCS#8.
#[op2]
pub fn op_crypto_mldsa_export_pkcs8(
  #[string] name: &str,
  #[buffer] seed: &[u8],
) -> Result<Uint8Array, MlDsaError> {
  let p = params(name)?;
  let key_pair = PqdsaKeyPair::from_seed(p.signing, seed)
    .map_err(|_| MlDsaError::InvalidKeyData)?;
  let pkcs8 = key_pair.to_pkcs8().map_err(|_| MlDsaError::FailedExport)?;
  Ok(pkcs8.as_ref().to_vec().into())
}

#[op2]
pub fn op_crypto_mldsa_export_spki(
  #[string] name: &str,
  #[buffer] public_key_bytes: &[u8],
) -> Result<Uint8Array, MlDsaError> {
  let p = params(name)?;
  if public_key_bytes.len() != p.pub_key_len {
    return Err(MlDsaError::InvalidKeyData);
  }
  let key_info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierOwned {
      oid: p.oid,
      parameters: None,
    },
    subject_public_key: BitString::from_bytes(public_key_bytes)?,
  };
  let der = key_info.to_der().map_err(|_| MlDsaError::FailedExport)?;
  Ok(der.into())
}

#[op2]
pub fn op_crypto_sign_mldsa(
  #[string] name: &str,
  #[cppgc] key: &CryptoKeyHandle,
  #[buffer] data: &[u8],
  #[buffer] context: Option<&[u8]>,
) -> Result<Uint8Array, MlDsaError> {
  let private_key_bytes = key.data().expanded_private_key();
  let p = params(name)?;
  // aws-lc-rs 1.16 does not expose a way to set the FIPS 204 §5.2 context
  // parameter for ML-DSA. The empty context is signed by default; reject
  // non-empty contexts until the underlying API supports them.
  if context.is_some_and(|c| !c.is_empty()) {
    return Err(MlDsaError::ContextNotSupported);
  }
  let key_pair =
    PqdsaKeyPair::from_raw_private_key(p.signing, private_key_bytes)
      .map_err(|_| MlDsaError::InvalidKeyData)?;
  let mut signature = vec![0u8; p.sig_len];
  key_pair
    .sign(data, &mut signature)
    .map_err(|_| MlDsaError::SigningFailed)?;
  Ok(signature.into())
}

#[op2]
pub fn op_crypto_verify_mldsa(
  #[string] name: &str,
  #[cppgc] key: &CryptoKeyHandle,
  #[buffer] data: &[u8],
  #[buffer] signature: &[u8],
  #[buffer] context: Option<&[u8]>,
) -> Result<bool, MlDsaError> {
  let p = params(name)?;
  // Match the limitation in op_crypto_sign_mldsa: only empty context is
  // currently supported. Reject a non-empty context with the same
  // NotSupportedError that signing raises, rather than silently returning
  // false, so the two paths stay symmetric.
  if context.is_some_and(|c| !c.is_empty()) {
    return Err(MlDsaError::ContextNotSupported);
  }
  let public_key_bytes = key.data().bytes();
  Ok(
    UnparsedPublicKey::new(p.verifying, public_key_bytes)
      .verify(data, signature)
      .is_ok(),
  )
}

trait AsRawBytesVec {
  fn as_raw_bytes_vec(&self) -> Result<Vec<u8>, ()>;
}

impl AsRawBytesVec for aws_lc_rs::unstable::signature::PqdsaPrivateKey<'_> {
  fn as_raw_bytes_vec(&self) -> Result<Vec<u8>, ()> {
    use aws_lc_rs::encoding::AsRawBytes;
    let raw = self.as_raw_bytes().map_err(|_| ())?;
    Ok(raw.as_ref().to_vec())
  }
}
