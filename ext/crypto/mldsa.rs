// Copyright 2018-2026 the Deno authors. MIT license.

//! ML-DSA (FIPS 204 post-quantum signatures) support.
//!
//! This wraps `aws_lc_rs::unstable::signature`, which is only compiled into
//! `aws-lc-rs` when its own `fips` cargo feature is OFF (see
//! `aws-lc-rs-*/src/unstable.rs`: `#[cfg(not(feature = "fips"))] pub mod
//! signature;`). The AWS-LC-FIPS 3.x validated module does not yet expose
//! ML-DSA through this crate. When deno's own `fips` feature is on (which
//! turns on `rustls/fips` -> `aws-lc-rs/fips`), that module simply is not
//! there to link against.
//!
//! Rather than let the whole `deno_crypto` crate fail to compile under
//! `--features fips`, every public entry point below is split into a real
//! implementation (`not(feature = "fips")`) and a stub
//! (`feature = "fips"`) that reports ML-DSA as unsupported. Callers
//! (`subtle_sign`, `subtle_verify`, `subtle_import_key`, etc.) are
//! unchanged either way -- they see the same function signatures and the
//! same [`MlDsaError`] type.

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
  #[class("DOMExceptionNotSupportedError")]
  #[error(
    "ML-DSA is not available in this build: it was compiled with the \
     `fips` feature, and the linked AWS-LC-FIPS module does not yet \
     expose aws-lc-rs's unstable ML-DSA API"
  )]
  NotSupportedInFipsBuild,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] spki::der::Error),
}

pub struct MlDsaSeedBytes {
  pub private_key: Vec<u8>,
  pub public_key: Vec<u8>,
}

pub struct MlDsaPkcs8Bytes {
  pub private_key: Vec<u8>,
  #[allow(
    dead_code,
    reason = "exposed via the from_pkcs8 helper for the JS importKey \
              path that's still being slimmed"
  )]
  pub public_key: Vec<u8>,
  pub seed: Option<Vec<u8>>,
}

#[cfg(feature = "fips")]
pub use fips_stub::*;
#[cfg(not(feature = "fips"))]
pub use real::*;

/// Stand-in for every ML-DSA entry point when built with `--features fips`.
/// Each function has the exact signature of its real-implementation
/// counterpart in [`real`] so call sites need no `cfg` of their own.
#[cfg(feature = "fips")]
mod fips_stub {
  use super::MlDsaError;
  use super::MlDsaPkcs8Bytes;
  use super::MlDsaSeedBytes;

  pub(crate) fn mldsa_from_seed(
    _variant: u8,
    _seed: &[u8],
  ) -> Result<(Vec<u8>, Vec<u8>), MlDsaError> {
    Err(MlDsaError::NotSupportedInFipsBuild)
  }

  pub fn from_seed(
    _variant: u8,
    _seed: &[u8],
  ) -> Result<MlDsaSeedBytes, MlDsaError> {
    Err(MlDsaError::NotSupportedInFipsBuild)
  }

  pub fn from_pkcs8_native(
    _variant: u8,
    _pkcs8: &[u8],
  ) -> Result<MlDsaPkcs8Bytes, MlDsaError> {
    Err(MlDsaError::NotSupportedInFipsBuild)
  }

  pub fn from_spki(_variant: u8, _spki: &[u8]) -> Result<Vec<u8>, MlDsaError> {
    Err(MlDsaError::NotSupportedInFipsBuild)
  }

  pub(crate) fn mldsa_export_pkcs8(
    _variant: u8,
    _seed: &[u8],
  ) -> Result<Vec<u8>, MlDsaError> {
    Err(MlDsaError::NotSupportedInFipsBuild)
  }

  pub(crate) fn mldsa_export_spki(
    _variant: u8,
    _public_key_bytes: &[u8],
  ) -> Result<Vec<u8>, MlDsaError> {
    Err(MlDsaError::NotSupportedInFipsBuild)
  }

  pub(crate) fn mldsa_sign(
    _variant: u8,
    _private_key_bytes: &[u8],
    _data: &[u8],
    _context: Option<&[u8]>,
  ) -> Result<Vec<u8>, MlDsaError> {
    Err(MlDsaError::NotSupportedInFipsBuild)
  }

