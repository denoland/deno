// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::error::generic_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::unsync::spawn_blocking;
use deno_core::JsBuffer;
use deno_core::OpState;
use deno_core::StringOrBuffer;
use deno_core::ToJsBuffer;
use elliptic_curve::sec1::ToEncodedPoint;
use hkdf::Hkdf;
use keys::AsymmetricPrivateKey;
use keys::AsymmetricPublicKey;
use keys::EcPrivateKey;
use keys::EcPublicKey;
use keys::KeyObjectHandle;
use num_bigint::BigInt;
use num_bigint_dig::BigUint;
use rand::distributions::Distribution;
use rand::distributions::Uniform;
use rand::Rng;
use ring::signature::Ed25519KeyPair;
use std::future::Future;
use std::rc::Rc;

use p224::NistP224;
use p256::NistP256;
use p384::NistP384;
use rsa::pkcs8::DecodePrivateKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::Oaep;
use rsa::Pkcs1v15Encrypt;
use rsa::RsaPrivateKey;
use rsa::RsaPublicKey;

mod cipher;
mod dh;
mod digest;
pub mod keys;
mod md5_sha1;
mod pkcs3;
mod primes;
mod sign;
pub mod x509;

use self::digest::match_fixed_digest_with_eager_block_buffer;

#[op2(fast)]
pub fn op_node_check_prime(
  #[bigint] num: i64,
  #[number] checks: usize,
) -> bool {
  primes::is_probably_prime(&BigInt::from(num), checks)
}

#[op2]
pub fn op_node_check_prime_bytes(
  #[anybuffer] bytes: &[u8],
  #[number] checks: usize,
) -> Result<bool, AnyError> {
  let candidate = BigInt::from_bytes_be(num_bigint::Sign::Plus, bytes);
  Ok(primes::is_probably_prime(&candidate, checks))
}

#[op2(async)]
pub async fn op_node_check_prime_async(
  #[bigint] num: i64,
  #[number] checks: usize,
) -> Result<bool, AnyError> {
  // TODO(@littledivy): use rayon for CPU-bound tasks
  Ok(
    spawn_blocking(move || {
      primes::is_probably_prime(&BigInt::from(num), checks)
    })
    .await?,
  )
}

#[op2(async)]
pub fn op_node_check_prime_bytes_async(
  #[anybuffer] bytes: &[u8],
  #[number] checks: usize,
) -> Result<impl Future<Output = Result<bool, AnyError>>, AnyError> {
  let candidate = BigInt::from_bytes_be(num_bigint::Sign::Plus, bytes);
  // TODO(@littledivy): use rayon for CPU-bound tasks
  Ok(async move {
    Ok(
      spawn_blocking(move || primes::is_probably_prime(&candidate, checks))
        .await?,
    )
  })
}

#[op2]
#[cppgc]
pub fn op_node_create_hash(
  #[string] algorithm: &str,
  output_length: Option<u32>,
) -> Result<digest::Hasher, AnyError> {
  digest::Hasher::new(algorithm, output_length.map(|l| l as usize))
}

#[op2]
#[serde]
pub fn op_node_get_hashes() -> Vec<&'static str> {
  digest::Hash::get_hashes()
}

#[op2(fast)]
pub fn op_node_hash_update(
  #[cppgc] hasher: &digest::Hasher,
  #[buffer] data: &[u8],
) -> bool {
  hasher.update(data)
}

#[op2(fast)]
pub fn op_node_hash_update_str(
  #[cppgc] hasher: &digest::Hasher,
  #[string] data: &str,
) -> bool {
  hasher.update(data.as_bytes())
}

#[op2]
#[buffer]
pub fn op_node_hash_digest(
  #[cppgc] hasher: &digest::Hasher,
) -> Option<Box<[u8]>> {
  hasher.digest()
}

#[op2]
#[string]
pub fn op_node_hash_digest_hex(
  #[cppgc] hasher: &digest::Hasher,
) -> Option<String> {
  let digest = hasher.digest()?;
  Some(faster_hex::hex_string(&digest))
}

