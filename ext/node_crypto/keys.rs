// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;

use base64::Engine;
use deno_core::FromV8;
use deno_core::GarbageCollected;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::serde_v8::BigInt as V8BigInt;
use deno_core::unsync::spawn_blocking;
use deno_error::JsErrorBox;
use ed25519_dalek::pkcs8::BitStringRef;
use elliptic_curve::JwkEcKey;
use num_bigint::BigInt;
use num_traits::FromPrimitive as _;
use pkcs8::DecodePrivateKey as _;
use pkcs8::Document;
use pkcs8::EncodePrivateKey as _;
use pkcs8::EncryptedPrivateKeyInfo;
use pkcs8::PrivateKeyInfo;
use pkcs8::SecretDocument;
use rand::RngCore as _;
use rand::thread_rng;
use rsa::RsaPrivateKey;
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPrivateKey as _;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs1::EncodeRsaPrivateKey as _;
use rsa::pkcs1::EncodeRsaPublicKey;
use rsa::traits::PrivateKeyParts;
use rsa::traits::PublicKeyParts;
use sec1::DecodeEcPrivateKey as _;
use sec1::LineEnding;
use sec1::der::Tag;
use sec1::der::Writer as _;
use sec1::pem::PemLabel as _;
use spki::DecodePublicKey as _;
use spki::EncodePublicKey as _;
use spki::SubjectPublicKeyInfoRef;
use spki::der::AnyRef;
use spki::der::Decode as _;
use spki::der::Encode as _;
use spki::der::PemWriter;
use spki::der::Reader as _;
use spki::der::asn1;
use spki::der::asn1::OctetStringRef;
use x509_parser::error::X509Error;
use x509_parser::x509;

use super::dh;
use super::dh::DiffieHellmanGroup;
use super::digest::match_fixed_digest_with_oid;
use super::pkcs3;
use super::pkcs3::DhParameter;
use super::primes::Prime;

#[derive(Clone)]
pub enum KeyObjectHandle {
  AsymmetricPrivate(AsymmetricPrivateKey),
  AsymmetricPublic(AsymmetricPublicKey),
  Secret(Box<[u8]>),
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for KeyObjectHandle {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"KeyObjectHandle"
  }
}

#[derive(Clone)]
pub enum AsymmetricPrivateKey {
  Rsa(RsaPrivateKey),
  RsaPss(RsaPssPrivateKey),
  Dsa(dsa::SigningKey),
  Ec(EcPrivateKey),
  X25519(x25519_dalek::StaticSecret),
  Ed25519(ed25519_dalek::SigningKey),
  X448([u8; 56]),
  Ed448(ed448_goldilocks::SigningKey),
  Dh(DhPrivateKey),
}

#[derive(Clone)]
pub struct RsaPssPrivateKey {
  pub key: RsaPrivateKey,
  pub details: Option<RsaPssDetails>,
}

#[derive(Clone, Copy, PartialEq)]
pub struct RsaPssDetails {
  pub hash_algorithm: RsaPssHashAlgorithm,
  pub mf1_hash_algorithm: RsaPssHashAlgorithm,
  pub salt_length: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RsaPssHashAlgorithm {
  Sha1,
  Sha224,
  Sha256,
  Sha384,
  Sha512,
  Sha512_224,
  Sha512_256,
}

impl RsaPssHashAlgorithm {
  pub fn as_str(&self) -> &'static str {
    match self {
      RsaPssHashAlgorithm::Sha1 => "sha1",
      RsaPssHashAlgorithm::Sha224 => "sha224",
      RsaPssHashAlgorithm::Sha256 => "sha256",
      RsaPssHashAlgorithm::Sha384 => "sha384",
      RsaPssHashAlgorithm::Sha512 => "sha512",
      RsaPssHashAlgorithm::Sha512_224 => "sha512-224",
      RsaPssHashAlgorithm::Sha512_256 => "sha512-256",
    }
  }

  pub fn salt_length(&self) -> u32 {
    match self {
      RsaPssHashAlgorithm::Sha1 => 20,
      RsaPssHashAlgorithm::Sha224 | RsaPssHashAlgorithm::Sha512_224 => 28,
      RsaPssHashAlgorithm::Sha256 | RsaPssHashAlgorithm::Sha512_256 => 32,
      RsaPssHashAlgorithm::Sha384 => 48,
      RsaPssHashAlgorithm::Sha512 => 64,
    }
  }
}

#[derive(Clone)]
pub enum EcPrivateKey {
  P224(p224::SecretKey),
  P256(p256::SecretKey),
  P384(p384::SecretKey),
  P521(p521::SecretKey),
  Secp256k1(k256::SecretKey),
}

#[derive(Clone)]
pub struct DhPrivateKey {
  pub key: dh::PrivateKey,
  pub params: DhParameter,
}

#[derive(Clone)]
pub enum AsymmetricPublicKey {
  Rsa(rsa::RsaPublicKey),
  RsaPss(RsaPssPublicKey),
  Dsa(dsa::VerifyingKey),
  Ec(EcPublicKey),
  X25519(x25519_dalek::PublicKey),
  Ed25519(ed25519_dalek::VerifyingKey),
  X448([u8; 56]),
  Ed448(ed448_goldilocks::VerifyingKey),
  Dh(DhPublicKey),
}

#[derive(Clone)]
pub struct RsaPssPublicKey {
  pub key: rsa::RsaPublicKey,
  pub details: Option<RsaPssDetails>,
}

#[derive(Clone)]
pub enum EcPublicKey {
  P224(p224::PublicKey),
  P256(p256::PublicKey),
  P384(p384::PublicKey),
  P521(p521::PublicKey),
  Secp256k1(k256::PublicKey),
}

#[derive(Clone)]
pub struct DhPublicKey {
  pub key: dh::PublicKey,
  pub params: DhParameter,
}

impl KeyObjectHandle {
  /// Returns the private key if the handle is an asymmetric private key.
  pub fn as_private_key(&self) -> Option<&AsymmetricPrivateKey> {
    match self {
      KeyObjectHandle::AsymmetricPrivate(key) => Some(key),
      _ => None,
    }
  }

  /// Returns the public key if the handle is an asymmetric public key. If it is
  /// a private key, it derives the public key from it and returns that.
  pub fn as_public_key(&self) -> Option<Cow<'_, AsymmetricPublicKey>> {
    match self {
      KeyObjectHandle::AsymmetricPrivate(key) => {
        Some(Cow::Owned(key.to_public_key()))
      }
      KeyObjectHandle::AsymmetricPublic(key) => Some(Cow::Borrowed(key)),
      _ => None,
    }
  }

  /// Returns the secret key if the handle is a secret key.
  pub fn as_secret_key(&self) -> Option<&[u8]> {
    match self {
      KeyObjectHandle::Secret(key) => Some(key),
      _ => None,
    }
  }
}

impl AsymmetricPrivateKey {
  /// Derives the public key from the private key.
  pub fn to_public_key(&self) -> AsymmetricPublicKey {
    match self {
      AsymmetricPrivateKey::Rsa(key) => {
        AsymmetricPublicKey::Rsa(key.to_public_key())
      }
      AsymmetricPrivateKey::RsaPss(key) => {
        AsymmetricPublicKey::RsaPss(key.to_public_key())
      }
      AsymmetricPrivateKey::Dsa(key) => {
        AsymmetricPublicKey::Dsa(key.verifying_key().clone())
      }
      AsymmetricPrivateKey::Ec(key) => {
        AsymmetricPublicKey::Ec(key.to_public_key())
      }
      AsymmetricPrivateKey::X25519(key) => {
        AsymmetricPublicKey::X25519(x25519_dalek::PublicKey::from(key))
      }
      AsymmetricPrivateKey::Ed25519(key) => {
        AsymmetricPublicKey::Ed25519(key.verifying_key())
      }
      AsymmetricPrivateKey::X448(key) => {
        let mut scalar_bytes = [0u8; 57];
        scalar_bytes[..56].copy_from_slice(&key[..56]);
        let scalar = ed448_goldilocks::EdwardsScalar::from_bytes_mod_order(
          &scalar_bytes.into(),
        );
        let point = &ed448_goldilocks::MontgomeryPoint::GENERATOR * &scalar;
        AsymmetricPublicKey::X448(point.0)
      }
      AsymmetricPrivateKey::Ed448(key) => {
        AsymmetricPublicKey::Ed448(key.verifying_key())
      }
      AsymmetricPrivateKey::Dh(dh_key) => {
        let prime = num_bigint_dig::BigUint::from_bytes_be(
          dh_key.params.prime.as_bytes(),
        );
        let base =
          num_bigint_dig::BigUint::from_bytes_be(dh_key.params.base.as_bytes());
        let public_key = dh_key.key.compute_public_key(&base, &prime);
        AsymmetricPublicKey::Dh(DhPublicKey {
          key: public_key,
          params: dh_key.params.clone(),
        })
      }
    }
  }
}

impl RsaPssPrivateKey {
  /// Derives the public key from the private key.
  pub fn to_public_key(&self) -> RsaPssPublicKey {
    RsaPssPublicKey {
      key: self.key.to_public_key(),
      details: self.details,
    }
  }
}

impl EcPublicKey {
  pub fn to_jwk(&self) -> Result<JwkEcKey, AsymmetricPublicKeyJwkError> {
    match self {
      EcPublicKey::P224(_) => {
        Err(AsymmetricPublicKeyJwkError::UnsupportedJwkEcCurveP224)
      }
      EcPublicKey::P256(key) => Ok(key.to_jwk()),
      EcPublicKey::P384(key) => Ok(key.to_jwk()),
      EcPublicKey::P521(key) => Ok(key.to_jwk()),
      EcPublicKey::Secp256k1(key) => Ok(key.to_jwk()),
    }
  }
}

impl EcPrivateKey {
  /// Derives the public key from the private key.
  pub fn to_public_key(&self) -> EcPublicKey {
    match self {
      EcPrivateKey::P224(key) => EcPublicKey::P224(key.public_key()),
      EcPrivateKey::P256(key) => EcPublicKey::P256(key.public_key()),
      EcPrivateKey::P384(key) => EcPublicKey::P384(key.public_key()),
      EcPrivateKey::P521(key) => EcPublicKey::P521(key.public_key()),
      EcPrivateKey::Secp256k1(key) => EcPublicKey::Secp256k1(key.public_key()),
    }
  }

  pub fn to_jwk(&self) -> Result<JwkEcKey, AsymmetricPrivateKeyJwkError> {
    match self {
      EcPrivateKey::P224(_) => {
        Err(AsymmetricPrivateKeyJwkError::UnsupportedJwkEcCurveP224)
      }
      EcPrivateKey::P256(key) => Ok(key.to_jwk()),
      EcPrivateKey::P384(key) => Ok(key.to_jwk()),
      EcPrivateKey::P521(key) => Ok(key.to_jwk()),
      EcPrivateKey::Secp256k1(key) => Ok(key.to_jwk()),
    }
  }
}

impl PartialEq for EcPublicKey {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (EcPublicKey::P224(a), EcPublicKey::P224(b)) => a == b,
      (EcPublicKey::P256(a), EcPublicKey::P256(b)) => a == b,
      (EcPublicKey::P384(a), EcPublicKey::P384(b)) => a == b,
      (EcPublicKey::P521(a), EcPublicKey::P521(b)) => a == b,
      (EcPublicKey::Secp256k1(a), EcPublicKey::Secp256k1(b)) => a == b,
      _ => false,
    }
  }
}

impl PartialEq for EcPrivateKey {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (EcPrivateKey::P224(a), EcPrivateKey::P224(b)) => a == b,
      (EcPrivateKey::P256(a), EcPrivateKey::P256(b)) => a == b,
      (EcPrivateKey::P384(a), EcPrivateKey::P384(b)) => a == b,
      (EcPrivateKey::P521(a), EcPrivateKey::P521(b)) => a == b,
      (EcPrivateKey::Secp256k1(a), EcPrivateKey::Secp256k1(b)) => a == b,
      _ => false,
    }
  }
}

impl PartialEq for RsaPssPublicKey {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key && self.details == other.details
  }
}

impl PartialEq for RsaPssPrivateKey {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key && self.details == other.details
  }
}

fn dh_params_eq(a: &DhParameter, b: &DhParameter) -> bool {
  let a_der = a.to_der().unwrap_or_default();
  let b_der = b.to_der().unwrap_or_default();
  a_der == b_der
}

impl PartialEq for DhPublicKey {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key && dh_params_eq(&self.params, &other.params)
  }
}

impl PartialEq for DhPrivateKey {
  fn eq(&self, other: &Self) -> bool {
    self.key == other.key && dh_params_eq(&self.params, &other.params)
  }
}

impl PartialEq for AsymmetricPublicKey {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Rsa(a), Self::Rsa(b)) => a == b,
      (Self::RsaPss(a), Self::RsaPss(b)) => a == b,
      (Self::Dsa(a), Self::Dsa(b)) => {
        a.to_public_key_der().ok() == b.to_public_key_der().ok()
      }
      (Self::Ec(a), Self::Ec(b)) => a == b,
      (Self::X25519(a), Self::X25519(b)) => a == b,
      (Self::Ed25519(a), Self::Ed25519(b)) => a == b,
      (Self::X448(a), Self::X448(b)) => a == b,
      (Self::Ed448(a), Self::Ed448(b)) => a == b,
      (Self::Dh(a), Self::Dh(b)) => a == b,
      _ => false,
    }
  }
}

impl PartialEq for AsymmetricPrivateKey {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Rsa(a), Self::Rsa(b)) => a == b,
      (Self::RsaPss(a), Self::RsaPss(b)) => a == b,
      (Self::Dsa(a), Self::Dsa(b)) => {
        a.to_pkcs8_der().ok().map(|d| d.to_bytes())
          == b.to_pkcs8_der().ok().map(|d| d.to_bytes())
      }
      (Self::Ec(a), Self::Ec(b)) => a == b,
      (Self::X25519(a), Self::X25519(b)) => a.to_bytes() == b.to_bytes(),
      (Self::Ed25519(a), Self::Ed25519(b)) => a.to_bytes() == b.to_bytes(),
      (Self::X448(a), Self::X448(b)) => a == b,
      (Self::Ed448(a), Self::Ed448(b)) => a == b,
      (Self::Dh(a), Self::Dh(b)) => a == b,
      _ => false,
    }
  }
}

impl PartialEq for KeyObjectHandle {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::AsymmetricPrivate(a), Self::AsymmetricPrivate(b)) => a == b,
      (Self::AsymmetricPublic(a), Self::AsymmetricPublic(b)) => a == b,
      (Self::Secret(a), Self::Secret(b)) => {
        use subtle::ConstantTimeEq;
        a.ct_eq(b).into()
      }
      _ => false,
    }
  }
}

// https://oidref.com/
const ID_SHA1_OID: rsa::pkcs8::ObjectIdentifier =
  rsa::pkcs8::ObjectIdentifier::new_unwrap("1.3.14.3.2.26");
const ID_SHA224_OID: rsa::pkcs8::ObjectIdentifier =
  rsa::pkcs8::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.4");
const ID_SHA256_OID: rsa::pkcs8::ObjectIdentifier =
  rsa::pkcs8::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.1");
const ID_SHA384_OID: rsa::pkcs8::ObjectIdentifier =
  rsa::pkcs8::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.2");
const ID_SHA512_OID: rsa::pkcs8::ObjectIdentifier =
  rsa::pkcs8::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.3");
