use curve25519_dalek::montgomery::MontgomeryPoint;
use deno_core::op;
use elliptic_curve::subtle::ConstantTimeEq;
use rand::rngs::OsRng;
use rand::RngCore;

#[op(fast)]
pub fn op_generate_x25519_keypair(pkey: &mut [u8], pubkey: &mut [u8]) {
  // u-coordinate of the base point.
  const X25519_BASEPOINT_BYTES: [u8; 32] = [
    9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
  ];
  let mut rng = OsRng;
  rng.fill_bytes(pkey);
  // https://www.rfc-editor.org/rfc/rfc7748#section-6.1
  // pubkey = x25519(a, 9) which is constant-time Montgomery ladder.
  //   https://eprint.iacr.org/2014/140.pdf page 4
  //   https://eprint.iacr.org/2017/212.pdf algorithm 8
  // pubkey is in LE order.
  let pkey: [u8; 32] = pkey.try_into().expect("Expected byteLength 32");
  pubkey.copy_from_slice(&x25519_dalek::x25519(pkey, X25519_BASEPOINT_BYTES));
}

const MONTGOMERY_IDENTITY: MontgomeryPoint = MontgomeryPoint([0; 32]);

#[op(fast)]
pub fn op_derive_bits_x25519(k: &[u8], u: &[u8], secret: &mut [u8]) -> bool {
  let k: [u8; 32] = k.try_into().expect("Expected byteLength 32");
  let u: [u8; 32] = u.try_into().expect("Expected byteLength 32");
  let sh_sec = x25519_dalek::x25519(k, u);
  let point = MontgomeryPoint(sh_sec);
  if point.ct_eq(&MONTGOMERY_IDENTITY).unwrap_u8() == 1 {
    return false;
  }
  secret.copy_from_slice(&sh_sec);
  true
}