#[op2]
#[cppgc]
pub fn op_node_hash_clone(
  #[cppgc] hasher: &digest::Hasher,
  output_length: Option<u32>,
) -> Result<Option<digest::Hasher>, AnyError> {
  hasher.clone_inner(output_length.map(|l| l as usize))
}

#[op2]
#[serde]
pub fn op_node_private_encrypt(
  #[serde] key: StringOrBuffer,
  #[serde] msg: StringOrBuffer,
  #[smi] padding: u32,
) -> Result<ToJsBuffer, AnyError> {
  let key = RsaPrivateKey::from_pkcs8_pem((&key).try_into()?)?;

  let mut rng = rand::thread_rng();
  match padding {
    1 => Ok(
      key
        .as_ref()
        .encrypt(&mut rng, Pkcs1v15Encrypt, &msg)?
        .into(),
    ),
    4 => Ok(
      key
        .as_ref()
        .encrypt(&mut rng, Oaep::new::<sha1::Sha1>(), &msg)?
        .into(),
    ),
    _ => Err(type_error("Unknown padding")),
  }
}

#[op2]
#[serde]
pub fn op_node_private_decrypt(
  #[serde] key: StringOrBuffer,
  #[serde] msg: StringOrBuffer,
  #[smi] padding: u32,
) -> Result<ToJsBuffer, AnyError> {
  let key = RsaPrivateKey::from_pkcs8_pem((&key).try_into()?)?;

  match padding {
    1 => Ok(key.decrypt(Pkcs1v15Encrypt, &msg)?.into()),
    4 => Ok(key.decrypt(Oaep::new::<sha1::Sha1>(), &msg)?.into()),
    _ => Err(type_error("Unknown padding")),
  }
}

#[op2]
#[serde]
pub fn op_node_public_encrypt(
  #[serde] key: StringOrBuffer,
  #[serde] msg: StringOrBuffer,
  #[smi] padding: u32,
) -> Result<ToJsBuffer, AnyError> {
  let key = RsaPublicKey::from_public_key_pem((&key).try_into()?)?;

  let mut rng = rand::thread_rng();
  match padding {
    1 => Ok(key.encrypt(&mut rng, Pkcs1v15Encrypt, &msg)?.into()),
    4 => Ok(
      key
        .encrypt(&mut rng, Oaep::new::<sha1::Sha1>(), &msg)?
        .into(),
    ),
    _ => Err(type_error("Unknown padding")),
  }
}

#[op2(fast)]
#[smi]
pub fn op_node_create_cipheriv(
  state: &mut OpState,
  #[string] algorithm: &str,
  #[buffer] key: &[u8],
  #[buffer] iv: &[u8],
) -> Result<u32, AnyError> {
  let context = cipher::CipherContext::new(algorithm, key, iv)?;
  Ok(state.resource_table.add(context))
}

#[op2(fast)]
pub fn op_node_cipheriv_set_aad(
  state: &mut OpState,
  #[smi] rid: u32,
  #[buffer] aad: &[u8],
) -> bool {
  let context = match state.resource_table.get::<cipher::CipherContext>(rid) {
    Ok(context) => context,
    Err(_) => return false,
  };
  context.set_aad(aad);
  true
}

#[op2(fast)]
pub fn op_node_cipheriv_encrypt(
  state: &mut OpState,
  #[smi] rid: u32,
  #[buffer] input: &[u8],
  #[buffer] output: &mut [u8],
) -> bool {
  let context = match state.resource_table.get::<cipher::CipherContext>(rid) {
    Ok(context) => context,
    Err(_) => return false,
  };
  context.encrypt(input, output);
  true
}

#[op2]
#[serde]
pub fn op_node_cipheriv_final(
  state: &mut OpState,
  #[smi] rid: u32,
  auto_pad: bool,
  #[buffer] input: &[u8],
  #[anybuffer] output: &mut [u8],
) -> Result<Option<Vec<u8>>, AnyError> {
  let context = state.resource_table.take::<cipher::CipherContext>(rid)?;
  let context = Rc::try_unwrap(context)
    .map_err(|_| type_error("Cipher context is already in use"))?;
  context.r#final(auto_pad, input, output)
}