const ID_SHA512_224_OID: rsa::pkcs8::ObjectIdentifier =
  rsa::pkcs8::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.5");
const ID_SHA512_256_OID: rsa::pkcs8::ObjectIdentifier =
  rsa::pkcs8::ObjectIdentifier::new_unwrap("2.16.840.1.101.3.4.2.6");

const ID_MFG1: rsa::pkcs8::ObjectIdentifier =
  rsa::pkcs8::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.8");
pub const ID_SECP224R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.132.0.33");
pub const ID_SECP256R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.10045.3.1.7");
pub const ID_SECP384R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.132.0.34");
pub const ID_SECP521R1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.132.0.35");
pub const ID_SECP256K1_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.132.0.10");

pub const RSA_ENCRYPTION_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.1");
pub const RSASSA_PSS_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.1.10");
pub const DSA_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.10040.4.1");
pub const EC_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.10045.2.1");
pub const X25519_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.110");
pub const ED25519_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.112");
pub const X448_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.111");
pub const ED448_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.3.101.113");
pub const DH_KEY_AGREEMENT_OID: const_oid::ObjectIdentifier =
  const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.3.1");

// The parameters field associated with OID id-RSASSA-PSS
// Defined in RFC 3447, section A.2.3
//
// RSASSA-PSS-params ::= SEQUENCE {
//   hashAlgorithm      [0] HashAlgorithm    DEFAULT sha1,
//   maskGenAlgorithm   [1] MaskGenAlgorithm DEFAULT mgf1SHA1,
//   saltLength         [2] INTEGER          DEFAULT 20,
//   trailerField       [3] TrailerField     DEFAULT trailerFieldBC
// }
pub struct RsaPssParameters<'a> {
  pub hash_algorithm: Option<rsa::pkcs8::AlgorithmIdentifierRef<'a>>,
  pub mask_gen_algorithm: Option<rsa::pkcs8::AlgorithmIdentifierRef<'a>>,
  pub salt_length: Option<u32>,
}

// Context-specific tag number for hashAlgorithm.
const HASH_ALGORITHM_TAG: rsa::pkcs8::der::TagNumber =
  rsa::pkcs8::der::TagNumber::new(0);

// Context-specific tag number for maskGenAlgorithm.
const MASK_GEN_ALGORITHM_TAG: rsa::pkcs8::der::TagNumber =
  rsa::pkcs8::der::TagNumber::new(1);

// Context-specific tag number for saltLength.
const SALT_LENGTH_TAG: rsa::pkcs8::der::TagNumber =
  rsa::pkcs8::der::TagNumber::new(2);