  pub(crate) fn mldsa_verify(
    _variant: u8,
    _public_key_bytes: &[u8],
    _data: &[u8],
    _signature: &[u8],
    _context: Option<&[u8]>,
  ) -> bool {
    false
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    /// Under `--features fips`, ML-DSA must fail closed -- report
    /// unsupported, never silently succeed or panic -- since the linked
    /// AWS-LC-FIPS module has no ML-DSA implementation to fall back on.
    #[test]
    fn stub_reports_not_supported_rather_than_succeeding() {
      let seed = [1u8; 32];
      assert!(matches!(
        from_seed(0, &seed),
        Err(MlDsaError::NotSupportedInFipsBuild)
      ));
      assert!(matches!(
        mldsa_export_pkcs8(0, &seed),
        Err(MlDsaError::NotSupportedInFipsBuild)
      ));
      assert!(!mldsa_verify(0, &[], b"msg", &[], None));
    }
  }
}

#[cfg(not(feature = "fips"))]
mod real {
  use super::MlDsaError;
  use super::MlDsaPkcs8Bytes;
  use super::MlDsaSeedBytes;
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
  use spki::der::Encode;
  use spki::der::asn1::BitString;

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

  fn params(variant: u8) -> Result<MlDsaParams, MlDsaError> {
    match variant {
      0 => Ok(ML_DSA_44_PARAMS),
      1 => Ok(ML_DSA_65_PARAMS),
      2 => Ok(ML_DSA_87_PARAMS),
      _ => Err(MlDsaError::UnknownVariant),
    }
  }