#[op2]
#[buffer]
pub fn op_node_cipheriv_take(
  state: &mut OpState,
  #[smi] rid: u32,
) -> Result<Option<Vec<u8>>, AnyError> {
  let context = state.resource_table.take::<cipher::CipherContext>(rid)?;
  let context = Rc::try_unwrap(context)
    .map_err(|_| type_error("Cipher context is already in use"))?;
  Ok(context.take_tag())
}

#[op2(fast)]
#[smi]
pub fn op_node_create_decipheriv(
  state: &mut OpState,
  #[string] algorithm: &str,
  #[buffer] key: &[u8],
  #[buffer] iv: &[u8],
) -> Result<u32, AnyError> {
  let context = cipher::DecipherContext::new(algorithm, key, iv)?;
  Ok(state.resource_table.add(context))
}

#[op2(fast)]
pub fn op_node_decipheriv_set_aad(
  state: &mut OpState,
  #[smi] rid: u32,
  #[buffer] aad: &[u8],
) -> bool {
  let context = match state.resource_table.get::<cipher::DecipherContext>(rid) {
    Ok(context) => context,
    Err(_) => return false,
  };
  context.set_aad(aad);
  true
}

#[op2(fast)]
pub fn op_node_decipheriv_decrypt(
  state: &mut OpState,
  #[smi] rid: u32,
  #[buffer] input: &[u8],
  #[buffer] output: &mut [u8],
) -> bool {
  let context = match state.resource_table.get::<cipher::DecipherContext>(rid) {
    Ok(context) => context,
    Err(_) => return false,
  };
  context.decrypt(input, output);
  true
}

#[op2(fast)]
pub fn op_node_decipheriv_take(
  state: &mut OpState,
  #[smi] rid: u32,
) -> Result<(), AnyError> {
  let context = state.resource_table.take::<cipher::DecipherContext>(rid)?;
  Rc::try_unwrap(context)
    .map_err(|_| type_error("Cipher context is already in use"))?;
  Ok(())
}

#[op2]
pub fn op_node_decipheriv_final(
  state: &mut OpState,
  #[smi] rid: u32,
  auto_pad: bool,
  #[buffer] input: &[u8],
  #[anybuffer] output: &mut [u8],
  #[buffer] auth_tag: &[u8],
) -> Result<(), AnyError> {
  let context = state.resource_table.take::<cipher::DecipherContext>(rid)?;
  let context = Rc::try_unwrap(context)
    .map_err(|_| type_error("Cipher context is already in use"))?;
  context.r#final(auto_pad, input, output, auth_tag)
}

#[op2]
#[buffer]
pub fn op_node_sign(
  #[cppgc] handle: &KeyObjectHandle,
  #[buffer] digest: &[u8],
  #[string] digest_type: &str,
  #[smi] pss_salt_length: Option<u32>,
  #[smi] dsa_signature_encoding: u32,
) -> Result<Box<[u8]>, AnyError> {
  handle.sign_prehashed(
    digest_type,
    digest,
    pss_salt_length,
    dsa_signature_encoding,
  )
}

#[op2]
pub fn op_node_verify(
  #[cppgc] handle: &KeyObjectHandle,
  #[buffer] digest: &[u8],
  #[string] digest_type: &str,
  #[buffer] signature: &[u8],
  #[smi] pss_salt_length: Option<u32>,
  #[smi] dsa_signature_encoding: u32,
) -> Result<bool, AnyError> {
  handle.verify_prehashed(
    digest_type,
    digest,
    signature,
    pss_salt_length,
    dsa_signature_encoding,
  )
}

fn pbkdf2_sync(
  password: &[u8],
  salt: &[u8],
  iterations: u32,
  algorithm_name: &str,
  derived_key: &mut [u8],
) -> Result<(), AnyError> {
  match_fixed_digest_with_eager_block_buffer!(
    algorithm_name,
    fn <D>() {
      pbkdf2::pbkdf2_hmac::<D>(password, salt, iterations, derived_key);
      Ok(())
    },
    _ => {
      Err(type_error(format!(
        "unsupported digest: {}",
        algorithm_name
      )))
    }
  )
}