impl<'a> TryFrom<rsa::pkcs8::der::asn1::AnyRef<'a>> for RsaPssParameters<'a> {
  type Error = rsa::pkcs8::der::Error;

  fn try_from(
    any: rsa::pkcs8::der::asn1::AnyRef<'a>,
  ) -> rsa::pkcs8::der::Result<RsaPssParameters<'a>> {
    any.sequence(|decoder| {
      let hash_algorithm = decoder
        .context_specific::<rsa::pkcs8::AlgorithmIdentifierRef>(
          HASH_ALGORITHM_TAG,
          pkcs8::der::TagMode::Explicit,
        )?
        .map(TryInto::try_into)
        .transpose()?;

      let mask_gen_algorithm = decoder
        .context_specific::<rsa::pkcs8::AlgorithmIdentifierRef>(
          MASK_GEN_ALGORITHM_TAG,
          pkcs8::der::TagMode::Explicit,
        )?
        .map(TryInto::try_into)
        .transpose()?;

      let salt_length = decoder
        .context_specific::<u32>(
          SALT_LENGTH_TAG,
          pkcs8::der::TagMode::Explicit,
        )?
        .map(TryInto::try_into)
        .transpose()?;

      Ok(Self {
        hash_algorithm,
        mask_gen_algorithm,
        salt_length,
      })
    })
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum X509PublicKeyError {
  #[class(generic)]
  #[error(transparent)]
  X509(#[from] X509Error),
  #[class(generic)]
  #[error(transparent)]
  Rsa(#[from] rsa::Error),
  #[class(generic)]
  #[error(transparent)]
  Asn1(#[from] x509_parser::der_parser::asn1_rs::Error),
  #[class(generic)]
  #[error(transparent)]
  Ec(#[from] elliptic_curve::Error),
  #[class(type)]
  #[error("unsupported ec named curve")]
  UnsupportedEcNamedCurve,
  #[class(type)]
  #[error("missing ec parameters")]
  MissingEcParameters,
  #[class(type)]
  #[error("malformed DSS public key")]
  MalformedDssPublicKey,
  #[class(type)]
  #[error("invalid Ed25519 public key")]
  InvalidEd25519Key,
  #[class(type)]
  #[error("invalid Ed448 public key")]
  InvalidEd448Key,
  #[class(type)]
  #[error("invalid X25519 public key")]
  InvalidX25519Key,
  #[class(type)]
  #[error("unsupported x509 public key type")]
  UnsupportedX509KeyType,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum RsaJwkError {
  #[class(generic)]
  #[error(transparent)]
  Base64(#[from] base64::DecodeError),
  #[class(generic)]
  #[error(transparent)]
  Rsa(#[from] rsa::Error),
  #[class(type)]
  #[error("missing RSA private component")]
  MissingRsaPrivateComponent,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum EcJwkError {
  #[class(generic)]
  #[error(transparent)]
  Ec(#[from] elliptic_curve::Error),
  #[class(type)]
  #[error("unsupported curve: {0}")]
  UnsupportedCurve(String),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum EdRawError {
  #[class(generic)]
  #[error(transparent)]
  Ed25519Signature(#[from] ed25519_dalek::SignatureError),
  #[class(type)]
  #[error("invalid Ed25519 key")]
  InvalidEd25519Key,
  #[class(type)]
  #[error("invalid Ed448 key")]
  InvalidEd448Key,
  #[class(type)]
  #[error("invalid X448 key")]
  InvalidX448Key,
  #[class(type)]
  #[error("unsupported curve")]
  UnsupportedCurve,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
#[error("unsupported")]
#[property("code" = "ERR_OSSL_UNSUPPORTED")]
pub struct UnsupportedPrivateKeyOidError;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum AsymmetricPrivateKeyError {
  #[error("invalid PEM private key: not valid utf8 starting at byte {0}")]
  InvalidPemPrivateKeyInvalidUtf8(usize),
  #[error("invalid encrypted PEM private key")]
  InvalidEncryptedPemPrivateKey,
  #[error("invalid PEM private key")]
  InvalidPemPrivateKey,
  #[error("encrypted private key requires a passphrase to decrypt")]
  EncryptedPrivateKeyRequiresPassphraseToDecrypt,
  #[error("invalid PKCS#1 private key")]
  InvalidPkcs1PrivateKey,
  #[error("invalid SEC1 private key")]
  InvalidSec1PrivateKey,
  #[error("unsupported PEM label: {0}")]
  UnsupportedPemLabel(String),
  #[class(inherit)]
  #[error(transparent)]
  RsaPssParamsParse(
    #[from]
    #[inherit]
    RsaPssParamsParseError,
  ),
  #[error("invalid encrypted PKCS#8 private key")]
  InvalidEncryptedPkcs8PrivateKey,
  #[error("invalid PKCS#8 private key")]
  InvalidPkcs8PrivateKey,
  #[error("PKCS#1 private key does not support encryption with passphrase")]
  Pkcs1PrivateKeyDoesNotSupportEncryptionWithPassphrase,
  #[error("SEC1 private key does not support encryption with passphrase")]
  Sec1PrivateKeyDoesNotSupportEncryptionWithPassphrase,
  #[error("unsupported ec named curve")]
  UnsupportedEcNamedCurve,
  #[error("invalid private key")]
  InvalidPrivateKey,
  #[error("invalid DSA private key")]
  InvalidDsaPrivateKey,
  #[error("malformed or missing named curve in ec parameters")]
  MalformedOrMissingNamedCurveInEcParameters,
  #[error("unsupported key type: {0}")]
  UnsupportedKeyType(String),
  #[error("unsupported key format: {0}")]
  UnsupportedKeyFormat(String),
  #[error("invalid x25519 private key")]
  InvalidX25519PrivateKey,
  #[error("x25519 private key is the wrong length")]
  X25519PrivateKeyIsWrongLength,
  #[error("invalid Ed25519 private key")]
  InvalidEd25519PrivateKey,
  #[error("invalid x448 private key")]
  InvalidX448PrivateKey,
  #[error("x448 private key is the wrong length")]
  X448PrivateKeyIsWrongLength,
  #[error("invalid Ed448 private key")]
  InvalidEd448PrivateKey,
  #[error("missing dh parameters")]
  MissingDhParameters,
  #[class(inherit)]
  #[error(transparent)]
  UnsupportedPrivateKeyOid(
    #[from]
    #[inherit]
    UnsupportedPrivateKeyOidError,
  ),
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum AsymmetricPublicKeyError {
  #[class(type)]
  #[error("invalid PEM private key: not valid utf8 starting at byte {0}")]
  InvalidPemPrivateKeyInvalidUtf8(usize),
  #[class(type)]
  #[error("invalid PEM public key")]
  InvalidPemPublicKey,
  #[class(type)]
  #[error("invalid PKCS#1 public key")]
  InvalidPkcs1PublicKey,
  #[class(inherit)]
  #[error(transparent)]
  AsymmetricPrivateKey(
    #[from]
    #[inherit]
    AsymmetricPrivateKeyError,
  ),
  #[class(type)]
  #[error("invalid x509 certificate")]
  InvalidX509Certificate,
  #[class(generic)]
  #[error(transparent)]
  X509(#[from] x509_parser::nom::Err<X509Error>),
  #[class(inherit)]
  #[error(transparent)]
  X509PublicKey(
    #[from]
    #[inherit]
    X509PublicKeyError,
  ),
  #[class(type)]
  #[error("unsupported PEM label: {0}")]
  UnsupportedPemLabel(String),
  #[class(type)]
  #[error("invalid SPKI public key")]
  InvalidSpkiPublicKey,
  #[class(type)]
  #[error("unsupported key type: {0}")]
  UnsupportedKeyType(String),
  #[class(type)]
  #[error("unsupported key format: {0}")]
  UnsupportedKeyFormat(String),
  #[class(generic)]
  #[error(transparent)]
  Spki(#[from] spki::Error),
  #[class(generic)]
  #[error(transparent)]
  Pkcs1(#[from] rsa::pkcs1::Error),
  #[class(inherit)]
  #[error(transparent)]
  RsaPssParamsParse(
    #[from]
    #[inherit]
    RsaPssParamsParseError,
  ),
  #[class(type)]
  #[error("malformed DSS public key")]
  MalformedDssPublicKey,
  #[class(type)]
  #[error("malformed or missing named curve in ec parameters")]
  MalformedOrMissingNamedCurveInEcParameters,
  #[class(type)]
  #[error("malformed or missing public key in ec spki")]
  MalformedOrMissingPublicKeyInEcSpki,
  #[class(generic)]
  #[error(transparent)]
  Ec(#[from] elliptic_curve::Error),
  #[class(type)]
  #[error("unsupported ec named curve")]
  UnsupportedEcNamedCurve,
  #[class(type)]
  #[error("malformed or missing public key in x25519 spki")]
  MalformedOrMissingPublicKeyInX25519Spki,
  #[class(type)]
  #[error("x25519 public key is too short")]
  X25519PublicKeyIsTooShort,
  #[class(type)]
  #[error("invalid Ed25519 public key")]
  InvalidEd25519PublicKey,
  #[class(type)]
  #[error("malformed or missing public key in x448 spki")]
  MalformedOrMissingPublicKeyInX448Spki,
  #[class(type)]
  #[error("x448 public key is too short")]
  X448PublicKeyIsTooShort,
  #[class(type)]
  #[error("invalid Ed448 public key")]
  InvalidEd448PublicKey,
  #[class(type)]
  #[error("missing dh parameters")]
  MissingDhParameters,
  #[class(type)]
  #[error("malformed dh parameters")]
  MalformedDhParameters,
  #[class(type)]
  #[error("malformed or missing public key in dh spki")]
  MalformedOrMissingPublicKeyInDhSpki,
  #[class(generic)]
  #[error("unsupported")]
  #[property("code" = "ERR_OSSL_EVP_DECODE_ERROR")]
  UnsupportedPrivateKeyOid,
}

/// Parse an EC private key from SEC1 DER bytes using the named curve OID.
/// The curve OID is required — inferring from key length is unreliable
/// (e.g. P-256 and secp256k1 both use 32-byte keys).
fn ec_private_key_from_named_curve_and_sec1_der(
  named_curve: Option<const_oid::ObjectIdentifier>,
  sec1_der: &[u8],
) -> Result<AsymmetricPrivateKey, AsymmetricPrivateKeyError> {
  let ec_key = sec1::EcPrivateKey::from_der(sec1_der)
    .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?;

  let oid = named_curve.ok_or(
    AsymmetricPrivateKeyError::MalformedOrMissingNamedCurveInEcParameters,
  )?;

  match oid {
    ID_SECP224R1_OID => {
      let key = p224::SecretKey::try_from(ec_key)
        .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?;
      Ok(AsymmetricPrivateKey::Ec(EcPrivateKey::P224(key)))
    }
    ID_SECP256R1_OID => {
      let key = p256::SecretKey::try_from(ec_key)
        .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?;
      Ok(AsymmetricPrivateKey::Ec(EcPrivateKey::P256(key)))
    }
    ID_SECP384R1_OID => {
      let key = p384::SecretKey::try_from(ec_key)
        .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?;
      Ok(AsymmetricPrivateKey::Ec(EcPrivateKey::P384(key)))
    }
    ID_SECP521R1_OID => {
      let key = p521::SecretKey::try_from(ec_key)
        .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?;
      Ok(AsymmetricPrivateKey::Ec(EcPrivateKey::P521(key)))
    }
    ID_SECP256K1_OID => {
      let key = k256::SecretKey::try_from(ec_key)
        .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?;
      Ok(AsymmetricPrivateKey::Ec(EcPrivateKey::Secp256k1(key)))
    }
    _ => Err(AsymmetricPrivateKeyError::UnsupportedEcNamedCurve),
  }
}

impl KeyObjectHandle {
  pub fn new_asymmetric_private_key_from_js(
    key: &[u8],
    format: &str,
    typ: &str,
    passphrase: Option<&[u8]>,
  ) -> Result<KeyObjectHandle, AsymmetricPrivateKeyError> {
    let document = match format {
      "pem" => {
        let pem = std::str::from_utf8(key).map_err(|err| {
          AsymmetricPrivateKeyError::InvalidPemPrivateKeyInvalidUtf8(
            err.valid_up_to(),
          )
        })?;

        if let Some(passphrase) = passphrase {
          SecretDocument::from_pkcs8_encrypted_pem(pem, passphrase).map_err(
            |_| AsymmetricPrivateKeyError::InvalidEncryptedPemPrivateKey,
          )?
        } else {
          let (label, doc) = SecretDocument::from_pem(pem)
            .map_err(|_| AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;

          match label {
            EncryptedPrivateKeyInfo::PEM_LABEL => {
              return Err(AsymmetricPrivateKeyError::EncryptedPrivateKeyRequiresPassphraseToDecrypt);
            }
            PrivateKeyInfo::PEM_LABEL => doc,
            rsa::pkcs1::RsaPrivateKey::PEM_LABEL => {
              SecretDocument::from_pkcs1_der(doc.as_bytes()).map_err(|_| {
                AsymmetricPrivateKeyError::InvalidPkcs1PrivateKey
              })?
            }
            sec1::EcPrivateKey::PEM_LABEL => {
              SecretDocument::from_sec1_der(doc.as_bytes())
                .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?
            }
            "DSA PRIVATE KEY" => {
              // Traditional DSA private key format:
              // DSAPrivateKey ::= SEQUENCE {
              //   version  INTEGER,
              //   p        INTEGER,
              //   q        INTEGER,
              //   g        INTEGER,
              //   pub_key  INTEGER,
              //   priv_key INTEGER
              // }
              let private_key =
                parse_traditional_dsa_private_key(doc.as_bytes())?;
              return Ok(KeyObjectHandle::AsymmetricPrivate(
                AsymmetricPrivateKey::Dsa(private_key),
              ));
            }
            _ => {
              return Err(AsymmetricPrivateKeyError::UnsupportedPemLabel(
                label.to_string(),
              ));
            }
          }
        }
      }
      "der" => match typ {
        "pkcs8" => {
          if let Some(passphrase) = passphrase {
            SecretDocument::from_pkcs8_encrypted_der(key, passphrase).map_err(
              |_| AsymmetricPrivateKeyError::InvalidEncryptedPkcs8PrivateKey,
            )?
          } else {
            SecretDocument::from_pkcs8_der(key)
              .map_err(|_| AsymmetricPrivateKeyError::InvalidPkcs8PrivateKey)?
          }
        }
        "pkcs1" => {
          if passphrase.is_some() {
            return Err(AsymmetricPrivateKeyError::Pkcs1PrivateKeyDoesNotSupportEncryptionWithPassphrase);
          }
          SecretDocument::from_pkcs1_der(key)
            .map_err(|_| AsymmetricPrivateKeyError::InvalidPkcs1PrivateKey)?
        }
        "sec1" => {
          if passphrase.is_some() {
            return Err(AsymmetricPrivateKeyError::Sec1PrivateKeyDoesNotSupportEncryptionWithPassphrase);
          }
          SecretDocument::from_sec1_der(key)
            .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?
        }
        _ => {
          return Err(AsymmetricPrivateKeyError::UnsupportedKeyType(
            typ.to_string(),
          ));
        }
      },
      _ => {
        return Err(AsymmetricPrivateKeyError::UnsupportedKeyFormat(
          format.to_string(),
        ));
      }
    };

    let pk_info = PrivateKeyInfo::try_from(document.as_bytes())
      .map_err(|_| AsymmetricPrivateKeyError::InvalidPrivateKey)?;

    let alg = pk_info.algorithm.oid;
    let private_key = match alg {
      RSA_ENCRYPTION_OID => {
        let private_key =
          rsa::RsaPrivateKey::from_pkcs1_der(pk_info.private_key)
            .map_err(|_| AsymmetricPrivateKeyError::InvalidPkcs1PrivateKey)?;
        AsymmetricPrivateKey::Rsa(private_key)
      }
      RSASSA_PSS_OID => {
        let details = parse_rsa_pss_params(pk_info.algorithm.parameters)?;
        let private_key =
          rsa::RsaPrivateKey::from_pkcs1_der(pk_info.private_key)
            .map_err(|_| AsymmetricPrivateKeyError::InvalidPkcs1PrivateKey)?;
        AsymmetricPrivateKey::RsaPss(RsaPssPrivateKey {
          key: private_key,
          details,
        })
      }
      DSA_OID => {
        let private_key = dsa::SigningKey::try_from(pk_info)
          .map_err(|_| AsymmetricPrivateKeyError::InvalidDsaPrivateKey)?;
        AsymmetricPrivateKey::Dsa(private_key)
      }
      EC_OID => {
        // Try to get the named curve from the PKCS#8 AlgorithmIdentifier
        // parameters. If that fails, fall back to extracting it from the
        // inner SEC1 ECPrivateKey structure's parameters field.
        let named_curve =
          pk_info.algorithm.parameters_oid().ok().or_else(|| {
            let ec_key =
              sec1::EcPrivateKey::from_der(pk_info.private_key).ok()?;
            ec_key.parameters.and_then(|p| p.named_curve())
          });
        ec_private_key_from_named_curve_and_sec1_der(
          named_curve,
          pk_info.private_key,
        )?
      }
      X25519_OID => {
        let string_ref = OctetStringRef::from_der(pk_info.private_key)
          .map_err(|_| AsymmetricPrivateKeyError::InvalidX25519PrivateKey)?;
        if string_ref.as_bytes().len() != 32 {
          return Err(AsymmetricPrivateKeyError::X25519PrivateKeyIsWrongLength);
        }
        let mut bytes = [0; 32];
        bytes.copy_from_slice(string_ref.as_bytes());
        AsymmetricPrivateKey::X25519(x25519_dalek::StaticSecret::from(bytes))
      }
      ED25519_OID => {
        let signing_key = ed25519_dalek::SigningKey::try_from(pk_info)
          .map_err(|_| AsymmetricPrivateKeyError::InvalidEd25519PrivateKey)?;
        AsymmetricPrivateKey::Ed25519(signing_key)
      }
      X448_OID => {
        let string_ref = OctetStringRef::from_der(pk_info.private_key)
          .map_err(|_| AsymmetricPrivateKeyError::InvalidX448PrivateKey)?;
        if string_ref.as_bytes().len() != 56 {
          return Err(AsymmetricPrivateKeyError::X448PrivateKeyIsWrongLength);
        }
        let mut bytes = [0u8; 56];
        bytes.copy_from_slice(string_ref.as_bytes());
        AsymmetricPrivateKey::X448(bytes)
      }
      ED448_OID => {
        let string_ref = OctetStringRef::from_der(pk_info.private_key)
          .map_err(|_| AsymmetricPrivateKeyError::InvalidEd448PrivateKey)?;
        let key_bytes = string_ref.as_bytes();
        if key_bytes.len() != 57 {
          return Err(AsymmetricPrivateKeyError::InvalidEd448PrivateKey);
        }
        let mut seed = [0u8; 57];
        seed.copy_from_slice(key_bytes);
        let seed = ed448_goldilocks::EdwardsScalarBytes::from(seed);
        AsymmetricPrivateKey::Ed448(ed448_goldilocks::SigningKey::from(seed))
      }
      DH_KEY_AGREEMENT_OID => {
        let params = pk_info
          .algorithm
          .parameters
          .ok_or(AsymmetricPrivateKeyError::MissingDhParameters)?;
        let params = pkcs3::DhParameter::from_der(&params.to_der().unwrap())
          .map_err(|_| AsymmetricPrivateKeyError::MissingDhParameters)?;
        // pk_info.private_key is a DER-encoded INTEGER (tag + length + value)
        // inside the PKCS#8 OCTET STRING, so we need to decode it first.
        let private_key_int =
          <AnyRef<'_> as spki::der::Decode>::from_der(pk_info.private_key)
            .map_err(|_| AsymmetricPrivateKeyError::MissingDhParameters)?;
        AsymmetricPrivateKey::Dh(DhPrivateKey {
          key: dh::PrivateKey::from_bytes(private_key_int.value()),
          params,
        })
      }
      _ => return Err(UnsupportedPrivateKeyOidError.into()),
    };

    Ok(KeyObjectHandle::AsymmetricPrivate(private_key))
  }

  pub fn new_x509_public_key(
    spki: &x509::SubjectPublicKeyInfo,
  ) -> Result<KeyObjectHandle, X509PublicKeyError> {
    use x509_parser::der_parser::asn1_rs::oid;
    use x509_parser::public_key::PublicKey;

    let key = match spki.parsed()? {
      PublicKey::RSA(key) => {
        let public_key = RsaPublicKey::new(
          rsa::BigUint::from_bytes_be(key.modulus),
          rsa::BigUint::from_bytes_be(key.exponent),
        )?;
        AsymmetricPublicKey::Rsa(public_key)
      }
      PublicKey::EC(point) => {
        let data = point.data();
        if let Some(params) = &spki.algorithm.parameters {
          let curve_oid = params.as_oid()?;
          const ID_SECP224R1: &[u8] = &oid!(raw 1.3.132.0.33);
          const ID_SECP256R1: &[u8] = &oid!(raw 1.2.840.10045.3.1.7);
          const ID_SECP384R1: &[u8] = &oid!(raw 1.3.132.0.34);
          const ID_SECP521R1: &[u8] = &oid!(raw 1.3.132.0.35);
          const ID_SECP256K1: &[u8] = &oid!(raw 1.3.132.0.10);

          match curve_oid.as_bytes() {
            ID_SECP224R1 => {
              let public_key = p224::PublicKey::from_sec1_bytes(data)?;
              AsymmetricPublicKey::Ec(EcPublicKey::P224(public_key))
            }
            ID_SECP256R1 => {
              let public_key = p256::PublicKey::from_sec1_bytes(data)?;
              AsymmetricPublicKey::Ec(EcPublicKey::P256(public_key))
            }
            ID_SECP384R1 => {
              let public_key = p384::PublicKey::from_sec1_bytes(data)?;
              AsymmetricPublicKey::Ec(EcPublicKey::P384(public_key))
            }
            ID_SECP521R1 => {
              let public_key = p521::PublicKey::from_sec1_bytes(data)?;
              AsymmetricPublicKey::Ec(EcPublicKey::P521(public_key))
            }
            ID_SECP256K1 => {
              let public_key = k256::PublicKey::from_sec1_bytes(data)?;
              AsymmetricPublicKey::Ec(EcPublicKey::Secp256k1(public_key))
            }
            _ => return Err(X509PublicKeyError::UnsupportedEcNamedCurve),
          }
        } else {
          return Err(X509PublicKeyError::MissingEcParameters);
        }
      }
      PublicKey::DSA(_) => {
        let verifying_key = dsa::VerifyingKey::from_public_key_der(spki.raw)
          .map_err(|_| X509PublicKeyError::MalformedDssPublicKey)?;
        AsymmetricPublicKey::Dsa(verifying_key)
      }
      _ => {
        const ID_ED25519: &[u8] = &oid!(raw 1.3.101.112);
        const ID_X25519: &[u8] = &oid!(raw 1.3.101.110);
        const ID_ED448: &[u8] = &oid!(raw 1.3.101.113);
        const ID_X448: &[u8] = &oid!(raw 1.3.101.111);

        match spki.algorithm.algorithm.as_bytes() {
          ID_ED25519 => {
            let data = spki.subject_public_key.as_ref();
            let key_bytes: [u8; 32] = data
              .try_into()
              .map_err(|_| X509PublicKeyError::InvalidEd25519Key)?;
            let verifying_key =
              ed25519_dalek::VerifyingKey::from_bytes(&key_bytes)
                .map_err(|_| X509PublicKeyError::InvalidEd25519Key)?;
            AsymmetricPublicKey::Ed25519(verifying_key)
          }
          ID_X25519 => {
            let data: &[u8] = spki.subject_public_key.as_ref();
            let data: [u8; 32] = data
              .try_into()
              .map_err(|_| X509PublicKeyError::InvalidX25519Key)?;
            AsymmetricPublicKey::X25519(x25519_dalek::PublicKey::from(data))
          }
          ID_ED448 => {
            let data = spki.subject_public_key.as_ref();
            let point_bytes: &[u8; 57] = data
              .try_into()
              .map_err(|_| X509PublicKeyError::InvalidEd448Key)?;
            let vk = ed448_goldilocks::VerifyingKey::from_bytes(point_bytes)
              .map_err(|_| X509PublicKeyError::InvalidEd448Key)?;
            AsymmetricPublicKey::Ed448(vk)
          }
          ID_X448 => {
            let data: &[u8] = spki.subject_public_key.as_ref();
            let data: [u8; 56] = data
              .try_into()
              .map_err(|_| X509PublicKeyError::InvalidX25519Key)?;
            AsymmetricPublicKey::X448(data)
          }
          _ => return Err(X509PublicKeyError::UnsupportedX509KeyType),
        }
      }
    };

    Ok(KeyObjectHandle::AsymmetricPublic(key))
  }

  pub fn new_rsa_jwk(
    jwk: RsaJwkKey,
    is_public: bool,
  ) -> Result<KeyObjectHandle, RsaJwkError> {
    use base64::prelude::BASE64_URL_SAFE_NO_PAD;

    let n = BASE64_URL_SAFE_NO_PAD.decode(jwk.n.as_bytes())?;
    let e = BASE64_URL_SAFE_NO_PAD.decode(jwk.e.as_bytes())?;

    if is_public {
      let public_key = RsaPublicKey::new(
        rsa::BigUint::from_bytes_be(&n),
        rsa::BigUint::from_bytes_be(&e),
      )?;

      Ok(KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Rsa(
        public_key,
      )))
    } else {
      let d = BASE64_URL_SAFE_NO_PAD.decode(
        jwk
          .d
          .ok_or(RsaJwkError::MissingRsaPrivateComponent)?
          .as_bytes(),
      )?;
      let p = BASE64_URL_SAFE_NO_PAD.decode(
        jwk
          .p
          .ok_or(RsaJwkError::MissingRsaPrivateComponent)?
          .as_bytes(),
      )?;
      let q = BASE64_URL_SAFE_NO_PAD.decode(
        jwk
          .q
          .ok_or(RsaJwkError::MissingRsaPrivateComponent)?
          .as_bytes(),
      )?;

      let mut private_key = RsaPrivateKey::from_components(
        rsa::BigUint::from_bytes_be(&n),
        rsa::BigUint::from_bytes_be(&e),
        rsa::BigUint::from_bytes_be(&d),
        vec![
          rsa::BigUint::from_bytes_be(&p),
          rsa::BigUint::from_bytes_be(&q),
        ],
      )?;
      private_key.precompute()?; // precompute CRT params

      Ok(KeyObjectHandle::AsymmetricPrivate(
        AsymmetricPrivateKey::Rsa(private_key),
      ))
    }
  }

  pub fn new_ec_jwk(
    jwk: &JwkEcKey,
    is_public: bool,
  ) -> Result<KeyObjectHandle, EcJwkError> {
    //  https://datatracker.ietf.org/doc/html/rfc7518#section-6.2.1.1
    let handle = match jwk.crv() {
      "P-256" if is_public => {
        KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Ec(
          EcPublicKey::P256(p256::PublicKey::from_jwk(jwk)?),
        ))
      }
      "P-256" => KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Ec(
        EcPrivateKey::P256(p256::SecretKey::from_jwk(jwk)?),
      )),
      "P-384" if is_public => {
        KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Ec(
          EcPublicKey::P384(p384::PublicKey::from_jwk(jwk)?),
        ))
      }
      "P-384" => KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Ec(
        EcPrivateKey::P384(p384::SecretKey::from_jwk(jwk)?),
      )),
      "P-521" if is_public => {
        KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Ec(
          EcPublicKey::P521(p521::PublicKey::from_jwk(jwk)?),
        ))
      }
      "P-521" => KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Ec(
        EcPrivateKey::P521(p521::SecretKey::from_jwk(jwk)?),
      )),
      "secp256k1" if is_public => {
        KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Ec(
          EcPublicKey::Secp256k1(k256::PublicKey::from_jwk(jwk)?),
        ))
      }
      "secp256k1" => {
        KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Ec(
          EcPrivateKey::Secp256k1(k256::SecretKey::from_jwk(jwk)?),
        ))
      }
      _ => {
        return Err(EcJwkError::UnsupportedCurve(jwk.crv().to_string()));
      }
    };

    Ok(handle)
  }

  pub fn new_ed_raw(
    curve: &str,
    data: &[u8],
    is_public: bool,
  ) -> Result<KeyObjectHandle, EdRawError> {
    match curve {
      "Ed25519" => {
        let data =
          data.try_into().map_err(|_| EdRawError::InvalidEd25519Key)?;
        if !is_public {
          Ok(KeyObjectHandle::AsymmetricPrivate(
            AsymmetricPrivateKey::Ed25519(
              ed25519_dalek::SigningKey::from_bytes(data),
            ),
          ))
        } else {
          Ok(KeyObjectHandle::AsymmetricPublic(
            AsymmetricPublicKey::Ed25519(
              ed25519_dalek::VerifyingKey::from_bytes(data)?,
            ),
          ))
        }
      }
      "X25519" => {
        let data: [u8; 32] =
          data.try_into().map_err(|_| EdRawError::InvalidEd25519Key)?;
        if !is_public {
          Ok(KeyObjectHandle::AsymmetricPrivate(
            AsymmetricPrivateKey::X25519(x25519_dalek::StaticSecret::from(
              data,
            )),
          ))
        } else {
          Ok(KeyObjectHandle::AsymmetricPublic(
            AsymmetricPublicKey::X25519(x25519_dalek::PublicKey::from(data)),
          ))
        }
      }
      "Ed448" => {
        if !is_public {
          let key_bytes: [u8; 57] =
            data.try_into().map_err(|_| EdRawError::InvalidEd448Key)?;
          let seed = ed448_goldilocks::EdwardsScalarBytes::from(key_bytes);
          Ok(KeyObjectHandle::AsymmetricPrivate(
            AsymmetricPrivateKey::Ed448(ed448_goldilocks::SigningKey::from(
              seed,
            )),
          ))
        } else {
          let point_bytes: &[u8; 57] =
            data.try_into().map_err(|_| EdRawError::InvalidEd448Key)?;
          let vk = ed448_goldilocks::VerifyingKey::from_bytes(point_bytes)
            .map_err(|_| EdRawError::InvalidEd448Key)?;
          Ok(KeyObjectHandle::AsymmetricPublic(
            AsymmetricPublicKey::Ed448(vk),
          ))
        }
      }
      "X448" => {
        let data: [u8; 56] =
          data.try_into().map_err(|_| EdRawError::InvalidX448Key)?;
        if !is_public {
          Ok(KeyObjectHandle::AsymmetricPrivate(
            AsymmetricPrivateKey::X448(data),
          ))
        } else {
          Ok(KeyObjectHandle::AsymmetricPublic(
            AsymmetricPublicKey::X448(data),
          ))
        }
      }
      _ => Err(EdRawError::UnsupportedCurve),
    }
  }

  pub fn new_asymmetric_public_key_from_js(
    key: &[u8],
    format: &str,
    typ: &str,
    passphrase: Option<&[u8]>,
  ) -> Result<KeyObjectHandle, AsymmetricPublicKeyError> {
    let document = match format {
      "pem" => {
        let pem = std::str::from_utf8(key).map_err(|err| {
          AsymmetricPublicKeyError::InvalidPemPrivateKeyInvalidUtf8(
            err.valid_up_to(),
          )
        })?;

        let (label, document) = Document::from_pem(pem)
          .map_err(|_| AsymmetricPublicKeyError::InvalidPemPublicKey)?;

        match label {
          SubjectPublicKeyInfoRef::PEM_LABEL => document,
          rsa::pkcs1::RsaPublicKey::PEM_LABEL => {
            Document::from_pkcs1_der(document.as_bytes())
              .map_err(|_| AsymmetricPublicKeyError::InvalidPkcs1PublicKey)?
          }
          EncryptedPrivateKeyInfo::PEM_LABEL
          | PrivateKeyInfo::PEM_LABEL
          | sec1::EcPrivateKey::PEM_LABEL
          | rsa::pkcs1::RsaPrivateKey::PEM_LABEL
          | "DSA PRIVATE KEY" => {
            let handle = KeyObjectHandle::new_asymmetric_private_key_from_js(
              key, format, typ, passphrase,
            )?;
            match handle {
              KeyObjectHandle::AsymmetricPrivate(private) => {
                return Ok(KeyObjectHandle::AsymmetricPublic(
                  private.to_public_key(),
                ));
              }
              KeyObjectHandle::AsymmetricPublic(_)
              | KeyObjectHandle::Secret(_) => unreachable!(),
            }
          }
          "CERTIFICATE" => {
            let (_, pem) = x509_parser::pem::parse_x509_pem(pem.as_bytes())
              .map_err(|_| AsymmetricPublicKeyError::InvalidX509Certificate)?;

            let cert = pem.parse_x509()?;
            let public_key = cert.tbs_certificate.subject_pki;

            return KeyObjectHandle::new_x509_public_key(&public_key)
              .map_err(Into::into);
          }
          _ => {
            return Err(AsymmetricPublicKeyError::UnsupportedPemLabel(
              label.to_string(),
            ));
          }
        }
      }
      "der" => match typ {
        "pkcs1" => Document::from_pkcs1_der(key)
          .map_err(|_| AsymmetricPublicKeyError::InvalidPkcs1PublicKey)?,
        "spki" => Document::from_public_key_der(key)
          .map_err(|_| AsymmetricPublicKeyError::InvalidSpkiPublicKey)?,
        _ => {
          return Err(AsymmetricPublicKeyError::UnsupportedKeyType(
            typ.to_string(),
          ));
        }
      },
      _ => {
        return Err(AsymmetricPublicKeyError::UnsupportedKeyType(
          format.to_string(),
        ));
      }
    };

    let spki = SubjectPublicKeyInfoRef::try_from(document.as_bytes())?;

    let public_key = match spki.algorithm.oid {
      RSA_ENCRYPTION_OID => {
        let public_key = RsaPublicKey::from_pkcs1_der(
          spki.subject_public_key.as_bytes().unwrap(),
        )?;
        AsymmetricPublicKey::Rsa(public_key)
      }
      RSASSA_PSS_OID => {
        let details = parse_rsa_pss_params(spki.algorithm.parameters)?;
        let public_key = RsaPublicKey::from_pkcs1_der(
          spki.subject_public_key.as_bytes().unwrap(),
        )?;
        AsymmetricPublicKey::RsaPss(RsaPssPublicKey {
          key: public_key,
          details,
        })
      }
      DSA_OID => {
        let verifying_key = dsa::VerifyingKey::try_from(spki)
          .map_err(|_| AsymmetricPublicKeyError::MalformedDssPublicKey)?;
        AsymmetricPublicKey::Dsa(verifying_key)
      }
      EC_OID => {
        let named_curve = spki.algorithm.parameters_oid().map_err(|_| {
          AsymmetricPublicKeyError::MalformedOrMissingNamedCurveInEcParameters
        })?;
        let data = spki.subject_public_key.as_bytes().ok_or(
          AsymmetricPublicKeyError::MalformedOrMissingPublicKeyInEcSpki,
        )?;

        match named_curve {
          ID_SECP224R1_OID => {
            let public_key = p224::PublicKey::from_sec1_bytes(data)?;
            AsymmetricPublicKey::Ec(EcPublicKey::P224(public_key))
          }
          ID_SECP256R1_OID => {
            let public_key = p256::PublicKey::from_sec1_bytes(data)?;
            AsymmetricPublicKey::Ec(EcPublicKey::P256(public_key))
          }
          ID_SECP384R1_OID => {
            let public_key = p384::PublicKey::from_sec1_bytes(data)?;
            AsymmetricPublicKey::Ec(EcPublicKey::P384(public_key))
          }
          ID_SECP521R1_OID => {
            let public_key = p521::PublicKey::from_sec1_bytes(data)?;
            AsymmetricPublicKey::Ec(EcPublicKey::P521(public_key))
          }
          ID_SECP256K1_OID => {
            let public_key = k256::PublicKey::from_sec1_bytes(data)?;
            AsymmetricPublicKey::Ec(EcPublicKey::Secp256k1(public_key))
          }
          _ => return Err(AsymmetricPublicKeyError::UnsupportedEcNamedCurve),
        }
      }
      X25519_OID => {
        let mut bytes = [0; 32];
        let data = spki.subject_public_key.as_bytes().ok_or(
          AsymmetricPublicKeyError::MalformedOrMissingPublicKeyInX25519Spki,
        )?;
        if data.len() < 32 {
          return Err(AsymmetricPublicKeyError::X25519PublicKeyIsTooShort);
        }
        bytes.copy_from_slice(&data[0..32]);
        AsymmetricPublicKey::X25519(x25519_dalek::PublicKey::from(bytes))
      }
      ED25519_OID => {
        let verifying_key = ed25519_dalek::VerifyingKey::try_from(spki)
          .map_err(|_| AsymmetricPublicKeyError::InvalidEd25519PublicKey)?;
        AsymmetricPublicKey::Ed25519(verifying_key)
      }
      X448_OID => {
        let mut bytes = [0; 56];
        let data = spki.subject_public_key.as_bytes().ok_or(
          AsymmetricPublicKeyError::MalformedOrMissingPublicKeyInX448Spki,
        )?;
        if data.len() < 56 {
          return Err(AsymmetricPublicKeyError::X448PublicKeyIsTooShort);
        }
        bytes.copy_from_slice(&data[0..56]);
        AsymmetricPublicKey::X448(bytes)
      }
      ED448_OID => {
        let data = spki
          .subject_public_key
          .as_bytes()
          .ok_or(AsymmetricPublicKeyError::InvalidEd448PublicKey)?;
        let point_bytes: &[u8; 57] = data
          .try_into()
          .map_err(|_| AsymmetricPublicKeyError::InvalidEd448PublicKey)?;
        let vk = ed448_goldilocks::VerifyingKey::from_bytes(point_bytes)
          .map_err(|_| AsymmetricPublicKeyError::InvalidEd448PublicKey)?;
        AsymmetricPublicKey::Ed448(vk)
      }
      DH_KEY_AGREEMENT_OID => {
        let params = spki
          .algorithm
          .parameters
          .ok_or(AsymmetricPublicKeyError::MissingDhParameters)?;
        let params = pkcs3::DhParameter::from_der(&params.to_der().unwrap())
          .map_err(|_| AsymmetricPublicKeyError::MalformedDhParameters)?;
        let Some(subject_public_key) = spki.subject_public_key.as_bytes()
        else {
          return Err(
            AsymmetricPublicKeyError::MalformedOrMissingPublicKeyInDhSpki,
          );
        };
        // subject_public_key is a DER-encoded INTEGER inside the BIT STRING,
        // so we need to decode it to get the raw key bytes.
        let public_key_int =
          <AnyRef<'_> as spki::der::Decode>::from_der(subject_public_key)
            .map_err(|_| {
              AsymmetricPublicKeyError::MalformedOrMissingPublicKeyInDhSpki
            })?;
        AsymmetricPublicKey::Dh(DhPublicKey {
          key: dh::PublicKey::from_bytes(public_key_int.value()),
          params,
        })
      }
      _ => return Err(AsymmetricPublicKeyError::UnsupportedPrivateKeyOid),
    };

    Ok(KeyObjectHandle::AsymmetricPublic(public_key))
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum RsaPssParamsParseError {
  #[error("malformed pss private key parameters")]
  MalformedPssPrivateKeyParameters,
  #[error("unsupported pss hash algorithm")]
  UnsupportedPssHashAlgorithm,
  #[error("unsupported pss mask gen algorithm")]
  UnsupportedPssMaskGenAlgorithm,
  #[error("malformed or missing pss mask gen algorithm parameters")]
  MalformedOrMissingPssMaskGenAlgorithm,
}

/// Parse a traditional DSA private key (PEM label "DSA PRIVATE KEY").
///
/// The traditional format is:
/// ```asn1
/// DSAPrivateKey ::= SEQUENCE {
///   version  INTEGER,
///   p        INTEGER,
///   q        INTEGER,
///   g        INTEGER,
///   pub_key  INTEGER,
///   priv_key INTEGER
/// }
/// ```
fn parse_traditional_dsa_private_key(
  der: &[u8],
) -> Result<dsa::SigningKey, AsymmetricPrivateKeyError> {
  use spki::der::Decode;
  use spki::der::Reader as _;
  use spki::der::SliceReader;

  let err = || AsymmetricPrivateKeyError::InvalidDsaPrivateKey;

  let mut reader = SliceReader::new(der).map_err(|_| err())?;
  reader
    .sequence(|seq_reader| {
      // version
      let _version = asn1::UintRef::decode(seq_reader)?;
      // p
      let p_ref = asn1::UintRef::decode(seq_reader)?;
      // q
      let q_ref = asn1::UintRef::decode(seq_reader)?;
      // g
      let g_ref = asn1::UintRef::decode(seq_reader)?;
      // y (public key)
      let y_ref = asn1::UintRef::decode(seq_reader)?;
      // x (private key)
      let x_ref = asn1::UintRef::decode(seq_reader)?;

      let p = num_bigint_dig::BigUint::from_bytes_be(p_ref.as_bytes());
      let q = num_bigint_dig::BigUint::from_bytes_be(q_ref.as_bytes());
      let g = num_bigint_dig::BigUint::from_bytes_be(g_ref.as_bytes());
      let y = num_bigint_dig::BigUint::from_bytes_be(y_ref.as_bytes());
      let x = num_bigint_dig::BigUint::from_bytes_be(x_ref.as_bytes());

      let components = dsa::Components::from_components(p, q, g)
        .map_err(|_| spki::der::Tag::Sequence.value_error())?;
      let verifying_key = dsa::VerifyingKey::from_components(components, y)
        .map_err(|_| spki::der::Tag::Sequence.value_error())?;
      dsa::SigningKey::from_components(verifying_key, x)
        .map_err(|_| spki::der::Tag::Sequence.value_error())
    })
    .map_err(|_| err())
}

fn parse_rsa_pss_params(
  parameters: Option<AnyRef<'_>>,
) -> Result<Option<RsaPssDetails>, RsaPssParamsParseError> {
  let details = if let Some(parameters) = parameters {
    let params = RsaPssParameters::try_from(parameters)
      .map_err(|_| RsaPssParamsParseError::MalformedPssPrivateKeyParameters)?;

    let hash_algorithm = match params.hash_algorithm.map(|k| k.oid) {
      Some(ID_SHA1_OID) => RsaPssHashAlgorithm::Sha1,
      Some(ID_SHA224_OID) => RsaPssHashAlgorithm::Sha224,
      Some(ID_SHA256_OID) => RsaPssHashAlgorithm::Sha256,
      Some(ID_SHA384_OID) => RsaPssHashAlgorithm::Sha384,
      Some(ID_SHA512_OID) => RsaPssHashAlgorithm::Sha512,
      Some(ID_SHA512_224_OID) => RsaPssHashAlgorithm::Sha512_224,
      Some(ID_SHA512_256_OID) => RsaPssHashAlgorithm::Sha512_256,
      None => RsaPssHashAlgorithm::Sha1,
      _ => return Err(RsaPssParamsParseError::UnsupportedPssHashAlgorithm),
    };

    let mf1_hash_algorithm = match params.mask_gen_algorithm {
      Some(alg) => {
        if alg.oid != ID_MFG1 {
          return Err(RsaPssParamsParseError::UnsupportedPssMaskGenAlgorithm);
        }
        let params = alg.parameters_oid().map_err(|_| {
          RsaPssParamsParseError::MalformedOrMissingPssMaskGenAlgorithm
        })?;
        match params {
          ID_SHA1_OID => RsaPssHashAlgorithm::Sha1,
          ID_SHA224_OID => RsaPssHashAlgorithm::Sha224,
          ID_SHA256_OID => RsaPssHashAlgorithm::Sha256,
          ID_SHA384_OID => RsaPssHashAlgorithm::Sha384,
          ID_SHA512_OID => RsaPssHashAlgorithm::Sha512,
          ID_SHA512_224_OID => RsaPssHashAlgorithm::Sha512_224,
          ID_SHA512_256_OID => RsaPssHashAlgorithm::Sha512_256,
          _ => {
            return Err(RsaPssParamsParseError::UnsupportedPssMaskGenAlgorithm);
          }
        }
      }
      None => hash_algorithm,
    };

    let salt_length = params
      .salt_length
      .unwrap_or_else(|| hash_algorithm.salt_length());

    Some(RsaPssDetails {
      hash_algorithm,
      mf1_hash_algorithm,
      salt_length,
    })
  } else {
    None
  };
  Ok(details)
}

fn bytes_to_b64(bytes: &[u8]) -> String {
  use base64::prelude::BASE64_URL_SAFE_NO_PAD;
  BASE64_URL_SAFE_NO_PAD.encode(bytes)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum AsymmetricPrivateKeyJwkError {
  #[error("key is not an asymmetric private key")]
  KeyIsNotAsymmetricPrivateKey,
  #[class(generic)]
  #[property("code" = "ERR_CRYPTO_JWK_UNSUPPORTED_CURVE")]
  #[error("Unsupported JWK EC curve: secp224r1.")]
  UnsupportedJwkEcCurveP224,
  #[class(generic)]
  #[property("code" = "ERR_CRYPTO_JWK_UNSUPPORTED_KEY_TYPE")]
  #[error("Unsupported JWK Key Type.")]
  JwkExportNotImplementedForKeyType,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum AsymmetricPublicKeyJwkError {
  #[error("key is not an asymmetric public key")]
  KeyIsNotAsymmetricPublicKey,
  #[class(generic)]
  #[property("code" = "ERR_CRYPTO_JWK_UNSUPPORTED_CURVE")]
  #[error("Unsupported JWK EC curve: secp224r1.")]
  UnsupportedJwkEcCurveP224,
  #[class(generic)]
  #[property("code" = "ERR_CRYPTO_JWK_UNSUPPORTED_KEY_TYPE")]
  #[error("Unsupported JWK Key Type.")]
  JwkExportNotImplementedForKeyType,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum AsymmetricPublicKeyDerError {
  #[error("key is not an asymmetric public key")]
  KeyIsNotAsymmetricPublicKey,
  #[error("invalid RSA public key")]
  InvalidRsaPublicKey,
  #[error("exporting non-RSA public key as PKCS#1 is not supported")]
  ExportingNonRsaPublicKeyAsPkcs1Unsupported,
  #[error("invalid EC public key")]
  InvalidEcPublicKey,
  #[error("exporting RSA-PSS public key as SPKI is not supported yet")]
  ExportingNonRsaPssPublicKeyAsSpkiUnsupported,
  #[error("invalid DSA public key")]
  InvalidDsaPublicKey,
  #[error("invalid X25519 public key")]
  InvalidX25519PublicKey,
  #[error("invalid Ed25519 public key")]
  InvalidEd25519PublicKey,
  #[error("invalid X448 public key")]
  InvalidX448PublicKey,
  #[error("invalid Ed448 public key")]
  InvalidEd448PublicKey,
  #[error("invalid DH public key")]
  InvalidDhPublicKey,
  #[error("unsupported key type: {0}")]
  UnsupportedKeyType(String),
}

impl AsymmetricPublicKey {
  fn export_jwk(
    &self,
  ) -> Result<deno_core::serde_json::Value, AsymmetricPublicKeyJwkError> {
    match self {
      AsymmetricPublicKey::Ec(key) => {
        let jwk = key.to_jwk()?;
        Ok(deno_core::serde_json::json!(jwk))
      }
      AsymmetricPublicKey::X25519(key) => {
        let bytes = key.as_bytes();
        let jwk = deno_core::serde_json::json!({
            "kty": "OKP",
            "crv": "X25519",
            "x": bytes_to_b64(bytes),
        });
        Ok(jwk)
      }
      AsymmetricPublicKey::Ed25519(key) => {
        let bytes = key.to_bytes();
        let jwk = deno_core::serde_json::json!({
            "kty": "OKP",
            "crv": "Ed25519",
            "x": bytes_to_b64(&bytes),
        });
        Ok(jwk)
      }
      AsymmetricPublicKey::X448(key) => {
        let jwk = deno_core::serde_json::json!({
            "kty": "OKP",
            "crv": "X448",
            "x": bytes_to_b64(key),
        });
        Ok(jwk)
      }
      AsymmetricPublicKey::Ed448(key) => {
        let bytes = key.to_bytes();
        let jwk = deno_core::serde_json::json!({
            "kty": "OKP",
            "crv": "Ed448",
            "x": bytes_to_b64(&bytes),
        });
        Ok(jwk)
      }
      AsymmetricPublicKey::Rsa(key) => {
        let n = key.n();
        let e = key.e();

        let jwk = deno_core::serde_json::json!({
            "kty": "RSA",
            "n": bytes_to_b64(&n.to_bytes_be()),
            "e": bytes_to_b64(&e.to_bytes_be()),
        });
        Ok(jwk)
      }
      AsymmetricPublicKey::RsaPss(key) => {
        let n = key.key.n();
        let e = key.key.e();

        let jwk = deno_core::serde_json::json!({
            "kty": "RSA",
            "n": bytes_to_b64(&n.to_bytes_be()),
            "e": bytes_to_b64(&e.to_bytes_be()),
        });
        Ok(jwk)
      }
      _ => Err(AsymmetricPublicKeyJwkError::JwkExportNotImplementedForKeyType),
    }
  }

  pub(crate) fn export_der(
    &self,
    typ: &str,
  ) -> Result<Box<[u8]>, AsymmetricPublicKeyDerError> {
    match typ {
      "pkcs1" => match self {
        AsymmetricPublicKey::Rsa(key) => {
          let der = key
            .to_pkcs1_der()
            .map_err(|_| AsymmetricPublicKeyDerError::InvalidRsaPublicKey)?
            .into_vec()
            .into_boxed_slice();
          Ok(der)
        }
        _ => Err(AsymmetricPublicKeyDerError::ExportingNonRsaPublicKeyAsPkcs1Unsupported),
      },
      "spki" => {
        let der = match self {
          AsymmetricPublicKey::Rsa(key) => key
            .to_public_key_der()
            .map_err(|_| AsymmetricPublicKeyDerError::InvalidRsaPublicKey)?
            .into_vec()
            .into_boxed_slice(),
          AsymmetricPublicKey::RsaPss(key) => {
            let pkcs1_der = key.key
              .to_pkcs1_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidRsaPublicKey)?;
            let spki = SubjectPublicKeyInfoRef {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: RSASSA_PSS_OID,
                parameters: None,
              },
              subject_public_key: BitStringRef::from_bytes(pkcs1_der.as_bytes())
                .map_err(|_| AsymmetricPublicKeyDerError::InvalidRsaPublicKey)?,
            };
            spki
              .to_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidRsaPublicKey)?
              .into_boxed_slice()
          }
          AsymmetricPublicKey::Dsa(key) => key
            .to_public_key_der()
            .map_err(|_| AsymmetricPublicKeyDerError::InvalidDsaPublicKey)?
            .into_vec()
            .into_boxed_slice(),
          AsymmetricPublicKey::Ec(key) => {
            let (sec1, oid) = match key {
              EcPublicKey::P224(key) => (key.to_sec1_bytes(), ID_SECP224R1_OID),
              EcPublicKey::P256(key) => (key.to_sec1_bytes(), ID_SECP256R1_OID),
              EcPublicKey::P384(key) => (key.to_sec1_bytes(), ID_SECP384R1_OID),
              EcPublicKey::P521(key) => (key.to_sec1_bytes(), ID_SECP521R1_OID),
              EcPublicKey::Secp256k1(key) => (key.to_sec1_bytes(), ID_SECP256K1_OID),
            };

            let spki = SubjectPublicKeyInfoRef {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: EC_OID,
                parameters: Some(asn1::AnyRef::from(&oid)),
              },
              subject_public_key: BitStringRef::from_bytes(&sec1)
                .map_err(|_| AsymmetricPublicKeyDerError::InvalidEcPublicKey)?,
            };

            spki
              .to_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidEcPublicKey)?
              .into_boxed_slice()
          }
          AsymmetricPublicKey::X25519(key) => {
            let spki = SubjectPublicKeyInfoRef {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: X25519_OID,
                parameters: None,
              },
              subject_public_key: BitStringRef::from_bytes(key.as_bytes())
                .map_err(|_| AsymmetricPublicKeyDerError::InvalidX25519PublicKey)?,
            };

            spki
              .to_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidX25519PublicKey)?
              .into_boxed_slice()
          }
          AsymmetricPublicKey::Ed25519(key) => {
            let spki = SubjectPublicKeyInfoRef {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: ED25519_OID,
                parameters: None,
              },
              subject_public_key: BitStringRef::from_bytes(key.as_bytes())
                .map_err(|_| AsymmetricPublicKeyDerError::InvalidEd25519PublicKey)?,
            };

            spki
              .to_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidEd25519PublicKey)?
              .into_boxed_slice()
          }
          AsymmetricPublicKey::X448(key) => {
            let spki = SubjectPublicKeyInfoRef {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: X448_OID,
                parameters: None,
              },
              subject_public_key: BitStringRef::from_bytes(key)
                .map_err(|_| AsymmetricPublicKeyDerError::InvalidX448PublicKey)?,
            };

            spki
              .to_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidX448PublicKey)?
              .into_boxed_slice()
          }
          AsymmetricPublicKey::Ed448(key) => {
            let bytes = key.to_bytes();
            let spki = SubjectPublicKeyInfoRef {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: ED448_OID,
                parameters: None,
              },
              subject_public_key: BitStringRef::from_bytes(&bytes)
                .map_err(|_| AsymmetricPublicKeyDerError::InvalidEd448PublicKey)?,
            };

            spki
              .to_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidEd448PublicKey)?
              .into_boxed_slice()
          }
          AsymmetricPublicKey::Dh(key) => {
            // The public key in SPKI for DH must be a DER-encoded INTEGER
            let raw_key = key.key.clone().into_vec();
            let public_key_int = asn1::Int::new(&raw_key).unwrap();
            let public_key_der = public_key_int
              .to_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidDhPublicKey)?;
            let params = key.params.to_der().unwrap();
            let spki = SubjectPublicKeyInfoRef {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: DH_KEY_AGREEMENT_OID,
                parameters: Some(AnyRef::new(Tag::Sequence, &params).unwrap()),
              },
              subject_public_key: BitStringRef::from_bytes(&public_key_der)
                .map_err(|_| {
                  AsymmetricPublicKeyDerError::InvalidDhPublicKey
              })?,
            };
            spki
              .to_der()
              .map_err(|_| AsymmetricPublicKeyDerError::InvalidDhPublicKey)?
              .into_boxed_slice()
          }
        };
        Ok(der)
      }
      _ => Err(AsymmetricPublicKeyDerError::UnsupportedKeyType(typ.to_string())),
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
pub enum AsymmetricPrivateKeyDerError {
  #[error("key is not an asymmetric private key")]
  KeyIsNotAsymmetricPrivateKey,
  #[error("invalid RSA private key")]
  InvalidRsaPrivateKey,
  #[error("exporting non-RSA private key as PKCS#1 is not supported")]
  ExportingNonRsaPrivateKeyAsPkcs1Unsupported,
  #[error("invalid EC private key")]
  InvalidEcPrivateKey,
  #[error("exporting non-EC private key as SEC1 is not supported")]
  ExportingNonEcPrivateKeyAsSec1Unsupported,
  #[class(type)]
  #[error("exporting RSA-PSS private key as PKCS#8 is not supported yet")]
  ExportingNonRsaPssPrivateKeyAsPkcs8Unsupported,
  #[error("invalid DSA private key")]
  InvalidDsaPrivateKey,
  #[error("invalid X25519 private key")]
  InvalidX25519PrivateKey,
  #[error("invalid Ed25519 private key")]
  InvalidEd25519PrivateKey,
  #[error("invalid X448 private key")]
  InvalidX448PrivateKey,
  #[error("invalid Ed448 private key")]
  InvalidEd448PrivateKey,
  #[error("invalid DH private key")]
  InvalidDhPrivateKey,
  #[error("unsupported key type: {0}")]
  UnsupportedKeyType(String),
}

// https://datatracker.ietf.org/doc/html/rfc7518#section-6.3.2
fn rsa_private_to_jwk(key: &RsaPrivateKey) -> deno_core::serde_json::Value {
  let n = key.n();
  let e = key.e();
  let d = key.d();
  let p = &key.primes()[0];
  let q = &key.primes()[1];
  let dp = key.dp();
  let dq = key.dq();
  let qi = key.crt_coefficient();
  let oth = &key.primes()[2..];

  let mut obj = deno_core::serde_json::json!({
      "kty": "RSA",
      "n": bytes_to_b64(&n.to_bytes_be()),
      "e": bytes_to_b64(&e.to_bytes_be()),
      "d": bytes_to_b64(&d.to_bytes_be()),
      "p": bytes_to_b64(&p.to_bytes_be()),
      "q": bytes_to_b64(&q.to_bytes_be()),
      "dp": dp.map(|dp| bytes_to_b64(&dp.to_bytes_be())),
      "dq": dq.map(|dq| bytes_to_b64(&dq.to_bytes_be())),
      "qi": qi.map(|qi| bytes_to_b64(&qi.to_bytes_be())),
  });

  if !oth.is_empty() {
    obj["oth"] = deno_core::serde_json::json!(
      oth.iter().map(|o| o.to_bytes_be()).collect::<Vec<_>>()
    );
  }

  obj
}

impl AsymmetricPrivateKey {
  fn export_jwk(
    &self,
  ) -> Result<deno_core::serde_json::Value, AsymmetricPrivateKeyJwkError> {
    match self {
      AsymmetricPrivateKey::Rsa(key) => Ok(rsa_private_to_jwk(key)),
      AsymmetricPrivateKey::RsaPss(key) => Ok(rsa_private_to_jwk(&key.key)),
      AsymmetricPrivateKey::Ec(key) => {
        let jwk = key.to_jwk()?;
        Ok(deno_core::serde_json::json!(jwk))
      }
      AsymmetricPrivateKey::X25519(static_secret) => {
        let bytes = static_secret.to_bytes();

        let AsymmetricPublicKey::X25519(x) = self.to_public_key() else {
          unreachable!();
        };

        Ok(deno_core::serde_json::json!({
            "crv": "X25519",
            "x": bytes_to_b64(x.as_bytes()),
            "d": bytes_to_b64(&bytes),
            "kty": "OKP",
        }))
      }
      AsymmetricPrivateKey::Ed25519(key) => {
        let bytes = key.to_bytes();
        let AsymmetricPublicKey::Ed25519(x) = self.to_public_key() else {
          unreachable!();
        };

        Ok(deno_core::serde_json::json!({
            "crv": "Ed25519",
            "x": bytes_to_b64(x.as_bytes()),
            "d": bytes_to_b64(&bytes),
            "kty": "OKP",
        }))
      }
      AsymmetricPrivateKey::X448(key) => {
        let AsymmetricPublicKey::X448(x) = self.to_public_key() else {
          unreachable!();
        };

        Ok(deno_core::serde_json::json!({
            "crv": "X448",
            "x": bytes_to_b64(&x),
            "d": bytes_to_b64(&key[..56]),
            "kty": "OKP",
        }))
      }
      AsymmetricPrivateKey::Ed448(key) => {
        let bytes = key.to_bytes();
        let AsymmetricPublicKey::Ed448(x) = self.to_public_key() else {
          unreachable!();
        };
        let x_bytes = x.to_bytes();

        Ok(deno_core::serde_json::json!({
            "crv": "Ed448",
            "x": bytes_to_b64(&x_bytes),
            "d": bytes_to_b64(&bytes),
            "kty": "OKP",
        }))
      }
      _ => Err(AsymmetricPrivateKeyJwkError::JwkExportNotImplementedForKeyType),
    }
  }

  fn export_der(
    &self,
    typ: &str,
    // cipher: Option<&str>,
    // passphrase: Option<&str>,
  ) -> Result<Box<[u8]>, AsymmetricPrivateKeyDerError> {
    match typ {
      "pkcs1" => match self {
        AsymmetricPrivateKey::Rsa(key) => {
          let der = key
            .to_pkcs1_der()
            .map_err(|_| AsymmetricPrivateKeyDerError::InvalidRsaPrivateKey)?
            .to_bytes()
            .to_vec()
            .into_boxed_slice();
          Ok(der)
        }
        _ => Err(AsymmetricPrivateKeyDerError::ExportingNonRsaPrivateKeyAsPkcs1Unsupported),
      },
      "sec1" => match self {
        AsymmetricPrivateKey::Ec(key) => {
          let (sec1_der, curve_oid) = match key {
            EcPrivateKey::P224(key) => {
              (key.to_sec1_der(), ID_SECP224R1_OID)
            }
            EcPrivateKey::P256(key) => {
              (key.to_sec1_der(), ID_SECP256R1_OID)
            }
            EcPrivateKey::P384(key) => {
              (key.to_sec1_der(), ID_SECP384R1_OID)
            }
            EcPrivateKey::P521(key) => {
              (key.to_sec1_der(), ID_SECP521R1_OID)
            }
            EcPrivateKey::Secp256k1(key) => {
              (key.to_sec1_der(), ID_SECP256K1_OID)
            }
          };
          let sec1_der = sec1_der
            .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEcPrivateKey)?;
          // The elliptic-curve crate's to_sec1_der() omits the optional
          // `parameters` field. Re-encode with the curve OID included to
          // match OpenSSL/Node.js behavior.
          let mut ec_key =
            sec1::EcPrivateKey::from_der(&sec1_der)
              .map_err(|_| {
                AsymmetricPrivateKeyDerError::InvalidEcPrivateKey
              })?;
          ec_key.parameters = Some(curve_oid.into());
          let der = ec_key
            .to_der()
            .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEcPrivateKey)?;
          Ok(der.into_boxed_slice())
        }
        _ => Err(AsymmetricPrivateKeyDerError::ExportingNonEcPrivateKeyAsSec1Unsupported),
      },
      "pkcs8" => {
        let der = match self {
          AsymmetricPrivateKey::Rsa(key) => {
            let document = key
              .to_pkcs8_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidRsaPrivateKey)?;
            document.to_bytes().to_vec().into_boxed_slice()
          }
          AsymmetricPrivateKey::RsaPss(_key) => {
            return Err(AsymmetricPrivateKeyDerError::ExportingNonRsaPssPrivateKeyAsPkcs8Unsupported)
          }
          AsymmetricPrivateKey::Dsa(key) => {
            let document = key
              .to_pkcs8_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidDsaPrivateKey)?;
            document.to_bytes().to_vec().into_boxed_slice()
          }
          AsymmetricPrivateKey::Ec(key) => {
            let document = match key {
              EcPrivateKey::P224(key) => key.to_pkcs8_der(),
              EcPrivateKey::P256(key) => key.to_pkcs8_der(),
              EcPrivateKey::P384(key) => key.to_pkcs8_der(),
              EcPrivateKey::P521(key) => key.to_pkcs8_der(),
              EcPrivateKey::Secp256k1(key) => key.to_pkcs8_der(),
            }
            .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEcPrivateKey)?;
            document.to_bytes().to_vec().into_boxed_slice()
          }
          AsymmetricPrivateKey::X25519(key) => {
            let private_key = OctetStringRef::new(key.as_bytes())
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidX25519PrivateKey)?
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidX25519PrivateKey)?;

            let private_key = PrivateKeyInfo {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: X25519_OID,
                parameters: None,
              },
              private_key: &private_key,
              public_key: None,
            };

            let der = private_key
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidX25519PrivateKey)?
              .into_boxed_slice();
            return Ok(der);
          }
          AsymmetricPrivateKey::Ed25519(key) => {
            let private_key = OctetStringRef::new(key.as_bytes())
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEd25519PrivateKey)?
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEd25519PrivateKey)?;

            let private_key = PrivateKeyInfo {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: ED25519_OID,
                parameters: None,
              },
              private_key: &private_key,
              public_key: None,
            };

            private_key
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEd25519PrivateKey)?
              .into_boxed_slice()
          }
          AsymmetricPrivateKey::X448(key) => {
            let private_key = OctetStringRef::new(&key[..56])
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidX448PrivateKey)?
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidX448PrivateKey)?;

            let private_key = PrivateKeyInfo {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: X448_OID,
                parameters: None,
              },
              private_key: &private_key,
              public_key: None,
            };

            private_key
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidX448PrivateKey)?
              .into_boxed_slice()
          }
          AsymmetricPrivateKey::Ed448(key) => {
            let seed = key.to_bytes();
            let private_key = OctetStringRef::new(&seed)
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEd448PrivateKey)?
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEd448PrivateKey)?;

            let private_key = PrivateKeyInfo {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: ED448_OID,
                parameters: None,
              },
              private_key: &private_key,
              public_key: None,
            };

            private_key
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidEd448PrivateKey)?
              .into_boxed_slice()
          }
          AsymmetricPrivateKey::Dh(key) => {
            // The private key in PKCS#8 for DH must be a DER-encoded INTEGER
            let raw_key = key.key.clone().into_vec();
            let private_key_int = asn1::Int::new(&raw_key).unwrap();
            let private_key_der = private_key_int
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidDhPrivateKey)?;
            let params = key.params.to_der().unwrap();
            let private_key = PrivateKeyInfo {
              algorithm: rsa::pkcs8::AlgorithmIdentifierRef {
                oid: DH_KEY_AGREEMENT_OID,
                parameters: Some(AnyRef::new(Tag::Sequence, &params).unwrap()),
              },
              private_key: &private_key_der,
              public_key: None,
            };

            private_key
              .to_der()
              .map_err(|_| AsymmetricPrivateKeyDerError::InvalidDhPrivateKey)?
              .into_boxed_slice()
          }
        };

        Ok(der)
      }
      _ => Err(AsymmetricPrivateKeyDerError::UnsupportedKeyType(typ.to_string())),
    }
  }
}

