// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use aes::cipher::BlockEncryptMut;
use aes::cipher::KeyIvInit;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::Resource;

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

pub enum Cipher {
  Aes128Cbc(Box<cbc::Encryptor::<aes::Aes128>>),
}

pub enum Decipher {
  // Aes128Cbc(Box<cbc::Decryptor<aes::Aes128>>),
}

pub struct CipherContext {
  pub cipher: Rc<RefCell<Cipher>>,
}

pub struct DecipherContext {
  pub decipher: Rc<RefCell<Decipher>>,
}

impl CipherContext {
  pub fn new(algorithm: &str, key: &[u8], iv: &[u8]) -> Result<Self, AnyError> {
    Ok(Self {
      cipher: Rc::new(RefCell::new(Cipher::new(algorithm, key, iv)?)),
    })
  }

  pub fn encrypt(&self, input: &[u8], output: &mut [u8]) {
    self.cipher.borrow_mut().encrypt(input, output);
  }
}

impl Resource for CipherContext {
  fn name(&self) -> Cow<str> {
    "cryptoCipher".into()
  }
}

impl Resource for DecipherContext {
  fn name(&self) -> Cow<str> {
    "cryptoDecipher".into()
  }
}

use Cipher::*;

impl Cipher {
  pub fn new(algorithm_name: &str, key: &[u8], iv: &[u8]) -> Result<Self, AnyError> {
    Ok(match algorithm_name {
      "aes-128-cbc" => Aes128Cbc(Box::new(cbc::Encryptor::new(key.into(), iv.into()))),
      _ => return Err(type_error(format!("Unknown cipher {algorithm_name}"))),
    })
  }

  pub fn encrypt(&mut self, input: &[u8], output: &mut [u8]) {
    match self {
      Aes128Cbc(encryptor) => {
        let len = input.len();
        if len == 16 {
          encryptor.as_mut().encrypt_block_b2b_mut(input.into(), output.into());
        } else {
          let mut block = [0; 16];
          block[..input.len()].copy_from_slice(input);
          pad_block(&mut block, len);
          encryptor.as_mut().encrypt_block_b2b_mut(&block.into(), output.into());
        }
      },
    }
  }
}

/// padding the last block of cbc mode
fn pad_block(data: &mut [u8; 16], pos: usize) {
  let v = (16 - pos) as u8;
  for b in &mut data[pos..] {
    *b = v;
  }
}