#[op2]
pub fn op_node_pbkdf2(
  #[serde] password: StringOrBuffer,
  #[serde] salt: StringOrBuffer,
  #[smi] iterations: u32,
  #[string] digest: &str,
  #[buffer] derived_key: &mut [u8],
) -> bool {
  pbkdf2_sync(&password, &salt, iterations, digest, derived_key).is_ok()
}

#[op2(async)]
#[serde]
pub async fn op_node_pbkdf2_async(
  #[serde] password: StringOrBuffer,
  #[serde] salt: StringOrBuffer,
  #[smi] iterations: u32,
  #[string] digest: String,
  #[number] keylen: usize,
) -> Result<ToJsBuffer, AnyError> {
  spawn_blocking(move || {
    let mut derived_key = vec![0; keylen];
    pbkdf2_sync(&password, &salt, iterations, &digest, &mut derived_key)
      .map(|_| derived_key.into())
  })
  .await?
}

#[op2(fast)]
pub fn op_node_fill_random(#[buffer] buf: &mut [u8]) {
  rand::thread_rng().fill(buf);
}

#[op2(async)]
#[serde]
pub async fn op_node_fill_random_async(#[smi] len: i32) -> ToJsBuffer {
  spawn_blocking(move || {
    let mut buf = vec![0u8; len as usize];
    rand::thread_rng().fill(&mut buf[..]);
    buf.into()
  })
  .await
  .unwrap()
}

fn hkdf_sync(
  digest_algorithm: &str,
  handle: &KeyObjectHandle,
  salt: &[u8],
  info: &[u8],
  okm: &mut [u8],
) -> Result<(), AnyError> {
  let Some(ikm) = handle.as_secret_key() else {
    return Err(type_error("expected secret key"));
  };

  match_fixed_digest_with_eager_block_buffer!(
    digest_algorithm,
    fn <D>() {
      let hk = Hkdf::<D>::new(Some(salt), ikm);
      hk.expand(info, okm)
        .map_err(|_| type_error("HKDF-Expand failed"))
    },
    _ => {
      Err(type_error(format!("Unsupported digest: {}", digest_algorithm)))
    }
  )
}

#[op2(fast)]
pub fn op_node_hkdf(
  #[string] digest_algorithm: &str,
  #[cppgc] handle: &KeyObjectHandle,
  #[buffer] salt: &[u8],
  #[buffer] info: &[u8],
  #[buffer] okm: &mut [u8],
) -> Result<(), AnyError> {
  hkdf_sync(digest_algorithm, handle, salt, info, okm)
}

#[op2(async)]
#[serde]
pub async fn op_node_hkdf_async(
  #[string] digest_algorithm: String,
  #[cppgc] handle: &KeyObjectHandle,
  #[buffer] salt: JsBuffer,
  #[buffer] info: JsBuffer,
  #[number] okm_len: usize,
) -> Result<ToJsBuffer, AnyError> {
  let handle = handle.clone();
  spawn_blocking(move || {
    let mut okm = vec![0u8; okm_len];
    hkdf_sync(&digest_algorithm, &handle, &salt, &info, &mut okm)?;
    Ok(okm.into())
  })
  .await?
}

#[op2]
#[serde]
pub fn op_node_dh_compute_secret(
  #[buffer] prime: JsBuffer,
  #[buffer] private_key: JsBuffer,
  #[buffer] their_public_key: JsBuffer,
) -> Result<ToJsBuffer, AnyError> {
  let pubkey: BigUint = BigUint::from_bytes_be(their_public_key.as_ref());
  let privkey: BigUint = BigUint::from_bytes_be(private_key.as_ref());
  let primei: BigUint = BigUint::from_bytes_be(prime.as_ref());
  let shared_secret: BigUint = pubkey.modpow(&privkey, &primei);

  Ok(shared_secret.to_bytes_be().into())
}