  pub(crate) fn mldsa_from_seed(
    variant: u8,
    seed: &[u8],
  ) -> Result<(Vec<u8>, Vec<u8>), MlDsaError> {
    let p = params(variant)?;
    let key_pair = PqdsaKeyPair::from_seed(p.signing, seed)
      .map_err(|_| MlDsaError::InvalidKeyData)?;
    let private_key = key_pair
      .private_key()
      .as_raw_bytes_vec()
      .map_err(|_| MlDsaError::FailedExport)?;
    let public_key = key_pair.public_key().as_ref().to_vec();
    Ok((private_key, public_key))
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
    use rsa::pkcs1::der::Decode;
    let pk_info = rsa::pkcs8::PrivateKeyInfo::from_der(pkcs8).ok()?;
    let inner = pk_info.private_key;
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
    use rsa::pkcs1::der::Decode;
    let pk_info = rsa::pkcs8::PrivateKeyInfo::from_der(pkcs8).ok()?;
    let inner = pk_info.private_key;
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

  /// Rust-side callable view of [`op_crypto_mldsa_from_seed`].
  pub fn from_seed(
    variant: u8,
    seed: &[u8],
  ) -> Result<MlDsaSeedBytes, MlDsaError> {
    let (private_key, public_key) = mldsa_from_seed(variant, seed)?;
    Ok(MlDsaSeedBytes {
      private_key,
      public_key,
    })
  }

  /// Rust-side callable view of [`op_crypto_mldsa_from_pkcs8`]; returns the
  /// triple as plain `Vec<u8>` for use by the Rust-native importKey
  /// dispatcher (no `Uint8Array` allocation through deno_core).
  pub fn from_pkcs8_native(
    variant: u8,
    pkcs8: &[u8],
  ) -> Result<MlDsaPkcs8Bytes, MlDsaError> {
    let p = params(variant)?;
    match classify_pkcs8_inner(pkcs8) {
      Some(Pkcs8Inner::Expanded) => {
        return Err(MlDsaError::UnsupportedPkcs8Format);
      }
      Some(Pkcs8Inner::Both { seed, expanded }) => {
        let derived = PqdsaKeyPair::from_seed(p.signing, &seed)
          .ok()
          .and_then(|kp| kp.private_key().as_raw_bytes_vec().ok());
        match derived {
          Some(d) if d == expanded => {}
          _ => return Err(MlDsaError::InvalidKeyData),
        }
      }
      Some(Pkcs8Inner::Seed(_)) | None => {}
    }
    let key_pair = PqdsaKeyPair::from_pkcs8(p.signing, pkcs8)
      .map_err(|_| MlDsaError::InvalidKeyData)?;
    let private_key = key_pair
      .private_key()
      .as_raw_bytes_vec()
      .map_err(|_| MlDsaError::FailedExport)?;
    let public_key = key_pair.public_key().as_ref().to_vec();
    let seed = extract_seed_from_pkcs8(pkcs8);
    Ok(MlDsaPkcs8Bytes {
      private_key,
      public_key,
      seed,
    })
  }

  /// Rust-side callable view of [`op_crypto_mldsa_from_spki`].
  pub fn from_spki(variant: u8, spki: &[u8]) -> Result<Vec<u8>, MlDsaError> {
    let p = params(variant)?;
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
    Ok(raw.to_vec())
  }

  /// PKCS#8 v1 export for ML-DSA encodes the seed-only form
  /// (`[0] (CONTEXT_SPECIFIC) OCTET STRING seed`), per the
  /// `draft-ietf-lamps-dilithium-certificates` proposal that aws-lc
  /// implements. The seed is therefore required; a key whose seed has
  /// been discarded (e.g. one imported from a `raw-private` expanded key)
  /// cannot be re-exported as PKCS#8.
  pub(crate) fn mldsa_export_pkcs8(
    variant: u8,
    seed: &[u8],
  ) -> Result<Vec<u8>, MlDsaError> {
    let p = params(variant)?;
    let key_pair = PqdsaKeyPair::from_seed(p.signing, seed)
      .map_err(|_| MlDsaError::InvalidKeyData)?;
    let pkcs8 = key_pair.to_pkcs8().map_err(|_| MlDsaError::FailedExport)?;
    Ok(pkcs8.as_ref().to_vec())
  }

  pub(crate) fn mldsa_export_spki(
    variant: u8,
    public_key_bytes: &[u8],
  ) -> Result<Vec<u8>, MlDsaError> {
    let p = params(variant)?;
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
    key_info.to_der().map_err(|_| MlDsaError::FailedExport)
  }

  /// ML-DSA sign. `private_key_bytes` is the FIPS 204 expanded private
  /// key (`d || z` for raw-seed imports), and `context` is the optional
  /// FIPS 204 §5.2 application context byte string (only `None` or empty
  /// is currently accepted). Called from [`crate::subtle_sign::run`].
  pub(crate) fn mldsa_sign(
    variant: u8,
    private_key_bytes: &[u8],
    data: &[u8],
    context: Option<&[u8]>,
  ) -> Result<Vec<u8>, MlDsaError> {
    let p = params(variant)?;
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
    Ok(signature)
  }

  /// ML-DSA verify. `public_key_bytes` is the raw FIPS 204 public key.
  /// Matches the sign-side limitation that only empty `context` is
  /// accepted. Called from [`crate::subtle_verify::run`].
  pub(crate) fn mldsa_verify(
    variant: u8,
    public_key_bytes: &[u8],
    data: &[u8],
    signature: &[u8],
    context: Option<&[u8]>,
  ) -> bool {
    let Ok(p) = params(variant) else {
      return false;
    };
    if context.is_some_and(|c| !c.is_empty()) {
      return false;
    }
    UnparsedPublicKey::new(p.verifying, public_key_bytes)
      .verify(data, signature)
      .is_ok()
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

  #[cfg(test)]
  mod tests {
    use super::*;

    #[test]
    fn roundtrip_from_seed() {
      let seed = [7u8; 32];
      let keys = from_seed(0, &seed).expect("ML-DSA-44 seed should be valid");
      assert_eq!(keys.public_key.len(), ML_DSA_44_PARAMS.pub_key_len);
    }

    #[test]
    fn sign_then_verify_roundtrips() {
      let seed = [9u8; 32];
      let (private_key, public_key) =
        mldsa_from_seed(1, &seed).expect("ML-DSA-65 seed should be valid");
      let msg = b"fips-mode smoke test";
      let sig =
        mldsa_sign(1, &private_key, msg, None).expect("signing should succeed");
      assert!(mldsa_verify(1, &public_key, msg, &sig, None));
      assert!(!mldsa_verify(1, &public_key, b"tampered", &sig, None));
    }

    #[test]
    fn unknown_variant_is_rejected() {
      assert!(matches!(params(99), Err(MlDsaError::UnknownVariant)));
    }
  }
} // mod real
