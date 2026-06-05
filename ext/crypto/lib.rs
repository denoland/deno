// Copyright 2018-2026 the Deno authors. MIT license.

use aws_lc_rs::digest;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_error::JsErrorBox;
pub use rand; // Re-export rand
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::thread_rng;
use serde::Deserialize;

mod decrypt;
mod ed25519;
mod encrypt;
mod export_key;
mod generate_key;
mod import_key;
mod key;
mod key_store;
mod mldsa;
mod mlkem;
mod shared;
mod web_cipher;
mod web_cryptokey;
mod web_derive;
mod web_generate;
mod web_import_export;
mod web_keymaker;
mod web_keyutil;
mod web_params;
mod web_signature;
mod web_subtle;
mod web_wrap_kem;
mod x25519;
mod x448;

pub use crate::decrypt::DecryptError;
pub use crate::ed25519::Ed25519Error;
pub use crate::encrypt::EncryptError;
pub use crate::export_key::ExportKeyError;
pub use crate::export_key::op_crypto_export_key;
pub use crate::generate_key::GenerateKeyError;
pub use crate::import_key::ImportKeyError;
use crate::key::CryptoHash;
pub use crate::mldsa::MlDsaError;
pub use crate::mlkem::MlKemError;
pub use crate::shared::RawKeyData;
pub use crate::shared::SharedError;
pub use crate::x448::X448Error;
pub use crate::x25519::X25519Error;

