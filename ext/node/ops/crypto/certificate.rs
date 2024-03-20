use der::asn1;

fn verify_spkac(
  spkac: &[u8]
) -> bool {

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
struct SignedPublicKeyAndChallenge<'a> {
}



struct PublicKeyAndChallenge<'a> {
}
