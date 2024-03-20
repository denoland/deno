use spki::der::asn1;
use spki::der::Decode;
use spki::der::EncodePem;
use spki::der::Sequence;
use spki::SubjectPublicKeyInfoRef;

use deno_core::error::AnyError;
use deno_core::op2;

fn export_challenge(spkac: &[u8]) -> Result<Vec<u8>, AnyError> {
  // base64 decode
  let spkac = base64::decode(spkac)?;
  let spkac = SignedPublicKeyAndChallenge::from_der(&spkac)?;

  // extract challenge
  let challenge = spkac.public_key_and_challenge.challenge;
  Ok(challenge.as_bytes().to_vec())
}

fn verify_spkac(spkac: &[u8]) -> Result<bool, AnyError> {
  let spkac = base64::decode(spkac)?;
  let spkac = SignedPublicKeyAndChallenge::from_der(&spkac)?;

  // extract public key
  let spki = spkac.public_key_and_challenge.spki;
  let public_key = spki.subject_public_key;

  Ok(false)
}

fn export_public_key(spkac: &[u8]) -> Result<Vec<u8>, AnyError> {
  let spkac = base64::decode(spkac)?;
  let spkac = SignedPublicKeyAndChallenge::from_der(&spkac)?;

  // extract public key
  let spki = spkac.public_key_and_challenge.spki;

  let pem = spki.to_pem(Default::default())?;
  Ok(pem.as_bytes().to_vec())
}

// PublicKeyAndChallenge ::= SEQUENCE {
//     spki SubjectPublicKeyInfo,
//     challenge IA5STRING
// }
//
// SignedPublicKeyAndChallenge ::= SEQUENCE {
//     publicKeyAndChallenge PublicKeyAndChallenge,
//     signatureAlgorithm AlgorithmIdentifier,
//     signature BIT STRING
// }
#[derive(Sequence)]
struct SignedPublicKeyAndChallenge<'a> {
  public_key_and_challenge: PublicKeyAndChallenge<'a>,
  signature_algorithm: AlgorithmIdentifier<'a>,
  signature: asn1::BitStringRef<'a>,
}

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

#[op2]
#[buffer]
pub fn op_node_export_challenge(
  #[buffer] spkac: &[u8],
) -> Result<Vec<u8>, AnyError> {
  export_challenge(spkac)
}

#[op2(fast)]
pub fn op_node_verify_spkac(#[buffer] spkac: &[u8]) -> Result<bool, AnyError> {
  verify_spkac(spkac)
}

#[op2]
#[buffer]
pub fn op_node_export_public_key(
  #[buffer] spkac: &[u8],
) -> Result<Vec<u8>, AnyError> {
  export_public_key(spkac)
}
