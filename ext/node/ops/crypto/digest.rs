// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use deno_core::GcResource;
use digest::Digest;
use digest::DynDigest;
use digest::ExtendableOutput;
use digest::Update;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Hasher {
  pub hash: Rc<RefCell<Option<Hash>>>,
}

impl GcResource for Hasher {}

impl Hasher {
  pub fn new(
    algorithm: &str,
    output_length: Option<usize>,
  ) -> Result<Self, AnyError> {
    let hash = Hash::new(algorithm, output_length)?;

    Ok(Self {
      hash: Rc::new(RefCell::new(Some(hash))),
    })
  }

  pub fn update(&self, data: &[u8]) -> bool {
    if let Some(hash) = self.hash.borrow_mut().as_mut() {
      hash.update(data);
      true
    } else {
      false
    }
  }

  pub fn digest(&self) -> Option<Box<[u8]>> {
    let hash = self.hash.borrow_mut().take()?;
    Some(hash.digest_and_drop())
  }

  pub fn clone_inner(
    &self,
    output_length: Option<usize>,
  ) -> Result<Option<Self>, AnyError> {
    let hash = self.hash.borrow();
    let Some(hash) = hash.as_ref() else {
      return Ok(None);
    };
    let hash = hash.clone_hash(output_length)?;
    Ok(Some(Self {
      hash: Rc::new(RefCell::new(Some(hash))),
    }))
  }
}

pub enum Hash {
  Blake2b512(Box<blake2::Blake2b512>),
  Blake2s256(Box<blake2::Blake2s256>),

  Md4(Box<md4::Md4>),
  Md5(Box<md5::Md5>),

  Ripemd160(Box<ripemd::Ripemd160>),

  Sha1(Box<sha1::Sha1>),

  Sha224(Box<sha2::Sha224>),
  Sha256(Box<sha2::Sha256>),
  Sha384(Box<sha2::Sha384>),
  Sha512(Box<sha2::Sha512>),
  Sha512_224(Box<sha2::Sha512_224>),
  Sha512_256(Box<sha2::Sha512_256>),

  Sha3_224(Box<sha3::Sha3_224>),
  Sha3_256(Box<sha3::Sha3_256>),
  Sha3_384(Box<sha3::Sha3_384>),
  Sha3_512(Box<sha3::Sha3_512>),

  Sm3(Box<sm3::Sm3>),

  Shake128(Box<sha3::Shake128>, /* output_length: */ Option<usize>),
  Shake256(Box<sha3::Shake256>, /* output_length: */ Option<usize>),
}

use Hash::*;

impl Hash {
  pub fn new(
    algorithm_name: &str,
    output_length: Option<usize>,
  ) -> Result<Self, AnyError> {
    match algorithm_name {
      "shake128" => return Ok(Shake128(Default::default(), output_length)),
      "shake256" => return Ok(Shake256(Default::default(), output_length)),
      _ => {}
    }

    let algorithm = match algorithm_name {
      "blake2b512" => Blake2b512(Default::default()),
      "blake2s256" => Blake2s256(Default::default()),

      "md4" => Md4(Default::default()),
      "md5" => Md5(Default::default()),

      "ripemd160" => Ripemd160(Default::default()),

      "sha1" => Sha1(Default::default()),
      "sha224" => Sha224(Default::default()),
      "sha256" => Sha256(Default::default()),
      "sha384" => Sha384(Default::default()),
      "sha512" => Sha512(Default::default()),
      "sha512-224" => Sha512_224(Default::default()),
      "sha512-256" => Sha512_256(Default::default()),

      "sha3-224" => Sha3_224(Default::default()),
      "sha3-256" => Sha3_256(Default::default()),
      "sha3-384" => Sha3_384(Default::default()),
      "sha3-512" => Sha3_512(Default::default()),

      "sm3" => Sm3(Default::default()),

      _ => {
        return Err(generic_error(format!(
          "Digest method not supported: {algorithm_name}"
        )))
      }
    };
    if let Some(length) = output_length {
      if length != algorithm.output_length() {
        return Err(generic_error(
          "Output length mismatch for non-extendable algorithm",
        ));
      }
    }
    Ok(algorithm)
  }

  pub fn output_length(&self) -> usize {
    match self {
      Blake2b512(context) => context.output_size(),
      Blake2s256(context) => context.output_size(),

      Md4(context) => context.output_size(),
      Md5(context) => context.output_size(),

      Ripemd160(context) => context.output_size(),

      Sha1(context) => context.output_size(),
      Sha224(context) => context.output_size(),
      Sha256(context) => context.output_size(),
      Sha384(context) => context.output_size(),
      Sha512(context) => context.output_size(),
      Sha512_224(context) => context.output_size(),
      Sha512_256(context) => context.output_size(),

      Sha3_224(context) => context.output_size(),
      Sha3_256(context) => context.output_size(),
      Sha3_384(context) => context.output_size(),
      Sha3_512(context) => context.output_size(),

      Sm3(context) => context.output_size(),

      Shake128(_, _) => unreachable!(
        "output_length() should not be called on extendable algorithms"
      ),
      Shake256(_, _) => unreachable!(
        "output_length() should not be called on extendable algorithms"
      ),
    }
  }

