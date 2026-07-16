// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;

use base64::Engine;
use deno_core::FromV8;
use deno_core::GarbageCollected;
use deno_core::ToV8;
use deno_core::convert::Uint8Array;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_error::JsErrorBox;
use digest::Digest;
use digest::FixedOutputReset;
use ed25519_dalek::pkcs8::BitStringRef;
use elliptic_curve::JwkEcKey;
use hmac::Hmac;
use hmac::Mac;
use num_bigint::BigInt;
use num_traits::FromPrimitive as _;
use p12::AlgorithmIdentifier as Pkcs12AlgorithmIdentifier;
use p12::CertBag as Pkcs12CertBag;
use p12::ContentInfo as Pkcs12ContentInfo;
use p12::PFX as Pkcs12;
use p12::SafeBag as Pkcs12SafeBag;
use p12::SafeBagKind as Pkcs12SafeBagKind;
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
  #[class(type)]
  #[error("Invalid JWK RSA key")]
  InvalidJwk,
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
  #[class(generic)]
  #[error("error:1E08010C:DECODER routines::unsupported")]
  InvalidEncryptedPemPrivateKey,
  #[class(generic)]
  #[error("error:1C800064:Provider routines::bad decrypt")]
  EncryptedPrivateKeyBadDecrypt,
  #[error("error:1E08010C:DECODER routines::unsupported")]
  InvalidPemPrivateKey,
  #[class(generic)]
  #[property("code" = "ERR_OSSL_EVP_BAD_DECRYPT")]
  #[error("error:1C800064:Provider routines::bad decrypt")]
  BadDecrypt,
  #[class(generic)]
  #[property("code" = "ERR_OSSL_CRYPTO_INTERRUPTED_OR_CANCELLED")]
  #[error("error:07880109:common libcrypto routines::interrupted or cancelled")]
  EncryptedPrivateKeyRequiresPassphraseToDecrypt,
  #[property("code" = "ERR_MISSING_PASSPHRASE")]
  #[error("Passphrase required for encrypted key")]
  EncryptedPkcs8DerRequiresPassphrase,
  #[error("error:1E08010C:DECODER routines::unsupported")]
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

trait RsaPublicKeyExt: Sized {
  /// Parse `RSAPublicKey` DER (`SEQUENCE { INTEGER n, INTEGER e }`) the way
  /// Node/OpenSSL do, treating the INTEGER payloads as unsigned. This accepts
  /// moduli that omit the leading `0x00` byte strict DER requires for positive
  /// values with the high bit set, which the `rsa` crate rejects as malformed.
  /// The length encoding is still validated as strict DER; only the
  /// signed/unsigned interpretation of the integer payload is relaxed.
  ///
  /// Returns `None` for input malformed by any other rule. Intended as a
  /// fallback once the strict parser has already rejected the input:
  ///
  /// ```ignore
  /// RsaPublicKey::from_pkcs1_der(der)
  ///   .or_else(|err| RsaPublicKey::from_pkcs1_der_lenient(der).ok_or(err))
  /// ```
  fn from_pkcs1_der_lenient(der: &[u8]) -> Option<Self>;
}

impl RsaPublicKeyExt for RsaPublicKey {
  fn from_pkcs1_der_lenient(der: &[u8]) -> Option<Self> {
    let mut pos = 0;
    let sequence_end = read_der_tag_and_len(der, &mut pos, 0x30)?;
    if sequence_end != der.len() {
      return None;
    }

    let n = read_lenient_positive_integer(der, &mut pos, sequence_end)?;
    let e = read_lenient_positive_integer(der, &mut pos, sequence_end)?;
    if pos != sequence_end {
      return None;
    }

    RsaPublicKey::new(n, e).ok()
  }
}

fn read_lenient_positive_integer(
  der: &[u8],
  pos: &mut usize,
  limit: usize,
) -> Option<rsa::BigUint> {
  let end = read_der_tag_and_len(der, pos, 0x02)?;
  if end > limit || end == *pos {
    return None;
  }

  let payload = &der[*pos..end];
  *pos = end;
  // Reject all-zero payloads: a valid RSA modulus or public exponent is
  // strictly positive, and BigUint::from_bytes_be(&[]) would produce 0.
  let first_non_zero = payload.iter().position(|&byte| byte != 0)?;
  Some(rsa::BigUint::from_bytes_be(&payload[first_non_zero..]))
}