deno_core::extension!(deno_crypto,
  deps = [ deno_webidl, deno_web ],
  ops = [
    op_crypto_get_random_values,
    op_crypto_export_key,
    web_cryptokey::op_crypto_construct_key,
    encrypt::op_crypto_encrypt,
    decrypt::op_crypto_decrypt,
    web_derive::op_crypto_get_key_length,
    web_generate::op_crypto_generate_key_web,
    web_params::op_crypto_normalize_algorithm,
    web_params::op_crypto_is_algorithm_registered,
    web_subtle::op_crypto_create_crypto,
    web_wrap_kem::op_crypto_wrap_key_web,
    web_wrap_kem::op_crypto_unwrap_key_web,
    web_wrap_kem::op_crypto_encapsulate_web,
    web_wrap_kem::op_crypto_decapsulate_web,
    op_crypto_digest,
    op_crypto_random_uuid,
    op_crypto_base64url_decode,
    op_crypto_base64url_encode,
    key_store::op_crypto_key_store_insert,
    key_store::op_crypto_key_store_get,
    x25519::op_crypto_derive_bits_x25519,
    x25519::op_crypto_export_spki_x25519,
    x25519::op_crypto_export_pkcs8_x25519,
    x448::op_crypto_generate_x448_keypair,
    x448::op_crypto_derive_bits_x448,
    x448::op_crypto_x448_public_key,
    x448::op_crypto_export_spki_x448,
    x448::op_crypto_export_pkcs8_x448,
    ed25519::op_crypto_generate_ed25519_keypair,
    ed25519::op_crypto_sign_ed25519,
    ed25519::op_crypto_verify_ed25519,
    ed25519::op_crypto_export_spki_ed25519,
    ed25519::op_crypto_export_pkcs8_ed25519,
    mldsa::op_crypto_mldsa_export_pkcs8,
    mldsa::op_crypto_mldsa_export_spki,
    mldsa::op_crypto_sign_mldsa,
    mldsa::op_crypto_verify_mldsa,
    mlkem::op_crypto_ml_kem_from_seed,
    mlkem::op_crypto_ml_kem_encapsulate,
    mlkem::op_crypto_ml_kem_decapsulate,
    mlkem::op_crypto_ml_kem_get_public_key,
  ],
  objects = [
    web_cryptokey::CryptoKey,
    web_subtle::Crypto,
    web_subtle::SubtleCrypto,
  ],
  lazy_loaded_js = [ "00_crypto.js" ],
  options = {
    maybe_seed: Option<u64>,
  },
  state = |state, options| {
    if let Some(seed) = options.maybe_seed {
      state.put(StdRng::seed_from_u64(seed));
    }
  },
);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CryptoError {
  #[class(inherit)]
  #[error(transparent)]
  General(
    #[from]
    #[inherit]
    SharedError,
  ),
  #[class(inherit)]
  #[error(transparent)]
  JoinError(
    #[from]
    #[inherit]
    tokio::task::JoinError,
  ),
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] rsa::pkcs1::der::Error),
  #[class(type)]
  #[error("unsupported algorithm")]
  UnsupportedAlgorithm,
  #[class("DOMExceptionNotSupportedError")]
  #[error("Unrecognized algorithm name")]
  UnrecognizedAlgorithm,
  #[class(type)]
  #[error(
    "Failed to read the 'outputLength' member: required member is undefined for {0}"
  )]
  XofOutputLengthMissing(&'static str),
  #[class("DOMExceptionOperationError")]
  #[error("'length' must be a positive multiple of 8 for {0}")]
  XofLengthRequired(&'static str),
  #[class("DOMExceptionOperationError")]
  #[error("'length' must be a multiple of 8 for {0}")]
  XofLengthMultiple(&'static str),
  #[class("DOMExceptionOperationError")]
  #[error("'domainSeparation' must be in [0x01, 0x7F]")]
  XofDomainSeparation,
  #[class(generic)]
  #[error(transparent)]
  KeyRejected(#[from] aws_lc_rs::error::KeyRejected),
  #[class(generic)]
  #[error(transparent)]
  RSA(#[from] rsa::Error),
  #[class(generic)]
  #[error(transparent)]
  Pkcs1(#[from] rsa::pkcs1::Error),
  #[class(generic)]
  #[error(transparent)]
  Unspecified(#[from] aws_lc_rs::error::Unspecified),
  #[class(generic)]
  #[error(transparent)]
  P256Ecdsa(#[from] p256::ecdsa::Error),
  #[class(generic)]
  #[error(transparent)]
  Base64Decode(#[from] base64::DecodeError),
  #[class(type)]
  #[error("Invalid key length")]
  InvalidKeyLength,
  #[class("DOMExceptionQuotaExceededError")]
  #[error(
    "The ArrayBufferView's byte length ({0}) exceeds the number of bytes of entropy available via this API (65536)"
  )]
  ArrayBufferViewLengthExceeded(usize),
  #[class(inherit)]
  #[error(transparent)]
  Other(
    #[from]
    #[inherit]
    JsErrorBox,
  ),
}

#[op2]
pub fn op_crypto_base64url_decode(
  #[string] data: String,
) -> Result<Uint8Array, CryptoError> {
  let data: Vec<u8> = BASE64_URL_SAFE_NO_PAD.decode(data)?;
  Ok(data.into())
}

#[op2]
#[string]
pub fn op_crypto_base64url_encode(#[buffer] data: JsBuffer) -> String {
  let data: String = BASE64_URL_SAFE_NO_PAD.encode(data);
  data
}

#[op2(fast)]
pub fn op_crypto_get_random_values(
  state: &mut OpState,
  #[buffer] out: &mut [u8],
) -> Result<(), CryptoError> {
  if out.len() > 65536 {
    return Err(CryptoError::ArrayBufferViewLengthExceeded(out.len()));
  }

  let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
  if let Some(seeded_rng) = maybe_seeded_rng {
    seeded_rng.fill(out);
  } else {
    let mut rng = thread_rng();
    rng.fill(out);
  }

  Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyFormat {
  Raw,
  Pkcs8,
  Spki,
}

#[op2]
#[string]
pub fn op_crypto_random_uuid(
  state: &mut OpState,
) -> Result<String, CryptoError> {
  let maybe_seeded_rng = state.try_borrow_mut::<StdRng>();
  let uuid = if let Some(seeded_rng) = maybe_seeded_rng {
    let mut bytes = [0u8; 16];
    seeded_rng.fill(&mut bytes);
    fast_uuid_v4(&mut bytes)
  } else {
    let mut rng = thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    fast_uuid_v4(&mut bytes)
  };

  Ok(uuid)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "name")]
pub enum SubtleDigestXof {
  #[serde(rename = "SHAKE128", rename_all = "camelCase")]
  Shake128 { length: u32 },
  #[serde(rename = "SHAKE256", rename_all = "camelCase")]
  Shake256 { length: u32 },
  #[serde(rename = "cSHAKE128", rename_all = "camelCase")]
  CShake128 {
    length: u32,
    #[serde(with = "serde_bytes", default)]
    function_name: Option<Vec<u8>>,
    #[serde(with = "serde_bytes", default)]
    customization: Option<Vec<u8>>,
  },
  #[serde(rename = "cSHAKE256", rename_all = "camelCase")]
  CShake256 {
    length: u32,
    #[serde(with = "serde_bytes", default)]
    function_name: Option<Vec<u8>>,
    #[serde(with = "serde_bytes", default)]
    customization: Option<Vec<u8>>,
  },
  #[serde(rename = "TurboSHAKE128", rename_all = "camelCase")]
  TurboShake128 {
    length: u32,
    domain_separation: Option<u8>,
  },
  #[serde(rename = "TurboSHAKE256", rename_all = "camelCase")]
  TurboShake256 {
    length: u32,
    domain_separation: Option<u8>,
  },
}

/// Compute a XOF (SHAKE/cSHAKE/TurboSHAKE) digest. Used by the normalized
/// `op_crypto_digest` / `digest_web`.
fn compute_xof(
  algorithm: SubtleDigestXof,
  data: &[u8],
) -> Result<Vec<u8>, CryptoError> {
  use sha3::digest::ExtendableOutput;
  use sha3::digest::Update;
  use sha3::digest::XofReader;

  let length_bits = match &algorithm {
    SubtleDigestXof::Shake128 { length, .. }
    | SubtleDigestXof::Shake256 { length, .. }
    | SubtleDigestXof::CShake128 { length, .. }
    | SubtleDigestXof::CShake256 { length, .. }
    | SubtleDigestXof::TurboShake128 { length, .. }
    | SubtleDigestXof::TurboShake256 { length, .. } => *length,
  };
  if !length_bits.is_multiple_of(8) {
    return Err(CryptoError::InvalidKeyLength);
  }
  let out_len = (length_bits / 8) as usize;
  let mut out = vec![0u8; out_len];

  match algorithm {
    SubtleDigestXof::Shake128 { .. } => {
      let mut h = sha3::Shake128::default();
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::Shake256 { .. } => {
      let mut h = sha3::Shake256::default();
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::CShake128 {
      function_name,
      customization,
      ..
    } => {
      use sha3::digest::core_api::CoreWrapper;
      let core = sha3::CShake128Core::new_with_function_name(
        function_name.as_deref().unwrap_or(&[]),
        customization.as_deref().unwrap_or(&[]),
      );
      let mut h: sha3::CShake128 = CoreWrapper::from_core(core);
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::CShake256 {
      function_name,
      customization,
      ..
    } => {
      use sha3::digest::core_api::CoreWrapper;
      let core = sha3::CShake256Core::new_with_function_name(
        function_name.as_deref().unwrap_or(&[]),
        customization.as_deref().unwrap_or(&[]),
      );
      let mut h: sha3::CShake256 = CoreWrapper::from_core(core);
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::TurboShake128 {
      domain_separation, ..
    } => {
      use sha3::digest::core_api::CoreWrapper;
      let d = domain_separation.unwrap_or(0x1F);
      if !(0x01..=0x7F).contains(&d) {
        return Err(CryptoError::UnsupportedAlgorithm);
      }
      let core = sha3::TurboShake128Core::new(d);
      let mut h: sha3::TurboShake128 = CoreWrapper::from_core(core);
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
    SubtleDigestXof::TurboShake256 {
      domain_separation, ..
    } => {
      use sha3::digest::core_api::CoreWrapper;
      let d = domain_separation.unwrap_or(0x1F);
      if !(0x01..=0x7F).contains(&d) {
        return Err(CryptoError::UnsupportedAlgorithm);
      }
      let core = sha3::TurboShake256Core::new(d);
      let mut h: sha3::TurboShake256 = CoreWrapper::from_core(core);
      h.update(data);
      h.finalize_xof().read(&mut out);
    }
  }

  Ok(out)
}

/// A WebCrypto `AlgorithmIdentifier` for `SubtleCrypto.digest`: either a bare
/// algorithm-name string, or an object with `name` plus the XOF parameters.
/// The case-insensitive normalization + validation that used to live in
/// `00_crypto.js` is performed in `op_crypto_digest`.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum DigestAlgorithm {
  Name(String),
  Detailed(DigestDetailed),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DigestDetailed {
  name: String,
  output_length: Option<u32>,
  #[serde(with = "serde_bytes", default)]
  function_name: Option<Vec<u8>>,
  #[serde(with = "serde_bytes", default)]
  customization: Option<Vec<u8>>,
  domain_separation: Option<u8>,
}

/// Canonicalize a digest algorithm name case-insensitively against the
/// `supportedAlgorithms["digest"]` registry (WebCrypto normalize-an-algorithm).
fn canonical_digest_name(name: &str) -> Option<&'static str> {
  const NAMES: &[&str] = &[
    "SHA-1",
    "SHA-256",
    "SHA-384",
    "SHA-512",
    "SHA3-256",
    "SHA3-384",
    "SHA3-512",
    "SHAKE128",
    "SHAKE256",
    "cSHAKE128",
    "cSHAKE256",
    "TurboSHAKE128",
    "TurboSHAKE256",
  ];
  NAMES.iter().copied().find(|n| n.eq_ignore_ascii_case(name))
}

fn sha_hash_from_name(name: &str) -> Option<CryptoHash> {
  Some(match name {
    "SHA-1" => CryptoHash::Sha1,
    "SHA-256" => CryptoHash::Sha256,
    "SHA-384" => CryptoHash::Sha384,
    "SHA-512" => CryptoHash::Sha512,
    "SHA3-256" => CryptoHash::Sha3_256,
    "SHA3-384" => CryptoHash::Sha3_384,
    "SHA3-512" => CryptoHash::Sha3_512,
    _ => return None,
  })
}

/// `SubtleCrypto.digest(algorithm, data)` implemented in Rust: normalizes the
/// algorithm, validates XOF parameters, and dispatches to the SHA or XOF
/// compute. Replaces the hand-written normalization/validation in
/// `00_crypto.js`.
#[op2]
pub async fn op_crypto_digest(
  #[serde] algorithm: DigestAlgorithm,
  #[buffer] data: JsBuffer,
) -> Result<Uint8Array, CryptoError> {
  digest_web(algorithm, &data).await
}

/// Shared implementation of `SubtleCrypto.digest`, reused by both
/// `op_crypto_digest` and the `SubtleCrypto::digest` cppgc method
/// (`web_subtle.rs`).
pub(crate) async fn digest_web(
  algorithm: DigestAlgorithm,
  data: &[u8],
) -> Result<Uint8Array, CryptoError> {
  let data = data.to_vec();
  let detailed = match algorithm {
    DigestAlgorithm::Name(name) => DigestDetailed {
      name,
      output_length: None,
      function_name: None,
      customization: None,
      domain_separation: None,
    },
    DigestAlgorithm::Detailed(d) => d,
  };

  let name = canonical_digest_name(&detailed.name)
    .ok_or(CryptoError::UnrecognizedAlgorithm)?;

  // SHA-1/2/3 fixed-length digests.
  if let Some(hash) = sha_hash_from_name(name) {
    let output = spawn_blocking(move || {
      digest::digest(hash.into(), &data).as_ref().to_vec().into()
    })
    .await?;
    return Ok(output);
  }

  // XOF digests (SHAKE/cSHAKE/TurboSHAKE): the `outputLength` dictionary
  // member is required (a missing member is a WebIDL TypeError, distinct from a
  // present-but-invalid value which is an OperationError).
  let length = detailed
    .output_length
    .ok_or(CryptoError::XofOutputLengthMissing(name))?;
  if length == 0 {
    return Err(CryptoError::XofLengthRequired(name));
  }
  if !length.is_multiple_of(8) {
    return Err(CryptoError::XofLengthMultiple(name));
  }
  if matches!(name, "TurboSHAKE128" | "TurboSHAKE256")
    && let Some(d) = detailed.domain_separation
    && !(0x01..=0x7F).contains(&d)
  {
    return Err(CryptoError::XofDomainSeparation);
  }

  let DigestDetailed {
    function_name,
    customization,
    domain_separation,
    ..
  } = detailed;
  let xof = match name {
    "SHAKE128" => SubtleDigestXof::Shake128 { length },
    "SHAKE256" => SubtleDigestXof::Shake256 { length },
    "cSHAKE128" => SubtleDigestXof::CShake128 {
      length,
      function_name,
      customization,
    },
    "cSHAKE256" => SubtleDigestXof::CShake256 {
      length,
      function_name,
      customization,
    },
    "TurboSHAKE128" => SubtleDigestXof::TurboShake128 {
      length,
      domain_separation,
    },
    "TurboSHAKE256" => SubtleDigestXof::TurboShake256 {
      length,
      domain_separation,
    },
    _ => unreachable!(),
  };
  let output = spawn_blocking(move || compute_xof(xof, &data)).await??;
  Ok(Uint8Array::from(output))
}

const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

fn fast_uuid_v4(bytes: &mut [u8; 16]) -> String {
  // Set UUID version to 4 and variant to 1.
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  let buf = [
    HEX_CHARS[(bytes[0] >> 4) as usize],
    HEX_CHARS[(bytes[0] & 0x0f) as usize],
    HEX_CHARS[(bytes[1] >> 4) as usize],
    HEX_CHARS[(bytes[1] & 0x0f) as usize],
    HEX_CHARS[(bytes[2] >> 4) as usize],
    HEX_CHARS[(bytes[2] & 0x0f) as usize],
    HEX_CHARS[(bytes[3] >> 4) as usize],
    HEX_CHARS[(bytes[3] & 0x0f) as usize],
    b'-',
    HEX_CHARS[(bytes[4] >> 4) as usize],
    HEX_CHARS[(bytes[4] & 0x0f) as usize],
    HEX_CHARS[(bytes[5] >> 4) as usize],
    HEX_CHARS[(bytes[5] & 0x0f) as usize],
    b'-',
    HEX_CHARS[(bytes[6] >> 4) as usize],
    HEX_CHARS[(bytes[6] & 0x0f) as usize],
    HEX_CHARS[(bytes[7] >> 4) as usize],
    HEX_CHARS[(bytes[7] & 0x0f) as usize],
    b'-',
    HEX_CHARS[(bytes[8] >> 4) as usize],
    HEX_CHARS[(bytes[8] & 0x0f) as usize],
    HEX_CHARS[(bytes[9] >> 4) as usize],
    HEX_CHARS[(bytes[9] & 0x0f) as usize],
    b'-',
    HEX_CHARS[(bytes[10] >> 4) as usize],
    HEX_CHARS[(bytes[10] & 0x0f) as usize],
    HEX_CHARS[(bytes[11] >> 4) as usize],
    HEX_CHARS[(bytes[11] & 0x0f) as usize],
    HEX_CHARS[(bytes[12] >> 4) as usize],
    HEX_CHARS[(bytes[12] & 0x0f) as usize],
    HEX_CHARS[(bytes[13] >> 4) as usize],
    HEX_CHARS[(bytes[13] & 0x0f) as usize],
    HEX_CHARS[(bytes[14] >> 4) as usize],
    HEX_CHARS[(bytes[14] & 0x0f) as usize],
    HEX_CHARS[(bytes[15] >> 4) as usize],
    HEX_CHARS[(bytes[15] & 0x0f) as usize],
  ];

  // Safety: the buffer is all valid UTF-8.
  unsafe { String::from_utf8_unchecked(buf.to_vec()) }
}

#[test]
fn test_fast_uuid_v4_correctness() {
  let mut rng = thread_rng();
  let mut bytes = [0u8; 16];
  rng.fill(&mut bytes);
  let uuid = fast_uuid_v4(&mut bytes.clone());
  let uuid_lib = uuid::Builder::from_bytes(bytes)
    .set_variant(uuid::Variant::RFC4122)
    .set_version(uuid::Version::Random)
    .as_uuid()
    .to_string();
  assert_eq!(uuid, uuid_lib);
}
