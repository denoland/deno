use spki::der::asn1;
use spki::der::Decode;
use spki::der::EncodePem;
use spki::der::Sequence;
use spki::SubjectPublicKeyInfoRef;

fn export_challenge(spkac: &[u8]) -> Vec<u8> {
  // base64 decode
  let spkac = base64::decode(spkac).unwrap();
  let spkac = SignedPublicKeyAndChallenge::from_der(&spkac).unwrap();

  // extract challenge
  let challenge = spkac.public_key_and_challenge.challenge;
  challenge.as_bytes().to_vec()
}

fn verify_spkac(spkac: &[u8]) -> bool {
  false
}

fn export_public_key(spkac: &[u8]) -> String {
  let spkac = base64::decode(spkac).unwrap();
  let spkac = SignedPublicKeyAndChallenge::from_der(&spkac).unwrap();

  // extract public key
  let spki = spkac.public_key_and_challenge.spki;

  // export PEM.
  let pem = spki.to_pem(Default::default()).unwrap();
  pem
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