fn read_der_tag_and_len(der: &[u8], pos: &mut usize, tag: u8) -> Option<usize> {
  if der.get(*pos) != Some(&tag) {
    return None;
  }
  *pos += 1;

  let len_byte = *der.get(*pos)?;
  *pos += 1;

  let len = if len_byte & 0x80 == 0 {
    usize::from(len_byte)
  } else {
    let len_len = usize::from(len_byte & 0x7f);
    if len_len == 0 || len_len > std::mem::size_of::<usize>() {
      return None;
    }

    let len_bytes = der.get(*pos..pos.checked_add(len_len)?)?;
    if len_bytes.first() == Some(&0) {
      return None;
    }

    *pos += len_len;
    let mut len = 0usize;
    for &byte in len_bytes {
      len = len.checked_shl(8)?.checked_add(usize::from(byte))?;
    }
    if len < 128 {
      return None;
    }
    len
  };

  pos.checked_add(len).filter(|&end| end <= der.len())
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

fn normalize_pem_line_width(pem: &str) -> Cow<'_, str> {
  let mut needs_reformat = false;
  let mut header = "";
  let mut footer = "";
  let mut base64_body = String::new();
  let mut in_body = false;

  for line in pem.lines() {
    if line.starts_with("-----BEGIN ") {
      header = line;
      in_body = true;
    } else if line.starts_with("-----END ") {
      footer = line;
      in_body = false;
    } else if in_body {
      let trimmed = line.trim();
      if trimmed.len() > 64 {
        needs_reformat = true;
      }
      base64_body.push_str(trimmed);
    }
  }

  if !needs_reformat || header.is_empty() || footer.is_empty() {
    return Cow::Borrowed(pem);
  }

  let mut result = String::with_capacity(pem.len() + 10);
  result.push_str(header);
  result.push('\n');
  for chunk in base64_body.as_bytes().chunks(64) {
    result.push_str(std::str::from_utf8(chunk).unwrap_or(""));
    result.push('\n');
  }
  result.push_str(footer);
  result.push('\n');
  Cow::Owned(result)
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

        // Legacy encrypted PEM (Proc-Type/DEK-Info) — handle before
        // SecretDocument::from_pem, which doesn't understand these headers.
        if let Some((label, decrypted)) =
          parse_legacy_encrypted_pem(pem, passphrase)?
        {
          match label {
            "RSA PRIVATE KEY" => SecretDocument::from_pkcs1_der(&decrypted)
              .map_err(|_| AsymmetricPrivateKeyError::InvalidPkcs1PrivateKey)?,
            "EC PRIVATE KEY" => SecretDocument::from_sec1_der(&decrypted)
              .map_err(|_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey)?,
            "PRIVATE KEY" => SecretDocument::from_pkcs8_der(&decrypted)
              .map_err(|_| AsymmetricPrivateKeyError::InvalidPkcs8PrivateKey)?,
            "DSA PRIVATE KEY" => {
              let private_key = parse_traditional_dsa_private_key(&decrypted)?;
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
        } else if let Some(passphrase) = passphrase {
          // Try standard PKCS#8 encrypted PEM. Distinguish wrong passphrase
          // (correct format, decryption fails) from PEM that isn't actually
          // encrypted PKCS#8 — Node ignores the passphrase in the latter.
          match SecretDocument::from_pkcs8_encrypted_pem(pem, passphrase) {
            Ok(doc) => doc,
            Err(pkcs8::Error::EncryptedPrivateKey(_)) => {
              return Err(
                AsymmetricPrivateKeyError::EncryptedPrivateKeyBadDecrypt,
              );
            }
            Err(_) => {
              let normalized = normalize_pem_line_width(pem);
              let (label, doc) = SecretDocument::from_pem(&normalized)
                .map_err(|_| AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;
              match label {
                PrivateKeyInfo::PEM_LABEL => doc,
                rsa::pkcs1::RsaPrivateKey::PEM_LABEL => {
                  SecretDocument::from_pkcs1_der(doc.as_bytes()).map_err(
                    |_| AsymmetricPrivateKeyError::InvalidPkcs1PrivateKey,
                  )?
                }
                sec1::EcPrivateKey::PEM_LABEL => {
                  SecretDocument::from_sec1_der(doc.as_bytes()).map_err(
                    |_| AsymmetricPrivateKeyError::InvalidSec1PrivateKey,
                  )?
                }
                _ => {
                  return Err(
                    AsymmetricPrivateKeyError::InvalidEncryptedPemPrivateKey,
                  );
                }
              }
            }
          }
        } else {
          // Skip EC PARAMETERS block if present (legacy EC key format includes both)
          let pem = skip_ec_parameters_block(pem);
          let normalized = normalize_pem_line_width(pem);
          let (label, doc) = SecretDocument::from_pem(&normalized)
            .map_err(|_| AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;

          match label {
            EncryptedPrivateKeyInfo::PEM_LABEL => {
              return Err(AsymmetricPrivateKeyError::EncryptedPrivateKeyRequiresPassphraseToDecrypt);
            }
            PrivateKeyInfo::PEM_LABEL => doc,
            rsa::pkcs1::RsaPrivateKey::PEM_LABEL => {
              let pkcs1_der = doc.as_bytes();
              SecretDocument::from_pkcs1_der(pkcs1_der).map_err(|_| {
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
            if EncryptedPrivateKeyInfo::try_from(key).is_ok() {
              SecretDocument::from_pkcs8_encrypted_der(key, passphrase)
                .map_err(|_| {
                  AsymmetricPrivateKeyError::InvalidEncryptedPkcs8PrivateKey
                })?
            } else {
              // Node ignores the passphrase when the key isn't actually
              // encrypted.
              SecretDocument::from_pkcs8_der(key).map_err(|_| {
                AsymmetricPrivateKeyError::InvalidPkcs8PrivateKey
              })?
            }
          } else if EncryptedPrivateKeyInfo::try_from(key).is_ok() {
            return Err(
              AsymmetricPrivateKeyError::EncryptedPkcs8DerRequiresPassphrase,
            );
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

    let document_bytes = document.as_bytes();
    let pk_info = PrivateKeyInfo::try_from(document_bytes)
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

    let n_bytes = BASE64_URL_SAFE_NO_PAD.decode(jwk.n.as_bytes())?;
    let e_bytes = BASE64_URL_SAFE_NO_PAD.decode(jwk.e.as_bytes())?;

    let n = rsa::BigUint::from_bytes_be(&n_bytes);
    let e = rsa::BigUint::from_bytes_be(&e_bytes);

    if is_public {
      let public_key = RsaPublicKey::new(n, e)?;

      Ok(KeyObjectHandle::AsymmetricPublic(AsymmetricPublicKey::Rsa(
        public_key,
      )))
    } else {
      let d_bytes = BASE64_URL_SAFE_NO_PAD.decode(
        jwk
          .d
          .ok_or(RsaJwkError::MissingRsaPrivateComponent)?
          .as_bytes(),
      )?;
      let p_bytes = BASE64_URL_SAFE_NO_PAD.decode(
        jwk
          .p
          .ok_or(RsaJwkError::MissingRsaPrivateComponent)?
          .as_bytes(),
      )?;
      let q_bytes = BASE64_URL_SAFE_NO_PAD.decode(
        jwk
          .q
          .ok_or(RsaJwkError::MissingRsaPrivateComponent)?
          .as_bytes(),
      )?;

      let d = rsa::BigUint::from_bytes_be(&d_bytes);
      let p = rsa::BigUint::from_bytes_be(&p_bytes);
      let q = rsa::BigUint::from_bytes_be(&q_bytes);

      if &p * &q != n {
        return Err(RsaJwkError::InvalidJwk);
      }

      let mut private_key =
        RsaPrivateKey::from_components(n, e, d, vec![p, q])?;

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

        // Legacy "Proc-Type: 4,ENCRYPTED" PEMs (e.g. EC PRIVATE KEY encrypted
        // with AES-128-CBC) cannot be parsed by decode_pem_lenient because the
        // DEK-Info header bytes aren't valid base64. Route directly through
        // the private key path, which knows how to decrypt them.
        if pem.contains("Proc-Type: 4,ENCRYPTED") {
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

        let (label, document) = decode_pem_lenient(pem)
          .ok_or(AsymmetricPublicKeyError::InvalidPemPublicKey)?;

        match label.as_str() {
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
        let der = spki
          .subject_public_key
          .as_bytes()
          .ok_or(AsymmetricPublicKeyError::InvalidSpkiPublicKey)?;
        let public_key = RsaPublicKey::from_pkcs1_der(der).or_else(|err| {
          RsaPublicKey::from_pkcs1_der_lenient(der).ok_or(err)
        })?;
        AsymmetricPublicKey::Rsa(public_key)
      }
      RSASSA_PSS_OID => {
        let details = parse_rsa_pss_params(spki.algorithm.parameters)?;
        let der = spki
          .subject_public_key
          .as_bytes()
          .ok_or(AsymmetricPublicKeyError::InvalidSpkiPublicKey)?;
        let public_key = RsaPublicKey::from_pkcs1_der(der).or_else(|err| {
          RsaPublicKey::from_pkcs1_der_lenient(der).ok_or(err)
        })?;
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
/// Skips the EC PARAMETERS PEM block if present at the start.
/// Legacy EC key files sometimes include an EC PARAMETERS block before the
/// actual EC PRIVATE KEY block; `SecretDocument::from_pem` reads only the
/// first block, so we need to skip it.
fn skip_ec_parameters_block(pem: &str) -> &str {
  const BEGIN_EC_PARAMS: &str = "-----BEGIN EC PARAMETERS-----";
  const END_EC_PARAMS: &str = "-----END EC PARAMETERS-----";
  let trimmed = pem.trim_start();
  if trimmed.starts_with(BEGIN_EC_PARAMS)
    && let Some(pos) = trimmed.find(END_EC_PARAMS)
  {
    return trimmed[pos + END_EC_PARAMS.len()..].trim_start();
  }
  trimmed
}

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

/// Leniently decode a PEM string, tolerating non-standard line widths.
/// The strict `Document::from_pem` / `SecretDocument::from_pem` reject PEM
/// with lines longer than 64 base64 characters (per RFC 7468), but OpenSSL
/// and Node.js accept any line width. This function falls back to manual
/// base64 decoding when the strict parser fails.
fn decode_pem_lenient(pem: &str) -> Option<(String, Document)> {
  // Try strict parsing first
  if let Ok((label, doc)) = Document::from_pem(pem) {
    return Some((label.to_string(), doc));
  }

  // Fall back to lenient parsing: extract label and base64 manually
  let pem = pem.trim();
  let first_line = pem.lines().next()?;
  let last_line = pem.lines().next_back()?;

  let label = first_line
    .strip_prefix("-----BEGIN ")
    .and_then(|s: &str| s.strip_suffix("-----"))?;
  let end_label = last_line
    .strip_prefix("-----END ")
    .and_then(|s: &str| s.strip_suffix("-----"))?;

  if label != end_label {
    return None;
  }

  let b64: String = pem
    .lines()
    .filter(|line| !line.starts_with("-----"))
    .flat_map(|line| line.chars())
    .filter(|c| !c.is_whitespace())
    .collect();

  let der = base64::engine::general_purpose::STANDARD
    .decode(&b64)
    .ok()?;

  let doc = Document::from_der(&der).ok()?;

  Some((label.to_string(), doc))
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
        let mgf1_params_any = alg.parameters.ok_or(
          RsaPssParamsParseError::MalformedOrMissingPssMaskGenAlgorithm,
        )?;
        // MGF1 parameters are an AlgorithmIdentifier (SEQUENCE { OID, params? }).
        // parameters_oid() fails because it expects a bare OID, but the actual
        // value is a SEQUENCE. Decode the SEQUENCE to extract the hash OID.
        let mgf1_hash_oid = mgf1_params_any
          .sequence(|reader| {
            let oid = rsa::pkcs8::ObjectIdentifier::decode(reader)?;
            // Consume optional parameters (e.g. NULL) without failing
            while !reader.is_finished() {
              rsa::pkcs8::der::asn1::AnyRef::decode(reader)?;
            }
            Ok(oid)
          })
          .map_err(|_| {
            RsaPssParamsParseError::MalformedOrMissingPssMaskGenAlgorithm
          })?;
        match mgf1_hash_oid {
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

    // RFC 4055 / PKCS#1 v2.1 default for RSASSA-PSS-params.saltLength is 20,
    // independent of the hash algorithm.
    let salt_length = params.salt_length.unwrap_or(20);

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
      // RSA-PSS, DSA, X448 all map to "Unsupported JWK Key Type." in Node.js.
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
            // Always emit the uncompressed SEC1 form for SPKI export — that
            // is what Node.js / OpenSSL produce, regardless of the curve's
            // `PointCompression` default (k256 defaults to compressed).
            use elliptic_curve::sec1::ToEncodedPoint;
            let (sec1, oid): (Box<[u8]>, _) = match key {
              EcPublicKey::P224(key) => (
                key.to_encoded_point(false).as_bytes().to_vec().into_boxed_slice(),
                ID_SECP224R1_OID,
              ),
              EcPublicKey::P256(key) => (
                key.to_encoded_point(false).as_bytes().to_vec().into_boxed_slice(),
                ID_SECP256R1_OID,
              ),
              EcPublicKey::P384(key) => (
                key.to_encoded_point(false).as_bytes().to_vec().into_boxed_slice(),
                ID_SECP384R1_OID,
              ),
              EcPublicKey::P521(key) => (
                key.to_encoded_point(false).as_bytes().to_vec().into_boxed_slice(),
                ID_SECP521R1_OID,
              ),
              EcPublicKey::Secp256k1(key) => (
                key.to_encoded_point(false).as_bytes().to_vec().into_boxed_slice(),
                ID_SECP256K1_OID,
              ),
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
      AsymmetricPrivateKey::RsaPss(_) => {
        Err(AsymmetricPrivateKeyJwkError::JwkExportNotImplementedForKeyType)
      }
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

#[derive(ToV8)]
#[to_v8(untagged)]
pub enum AsymmetricKeyDetails {
  Rsa {
    modulus_length: usize,
    public_exponent: deno_core::convert::BigInt,
  },
  RsaPss {
    modulus_length: usize,
    public_exponent: deno_core::convert::BigInt,
    hash_algorithm: &'static str,
    mgf1_hash_algorithm: &'static str,
    salt_length: u32,
  },
  #[to_v8(rename = "rsaPss")]
  RsaPssBasic {
    modulus_length: usize,
    public_exponent: deno_core::convert::BigInt,
  },
  Dsa {
    modulus_length: usize,
    divisor_length: usize,
  },
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
          public_exponent: public_exponent.into(),
        })
      }
      AsymmetricPrivateKey::RsaPss(key) => {
        let modulus_length = key.key.n().bits();
        let public_exponent = BigInt::from_bytes_be(
          num_bigint::Sign::Plus,
          &key.key.e().to_bytes_be(),
        );
        let public_exponent = public_exponent.into();
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
          public_exponent: public_exponent.into(),
        })
      }
      AsymmetricPublicKey::RsaPss(key) => {
        let modulus_length = key.key.n().bits();
        let public_exponent = BigInt::from_bytes_be(
          num_bigint::Sign::Plus,
          &key.key.e().to_bytes_be(),
        );
        let public_exponent = public_exponent.into();
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

/// Generate DSA common components (p, q, g) for arbitrary L and N values.
///
/// The `dsa` crate's `KeySize` only exposes a fixed set of (L, N) variants, so
/// for non-standard `modulusLength` / `divisorLength` combinations (e.g.
/// `modulusLength: 2049`) we generate the parameters ourselves and construct
/// `Components` via `Components::from_components`. The algorithm mirrors
/// `dsa::generate::components::common`: generate prime q of N bits, then a
/// prime p of L bits such that q divides (p - 1), then a generator g of order
/// q using the unverifiable method (FIPS 186-4 Appendix A.2.1).
fn dsa_generate_components<R: rand::Rng + rand::CryptoRng>(
  rng: &mut R,
  l: u32,
  n: u32,
) -> (
  num_bigint_dig::BigUint,
  num_bigint_dig::BigUint,
  num_bigint_dig::BigUint,
) {
  use num_bigint_dig::BigUint;
  use num_bigint_dig::RandBigInt;
  use num_bigint_dig::RandPrime;
  use num_bigint_dig::prime::probably_prime;
  use num_traits::One;
  use num_traits::Pow;

  const MR_ROUNDS: usize = 64;
  let two = || BigUint::from(2u8);
  let bounds = |size: u32| -> (BigUint, BigUint) {
    let lower = two().pow(size - 1);
    let upper = two().pow(size);
    (lower, upper)
  };

  let (p_min, p_max) = bounds(l);
  let (q_min, q_max) = bounds(n);

  let (p, q) = 'gen_pq: loop {
    let q = rng.gen_prime(n as usize);
    if q < q_min || q > q_max {
      continue;
    }

    // Attempt to find a prime p which has a subgroup of the order q
    for _ in 0..4096 {
      let m = 'gen_m: loop {
        let m = rng.gen_biguint(l as usize);
        if m > p_min && m < p_max {
          break 'gen_m m;
        }
      };
      let mr = &m % (two() * &q);
      let p = m - mr + BigUint::one();

      if probably_prime(&p, MR_ROUNDS) {
        break 'gen_pq (p, q);
      }
    }
  };

  // Generate g using the unverifiable method as defined by Appendix A.2.1
  let e = (&p - BigUint::one()) / &q;
  let mut h = BigUint::one();
  let g = loop {
    let g = h.modpow(&e, &p);
    if !num_traits::One::is_one(&g) {
      break g;
    }
    h += BigUint::one();
  };

  (p, q, g)
}

fn dsa_generate(
  modulus_length: usize,
  divisor_length: usize,
) -> Result<KeyObjectHandlePair, JsErrorBox> {
  use dsa::Components;
  use dsa::SigningKey;

  // Validate (L, N) per Node.js / OpenSSL behavior: divisor (N) must be
  // smaller than modulus (L) and large enough to be meaningful. We deliberately
  // accept arbitrary L and N otherwise (e.g. L=2049) to match Node's
  // `crypto.generateKeyPair('dsa', { modulusLength, divisorLength })`.
  if modulus_length < 2 || divisor_length < 2 {
    return Err(JsErrorBox::type_error(
      "Invalid modulusLength+divisorLength combination",
    ));
  }
  if divisor_length >= modulus_length {
    return Err(JsErrorBox::type_error(
      "Invalid modulusLength+divisorLength combination",
    ));
  }
  if u32::try_from(modulus_length).is_err()
    || u32::try_from(divisor_length).is_err()
  {
    return Err(JsErrorBox::type_error(
      "Invalid modulusLength+divisorLength combination",
    ));
  }

  let mut rng = rand::thread_rng();
  let (p, q, g) = dsa_generate_components(
    &mut rng,
    modulus_length as u32,
    divisor_length as u32,
  );
  let components = Components::from_components(p, q, g).map_err(|_| {
    JsErrorBox::type_error("Invalid modulusLength+divisorLength combination")
  })?;
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
  // Prepend 0x00 if MSB is set, since ASN.1 INTEGERs are signed; without
  // the sign byte the leading 0xFF bytes of safe primes would be stripped.
  let mut prime_bytes = prime.0.to_bytes_be();
  if prime_bytes.first().is_some_and(|b| b & 0x80 != 0) {
    prime_bytes.insert(0, 0x00);
  }
  let params = DhParameter {
    prime: asn1::Int::new(&prime_bytes).unwrap(),
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

/// Parse a legacy encrypted PEM (Proc-Type/DEK-Info headers) and return the
/// label and decrypted DER data, or None if not a legacy encrypted PEM.
///
/// SECURITY: This format is known-weak — it derives the encryption key with a
/// single MD5 round of EVP_BytesToKey, has no MAC (only PKCS#7 padding to
/// detect corruption), and reuses the first 8 bytes of the IV as the salt.
/// PKCS#7 unpadding is implemented in constant time (`pkcs7_unpad_ct`) so the
/// decrypt path itself doesn't leak through padding-oracle timing, but the
/// format-level weaknesses above are inherent to RFC 1421. Acceptable for
/// one-shot local PEM imports; do not use as a transport.
fn parse_legacy_encrypted_pem<'a>(
  pem: &'a str,
  passphrase: Option<&[u8]>,
) -> Result<Option<(&'a str, Vec<u8>)>, AsymmetricPrivateKeyError> {
  if !pem.contains("Proc-Type: 4,ENCRYPTED") {
    return Ok(None);
  }

  let passphrase = match passphrase {
    Some(p) => p,
    None => {
      return Err(
        AsymmetricPrivateKeyError::EncryptedPrivateKeyRequiresPassphraseToDecrypt,
      );
    }
  };

  let mut lines = pem.lines();

  let label = loop {
    match lines.next() {
      Some(line)
        if line.starts_with("-----BEGIN ") && line.ends_with("-----") =>
      {
        break &line[11..line.len() - 5];
      }
      Some(_) => continue,
      None => return Err(AsymmetricPrivateKeyError::InvalidPemPrivateKey),
    }
  };

  let proc_type_line = lines
    .next()
    .ok_or(AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;
  if !proc_type_line.starts_with("Proc-Type:") {
    return Err(AsymmetricPrivateKeyError::InvalidPemPrivateKey);
  }

  let dek_info_line = lines
    .next()
    .ok_or(AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;
  let dek_info = dek_info_line
    .strip_prefix("DEK-Info: ")
    .ok_or(AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;
  let (cipher_name, iv_hex) = dek_info
    .split_once(',')
    .ok_or(AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;

  let _ = lines.next();

  let mut b64_data = String::new();
  for line in lines {
    if line.starts_with("-----END ") {
      break;
    }
    b64_data.push_str(line.trim());
  }

  let encrypted_data = base64::engine::general_purpose::STANDARD
    .decode(&b64_data)
    .map_err(|_| AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;

  if !iv_hex.len().is_multiple_of(2) {
    return Err(AsymmetricPrivateKeyError::InvalidPemPrivateKey);
  }
  let mut iv = vec![0u8; iv_hex.len() / 2];
  faster_hex::hex_decode(iv_hex.as_bytes(), &mut iv)
    .map_err(|_| AsymmetricPrivateKeyError::InvalidPemPrivateKey)?;

  let (key_len, expected_iv_len) = match cipher_name {
    "AES-128-CBC" => (16, 16),
    "AES-192-CBC" => (24, 16),
    "AES-256-CBC" => (32, 16),
    "DES-EDE3-CBC" => (24, 8),
    _ => {
      return Err(AsymmetricPrivateKeyError::InvalidEncryptedPemPrivateKey);
    }
  };

  if iv.len() != expected_iv_len {
    return Err(AsymmetricPrivateKeyError::InvalidEncryptedPemPrivateKey);
  }

  let mut salt = [0u8; 8];
  salt.copy_from_slice(&iv[..8]);
  let key = evp_bytes_to_key(passphrase, &salt, key_len);

  // Decryption failure here most commonly means wrong passphrase; map to
  // the OpenSSL-compatible "bad decrypt" error so Node.js tests that check
  // the error message work correctly.
  let decrypted =
    decrypt_legacy_pem_data(cipher_name, &key, &iv, &encrypted_data)
      .map_err(|_| AsymmetricPrivateKeyError::BadDecrypt)?;

  Ok(Some((label, decrypted)))
}

fn decrypt_legacy_pem_data(
  cipher_name: &str,
  key: &[u8],
  iv: &[u8],
  data: &[u8],
) -> Result<Vec<u8>, ()> {
  use aes::cipher::BlockDecryptMut;
  use aes::cipher::KeyIvInit;
  use aes::cipher::block_padding::NoPadding;

  let (mut decrypted, block_size) = match cipher_name {
    "AES-128-CBC" => (
      cbc::Decryptor::<aes::Aes128>::new_from_slices(key, iv)
        .map_err(|_| ())?
        .decrypt_padded_vec_mut::<NoPadding>(data)
        .map_err(|_| ())?,
      16usize,
    ),
    "AES-192-CBC" => (
      cbc::Decryptor::<aes::Aes192>::new_from_slices(key, iv)
        .map_err(|_| ())?
        .decrypt_padded_vec_mut::<NoPadding>(data)
        .map_err(|_| ())?,
      16usize,
    ),
    "AES-256-CBC" => (
      cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv)
        .map_err(|_| ())?
        .decrypt_padded_vec_mut::<NoPadding>(data)
        .map_err(|_| ())?,
      16usize,
    ),
    "DES-EDE3-CBC" => (
      cbc::Decryptor::<des::TdesEde3>::new_from_slices(key, iv)
        .map_err(|_| ())?
        .decrypt_padded_vec_mut::<NoPadding>(data)
        .map_err(|_| ())?,
      8usize,
    ),
    _ => return Err(()),
  };

  pkcs7_unpad_ct(&mut decrypted, block_size).ok_or(())?;
  Ok(decrypted)
}

/// Constant-time PKCS#7 unpadding. Verifies and strips the trailing padding
/// from a decrypted CBC buffer without branching on padding-byte values, so
/// the decrypt path doesn't expose a Vaudenay-style padding oracle.
///
/// All bytes in the last block are inspected on every call; the comparison
/// against the claimed pad length and the equality checks against the pad
/// byte are computed branchlessly and combined with bitwise AND so the only
/// data-dependent branch is the final accept/reject.
fn pkcs7_unpad_ct(buf: &mut Vec<u8>, block_size: usize) -> Option<()> {
  use subtle::Choice;
  use subtle::ConstantTimeEq;

  let len = buf.len();
  if len == 0 || len < block_size || !len.is_multiple_of(block_size) {
    return None;
  }

  let pad_byte = buf[len - 1];

  // valid_pad_len: pad_byte is in [1, block_size]. Computed branchlessly so
  // the decision doesn't leak the actual pad length.
  let pad_nonzero = !pad_byte.ct_eq(&0u8);
  let pad_in_range = u8_le_ct(pad_byte, block_size as u8);
  let mut valid: Choice = pad_nonzero & pad_in_range;

  // For each position in the last block, if position-from-end < pad_byte the
  // byte must equal pad_byte. Loop over all `block_size` positions every time
  // to avoid leaking pad_byte through loop length.
  let start = len - block_size;
  for i in 0..block_size {
    let pos_from_end = (block_size - 1 - i) as u8;
    // is_in_pad: pad_byte > pos_from_end. Branchless via i16 subtraction.
    let is_in_pad = u8_gt_ct(pad_byte, pos_from_end);
    let byte_matches = buf[start + i].ct_eq(&pad_byte);
    // valid &= !is_in_pad | byte_matches
    valid &= !is_in_pad | byte_matches;
  }

  if !bool::from(valid) {
    return None;
  }
  let pad_len = pad_byte as usize;
  buf.truncate(len - pad_len);
  Some(())
}

#[inline]
fn u8_gt_ct(a: u8, b: u8) -> subtle::Choice {
  let diff = (b as i16).wrapping_sub(a as i16);
  subtle::Choice::from(((diff as u16) >> 15) as u8)
}

#[inline]
fn u8_le_ct(a: u8, b: u8) -> subtle::Choice {
  !u8_gt_ct(a, b)
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

/// Encrypt PKCS#8 PrivateKeyInfo DER and format as a PEM-encoded
/// PKCS#8 `EncryptedPrivateKeyInfo` (RFC 5208 / RFC 5958), which is what
/// Node.js produces when exporting `{ type: 'pkcs8', cipher, passphrase }`.
fn encrypt_pkcs8_private_key_pem(
  data: &[u8],
  cipher_name: &str,
  passphrase: &[u8],
) -> Result<String, ExportPrivateKeyPemError> {
  use pkcs8::pkcs5::pbes2;

  // PBKDF2 salt and cipher IV. Node.js uses a 16-byte salt and 8-byte IV for
  // 3DES, 16-byte IV for AES-CBC.
  let mut salt = [0u8; 16];
  thread_rng().fill_bytes(&mut salt);

  let pbkdf2_iters: u32 = 2048;

  let mut aes_iv = [0u8; 16];

  let pbes2_params = match cipher_name {
    "aes-128-cbc" => {
      thread_rng().fill_bytes(&mut aes_iv);
      pbes2::Parameters::pbkdf2_sha256_aes128cbc(pbkdf2_iters, &salt, &aes_iv)
        .map_err(|_| {
        ExportPrivateKeyPemError::UnsupportedCipher(format!(
          "Unsupported cipher for PKCS#8 encryption: {cipher_name}"
        ))
      })?
    }
    "aes-192-cbc" => {
      thread_rng().fill_bytes(&mut aes_iv);
      let kdf = pbes2::Pbkdf2Params::hmac_with_sha256(pbkdf2_iters, &salt)
        .map_err(|_| {
          ExportPrivateKeyPemError::UnsupportedCipher(format!(
            "Unsupported cipher for PKCS#8 encryption: {cipher_name}"
          ))
        })?
        .into();
      pbes2::Parameters {
        kdf,
        encryption: pbes2::EncryptionScheme::Aes192Cbc { iv: &aes_iv },
      }
    }
    "aes-256-cbc" => {
      thread_rng().fill_bytes(&mut aes_iv);
      pbes2::Parameters::pbkdf2_sha256_aes256cbc(pbkdf2_iters, &salt, &aes_iv)
        .map_err(|_| {
        ExportPrivateKeyPemError::UnsupportedCipher(format!(
          "Unsupported cipher for PKCS#8 encryption: {cipher_name}"
        ))
      })?
    }
    _ => {
      return Err(ExportPrivateKeyPemError::UnsupportedCipher(format!(
        "Unsupported cipher for PKCS#8 encryption: {cipher_name}"
      )));
    }
  };

  let pk_info = PrivateKeyInfo::try_from(data).map_err(|_| {
    ExportPrivateKeyPemError::UnsupportedCipher(
      "could not parse PKCS#8 private key for encryption".to_string(),
    )
  })?;
  let secret_doc = pk_info
    .encrypt_with_params(pbes2_params, passphrase)
    .map_err(|_| {
      ExportPrivateKeyPemError::UnsupportedCipher(format!(
        "Failed to encrypt PKCS#8 with cipher {cipher_name}"
      ))
    })?;
  let pem = secret_doc
    .to_pem(EncryptedPrivateKeyInfo::PEM_LABEL, LineEnding::LF)
    .map_err(|_| ExportPrivateKeyPemError::VeryLargeData)?;
  Ok(pem.to_string())
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
      // Node.js uses PKCS#8 EncryptedPrivateKeyInfo for `pkcs8` exports with
      // a cipher (label "ENCRYPTED PRIVATE KEY"); for `pkcs1`/`sec1` it uses
      // the legacy OpenSSL PEM encryption format with Proc-Type/DEK-Info.
      if typ == "pkcs8" {
        return encrypt_pkcs8_private_key_pem(
          &data,
          cipher,
          passphrase.as_bytes(),
        );
      }
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

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum ExportPrivateKeyDerError {
  #[class(inherit)]
  #[error(transparent)]
  AsymmetricPrivateKeyDer(
    #[from]
    #[inherit]
    AsymmetricPrivateKeyDerError,
  ),
  #[class(type)]
  #[error("{0}")]
  UnsupportedCipher(String),
  #[class(type)]
  #[error(
    "cipher and passphrase must both be provided for encrypted key export"
  )]
  MissingCipherOrPassphrase,
  #[class(type)]
  #[error("encryption is only supported for PKCS#8 private keys")]
  EncryptionRequiresPkcs8,
  #[class(generic)]
  #[error("failed to encrypt private key")]
  EncryptionFailed,
}

/// Encrypt a PKCS#8 DER private key using PBES2 (PBKDF2-HMAC-SHA256 + AES-CBC),
/// matching the format produced by OpenSSL when given a cipher name like
/// "aes-128-cbc". Returns the DER-encoded `EncryptedPrivateKeyInfo`.
fn encrypt_private_key_pkcs8_der(
  data: &[u8],
  cipher_name: &str,
  passphrase: &[u8],
) -> Result<Vec<u8>, ExportPrivateKeyDerError> {
  use pkcs8::pkcs5::pbes2;

  // 16-byte PBKDF2 salt and 16-byte AES IV, both random.
  let mut salt = [0u8; 16];
  let mut iv = [0u8; 16];
  thread_rng().fill_bytes(&mut salt);
  thread_rng().fill_bytes(&mut iv);

  // OpenSSL's default for PKCS#8 PBES2 export is 2048 iterations.
  const ITERATIONS: u32 = 2048;

  let pbes2_params = match cipher_name {
    "aes-128-cbc" => {
      pbes2::Parameters::pbkdf2_sha256_aes128cbc(ITERATIONS, &salt, &iv)
        .map_err(|_| ExportPrivateKeyDerError::EncryptionFailed)?
    }
    "aes-256-cbc" => {
      pbes2::Parameters::pbkdf2_sha256_aes256cbc(ITERATIONS, &salt, &iv)
        .map_err(|_| ExportPrivateKeyDerError::EncryptionFailed)?
    }
    _ => {
      return Err(ExportPrivateKeyDerError::UnsupportedCipher(format!(
        "Unsupported cipher for PKCS#8 DER encryption: {cipher_name}"
      )));
    }
  };

  let encrypted_data = pbes2_params
    .encrypt(passphrase, data)
    .map_err(|_| ExportPrivateKeyDerError::EncryptionFailed)?;

  let info = pkcs8::EncryptedPrivateKeyInfo {
    encryption_algorithm: pbes2_params.into(),
    encrypted_data: &encrypted_data,
  };

  let doc: SecretDocument = (&info)
    .try_into()
    .map_err(|_| ExportPrivateKeyDerError::EncryptionFailed)?;
  Ok(doc.as_bytes().to_vec())
}

#[op2]
#[buffer]
pub fn op_node_export_private_key_der(
  #[cppgc] handle: &KeyObjectHandle,
  #[string] typ: &str,
  #[string] cipher: Option<String>,
  #[string] passphrase: Option<String>,
) -> Result<Box<[u8]>, ExportPrivateKeyDerError> {
  let private_key = handle
    .as_private_key()
    .ok_or(AsymmetricPrivateKeyDerError::KeyIsNotAsymmetricPrivateKey)?;
  let data = private_key.export_der(typ)?;

  match (&cipher, &passphrase) {
    (Some(cipher), Some(passphrase)) => {
      if typ != "pkcs8" {
        return Err(ExportPrivateKeyDerError::EncryptionRequiresPkcs8);
      }
      let encrypted =
        encrypt_private_key_pkcs8_der(&data, cipher, passphrase.as_bytes())?;
      Ok(encrypted.into_boxed_slice())
    }
    (Some(_), None) | (None, Some(_)) => {
      Err(ExportPrivateKeyDerError::MissingCipherOrPassphrase)
    }
    (None, None) => Ok(data),
  }
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

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum PfxLoadError {
  #[class(generic)]
  #[error("not enough data")]
  NotEnoughData,
  #[class(generic)]
  #[error("mac verify failure")]
  MacVerifyFailure,
  #[class(generic)]
  #[error("PFX contains no usable certificate")]
  NoCert,
  #[class(generic)]
  #[error("PFX contains no usable private key")]
  NoKey,
  #[class(generic)]
  #[error(
    "failed to decrypt PFX private key (wrong passphrase or unsupported encryption)"
  )]
  KeyDecryptFailed,
  #[class(generic)]
  #[error("failed to encode PFX contents: {0}")]
  Encode(String),
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadPfxResult {
  pub cert: String,
  pub key: String,
  pub ca: Vec<String>,
}

fn der_to_pem(label: &str, der: &[u8]) -> String {
  use base64::engine::general_purpose::STANDARD;
  let body = STANDARD.encode(der);
  let mut out = String::with_capacity(body.len() + 64);
  out.push_str("-----BEGIN ");
  out.push_str(label);
  out.push_str("-----\n");
  for chunk in body.as_bytes().chunks(64) {
    // SAFETY: base64 output is ASCII.
    out.push_str(std::str::from_utf8(chunk).unwrap());
    out.push('\n');
  }
  out.push_str("-----END ");
  out.push_str(label);
  out.push_str("-----\n");
  out
}

// Cap on the PBKDF iteration count for any PFX-controlled KDF — the
// PKCS#12 MAC PBKDF and any PBES2/PBKDF2 inside a SafeContents or shrouded
// key bag envelope. An attacker who controls the PFX bytes also controls
// the iteration count (a u32, up to ~4 billion), which would otherwise tie
// up CPU for tens of seconds per call. Matches the limit Mozilla NSS uses
// for the MAC KDF; well above any realistic legitimate value — OpenSSL
// defaults to 2048 on creation, and hardened producers rarely go beyond
// ~100k. OpenSSL itself doesn't cap here, but Node trusts the caller's
// PFX; in Deno the PFX often comes from untrusted input (e.g. server
// config), so capping is defensive.
const PFX_PBKDF_ITERATIONS_CAP: u64 = 600_000;

// Cap on the working-set memory of a PFX-controlled scrypt KDF. PBES2 also
// permits scrypt (RFC 7914), whose cost is attacker-controlled the same way
// as a PBKDF2 iteration count but is memory-hard rather than iteration-hard,
// so the iteration cap above does not bound it. scrypt's working set is
// `128 * N * r` bytes; cap it at 1 GiB so a crafted PFX cannot force a
// multi-gigabyte allocation. OpenSSL/Node never emit scrypt for PKCS#12
// (this is well above any value a legitimate producer would use — the
// OWASP-recommended N=2^17, r=8 needs ~128 MiB), but pkcs5 will decrypt it,
// so the bound is defensive.
const PFX_SCRYPT_MEMORY_CAP: u128 = 1 << 30;

#[op2]
#[serde]
pub fn op_node_load_pfx(
  #[buffer] pfx: &[u8],
  #[string] passphrase: Option<String>,
) -> Result<LoadPfxResult, PfxLoadError> {
  let parsed = Pkcs12::parse(pfx).map_err(|_| PfxLoadError::NotEnoughData)?;
  let password = passphrase.as_deref().unwrap_or("");
  let bmp_password = bmp_string(password);
  // If a MAC is present, verify it. Absent MACs are accepted (matches
  // OpenSSL/Node behaviour).
  if let Some(mac_data) = &parsed.mac_data {
    let iterations = u64::from(mac_data.iterations);
    if iterations > PFX_PBKDF_ITERATIONS_CAP {
      return Err(PfxLoadError::MacVerifyFailure);
    }
    let data = parsed
      .auth_safe
      .data(&bmp_password)
      .ok_or(PfxLoadError::MacVerifyFailure)?;
    let ok = verify_pkcs12_mac(
      &mac_data.mac.digest_algorithm,
      &mac_data.mac.digest,
      &mac_data.salt,
      iterations,
      &data,
      &bmp_password,
    )
    .ok_or(PfxLoadError::MacVerifyFailure)?;
    if !ok {
      return Err(PfxLoadError::MacVerifyFailure);
    }
  }

  let safe_bags = pfx_safe_bags(&parsed, password, &bmp_password)?;

  let mut cert_ders: Vec<Vec<u8>> = Vec::new();
  let mut key_ders: Vec<Vec<u8>> = Vec::new();
  for bag in &safe_bags {
    match &bag.bag {
      Pkcs12SafeBagKind::CertBag(Pkcs12CertBag::X509(der)) => {
        cert_ders.push(der.clone());
      }
      Pkcs12SafeBagKind::Pkcs8ShroudedKeyBag(epki) => {
        let der = decrypt_pfx_blob(
          &epki.encryption_algorithm,
          &epki.encrypted_data,
          password,
          &bmp_password,
        )
        .ok_or(PfxLoadError::KeyDecryptFailed)?;
        key_ders.push(der);
      }
      // Unencrypted KeyBags and other bag kinds are ignored, matching the
      // upstream `p12` behaviour this replaces: `SafeBagKind::get_key` only
      // extracts `Pkcs8ShroudedKeyBag`, and an unencrypted PKCS#8 KeyBag
      // parses as `OtherBagKind`. TLS PFX bundles always shroud the key, so
      // this is not a shape we need to support.
      _ => {}
    }
  }

  if cert_ders.is_empty() {
    return Err(PfxLoadError::NoCert);
  }
  if key_ders.is_empty() {
    return Err(PfxLoadError::NoKey);
  }

  // The first cert bag is taken as the leaf; any additional bags are
  // returned as CA/chain certificates.
  let mut iter = cert_ders.into_iter();
  let leaf = iter.next().unwrap();
  let cert = der_to_pem("CERTIFICATE", &leaf);
  let ca: Vec<String> =
    iter.map(|der| der_to_pem("CERTIFICATE", &der)).collect();

  // Shrouded key bags decrypt to PKCS#8 DER.
  let key = der_to_pem("PRIVATE KEY", &key_ders[0]);

  Ok(LoadPfxResult { cert, key, ca })
}

// PBES2 OID (id-PBES2) arcs from RFC 8018, compared against the parsed
// algorithm OID directly so we do not allocate a fresh `ObjectIdentifier`
// for every bag.
const PBES2_OID_ARCS: [u64; 7] = [1, 2, 840, 113_549, 1, 5, 13];

// Decrypt a PFX blob (the SafeContents body of an EncryptedData ContentInfo,
// or the EncryptedPrivateKeyInfo body of a shrouded key bag), dispatching
// by algorithm:
//
// - PKCS#12 PBE (RC2-40 / 3DES with SHA-1 KDF): the only schemes the `p12`
//   crate handles natively. These take the password as a BMPString.
// - PBES2 (PBKDF2 + AES-CBC, and the other PBES2-supported ciphers): the
//   shape every modern `openssl pkcs12 -export` invocation emits. Handled
//   here via `pkcs8::pkcs5::EncryptionScheme`. PBES2 within PKCS#12 takes
//   the password as raw UTF-8 (no BMPString conversion); this matches
//   what OpenSSL and Node accept.
fn decrypt_pfx_blob(
  alg: &Pkcs12AlgorithmIdentifier,
  ciphertext: &[u8],
  utf8_password: &str,
  bmp_password: &[u8],
) -> Option<Vec<u8>> {
  match alg {
    Pkcs12AlgorithmIdentifier::PbewithSHAAnd40BitRC2CBC(params)
    | Pkcs12AlgorithmIdentifier::PbeWithSHAAnd3KeyTripleDESCBC(params) => {
      // The legacy PKCS#12 PBE iteration count is attacker-controlled too
      // (`Pkcs12PbeParams::iterations`), and `p12::decrypt_pbe` does not cap
      // it; apply the same cap as the MAC and PBES2 paths.
      if params.iterations > PFX_PBKDF_ITERATIONS_CAP {
        return None;
      }
      alg.decrypt_pbe(ciphertext, bmp_password)
    }
    Pkcs12AlgorithmIdentifier::OtherAlg(other)
      if other.algorithm_type.components().as_slice()
        == PBES2_OID_ARCS.as_slice() =>
    {
      // `p12` keeps `OtherAlg` as a partial parse (just the raw params
      // bytes), so round-trip the whole AlgorithmIdentifier back to DER to
      // let the pkcs8/pkcs5 crate parse the PBES2 parameters.
      let alg_der = yasna::construct_der(|w| alg.write(w));
      let scheme = pkcs8::pkcs5::EncryptionScheme::from_der(&alg_der).ok()?;
      // Apply the same PBKDF iteration cap used for the MAC. The PBKDF2
      // iteration count here is attacker-controlled in the same way
      // (parsed straight out of the PFX bytes), and
      // `pkcs8::pkcs5::EncryptionScheme::decrypt` does not cap on its own.
      if let Some(pbkdf2) = scheme.pbes2().and_then(|p| p.kdf.pbkdf2())
        && u64::from(pbkdf2.iteration_count) > PFX_PBKDF_ITERATIONS_CAP
      {
        return None;
      }
      // PBES2 may also use a scrypt KDF, which the iteration cap above does
      // not cover; bound its working-set memory (`128 * N * r`) instead.
      if let Some(scrypt) = scheme.pbes2().and_then(|p| p.kdf.scrypt()) {
        let working_set = u128::from(scrypt.cost_parameter)
          .saturating_mul(u128::from(scrypt.block_size))
          .saturating_mul(128);
        if working_set > PFX_SCRYPT_MEMORY_CAP {
          return None;
        }
      }
      scheme.decrypt(utf8_password.as_bytes(), ciphertext).ok()
    }
    _ => None,
  }
}

// Walk a PFX and return the flat list of SafeBags, decrypting EncryptedData
// envelopes along the way. This is a reimplementation of `p12::PFX::bags`
// that adds PBES2 support; the p12 crate only handles the legacy PKCS#12
// PBE algorithms (RC2-40 and 3DES with the SHA-1 KDF). Without PBES2 we
// cannot read PFX files produced by `openssl pkcs12 -export` on OpenSSL 3,
// which is the default shape Node also produces and consumes.
fn pfx_safe_bags(
  parsed: &Pkcs12,
  utf8_password: &str,
  bmp_password: &[u8],
) -> Result<Vec<Pkcs12SafeBag>, PfxLoadError> {
  let outer = pfx_content_data(&parsed.auth_safe, utf8_password, bmp_password)?;
  let safe_contents: Vec<Pkcs12ContentInfo> = yasna::parse_der(&outer, |r| {
    r.collect_sequence_of(Pkcs12ContentInfo::parse)
  })
  .map_err(|e| PfxLoadError::Encode(e.to_string()))?;
  let mut bags: Vec<Pkcs12SafeBag> = Vec::new();
  for content in &safe_contents {
    let bytes = pfx_content_data(content, utf8_password, bmp_password)?;
    let parsed: Vec<Pkcs12SafeBag> =
      yasna::parse_der(&bytes, |r| r.collect_sequence_of(Pkcs12SafeBag::parse))
        .map_err(|e| PfxLoadError::Encode(e.to_string()))?;
    bags.extend(parsed);
  }
  Ok(bags)
}

fn pfx_content_data(
  ci: &Pkcs12ContentInfo,
  utf8_password: &str,
  bmp_password: &[u8],
) -> Result<Vec<u8>, PfxLoadError> {
  match ci {
    Pkcs12ContentInfo::Data(d) => Ok(d.clone()),
    Pkcs12ContentInfo::EncryptedData(e) => decrypt_pfx_blob(
      &e.encrypted_content_info.content_encryption_algorithm,
      &e.encrypted_content_info.encrypted_content,
      utf8_password,
      bmp_password,
    )
    .ok_or_else(|| {
      // A `None` here is either a wrong passphrase or an algorithm pkcs5
      // cannot decrypt; we cannot tell which apart, so report both rather
      // than blaming the algorithm (which misleads on the common case of a
      // bad passphrase against an encrypted cert bag).
      PfxLoadError::Encode(
        "failed to decrypt PFX contents (wrong passphrase or unsupported encryption)"
          .into(),
      )
    }),
    Pkcs12ContentInfo::OtherContext(_) => {
      Err(PfxLoadError::Encode("unsupported PFX content info".into()))
    }
  }
}

// Convert a UTF-8 password to the PKCS#12 BMPString form (RFC 7292
// section B.1): each character is encoded as UTF-16BE, followed by a
// U+0000 terminator.
fn bmp_string(s: &str) -> Vec<u8> {
  let utf16: Vec<u16> = s.encode_utf16().collect();
  let mut bytes = Vec::with_capacity(utf16.len() * 2 + 2);
  for c in utf16 {
    bytes.extend_from_slice(&c.to_be_bytes());
  }
  bytes.extend_from_slice(&[0x00, 0x00]);
  bytes
}

// PKCS#12 password-based key derivation, generic over a Digest. Implements
// the algorithm from RFC 7292 Appendix B.2:
//   https://www.rfc-editor.org/rfc/rfc7292#appendix-B.2
//
// `v` is the hash function's block size in bytes (64 for SHA-1, SHA-224,
// SHA-256; 128 for SHA-384, SHA-512, SHA-512/224, SHA-512/256). `id` is
// the diversifier (1 = encryption key, 2 = IV, 3 = MAC key). `size` is
// the number of output bytes desired.
fn pkcs12_pbkdf<D: Digest + FixedOutputReset>(
  pass: &[u8],
  salt: &[u8],
  iterations: u64,
  id: u8,
  size: usize,
  v: usize,
) -> Vec<u8> {
  // u = hash output size in bytes.
  let u = <D as Digest>::output_size();
  // Step 1: D = id repeated v bytes.
  let d = vec![id; v];
  // Steps 2 & 3: S and P are the salt / password padded to a multiple of
  // v bytes by cyclic repetition. Empty inputs contribute an empty string.
  let s: Vec<u8> = if salt.is_empty() {
    Vec::new()
  } else {
    salt
      .iter()
      .cycle()
      .take(v * salt.len().div_ceil(v))
      .copied()
      .collect()
  };
  let p: Vec<u8> = if pass.is_empty() {
    Vec::new()
  } else {
    pass
      .iter()
      .cycle()
      .take(v * pass.len().div_ceil(v))
      .copied()
      .collect()
  };
  // Step 4: I = S || P.
  let mut i: Vec<u8> = Vec::with_capacity(s.len() + p.len());
  i.extend_from_slice(&s);
  i.extend_from_slice(&p);
  // Step 5: c = ceil(size / u).
  let c = size.div_ceil(u);
  let mut out: Vec<u8> = Vec::with_capacity(c * u);
  let mut hasher = D::new();
  for _ in 0..c {
    // Step 6a: Ai = H^iterations(D || I).
    Digest::update(&mut hasher, &d);
    Digest::update(&mut hasher, &i);
    let mut ai = hasher.finalize_reset().to_vec();
    for _ in 1..iterations {
      Digest::update(&mut hasher, &ai);
      ai = hasher.finalize_reset().to_vec();
    }
    // Step 7 (partial): A = A || Ai.
    out.extend_from_slice(&ai);
    if i.is_empty() {
      continue;
    }
    // Step 6b: B = Ai repeated cyclically to v bytes.
    let b: Vec<u8> = ai.iter().cycle().take(v).copied().collect();
    // Step 6c: treating I as v-byte blocks, set each block to
    // (block + B + 1) mod 2^(8v). Big-endian add with carry.
    for chunk in i.chunks_mut(v) {
      let mut carry: u16 = 1;
      for j in (0..v).rev() {
        let sum = chunk[j] as u16 + b[j] as u16 + carry;
        chunk[j] = (sum & 0xff) as u8;
        carry = sum >> 8;
      }
    }
  }
  // Step 8: take the first `size` bytes of A.
  out.truncate(size);
  out
}

fn pkcs12_hmac<D>(key: &[u8], data: &[u8]) -> Vec<u8>
where
  D: Digest
    + digest::core_api::CoreProxy
    + FixedOutputReset
    + digest::core_api::BlockSizeUser,
  <D as digest::core_api::CoreProxy>::Core: digest::core_api::BufferKindUser<
      BufferKind = digest::block_buffer::Eager,
    > + digest::core_api::FixedOutputCore
    + digest::HashMarker
    + Default
    + Clone,
  <<D as digest::core_api::CoreProxy>::Core as digest::core_api::BlockSizeUser>::BlockSize: digest::typenum::IsLess<digest::typenum::U256>,
  digest::typenum::Le<<<D as digest::core_api::CoreProxy>::Core as digest::core_api::BlockSizeUser>::BlockSize, digest::typenum::U256>: digest::typenum::NonZero,
{
  let mut mac = <Hmac<D> as Mac>::new_from_slice(key).unwrap();
  Mac::update(&mut mac, data);
  mac.finalize().into_bytes().to_vec()
}

// OID identifiers in components.
const OID_SHA1: &[u64] = &[1, 3, 14, 3, 2, 26];
const OID_SHA224: &[u64] = &[2, 16, 840, 1, 101, 3, 4, 2, 4];
const OID_SHA256: &[u64] = &[2, 16, 840, 1, 101, 3, 4, 2, 1];
const OID_SHA384: &[u64] = &[2, 16, 840, 1, 101, 3, 4, 2, 2];
const OID_SHA512: &[u64] = &[2, 16, 840, 1, 101, 3, 4, 2, 3];
const OID_SHA512_224: &[u64] = &[2, 16, 840, 1, 101, 3, 4, 2, 5];
const OID_SHA512_256: &[u64] = &[2, 16, 840, 1, 101, 3, 4, 2, 6];

enum Pkcs12MacAlgorithm {
  Sha1,
  Sha224,
  Sha256,
  Sha384,
  Sha512,
  Sha512_224,
  Sha512_256,
}

fn pkcs12_mac_algorithm(
  algorithm: &Pkcs12AlgorithmIdentifier,
) -> Option<Pkcs12MacAlgorithm> {
  let oid = match algorithm {
    Pkcs12AlgorithmIdentifier::Sha1 => return Some(Pkcs12MacAlgorithm::Sha1),
    Pkcs12AlgorithmIdentifier::OtherAlg(other) => {
      other.algorithm_type.components().as_slice()
    }
    _ => return None,
  };
  if oid == OID_SHA1 {
    Some(Pkcs12MacAlgorithm::Sha1)
  } else if oid == OID_SHA224 {
    Some(Pkcs12MacAlgorithm::Sha224)
  } else if oid == OID_SHA256 {
    Some(Pkcs12MacAlgorithm::Sha256)
  } else if oid == OID_SHA384 {
    Some(Pkcs12MacAlgorithm::Sha384)
  } else if oid == OID_SHA512 {
    Some(Pkcs12MacAlgorithm::Sha512)
  } else if oid == OID_SHA512_224 {
    Some(Pkcs12MacAlgorithm::Sha512_224)
  } else if oid == OID_SHA512_256 {
    Some(Pkcs12MacAlgorithm::Sha512_256)
  } else {
    None
  }
}

fn verify_pkcs12_mac(
  algorithm: &Pkcs12AlgorithmIdentifier,
  expected: &[u8],
  salt: &[u8],
  iterations: u64,
  data: &[u8],
  password: &[u8],
) -> Option<bool> {
  let mac_alg = pkcs12_mac_algorithm(algorithm)?;
  let computed = match mac_alg {
    Pkcs12MacAlgorithm::Sha1 => {
      let key =
        pkcs12_pbkdf::<sha1::Sha1>(password, salt, iterations, 3, 20, 64);
      pkcs12_hmac::<sha1::Sha1>(&key, data)
    }
    Pkcs12MacAlgorithm::Sha224 => {
      let key =
        pkcs12_pbkdf::<sha2::Sha224>(password, salt, iterations, 3, 28, 64);
      pkcs12_hmac::<sha2::Sha224>(&key, data)
    }
    Pkcs12MacAlgorithm::Sha256 => {
      let key =
        pkcs12_pbkdf::<sha2::Sha256>(password, salt, iterations, 3, 32, 64);
      pkcs12_hmac::<sha2::Sha256>(&key, data)
    }
    Pkcs12MacAlgorithm::Sha384 => {
      let key =
        pkcs12_pbkdf::<sha2::Sha384>(password, salt, iterations, 3, 48, 128);
      pkcs12_hmac::<sha2::Sha384>(&key, data)
    }
    Pkcs12MacAlgorithm::Sha512 => {
      let key =
        pkcs12_pbkdf::<sha2::Sha512>(password, salt, iterations, 3, 64, 128);
      pkcs12_hmac::<sha2::Sha512>(&key, data)
    }
    Pkcs12MacAlgorithm::Sha512_224 => {
      let key = pkcs12_pbkdf::<sha2::Sha512_224>(
        password, salt, iterations, 3, 28, 128,
      );
      pkcs12_hmac::<sha2::Sha512_224>(&key, data)
    }
    Pkcs12MacAlgorithm::Sha512_256 => {
      let key = pkcs12_pbkdf::<sha2::Sha512_256>(
        password, salt, iterations, 3, 32, 128,
      );
      pkcs12_hmac::<sha2::Sha512_256>(&key, data)
    }
  };
  use subtle::ConstantTimeEq;
  Some(bool::from(computed.ct_eq(expected)))
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CrlValidationError {
  #[class(generic)]
  #[error("Failed to parse CRL")]
  ParseFailed,
}

#[op2(fast)]
pub fn op_node_validate_crl(
  #[buffer] crl: &[u8],
) -> Result<(), CrlValidationError> {
  if crl.starts_with(b"-----") {
    match x509_parser::pem::parse_x509_pem(crl) {
      Ok((_, pem)) => {
        x509_parser::parse_x509_crl(&pem.contents)
          .map_err(|_| CrlValidationError::ParseFailed)?;
      }
      Err(_) => return Err(CrlValidationError::ParseFailed),
    }
  } else {
    x509_parser::parse_x509_crl(crl)
      .map_err(|_| CrlValidationError::ParseFailed)?;
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  // Cross-check our generic implementation of the PKCS#12 PBKDF
  // (RFC 7292 Appendix B.2) against test vectors computed with an
  // independent reference implementation.

  // SHA-1 vector lifted from the `p12` crate's own test suite, which
  // matches output produced by Bouncy Castle.
  #[test]
  fn pkcs12_pbkdf_sha1() {
    let pass = bmp_string("");
    assert_eq!(pass, vec![0, 0]);
    let salt: [u8; 8] = [0x9a, 0xf4, 0x70, 0x29, 0x58, 0xa8, 0xe9, 0x5c];
    let got = pkcs12_pbkdf::<sha1::Sha1>(&pass, &salt, 2048, 1, 24, 64);
    let expected: [u8; 24] = [
      0xc2, 0x29, 0x4a, 0xa6, 0xd0, 0x29, 0x30, 0xeb, 0x5c, 0xe9, 0xc3, 0x29,
      0xec, 0xcb, 0x9a, 0xee, 0x1c, 0xb1, 0x36, 0xba, 0xea, 0x74, 0x65, 0x57,
    ];
    assert_eq!(got, expected);
  }

  // SHA-256 / SHA-384 / SHA-512 vectors were generated by porting the
  // RFC 7292 B.2 pseudocode to Python and feeding the same inputs.
  #[test]
  fn pkcs12_pbkdf_sha256() {
    let pass = bmp_string("secret");
    let salt: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
    let got = pkcs12_pbkdf::<sha2::Sha256>(&pass, &salt, 1000, 3, 32, 64);
    let expected: [u8; 32] = [
      0x7f, 0xe1, 0x91, 0x75, 0x7d, 0xca, 0xf1, 0xed, 0x6a, 0x29, 0x77, 0xb9,
      0xb9, 0x15, 0x3f, 0x60, 0x82, 0xaf, 0x0b, 0xda, 0xfd, 0x09, 0x35, 0x2d,
      0xcd, 0xaa, 0x96, 0x7f, 0x57, 0x17, 0x82, 0xb0,
    ];
    assert_eq!(got, expected);
  }

  #[test]
  fn pkcs12_pbkdf_sha384() {
    let pass = bmp_string("secret");
    let salt: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
    let got = pkcs12_pbkdf::<sha2::Sha384>(&pass, &salt, 1000, 3, 48, 128);
    let expected: [u8; 48] = [
      0x6a, 0x59, 0x71, 0x72, 0x05, 0x22, 0x76, 0x31, 0x21, 0xf4, 0x9a, 0x1d,
      0x5c, 0x04, 0x10, 0xa1, 0xdb, 0x42, 0x0b, 0xe4, 0x96, 0x6d, 0xc5, 0x2f,
      0x51, 0x91, 0x9d, 0x91, 0x15, 0x2d, 0x60, 0x2d, 0x31, 0x1c, 0x4c, 0xb0,
      0x8d, 0x99, 0x83, 0xad, 0xaf, 0x68, 0xff, 0x5d, 0xdd, 0x69, 0x0b, 0x87,
    ];
    assert_eq!(got, expected);
  }

  #[test]
  fn pkcs12_pbkdf_sha512() {
    let pass = bmp_string("secret");
    let salt: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
    let got = pkcs12_pbkdf::<sha2::Sha512>(&pass, &salt, 1000, 3, 64, 128);
    let expected: [u8; 64] = [
      0x3e, 0x5c, 0x5d, 0xb5, 0xf5, 0xe3, 0xd0, 0xc2, 0x23, 0x2e, 0xbf, 0xbb,
      0x86, 0x08, 0x23, 0x7a, 0x2b, 0x0a, 0xf6, 0x00, 0x30, 0xe6, 0xa0, 0x08,
      0x2a, 0xbe, 0x7c, 0x19, 0x3f, 0x52, 0x5e, 0x98, 0x97, 0x6f, 0xb2, 0xbb,
      0x1c, 0x88, 0xc3, 0xc6, 0xf8, 0xb5, 0x64, 0x91, 0x74, 0x3f, 0xc5, 0x04,
      0xfb, 0x3d, 0xd9, 0x10, 0xf4, 0xf9, 0x5e, 0xf6, 0x99, 0xc2, 0x48, 0x15,
      0xf1, 0x3e, 0xc1, 0x74,
    ];
    assert_eq!(got, expected);
  }

  #[test]
  fn bmp_string_basic() {
    assert_eq!(bmp_string(""), vec![0x00, 0x00]);
    // "Beavis" — every code unit becomes two big-endian bytes, then a
    // U+0000 terminator.
    assert_eq!(
      bmp_string("Beavis"),
      vec![
        0x00, 0x42, 0x00, 0x65, 0x00, 0x61, 0x00, 0x76, 0x00, 0x69, 0x00, 0x73,
        0x00, 0x00,
      ],
    );
  }
}