#[op2(fast)]
#[smi]
pub fn op_node_random_int(
  #[smi] min: i32,
  #[smi] max: i32,
) -> Result<i32, AnyError> {
  let mut rng = rand::thread_rng();
  // Uniform distribution is required to avoid Modulo Bias
  // https://en.wikipedia.org/wiki/Fisher–Yates_shuffle#Modulo_bias
  let dist = Uniform::from(min..max);

  Ok(dist.sample(&mut rng))
}

#[allow(clippy::too_many_arguments)]
fn scrypt(
  password: StringOrBuffer,
  salt: StringOrBuffer,
  keylen: u32,
  cost: u32,
  block_size: u32,
  parallelization: u32,
  _maxmem: u32,
  output_buffer: &mut [u8],
) -> Result<(), AnyError> {
  // Construct Params
  let params = scrypt::Params::new(
    cost as u8,
    block_size,
    parallelization,
    keylen as usize,
  )
  .unwrap();

  // Call into scrypt
  let res = scrypt::scrypt(&password, &salt, &params, output_buffer);
  if res.is_ok() {
    Ok(())
  } else {
    // TODO(lev): key derivation failed, so what?
    Err(generic_error("scrypt key derivation failed"))
  }
}

#[allow(clippy::too_many_arguments)]
#[op2]
pub fn op_node_scrypt_sync(
  #[serde] password: StringOrBuffer,
  #[serde] salt: StringOrBuffer,
  #[smi] keylen: u32,
  #[smi] cost: u32,
  #[smi] block_size: u32,
  #[smi] parallelization: u32,
  #[smi] maxmem: u32,
  #[anybuffer] output_buffer: &mut [u8],
) -> Result<(), AnyError> {
  scrypt(
    password,
    salt,
    keylen,
    cost,
    block_size,
    parallelization,
    maxmem,
    output_buffer,
  )
}

#[op2(async)]
#[serde]
pub async fn op_node_scrypt_async(
  #[serde] password: StringOrBuffer,
  #[serde] salt: StringOrBuffer,
  #[smi] keylen: u32,
  #[smi] cost: u32,
  #[smi] block_size: u32,
  #[smi] parallelization: u32,
  #[smi] maxmem: u32,
) -> Result<ToJsBuffer, AnyError> {
  spawn_blocking(move || {
    let mut output_buffer = vec![0u8; keylen as usize];
    let res = scrypt(
      password,
      salt,
      keylen,
      cost,
      block_size,
      parallelization,
      maxmem,
      &mut output_buffer,
    );

    if res.is_ok() {
      Ok(output_buffer.into())
    } else {
      // TODO(lev): rethrow the error?
      Err(generic_error("scrypt failure"))
    }
  })
  .await?
}

#[op2]
#[buffer]
pub fn op_node_ecdh_encode_pubkey(
  #[string] curve: &str,
  #[buffer] pubkey: &[u8],
  compress: bool,
) -> Result<Vec<u8>, AnyError> {
  use elliptic_curve::sec1::FromEncodedPoint;

  match curve {
    "secp256k1" => {
      let pubkey =
        elliptic_curve::PublicKey::<k256::Secp256k1>::from_encoded_point(
          &elliptic_curve::sec1::EncodedPoint::<k256::Secp256k1>::from_bytes(
            pubkey,
          )?,
        );
      // CtOption does not expose its variants.
      if pubkey.is_none().into() {
        return Err(type_error("Invalid public key"));
      }

      let pubkey = pubkey.unwrap();

      Ok(pubkey.to_encoded_point(compress).as_ref().to_vec())
    }
    "prime256v1" | "secp256r1" => {
      let pubkey = elliptic_curve::PublicKey::<NistP256>::from_encoded_point(
        &elliptic_curve::sec1::EncodedPoint::<NistP256>::from_bytes(pubkey)?,
      );
      // CtOption does not expose its variants.
      if pubkey.is_none().into() {
        return Err(type_error("Invalid public key"));
      }

      let pubkey = pubkey.unwrap();

      Ok(pubkey.to_encoded_point(compress).as_ref().to_vec())
    }
    "secp384r1" => {
      let pubkey = elliptic_curve::PublicKey::<NistP384>::from_encoded_point(
        &elliptic_curve::sec1::EncodedPoint::<NistP384>::from_bytes(pubkey)?,
      );
      // CtOption does not expose its variants.
      if pubkey.is_none().into() {
        return Err(type_error("Invalid public key"));
      }

      let pubkey = pubkey.unwrap();

      Ok(pubkey.to_encoded_point(compress).as_ref().to_vec())
    }
    "secp224r1" => {
      let pubkey = elliptic_curve::PublicKey::<NistP224>::from_encoded_point(
        &elliptic_curve::sec1::EncodedPoint::<NistP224>::from_bytes(pubkey)?,
      );
      // CtOption does not expose its variants.
      if pubkey.is_none().into() {
        return Err(type_error("Invalid public key"));
      }

      let pubkey = pubkey.unwrap();

      Ok(pubkey.to_encoded_point(compress).as_ref().to_vec())
    }
    &_ => Err(type_error("Unsupported curve")),
  }
}