#[op2]
#[cppgc]
pub fn op_node_create_private_key(
  #[buffer] key: &[u8],
  #[string] format: &str,
  #[string] typ: &str,
  #[buffer] passphrase: Option<&[u8]>,
) -> Result<KeyObjectHandle, AsymmetricPrivateKeyError> {
  KeyObjectHandle::new_asymmetric_private_key_from_js(
    key, format, typ, passphrase,
  )
}

#[op2]
#[cppgc]
pub fn op_node_create_ed_raw(
  #[string] curve: &str,
  #[buffer] key: &[u8],
  is_public: bool,
) -> Result<KeyObjectHandle, EdRawError> {
  KeyObjectHandle::new_ed_raw(curve, key, is_public)
}

#[derive(FromV8)]
pub struct RsaJwkKey {
  n: String,
  e: String,
  d: Option<String>,
  p: Option<String>,
  q: Option<String>,
}

#[op2]
#[cppgc]
pub fn op_node_create_rsa_jwk(
  #[scoped] jwk: RsaJwkKey,
  is_public: bool,
) -> Result<KeyObjectHandle, RsaJwkError> {
  KeyObjectHandle::new_rsa_jwk(jwk, is_public)
}

#[op2]
#[cppgc]
pub fn op_node_create_ec_jwk(
  #[serde] jwk: JwkEcKey,
  is_public: bool,
) -> Result<KeyObjectHandle, EcJwkError> {
  KeyObjectHandle::new_ec_jwk(&jwk, is_public)
}

