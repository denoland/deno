use deno_core::op;
use rand::rngs::OsRng;
use rand::RngCore;
use ring::signature::Ed25519KeyPair;
use ring::signature::KeyPair;

#[op(fast)]
pub fn op_generate_ed25519_keypair(pkey: &mut [u8], pubkey: &mut [u8]) -> bool {
  let mut rng = OsRng;
  rng.fill_bytes(pkey);

  let pair = match Ed25519KeyPair::from_seed_unchecked(pkey) {
    Ok(p) => p,
    Err(_) => return false,
  };
  pubkey.copy_from_slice(pair.public_key().as_ref());
  true
}

#[op(fast)]
pub fn op_sign_ed25519(key: &[u8], data: &[u8], signature: &mut [u8]) -> bool {
  let pair = match Ed25519KeyPair::from_seed_unchecked(key) {
    Ok(p) => p,
    Err(_) => return false,
  };
  signature.copy_from_slice(pair.sign(data).as_ref());
  true
}

#[op(fast)]
pub fn op_verify_ed25519(pubkey: &[u8], data: &[u8], signature: &[u8]) -> bool {
  ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, pubkey)
    .verify(data, signature)
    .is_ok()
}