#[op2(fast)]
pub fn op_node_ecdh_generate_keys(
  #[string] curve: &str,
  #[buffer] pubbuf: &mut [u8],
  #[buffer] privbuf: &mut [u8],
  #[string] format: &str,
) -> Result<(), AnyError> {
  let mut rng = rand::thread_rng();
  let compress = format == "compressed";
  match curve {
    "secp256k1" => {
      let privkey =
        elliptic_curve::SecretKey::<k256::Secp256k1>::random(&mut rng);
      let pubkey = privkey.public_key();
      pubbuf.copy_from_slice(pubkey.to_encoded_point(compress).as_ref());
      privbuf.copy_from_slice(privkey.to_nonzero_scalar().to_bytes().as_ref());

      Ok(())
    }
    "prime256v1" | "secp256r1" => {
      let privkey = elliptic_curve::SecretKey::<NistP256>::random(&mut rng);
      let pubkey = privkey.public_key();
      pubbuf.copy_from_slice(pubkey.to_encoded_point(compress).as_ref());
      privbuf.copy_from_slice(privkey.to_nonzero_scalar().to_bytes().as_ref());

      Ok(())
    }
    "secp384r1" => {
      let privkey = elliptic_curve::SecretKey::<NistP384>::random(&mut rng);
      let pubkey = privkey.public_key();
      pubbuf.copy_from_slice(pubkey.to_encoded_point(compress).as_ref());
      privbuf.copy_from_slice(privkey.to_nonzero_scalar().to_bytes().as_ref());

      Ok(())
    }
    "secp224r1" => {
      let privkey = elliptic_curve::SecretKey::<NistP224>::random(&mut rng);
      let pubkey = privkey.public_key();
      pubbuf.copy_from_slice(pubkey.to_encoded_point(compress).as_ref());
      privbuf.copy_from_slice(privkey.to_nonzero_scalar().to_bytes().as_ref());

      Ok(())
    }
    &_ => Err(type_error(format!("Unsupported curve: {}", curve))),
  }
}