#[op2]
#[cppgc]
pub fn op_node_create_public_key(
  #[buffer] key: &[u8],
  #[string] format: &str,
  #[string] typ: &str,
  #[buffer] passphrase: Option<&[u8]>,
) -> Result<KeyObjectHandle, AsymmetricPublicKeyError> {
  KeyObjectHandle::new_asymmetric_public_key_from_js(
    key, format, typ, passphrase,
  )
}

#[op2]
#[cppgc]
pub fn op_node_create_secret_key(
  #[buffer(copy)] key: Box<[u8]>,
) -> KeyObjectHandle {
  KeyObjectHandle::Secret(key)
}

#[op2]
#[string]
pub fn op_node_get_asymmetric_key_type(
  #[cppgc] handle: &KeyObjectHandle,
) -> Result<&'static str, JsErrorBox> {
  match handle {
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Rsa(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Rsa(_)) => {
      Ok("rsa")
    }
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::RsaPss(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::RsaPss(_)) => {
      Ok("rsa-pss")
    }
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Dsa(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Dsa(_)) => {
      Ok("dsa")
    }
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Ec(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Ec(_)) => Ok("ec"),
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::X25519(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::X25519(_)) => {
      Ok("x25519")
    }
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Ed25519(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Ed25519(_)) => {
      Ok("ed25519")
    }
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::X448(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::X448(_)) => {
      Ok("x448")
    }
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Ed448(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Ed448(_)) => {
      Ok("ed448")
    }
    KeyObjectHandle::AsymmetricPrivate(AsymmetricPrivateKey::Dh(_))
    | KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Dh(_)) => Ok("dh"),
    KeyObjectHandle::Secret(_) => Err(JsErrorBox::type_error(
      "symmetric key is not an asymmetric key",
    )),
  }
}

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum AsymmetricKeyDetails {
  #[serde(rename_all = "camelCase")]
  Rsa {
    modulus_length: usize,
    public_exponent: V8BigInt,
  },
  #[serde(rename_all = "camelCase")]
  RsaPss {
    modulus_length: usize,
    public_exponent: V8BigInt,
    hash_algorithm: &'static str,
    mgf1_hash_algorithm: &'static str,
    salt_length: u32,
  },
  #[serde(rename = "rsaPss", rename_all = "camelCase")]
  RsaPssBasic {
    modulus_length: usize,
    public_exponent: V8BigInt,
  },
  #[serde(rename_all = "camelCase")]
  Dsa {
    modulus_length: usize,
    divisor_length: usize,
  },
  #[serde(rename_all = "camelCase")]
  Ec {
    named_curve: &'static str,
  },
  X25519,
  Ed25519,
  X448,
  Ed448,
  Dh,
}

