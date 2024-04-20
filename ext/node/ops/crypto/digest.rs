// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::Resource;
use digest::Digest;
use digest::DynDigest;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

pub enum Hash {
  Md4(Box<md4::Md4>),
  Md5(Box<md5::Md5>),
  Ripemd160(Box<ripemd::Ripemd160>),
  Sha1(Box<sha1::Sha1>),
  Sha224(Box<sha2::Sha224>),
  Sha256(Box<sha2::Sha256>),
  Sha384(Box<sha2::Sha384>),
  Sha512(Box<sha2::Sha512>),
}

pub struct Context {
  pub hash: Rc<RefCell<Hash>>,
}

impl Context {
  pub fn new(algorithm: &str) -> Result<Self, AnyError> {
    Ok(Self {
      hash: Rc::new(RefCell::new(Hash::new(algorithm)?)),
    })
  }

  pub fn update(&self, data: &[u8]) {
    self.hash.borrow_mut().update(data);
  }

  pub fn digest(self) -> Result<Box<[u8]>, AnyError> {
    let hash = Rc::try_unwrap(self.hash)
      .map_err(|_| type_error("Hash context is already in use"))?;

    let hash = hash.into_inner();
    Ok(hash.digest_and_drop())
  }
}

impl Clone for Context {
  fn clone(&self) -> Self {
    Self {
      hash: Rc::new(RefCell::new(self.hash.borrow().clone())),
    }
  }
}

impl Resource for Context {
  fn name(&self) -> Cow<str> {
    "cryptoDigest".into()
  }
}

use Hash::*;

impl Hash {
  pub fn new(algorithm_name: &str) -> Result<Self, AnyError> {
    Ok(match algorithm_name {
      "md4" => Md4(Default::default()),
      "md5" => Md5(Default::default()),
      "ripemd160" => Ripemd160(Default::default()),
      "sha1" => Sha1(Default::default()),
      "sha224" => Sha224(Default::default()),
      "sha256" => Sha256(Default::default()),
      "sha384" => Sha384(Default::default()),
      "sha512" => Sha512(Default::default()),
      _ => return Err(type_error("unsupported algorithm")),
    })
  }

  pub fn update(&mut self, data: &[u8]) {
    match self {
      Md4(context) => Digest::update(&mut **context, data),
      Md5(context) => Digest::update(&mut **context, data),
      Ripemd160(context) => Digest::update(&mut **context, data),
      Sha1(context) => Digest::update(&mut **context, data),
      Sha224(context) => Digest::update(&mut **context, data),
      Sha256(context) => Digest::update(&mut **context, data),
      Sha384(context) => Digest::update(&mut **context, data),
      Sha512(context) => Digest::update(&mut **context, data),
    };
  }

  pub fn digest_and_drop(self) -> Box<[u8]> {
    match self {
      Md4(context) => context.finalize(),
      Md5(context) => context.finalize(),
      Ripemd160(context) => context.finalize(),
      Sha1(context) => context.finalize(),
      Sha224(context) => context.finalize(),
      Sha256(context) => context.finalize(),
      Sha384(context) => context.finalize(),
      Sha512(context) => context.finalize(),
    }
  }

  pub fn get_hashes() -> Vec<&'static str> {
    vec![
      "md4",
      "md5",
      "ripemd160",
      "sha1",
      "sha224",
      "sha256",
      "sha384",
      "sha512",
    ]
  }
}

impl Clone for Hash {
  fn clone(&self) -> Self {
    match self {
      Md4(_) => Md4(Default::default()),
      Md5(_) => Md5(Default::default()),
      Ripemd160(_) => Ripemd160(Default::default()),
      Sha1(_) => Sha1(Default::default()),
      Sha224(_) => Sha224(Default::default()),
      Sha256(_) => Sha256(Default::default()),
      Sha384(_) => Sha384(Default::default()),
      Sha512(_) => Sha512(Default::default()),
    }
  }
}