#[op2]
pub fn op_node_ecdh_compute_secret(
  #[string] curve: &str,
  #[buffer] this_priv: Option<JsBuffer>,
  #[buffer] their_pub: &mut [u8],
  #[buffer] secret: &mut [u8],
) -> Result<(), AnyError> {
  match curve {
    "secp256k1" => {
      let their_public_key =
        elliptic_curve::PublicKey::<k256::Secp256k1>::from_sec1_bytes(
          their_pub,
        )
        .expect("bad public key");
      let this_private_key =
        elliptic_curve::SecretKey::<k256::Secp256k1>::from_slice(
          &this_priv.expect("must supply private key"),
        )
        .expect("bad private key");
      let shared_secret = elliptic_curve::ecdh::diffie_hellman(
        this_private_key.to_nonzero_scalar(),
        their_public_key.as_affine(),
      );
      secret.copy_from_slice(shared_secret.raw_secret_bytes());

      Ok(())
    }
    "prime256v1" | "secp256r1" => {
      let their_public_key =
        elliptic_curve::PublicKey::<NistP256>::from_sec1_bytes(their_pub)
          .expect("bad public key");
      let this_private_key = elliptic_curve::SecretKey::<NistP256>::from_slice(
        &this_priv.expect("must supply private key"),
      )
      .expect("bad private key");
      let shared_secret = elliptic_curve::ecdh::diffie_hellman(
        this_private_key.to_nonzero_scalar(),
        their_public_key.as_affine(),
      );
      secret.copy_from_slice(shared_secret.raw_secret_bytes());

      Ok(())
    }
    "secp384r1" => {
      let their_public_key =
        elliptic_curve::PublicKey::<NistP384>::from_sec1_bytes(their_pub)
          .expect("bad public key");
      let this_private_key = elliptic_curve::SecretKey::<NistP384>::from_slice(
        &this_priv.expect("must supply private key"),
      )
      .expect("bad private key");
      let shared_secret = elliptic_curve::ecdh::diffie_hellman(
        this_private_key.to_nonzero_scalar(),
        their_public_key.as_affine(),
      );
      secret.copy_from_slice(shared_secret.raw_secret_bytes());

      Ok(())
    }
    "secp224r1" => {
      let their_public_key =
        elliptic_curve::PublicKey::<NistP224>::from_sec1_bytes(their_pub)
          .expect("bad public key");
      let this_private_key = elliptic_curve::SecretKey::<NistP224>::from_slice(
        &this_priv.expect("must supply private key"),
      )
      .expect("bad private key");
      let shared_secret = elliptic_curve::ecdh::diffie_hellman(
        this_private_key.to_nonzero_scalar(),
        their_public_key.as_affine(),
      );
      secret.copy_from_slice(shared_secret.raw_secret_bytes());

      Ok(())
    }
    &_ => todo!(),
  }
}

#[op2(fast)]
pub fn op_node_ecdh_compute_public_key(
  #[string] curve: &str,
  #[buffer] privkey: &[u8],
  #[buffer] pubkey: &mut [u8],
) -> Result<(), AnyError> {
  match curve {
    "secp256k1" => {
      let this_private_key =
        elliptic_curve::SecretKey::<k256::Secp256k1>::from_slice(privkey)
          .expect("bad private key");
      let public_key = this_private_key.public_key();
      pubkey.copy_from_slice(public_key.to_sec1_bytes().as_ref());

      Ok(())
    }
    "prime256v1" | "secp256r1" => {
      let this_private_key =
        elliptic_curve::SecretKey::<NistP256>::from_slice(privkey)
          .expect("bad private key");
      let public_key = this_private_key.public_key();
      pubkey.copy_from_slice(public_key.to_sec1_bytes().as_ref());
      Ok(())
    }
    "secp384r1" => {
      let this_private_key =
        elliptic_curve::SecretKey::<NistP384>::from_slice(privkey)
          .expect("bad private key");
      let public_key = this_private_key.public_key();
      pubkey.copy_from_slice(public_key.to_sec1_bytes().as_ref());
      Ok(())
    }
    "secp224r1" => {
      let this_private_key =
        elliptic_curve::SecretKey::<NistP224>::from_slice(privkey)
          .expect("bad private key");
      let public_key = this_private_key.public_key();
      pubkey.copy_from_slice(public_key.to_sec1_bytes().as_ref());
      Ok(())
    }
    &_ => todo!(),
  }
}

#[inline]
fn gen_prime(size: usize) -> ToJsBuffer {
  primes::Prime::generate(size).0.to_bytes_be().into()
}

#[op2]
#[serde]
pub fn op_node_gen_prime(#[number] size: usize) -> ToJsBuffer {
  gen_prime(size)
}

#[op2(async)]
#[serde]
pub async fn op_node_gen_prime_async(
  #[number] size: usize,
) -> Result<ToJsBuffer, AnyError> {
  Ok(spawn_blocking(move || gen_prime(size)).await?)
}