#[op2]
#[serde]
pub fn op_node_get_asymmetric_key_details(
  #[cppgc] handle: &KeyObjectHandle,
) -> Result<AsymmetricKeyDetails, JsErrorBox> {
  match handle {
    KeyObjectHandle::AsymmetricPrivate(private_key) => match private_key {
      AsymmetricPrivateKey::Rsa(key) => {
        let modulus_length = key.n().bits();
        let public_exponent =
          BigInt::from_bytes_be(num_bigint::Sign::Plus, &key.e().to_bytes_be());
        Ok(AsymmetricKeyDetails::Rsa {
          modulus_length,
          public_exponent: V8BigInt::from(public_exponent),
        })
      }
      AsymmetricPrivateKey::RsaPss(key) => {
        let modulus_length = key.key.n().bits();
        let public_exponent = BigInt::from_bytes_be(
          num_bigint::Sign::Plus,
          &key.key.e().to_bytes_be(),
        );
        let public_exponent = V8BigInt::from(public_exponent);
        let details = match key.details {
          Some(details) => AsymmetricKeyDetails::RsaPss {
            modulus_length,
            public_exponent,
            hash_algorithm: details.hash_algorithm.as_str(),
            mgf1_hash_algorithm: details.mf1_hash_algorithm.as_str(),
            salt_length: details.salt_length,
          },
          None => AsymmetricKeyDetails::RsaPssBasic {
            modulus_length,
            public_exponent,
          },
        };
        Ok(details)
      }
      AsymmetricPrivateKey::Dsa(key) => {
        let components = key.verifying_key().components();
        let modulus_length = components.p().bits();
        let divisor_length = components.q().bits();
        Ok(AsymmetricKeyDetails::Dsa {
          modulus_length,
          divisor_length,
        })
      }
      AsymmetricPrivateKey::Ec(key) => {
        let named_curve = match key {
          EcPrivateKey::P224(_) => "secp224r1",
          EcPrivateKey::P256(_) => "prime256v1",
          EcPrivateKey::P384(_) => "secp384r1",
          EcPrivateKey::P521(_) => "secp521r1",
          EcPrivateKey::Secp256k1(_) => "secp256k1",
        };
        Ok(AsymmetricKeyDetails::Ec { named_curve })
      }
      AsymmetricPrivateKey::X25519(_) => Ok(AsymmetricKeyDetails::X25519),
      AsymmetricPrivateKey::Ed25519(_) => Ok(AsymmetricKeyDetails::Ed25519),
      AsymmetricPrivateKey::X448(_) => Ok(AsymmetricKeyDetails::X448),
      AsymmetricPrivateKey::Ed448(_) => Ok(AsymmetricKeyDetails::Ed448),
      AsymmetricPrivateKey::Dh(_) => Ok(AsymmetricKeyDetails::Dh),
    },
    KeyObjectHandle::AsymmetricPublic(public_key) => match public_key {
      AsymmetricPublicKey::Rsa(key) => {
        let modulus_length = key.n().bits();
        let public_exponent =
          BigInt::from_bytes_be(num_bigint::Sign::Plus, &key.e().to_bytes_be());
        Ok(AsymmetricKeyDetails::Rsa {
          modulus_length,
          public_exponent: V8BigInt::from(public_exponent),
        })
      }
      AsymmetricPublicKey::RsaPss(key) => {
        let modulus_length = key.key.n().bits();
        let public_exponent = BigInt::from_bytes_be(
          num_bigint::Sign::Plus,
          &key.key.e().to_bytes_be(),
        );
        let public_exponent = V8BigInt::from(public_exponent);
        let details = match key.details {
          Some(details) => AsymmetricKeyDetails::RsaPss {
            modulus_length,
            public_exponent,
            hash_algorithm: details.hash_algorithm.as_str(),
            mgf1_hash_algorithm: details.mf1_hash_algorithm.as_str(),
            salt_length: details.salt_length,
          },
          None => AsymmetricKeyDetails::RsaPssBasic {
            modulus_length,
            public_exponent,
          },
        };
        Ok(details)
      }
      AsymmetricPublicKey::Dsa(key) => {
        let components = key.components();
        let modulus_length = components.p().bits();
        let divisor_length = components.q().bits();
        Ok(AsymmetricKeyDetails::Dsa {
          modulus_length,
          divisor_length,
        })
      }
      AsymmetricPublicKey::Ec(key) => {
        let named_curve = match key {
          EcPublicKey::P224(_) => "secp224r1",
          EcPublicKey::P256(_) => "prime256v1",
          EcPublicKey::P384(_) => "secp384r1",
          EcPublicKey::P521(_) => "secp521r1",
          EcPublicKey::Secp256k1(_) => "secp256k1",
        };
        Ok(AsymmetricKeyDetails::Ec { named_curve })
      }
      AsymmetricPublicKey::X25519(_) => Ok(AsymmetricKeyDetails::X25519),
      AsymmetricPublicKey::Ed25519(_) => Ok(AsymmetricKeyDetails::Ed25519),
      AsymmetricPublicKey::X448(_) => Ok(AsymmetricKeyDetails::X448),
      AsymmetricPublicKey::Ed448(_) => Ok(AsymmetricKeyDetails::Ed448),
      AsymmetricPublicKey::Dh(_) => Ok(AsymmetricKeyDetails::Dh),
    },
    KeyObjectHandle::Secret(_) => Err(JsErrorBox::type_error(
      "symmetric key is not an asymmetric key",
    )),
  }
}

