//! SPKAC (Signed Public Key and Challenge) is a format for public keys. It is used for CSR
//! and encodes the public key. It is usually obtained using HTML5 <keygen> element.
//!
//! No browser supports <keygen> anymore and SPKAC is now legacy so you should not use it.

use spki::der::asn1;
use spki::der::Decode;
use spki::der::Encode;
use spki::der::EncodePem;
use spki::der::Sequence;
use spki::SubjectPublicKeyInfoRef;

use super::digest::Hash;
use super::KeyObjectHandle;

use deno_core::error::AnyError;
use deno_core::op2;

/// SignedPublicKeyAndChallenge ::= SEQUENCE {
///     publicKeyAndChallenge PublicKeyAndChallenge,
///     signatureAlgorithm AlgorithmIdentifier,
///     signature BIT STRING
/// }
#[derive(Sequence)]
struct SignedPublicKeyAndChallenge<'a> {
  public_key_and_challenge: PublicKeyAndChallenge<'a>,
  signature_algorithm: AlgorithmIdentifier<'a>,
  signature: asn1::BitStringRef<'a>,
}

/// PublicKeyAndChallenge ::= SEQUENCE {
///     spki SubjectPublicKeyInfo,
///     challenge IA5STRING
/// }
#[derive(Sequence)]
struct PublicKeyAndChallenge<'a> {
  spki: SubjectPublicKeyInfoRef<'a>,
  challenge: asn1::Ia5StringRef<'a>,
}

#[derive(Sequence)]
struct AlgorithmIdentifier<'a> {
  algorithm: asn1::ObjectIdentifier,
  parameters: Option<asn1::AnyRef<'a>>,
}

struct Certificate;

impl Certificate {
  fn export_challenge(spkac: &[u8]) -> Result<Box<[u8]>, AnyError> {
    let spkac = base64::decode(spkac)?;
    let spkac = SignedPublicKeyAndChallenge::from_der(&spkac)?;

    let challenge = spkac.public_key_and_challenge.challenge;
    Ok(challenge.as_bytes().to_vec().into_boxed_slice())
  }

  fn verify_spkac(spkac: &[u8]) -> Result<bool, AnyError> {
    let spkac = base64::decode(spkac)?;
    let spkac = SignedPublicKeyAndChallenge::from_der(&spkac)?;

    let spki = spkac.public_key_and_challenge.spki;
    let spki_der = spki.to_der()?;

    let key = KeyObjectHandle::new_asymmetric_public_key_from_js(
      &spki_der, "der", "spki", None,
    )?;
    let Some(signature) = spkac.signature.as_bytes() else {
      return Ok(false);
    };

    let mut hasher = Hash::new("rsa-md5", None)?;
    hasher.update(&spki_der);
    let hash = hasher.digest_and_drop();

    key.verify_prehashed("rsa-md5", &hash, signature, None, 0)
  }

  fn export_public_key(spkac: &[u8]) -> Result<Box<[u8]>, AnyError> {
    let spkac = base64::decode(spkac)?;
    let spkac = SignedPublicKeyAndChallenge::from_der(&spkac)?;

    let spki = spkac.public_key_and_challenge.spki;

    let pem = spki.to_pem(Default::default())?;
    Ok(pem.as_bytes().to_vec().into_boxed_slice())
  }
}

#[op2]
#[buffer]
pub fn op_node_export_challenge(
  #[buffer] spkac: &[u8],
) -> Result<Box<[u8]>, AnyError> {
  Certificate::export_challenge(spkac)
}

#[op2(fast)]
pub fn op_node_verify_spkac(#[buffer] spkac: &[u8]) -> Result<bool, AnyError> {
  Certificate::verify_spkac(spkac)
}

#[op2]
#[buffer]
pub fn op_node_export_public_key(
  #[buffer] spkac: &[u8],
) -> Result<Box<[u8]>, AnyError> {
  Certificate::export_public_key(spkac)
}