  pub fn update(&mut self, data: &[u8]) {
    match self {
      Blake2b512(context) => Digest::update(&mut **context, data),
      Blake2s256(context) => Digest::update(&mut **context, data),

      Md4(context) => Digest::update(&mut **context, data),
      Md5(context) => Digest::update(&mut **context, data),

      Ripemd160(context) => Digest::update(&mut **context, data),

      Sha1(context) => Digest::update(&mut **context, data),
      Sha224(context) => Digest::update(&mut **context, data),
      Sha256(context) => Digest::update(&mut **context, data),
      Sha384(context) => Digest::update(&mut **context, data),
      Sha512(context) => Digest::update(&mut **context, data),
      Sha512_224(context) => Digest::update(&mut **context, data),
      Sha512_256(context) => Digest::update(&mut **context, data),

      Sha3_224(context) => Digest::update(&mut **context, data),
      Sha3_256(context) => Digest::update(&mut **context, data),
      Sha3_384(context) => Digest::update(&mut **context, data),
      Sha3_512(context) => Digest::update(&mut **context, data),

      Sm3(context) => Digest::update(&mut **context, data),

      Shake128(context, _) => Update::update(&mut **context, data),
      Shake256(context, _) => Update::update(&mut **context, data),
    };
  }

  pub fn digest_and_drop(self) -> Box<[u8]> {
    match self {
      Blake2b512(context) => context.finalize(),
      Blake2s256(context) => context.finalize(),

      Md4(context) => context.finalize(),
      Md5(context) => context.finalize(),

      Ripemd160(context) => context.finalize(),

      Sha1(context) => context.finalize(),
      Sha224(context) => context.finalize(),
      Sha256(context) => context.finalize(),
      Sha384(context) => context.finalize(),
      Sha512(context) => context.finalize(),
      Sha512_224(context) => context.finalize(),
      Sha512_256(context) => context.finalize(),

      Sha3_224(context) => context.finalize(),
      Sha3_256(context) => context.finalize(),
      Sha3_384(context) => context.finalize(),
      Sha3_512(context) => context.finalize(),

      Sm3(context) => context.finalize(),

      // The default output lengths align with Node.js
      Shake128(context, output_length) => {
        context.finalize_boxed(output_length.unwrap_or(16))
      }
      Shake256(context, output_length) => {
        context.finalize_boxed(output_length.unwrap_or(32))
      }
    }
  }

  pub fn clone_hash(
    &self,
    output_length: Option<usize>,
  ) -> Result<Self, AnyError> {
    let hash = match self {
      Shake128(context, _) => {
        return Ok(Shake128(context.clone(), output_length))
      }
      Shake256(context, _) => {
        return Ok(Shake256(context.clone(), output_length))
      }

      Blake2b512(context) => Blake2b512(context.clone()),
      Blake2s256(context) => Blake2s256(context.clone()),

      Md4(context) => Md4(context.clone()),
      Md5(context) => Md5(context.clone()),

      Ripemd160(context) => Ripemd160(context.clone()),

      Sha1(context) => Sha1(context.clone()),
      Sha224(context) => Sha224(context.clone()),
      Sha256(context) => Sha256(context.clone()),
      Sha384(context) => Sha384(context.clone()),
      Sha512(context) => Sha512(context.clone()),
      Sha512_224(context) => Sha512_224(context.clone()),
      Sha512_256(context) => Sha512_256(context.clone()),

      Sha3_224(context) => Sha3_224(context.clone()),
      Sha3_256(context) => Sha3_256(context.clone()),
      Sha3_384(context) => Sha3_384(context.clone()),
      Sha3_512(context) => Sha3_512(context.clone()),

      Sm3(context) => Sm3(context.clone()),
    };

    if let Some(length) = output_length {
      if length != hash.output_length() {
        return Err(generic_error(
          "Output length mismatch for non-extendable algorithm",
        ));
      }
    }

    Ok(hash)
  }

  pub fn get_hashes() -> Vec<&'static str> {
    vec![
      "blake2s256",
      "blake2b512",
      "md4",
      "md5",
      "ripemd160",
      "sha1",
      "sha224",
      "sha256",
      "sha384",
      "sha512",
      "sha512-224",
      "sha512-256",
      "sha3-224",
      "sha3-256",
      "sha3-384",
      "sha3-512",
      "shake128",
      "shake256",
      "sm3",
    ]
  }
}