#[op2(fast)]
#[smi]
pub fn op_node_get_symmetric_key_size(
  #[cppgc] handle: &KeyObjectHandle,
) -> Result<usize, JsErrorBox> {
  match handle {
    KeyObjectHandle::AsymmetricPrivate(_)
    | KeyObjectHandle::AsymmetricPublic(_) => Err(JsErrorBox::type_error(
      "asymmetric key is not a symmetric key",
    )),
    KeyObjectHandle::Secret(key) => Ok(key.len()),
  }
}

#[op2]
#[cppgc]
pub fn op_node_generate_secret_key(#[smi] len: usize) -> KeyObjectHandle {
  let mut key = vec![0u8; len];
  thread_rng().fill_bytes(&mut key);
  KeyObjectHandle::Secret(key.into_boxed_slice())
}

#[op2]
#[cppgc]
pub async fn op_node_generate_secret_key_async(
  #[smi] len: usize,
) -> KeyObjectHandle {
  spawn_blocking(move || {
    let mut key = vec![0u8; len];
    thread_rng().fill_bytes(&mut key);
    KeyObjectHandle::Secret(key.into_boxed_slice())
  })
  .await
  .unwrap()
}

struct KeyObjectHandlePair {
  private_key: RefCell<Option<KeyObjectHandle>>,
  public_key: RefCell<Option<KeyObjectHandle>>,
}

// SAFETY: we're sure this can be GCed
unsafe impl GarbageCollected for KeyObjectHandlePair {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"KeyObjectHandlePair"
  }
}

impl KeyObjectHandlePair {
  pub fn new(
    private_key: AsymmetricPrivateKey,
    public_key: AsymmetricPublicKey,
  ) -> Self {
    Self {
      private_key: RefCell::new(Some(KeyObjectHandle::AsymmetricPrivate(
        private_key,
      ))),
      public_key: RefCell::new(Some(KeyObjectHandle::AsymmetricPublic(
        public_key,
      ))),
    }
  }
}

#[op2]
#[cppgc]
pub fn op_node_get_public_key_from_pair(
  #[cppgc] pair: &KeyObjectHandlePair,
) -> Option<KeyObjectHandle> {
  pair.public_key.borrow_mut().take()
}

#[op2]
#[cppgc]
pub fn op_node_get_private_key_from_pair(
  #[cppgc] pair: &KeyObjectHandlePair,
) -> Option<KeyObjectHandle> {
  pair.private_key.borrow_mut().take()
}

fn generate_rsa(
  modulus_length: usize,
  public_exponent: usize,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  if public_exponent <= 1 || public_exponent.is_multiple_of(2) {
    return Err(JsErrorBox::generic(format!(
      "invalid RSA public exponent: {}",
      public_exponent
    )));
  }

  let private_key = RsaPrivateKey::new_with_exp(
    &mut thread_rng(),
    modulus_length,
    &rsa::BigUint::from_usize(public_exponent).unwrap(),
  )
  .map_err(|e| JsErrorBox::generic(e.to_string()))?;

  let private_key = AsymmetricPrivateKey::Rsa(private_key);
  let public_key = private_key.to_public_key();

  Ok(KeyObjectHandlePair::new(private_key, public_key))
}

#[op2]
#[cppgc]
pub fn op_node_generate_rsa_key(
  #[smi] modulus_length: usize,
  #[smi] public_exponent: usize,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  generate_rsa(modulus_length, public_exponent)
}

#[op2]
#[cppgc]
pub async fn op_node_generate_rsa_key_async(
  #[smi] modulus_length: usize,
  #[smi] public_exponent: usize,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  spawn_blocking(move || generate_rsa(modulus_length, public_exponent))
    .await
    .unwrap()
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(type)]
#[error("digest not allowed for RSA-PSS keys{}", .0.as_ref().map(|digest| format!(": {digest}")).unwrap_or_default())]
pub struct GenerateRsaPssError(Option<String>);

fn generate_rsa_pss(
  modulus_length: usize,
  public_exponent: usize,
  hash_algorithm: Option<&str>,
  mf1_hash_algorithm: Option<&str>,
  salt_length: Option<u32>,
) -> Result<KeyObjectHandlePair, GenerateRsaPssError> {
  let key = RsaPrivateKey::new_with_exp(
    &mut thread_rng(),
    modulus_length,
    &rsa::BigUint::from_usize(public_exponent).unwrap(),
  )
  .unwrap();

  let details = if hash_algorithm.is_none()
    && mf1_hash_algorithm.is_none()
    && salt_length.is_none()
  {
    None
  } else {
    let hash_algorithm = hash_algorithm.unwrap_or("sha1");
    let mf1_hash_algorithm = mf1_hash_algorithm.unwrap_or(hash_algorithm);
    let hash_algorithm = match_fixed_digest_with_oid!(
      hash_algorithm,
      fn (algorithm: Option<RsaPssHashAlgorithm>) {
        algorithm.ok_or(GenerateRsaPssError(None))?
      },
      _ => {
        return Err(GenerateRsaPssError(Some(hash_algorithm.to_string())))
      }
    );
    let mf1_hash_algorithm = match_fixed_digest_with_oid!(
      mf1_hash_algorithm,
      fn (algorithm: Option<RsaPssHashAlgorithm>) {
        algorithm.ok_or(GenerateRsaPssError(None))?
      },
      _ => {
        return Err(GenerateRsaPssError(Some(mf1_hash_algorithm.to_string())))
      }
    );
    let salt_length =
      salt_length.unwrap_or_else(|| hash_algorithm.salt_length());

    Some(RsaPssDetails {
      hash_algorithm,
      mf1_hash_algorithm,
      salt_length,
    })
  };

  let private_key =
    AsymmetricPrivateKey::RsaPss(RsaPssPrivateKey { key, details });
  let public_key = private_key.to_public_key();

  Ok(KeyObjectHandlePair::new(private_key, public_key))
}

#[op2]
#[cppgc]
pub fn op_node_generate_rsa_pss_key(
  #[smi] modulus_length: usize,
  #[smi] public_exponent: usize,
  #[string] hash_algorithm: Option<String>, // todo: Option<&str> not supproted in ops yet
  #[string] mf1_hash_algorithm: Option<String>, // todo: Option<&str> not supproted in ops yet
  #[smi] salt_length: Option<u32>,
) -> Result<KeyObjectHandlePair, GenerateRsaPssError> {
  generate_rsa_pss(
    modulus_length,
    public_exponent,
    hash_algorithm.as_deref(),
    mf1_hash_algorithm.as_deref(),
    salt_length,
  )
}

#[op2]
#[cppgc]
pub async fn op_node_generate_rsa_pss_key_async(
  #[smi] modulus_length: usize,
  #[smi] public_exponent: usize,
  #[string] hash_algorithm: Option<String>, // todo: Option<&str> not supproted in ops yet
  #[string] mf1_hash_algorithm: Option<String>, // todo: Option<&str> not supproted in ops yet
  #[smi] salt_length: Option<u32>,
) -> Result<KeyObjectHandlePair, GenerateRsaPssError> {
  spawn_blocking(move || {
    generate_rsa_pss(
      modulus_length,
      public_exponent,
      hash_algorithm.as_deref(),
      mf1_hash_algorithm.as_deref(),
      salt_length,
    )
  })
  .await
  .unwrap()
}

fn dsa_generate(
  modulus_length: usize,
  divisor_length: usize,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  let mut rng = rand::thread_rng();
  use dsa::Components;
  use dsa::KeySize;
  use dsa::SigningKey;

  let key_size = match (modulus_length, divisor_length) {
    #[allow(
      deprecated,
      reason = "needed for compatibility with legacy DSA key sizes"
    )]
    (1024, 160) => KeySize::DSA_1024_160,
    (2048, 224) => KeySize::DSA_2048_224,
    (2048, 256) => KeySize::DSA_2048_256,
    (3072, 256) => KeySize::DSA_3072_256,
    _ => {
      return Err(JsErrorBox::type_error(
        "Invalid modulusLength+divisorLength combination",
      ));
    }
  };
  let components = Components::generate(&mut rng, key_size);
  let signing_key = SigningKey::generate(&mut rng, components);
  let private_key = AsymmetricPrivateKey::Dsa(signing_key);
  let public_key = private_key.to_public_key();

  Ok(KeyObjectHandlePair::new(private_key, public_key))
}

#[op2]
#[cppgc]
pub fn op_node_generate_dsa_key(
  #[smi] modulus_length: usize,
  #[smi] divisor_length: usize,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  dsa_generate(modulus_length, divisor_length)
}

#[op2]
#[cppgc]
pub async fn op_node_generate_dsa_key_async(
  #[smi] modulus_length: usize,
  #[smi] divisor_length: usize,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  spawn_blocking(move || dsa_generate(modulus_length, divisor_length))
    .await
    .unwrap()
}

fn ec_generate(named_curve: &str) -> Result<KeyObjectHandlePair, JsErrorBox> {
  let mut rng = rand::thread_rng();
  // TODO(@littledivy): Support public key point encoding.
  // Default is uncompressed.
  let private_key = match named_curve {
    "P-224" | "prime224v1" | "secp224r1" => {
      let key = p224::SecretKey::random(&mut rng);
      AsymmetricPrivateKey::Ec(EcPrivateKey::P224(key))
    }
    "P-256" | "prime256v1" | "secp256r1" => {
      let key = p256::SecretKey::random(&mut rng);
      AsymmetricPrivateKey::Ec(EcPrivateKey::P256(key))
    }
    "P-384" | "prime384v1" | "secp384r1" => {
      let key = p384::SecretKey::random(&mut rng);
      AsymmetricPrivateKey::Ec(EcPrivateKey::P384(key))
    }
    "P-521" | "secp521r1" => {
      let key = p521::SecretKey::random(&mut rng);
      AsymmetricPrivateKey::Ec(EcPrivateKey::P521(key))
    }
    "secp256k1" => {
      let key = k256::SecretKey::random(&mut rng);
      AsymmetricPrivateKey::Ec(EcPrivateKey::Secp256k1(key))
    }
    _ => {
      return Err(JsErrorBox::type_error("Invalid EC curve name"));
    }
  };
  let public_key = private_key.to_public_key();
  Ok(KeyObjectHandlePair::new(private_key, public_key))
}

#[op2]
#[cppgc]
pub fn op_node_generate_ec_key(
  #[string] named_curve: &str,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  ec_generate(named_curve)
}

#[op2]
#[cppgc]
pub async fn op_node_generate_ec_key_async(
  #[string] named_curve: String,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  spawn_blocking(move || ec_generate(&named_curve))
    .await
    .unwrap()
}

fn x25519_generate() -> KeyObjectHandlePair {
  let keypair = x25519_dalek::StaticSecret::random_from_rng(thread_rng());
  let private_key = AsymmetricPrivateKey::X25519(keypair);
  let public_key = private_key.to_public_key();
  KeyObjectHandlePair::new(private_key, public_key)
}

#[op2]
#[cppgc]
pub fn op_node_generate_x25519_key() -> KeyObjectHandlePair {
  x25519_generate()
}

#[op2]
#[cppgc]
pub async fn op_node_generate_x25519_key_async() -> KeyObjectHandlePair {
  spawn_blocking(x25519_generate).await.unwrap()
}

fn ed25519_generate() -> KeyObjectHandlePair {
  let keypair = ed25519_dalek::SigningKey::generate(&mut thread_rng());
  let private_key = AsymmetricPrivateKey::Ed25519(keypair);
  let public_key = private_key.to_public_key();
  KeyObjectHandlePair::new(private_key, public_key)
}

#[op2]
#[cppgc]
pub fn op_node_generate_ed25519_key() -> KeyObjectHandlePair {
  ed25519_generate()
}

#[op2]
#[cppgc]
pub async fn op_node_generate_ed25519_key_async() -> KeyObjectHandlePair {
  spawn_blocking(ed25519_generate).await.unwrap()
}

fn x448_generate() -> KeyObjectHandlePair {
  let mut seed = [0u8; 56];
  thread_rng().fill_bytes(&mut seed);
  let private_key = AsymmetricPrivateKey::X448(seed);
  let public_key = private_key.to_public_key();
  KeyObjectHandlePair::new(private_key, public_key)
}

#[op2]
#[cppgc]
pub fn op_node_generate_x448_key() -> KeyObjectHandlePair {
  x448_generate()
}

#[op2]
#[cppgc]
pub async fn op_node_generate_x448_key_async() -> KeyObjectHandlePair {
  spawn_blocking(x448_generate).await.unwrap()
}

fn ed448_generate() -> KeyObjectHandlePair {
  let mut seed = [0u8; 57];
  thread_rng().fill_bytes(&mut seed);
  let signing_key = ed448_goldilocks::SigningKey::from(
    ed448_goldilocks::EdwardsScalarBytes::from(seed),
  );
  let private_key = AsymmetricPrivateKey::Ed448(signing_key);
  let public_key = private_key.to_public_key();
  KeyObjectHandlePair::new(private_key, public_key)
}

#[op2]
#[cppgc]
pub fn op_node_generate_ed448_key() -> KeyObjectHandlePair {
  ed448_generate()
}

#[op2]
#[cppgc]
pub async fn op_node_generate_ed448_key_async() -> KeyObjectHandlePair {
  spawn_blocking(ed448_generate).await.unwrap()
}