#[op2]
#[buffer]
pub fn op_node_diffie_hellman(
  #[cppgc] private: &KeyObjectHandle,
  #[cppgc] public: &KeyObjectHandle,
) -> Result<Box<[u8]>, AnyError> {
  let private = private
    .as_private_key()
    .ok_or_else(|| type_error("Expected private key"))?;
  let public = public
    .as_public_key()
    .ok_or_else(|| type_error("Expected public key"))?;

  let res = match (private, &*public) {
    (
      AsymmetricPrivateKey::Ec(EcPrivateKey::P224(private)),
      AsymmetricPublicKey::Ec(EcPublicKey::P224(public)),
    ) => p224::ecdh::diffie_hellman(
      private.to_nonzero_scalar(),
      public.as_affine(),
    )
    .raw_secret_bytes()
    .to_vec()
    .into_boxed_slice(),
    (
      AsymmetricPrivateKey::Ec(EcPrivateKey::P256(private)),
      AsymmetricPublicKey::Ec(EcPublicKey::P256(public)),
    ) => p256::ecdh::diffie_hellman(
      private.to_nonzero_scalar(),
      public.as_affine(),
    )
    .raw_secret_bytes()
    .to_vec()
    .into_boxed_slice(),
    (
      AsymmetricPrivateKey::Ec(EcPrivateKey::P384(private)),
      AsymmetricPublicKey::Ec(EcPublicKey::P384(public)),
    ) => p384::ecdh::diffie_hellman(
      private.to_nonzero_scalar(),
      public.as_affine(),
    )
    .raw_secret_bytes()
    .to_vec()
    .into_boxed_slice(),
    (
      AsymmetricPrivateKey::X25519(private),
      AsymmetricPublicKey::X25519(public),
    ) => private
      .diffie_hellman(public)
      .to_bytes()
      .into_iter()
      .collect(),
    (AsymmetricPrivateKey::Dh(private), AsymmetricPublicKey::Dh(public)) => {
      if private.params.prime != public.params.prime
        || private.params.base != public.params.base
      {
        return Err(type_error("DH parameters mismatch"));
      }

      // OSIP - Octet-String-to-Integer primitive
      let public_key = public.key.clone().into_vec();
      let pubkey = BigUint::from_bytes_be(&public_key);

      // Exponentiation (z = y^x mod p)
      let prime = BigUint::from_bytes_be(private.params.prime.as_bytes());
      let private_key = private.key.clone().into_vec();
      let private_key = BigUint::from_bytes_be(&private_key);
      let shared_secret = pubkey.modpow(&private_key, &prime);

      shared_secret.to_bytes_be().into()
    }
    _ => {
      return Err(type_error(
        "Unsupported key type for diffie hellman, or key type  mismatch",
      ))
    }
  };

  Ok(res)
}

#[op2(fast)]
pub fn op_node_sign_ed25519(
  #[cppgc] key: &KeyObjectHandle,
  #[buffer] data: &[u8],
  #[buffer] signature: &mut [u8],
) -> Result<(), AnyError> {
  let private = key
    .as_private_key()
    .ok_or_else(|| type_error("Expected private key"))?;

  let ed25519 = match private {
    AsymmetricPrivateKey::Ed25519(private) => private,
    _ => return Err(type_error("Expected Ed25519 private key")),
  };

  let pair = Ed25519KeyPair::from_seed_unchecked(ed25519.as_bytes().as_slice())
    .map_err(|_| type_error("Invalid Ed25519 private key"))?;
  signature.copy_from_slice(pair.sign(data).as_ref());

  Ok(())
}

#[op2(fast)]
pub fn op_node_verify_ed25519(
  #[cppgc] key: &KeyObjectHandle,
  #[buffer] data: &[u8],
  #[buffer] signature: &[u8],
) -> Result<bool, AnyError> {
  let public = key
    .as_public_key()
    .ok_or_else(|| type_error("Expected public key"))?;

  let ed25519 = match &*public {
    AsymmetricPublicKey::Ed25519(public) => public,
    _ => return Err(type_error("Expected Ed25519 public key")),
  };

  let verified = ring::signature::UnparsedPublicKey::new(
    &ring::signature::ED25519,
    ed25519.as_bytes().as_slice(),
  )
  .verify(data, signature)
  .is_ok();

  Ok(verified)
}