fn dh_group_generate(
  group_name: &str,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  let (dh, prime, generator) = match group_name {
    "modp5" => (
      dh::DiffieHellman::group::<dh::Modp1536>(),
      dh::Modp1536::MODULUS,
      dh::Modp1536::GENERATOR,
    ),
    "modp14" => (
      dh::DiffieHellman::group::<dh::Modp2048>(),
      dh::Modp2048::MODULUS,
      dh::Modp2048::GENERATOR,
    ),
    "modp15" => (
      dh::DiffieHellman::group::<dh::Modp3072>(),
      dh::Modp3072::MODULUS,
      dh::Modp3072::GENERATOR,
    ),
    "modp16" => (
      dh::DiffieHellman::group::<dh::Modp4096>(),
      dh::Modp4096::MODULUS,
      dh::Modp4096::GENERATOR,
    ),
    "modp17" => (
      dh::DiffieHellman::group::<dh::Modp6144>(),
      dh::Modp6144::MODULUS,
      dh::Modp6144::GENERATOR,
    ),
    "modp18" => (
      dh::DiffieHellman::group::<dh::Modp8192>(),
      dh::Modp8192::MODULUS,
      dh::Modp8192::GENERATOR,
    ),
    _ => return Err(JsErrorBox::type_error("Unsupported group name")),
  };
  // MODULUS arrays are big-endian u32 words. Convert to big-endian bytes
  // for ASN.1, matching the prime used during key generation.
  // Prepend 0x00 if MSB is set, since ASN.1 integers are signed.
  let mut prime_bytes: Vec<u8> =
    prime.iter().flat_map(|x| x.to_be_bytes()).collect();
  if prime_bytes.first().is_some_and(|b| b & 0x80 != 0) {
    prime_bytes.insert(0, 0x00);
  }
  let gen_bytes = [generator as u8];
  let params = DhParameter {
    prime: asn1::Int::new(&prime_bytes).unwrap(),
    base: asn1::Int::new(&gen_bytes).unwrap(),
    private_value_length: None,
  };
  Ok(KeyObjectHandlePair::new(
    AsymmetricPrivateKey::Dh(DhPrivateKey {
      key: dh.private_key,
      params: params.clone(),
    }),
    AsymmetricPublicKey::Dh(DhPublicKey {
      key: dh.public_key,
      params,
    }),
  ))
}

#[op2]
#[cppgc]
pub fn op_node_generate_dh_group_key(
  #[string] group_name: &str,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  dh_group_generate(group_name)
}

#[op2]
#[cppgc]
pub async fn op_node_generate_dh_group_key_async(
  #[string] group_name: String,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  spawn_blocking(move || dh_group_generate(&group_name))
    .await
    .unwrap()
}

fn dh_generate(
  prime: Option<&[u8]>,
  prime_len: usize,
  generator: usize,
) -> KeyObjectHandlePair {
  let prime = prime
    .map(|p| p.into())
    .unwrap_or_else(|| Prime::generate(prime_len));
  let dh = dh::DiffieHellman::new(prime.clone(), generator);
  let gen_bytes = if generator <= 0xFF {
    vec![generator as u8]
  } else if generator <= 0xFFFF {
    vec![(generator >> 8) as u8, generator as u8]
  } else {
    generator
      .to_be_bytes()
      .iter()
      .copied()
      .skip_while(|&b| b == 0)
      .collect()
  };
  let params = DhParameter {
    prime: asn1::Int::new(&prime.0.to_bytes_be()).unwrap(),
    base: asn1::Int::new(&gen_bytes).unwrap(),
    private_value_length: None,
  };
  KeyObjectHandlePair::new(
    AsymmetricPrivateKey::Dh(DhPrivateKey {
      key: dh.private_key,
      params: params.clone(),
    }),
    AsymmetricPublicKey::Dh(DhPublicKey {
      key: dh.public_key,
      params,
    }),
  )
}

#[op2]
#[cppgc]
pub fn op_node_generate_dh_key(
  #[buffer] prime: Option<&[u8]>,
  #[smi] prime_len: usize,
  #[smi] generator: usize,
) -> KeyObjectHandlePair {
  dh_generate(prime, prime_len, generator)
}

#[op2]
#[cppgc]
pub async fn op_node_generate_dh_key_async(
  #[buffer(copy)] prime: Option<Box<[u8]>>,
  #[smi] prime_len: usize,
  #[smi] generator: usize,
) -> KeyObjectHandlePair {
  spawn_blocking(move || dh_generate(prime.as_deref(), prime_len, generator))
    .await
    .unwrap()
}

#[op2]
pub fn op_node_dh_keys_generate_and_export(
  #[buffer] prime: Option<&[u8]>,
  #[smi] prime_len: usize,
  #[smi] generator: usize,
) -> (Uint8Array, Uint8Array) {
  let prime = prime
    .map(|p| p.into())
    .unwrap_or_else(|| Prime::generate(prime_len));
  let dh = dh::DiffieHellman::new(prime, generator);
  let private_key = dh.private_key.into_vec();
  let public_key = dh.public_key.into_vec();
  (private_key.into(), public_key.into())
}

#[op2]
pub fn op_node_export_secret_key(
  #[cppgc] handle: &KeyObjectHandle,
) -> Result<Uint8Array, JsErrorBox> {
  let key = handle
    .as_secret_key()
    .ok_or_else(|| JsErrorBox::type_error("key is not a secret key"))?;
  Ok(key.to_vec().into())
}

#[op2]
#[string]
pub fn op_node_export_secret_key_b64url(
  #[cppgc] handle: &KeyObjectHandle,
) -> Result<String, JsErrorBox> {
  let key = handle
    .as_secret_key()
    .ok_or_else(|| JsErrorBox::type_error("key is not a secret key"))?;
  Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key))
}

#[op2]
#[serde]
pub fn op_node_export_public_key_jwk(
  #[cppgc] handle: &KeyObjectHandle,
) -> Result<deno_core::serde_json::Value, AsymmetricPublicKeyJwkError> {
  let public_key = handle
    .as_public_key()
    .ok_or(AsymmetricPublicKeyJwkError::KeyIsNotAsymmetricPublicKey)?;

  public_key.export_jwk()
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ExportPublicKeyPemError {
  #[class(inherit)]
  #[error(transparent)]
  AsymmetricPublicKeyDer(
    #[from]
    #[inherit]
    AsymmetricPublicKeyDerError,
  ),
  #[class(type)]
  #[error("very large data")]
  VeryLargeData,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] der::Error),
}

#[op2]
#[string]
pub fn op_node_export_public_key_pem(
  #[cppgc] handle: &KeyObjectHandle,
  #[string] typ: &str,
) -> Result<String, ExportPublicKeyPemError> {
  let public_key = handle
    .as_public_key()
    .ok_or(AsymmetricPublicKeyDerError::KeyIsNotAsymmetricPublicKey)?;
  let data = public_key.export_der(typ)?;

  let label = match typ {
    "pkcs1" => "RSA PUBLIC KEY",
    "spki" => "PUBLIC KEY",
    _ => unreachable!("export_der would have errored"),
  };

  let pem_len = der::pem::encapsulated_len(label, LineEnding::LF, data.len())
    .map_err(|_| ExportPublicKeyPemError::VeryLargeData)?;
  let mut out = vec![0; pem_len];
  let mut writer = PemWriter::new(label, LineEnding::LF, &mut out)?;
  writer.write(&data)?;
  let len = writer.finish()?;
  out.truncate(len);

  Ok(String::from_utf8(out).expect("invalid pem is not possible"))
}

#[op2]
#[buffer]
pub fn op_node_export_public_key_der(
  #[cppgc] handle: &KeyObjectHandle,
  #[string] typ: &str,
) -> Result<Box<[u8]>, AsymmetricPublicKeyDerError> {
  let public_key = handle
    .as_public_key()
    .ok_or(AsymmetricPublicKeyDerError::KeyIsNotAsymmetricPublicKey)?;
  public_key.export_der(typ)
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ExportPrivateKeyPemError {
  #[class(inherit)]
  #[error(transparent)]
  AsymmetricPublicKeyDer(
    #[from]
    #[inherit]
    AsymmetricPrivateKeyDerError,
  ),
  #[class(type)]
  #[error("very large data")]
  VeryLargeData,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] der::Error),
  #[class(type)]
  #[error("{0}")]
  UnsupportedCipher(String),
  #[class(type)]
  #[error(
    "cipher and passphrase must both be provided for encrypted key export"
  )]
  MissingCipherOrPassphrase,
}

/// Derive an encryption key from a passphrase and salt using the legacy
/// OpenSSL EVP_BytesToKey algorithm (MD5-based). This is used for the
/// traditional PEM encryption format (Proc-Type/DEK-Info headers).
fn evp_bytes_to_key(
  passphrase: &[u8],
  salt: &[u8; 8],
  key_len: usize,
) -> Vec<u8> {
  use digest::Digest;

  let mut key = Vec::with_capacity(key_len);
  let mut prev_hash: Option<[u8; 16]> = None;

  while key.len() < key_len {
    let mut hasher = md5::Md5::new();
    if let Some(ref prev) = prev_hash {
      hasher.update(prev);
    }
    hasher.update(passphrase);
    hasher.update(salt);
    let hash: [u8; 16] = hasher.finalize().into();
    key.extend_from_slice(&hash);
    prev_hash = Some(hash);
  }

  key.truncate(key_len);
  key
}

/// Encrypt DER data and format as legacy OpenSSL encrypted PEM with
/// Proc-Type and DEK-Info headers.
fn encrypt_private_key_pem(
  label: &str,
  data: &[u8],
  cipher_name: &str,
  passphrase: &[u8],
) -> Result<String, ExportPrivateKeyPemError> {
  use aes::cipher::BlockEncryptMut;
  use aes::cipher::KeyIvInit;
  use aes::cipher::block_padding::Pkcs7;

  let (key_len, dek_info_name, iv_len) = match cipher_name {
    "aes-128-cbc" => (16, "AES-128-CBC", 16),
    "aes-192-cbc" => (24, "AES-192-CBC", 16),
    "aes-256-cbc" => (32, "AES-256-CBC", 16),
    "des-ede3-cbc" => (24, "DES-EDE3-CBC", 8),
    _ => {
      return Err(ExportPrivateKeyPemError::UnsupportedCipher(format!(
        "Unsupported cipher for PEM encryption: {cipher_name}"
      )));
    }
  };

  // Generate random IV
  let mut iv = vec![0u8; iv_len];
  thread_rng().fill_bytes(&mut iv);

  // Derive key using EVP_BytesToKey (uses first 8 bytes of IV as salt)
  let mut salt = [0u8; 8];
  salt.copy_from_slice(&iv[..8]);
  let key = evp_bytes_to_key(passphrase, &salt, key_len);

  // Encrypt with PKCS#7 padding
  let encrypted = match cipher_name {
    "aes-128-cbc" => cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &iv)
      .unwrap()
      .encrypt_padded_vec_mut::<Pkcs7>(data),
    "aes-192-cbc" => cbc::Encryptor::<aes::Aes192>::new_from_slices(&key, &iv)
      .unwrap()
      .encrypt_padded_vec_mut::<Pkcs7>(data),
    "aes-256-cbc" => cbc::Encryptor::<aes::Aes256>::new_from_slices(&key, &iv)
      .unwrap()
      .encrypt_padded_vec_mut::<Pkcs7>(data),
    "des-ede3-cbc" => {
      cbc::Encryptor::<des::TdesEde3>::new_from_slices(&key, &iv)
        .unwrap()
        .encrypt_padded_vec_mut::<Pkcs7>(data)
    }
    _ => unreachable!(),
  };

  // Format as legacy encrypted PEM
  let iv_hex = iv.iter().map(|b| format!("{b:02X}")).collect::<String>();
  let b64 = base64::engine::general_purpose::STANDARD.encode(&encrypted);

  // Split base64 into 64-char lines
  let mut pem = String::new();
  pem.push_str(&format!("-----BEGIN {label}-----\n"));
  pem.push_str("Proc-Type: 4,ENCRYPTED\n");
  pem.push_str(&format!("DEK-Info: {dek_info_name},{iv_hex}\n"));
  pem.push('\n');
  for chunk in b64.as_bytes().chunks(64) {
    pem.push_str(std::str::from_utf8(chunk).unwrap());
    pem.push('\n');
  }
  pem.push_str(&format!("-----END {label}-----\n"));

  Ok(pem)
}

#[op2]
#[string]
pub fn op_node_export_private_key_pem(
  #[cppgc] handle: &KeyObjectHandle,
  #[string] typ: &str,
  #[string] cipher: Option<String>,
  #[string] passphrase: Option<String>,
) -> Result<String, ExportPrivateKeyPemError> {
  let private_key = handle
    .as_private_key()
    .ok_or(AsymmetricPrivateKeyDerError::KeyIsNotAsymmetricPrivateKey)?;
  let data = private_key.export_der(typ)?;

  let label = match typ {
    "pkcs1" => "RSA PRIVATE KEY",
    "pkcs8" => "PRIVATE KEY",
    "sec1" => "EC PRIVATE KEY",
    _ => unreachable!("export_der would have errored"),
  };

  match (&cipher, &passphrase) {
    (Some(cipher), Some(passphrase)) => {
      return encrypt_private_key_pem(
        label,
        &data,
        cipher,
        passphrase.as_bytes(),
      );
    }
    (Some(_), None) | (None, Some(_)) => {
      return Err(ExportPrivateKeyPemError::MissingCipherOrPassphrase);
    }
    (None, None) => {}
  }

  let pem_len = der::pem::encapsulated_len(label, LineEnding::LF, data.len())
    .map_err(|_| ExportPrivateKeyPemError::VeryLargeData)?;
  let mut out = vec![0; pem_len];
  let mut writer = PemWriter::new(label, LineEnding::LF, &mut out)?;
  writer.write(&data)?;
  let len = writer.finish()?;
  out.truncate(len);

  Ok(String::from_utf8(out).expect("invalid pem is not possible"))
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ExportPrivateKeyJwkError {
  #[class(inherit)]
  #[error(transparent)]
  AsymmetricPublicKeyJwk(#[from] AsymmetricPrivateKeyJwkError),
  #[class(type)]
  #[error("very large data")]
  VeryLargeData,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] der::Error),
}

#[op2]
#[serde]
pub fn op_node_export_private_key_jwk(
  #[cppgc] handle: &KeyObjectHandle,
) -> Result<deno_core::serde_json::Value, ExportPrivateKeyJwkError> {
  let private_key = handle
    .as_private_key()
    .ok_or(AsymmetricPrivateKeyJwkError::KeyIsNotAsymmetricPrivateKey)?;

  Ok(private_key.export_jwk()?)
}

#[op2]
#[buffer]
pub fn op_node_export_private_key_der(
  #[cppgc] handle: &KeyObjectHandle,
  #[string] typ: &str,
) -> Result<Box<[u8]>, AsymmetricPrivateKeyDerError> {
  let private_key = handle
    .as_private_key()
    .ok_or(AsymmetricPrivateKeyDerError::KeyIsNotAsymmetricPrivateKey)?;
  private_key.export_der(typ)
}

#[op2]
#[string]
pub fn op_node_key_type(#[cppgc] handle: &KeyObjectHandle) -> &'static str {
  match handle {
    KeyObjectHandle::AsymmetricPrivate(_) => "private",
    KeyObjectHandle::AsymmetricPublic(_) => "public",
    KeyObjectHandle::Secret(_) => "secret",
  }
}

#[op2(fast)]
pub fn op_node_key_equals(
  #[cppgc] handle: &KeyObjectHandle,
  #[cppgc] other: &KeyObjectHandle,
) -> bool {
  handle == other
}

#[op2]
#[cppgc]
pub fn op_node_derive_public_key_from_private_key(
  #[cppgc] handle: &KeyObjectHandle,
) -> Result<KeyObjectHandle, JsErrorBox> {
  let Some(private_key) = handle.as_private_key() else {
    return Err(JsErrorBox::type_error("expected private key"));
  };

  Ok(KeyObjectHandle::AsymmetricPublic(
    private_key.to_public_key(),
  ))
}
