// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::BlockDecryptMut;
use aes::cipher::BlockEncryptMut;
use aes::cipher::KeyIvInit;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::Resource;
use digest::KeyInit;

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

enum Cipher {
  Aes128Cbc(Box<cbc::Encryptor<aes::Aes128>>),
  Aes128Ecb(Box<ecb::Encryptor<aes::Aes128>>),
  // TODO(kt3k): add more algorithms Aes192Cbc, Aes256Cbc, Aes128GCM, etc.
}

enum Decipher {
  Aes128Cbc(Box<cbc::Decryptor<aes::Aes128>>),
  Aes128Ecb(Box<ecb::Decryptor<aes::Aes128>>),
  // TODO(kt3k): add more algorithms Aes192Cbc, Aes256Cbc, Aes128GCM, etc.
}

pub struct CipherContext {
  cipher: Rc<RefCell<Cipher>>,
}

pub struct DecipherContext {
  decipher: Rc<RefCell<Decipher>>,
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

  pub fn r#final(
    self,
    input: &[u8],
    output: &mut [u8],
  ) -> Result<(), AnyError> {
    Rc::try_unwrap(self.cipher)
      .map_err(|_| type_error("Cipher context is already in use"))?
      .into_inner()
      .r#final(input, output)
  }
}

impl DecipherContext {
  pub fn new(algorithm: &str, key: &[u8], iv: &[u8]) -> Result<Self, AnyError> {
    Ok(Self {
      decipher: Rc::new(RefCell::new(Decipher::new(algorithm, key, iv)?)),
    })
  }

  pub fn decrypt(&self, input: &[u8], output: &mut [u8]) {
    self.decipher.borrow_mut().decrypt(input, output);
  }

  pub fn r#final(
    self,
    input: &[u8],
    output: &mut [u8],
  ) -> Result<(), AnyError> {
    Rc::try_unwrap(self.decipher)
      .map_err(|_| type_error("Decipher context is already in use"))?
      .into_inner()
      .r#final(input, output)
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

impl Cipher {
  fn new(
    algorithm_name: &str,
    key: &[u8],
    iv: &[u8],
  ) -> Result<Self, AnyError> {
    use Cipher::*;
    Ok(match algorithm_name {
      "aes-128-cbc" => {
        Aes128Cbc(Box::new(cbc::Encryptor::new(key.into(), iv.into())))
      }
      "aes-128-ecb" => Aes128Ecb(Box::new(ecb::Encryptor::new(key.into()))),
      _ => return Err(type_error(format!("Unknown cipher {algorithm_name}"))),
    })
  }

  /// encrypt encrypts the data in the middle of the input.
  fn encrypt(&mut self, input: &[u8], output: &mut [u8]) {
    use Cipher::*;
    match self {
      Aes128Cbc(encryptor) => {
        assert!(input.len() % 16 == 0);
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          encryptor.encrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes128Ecb(encryptor) => {
        assert!(input.len() % 16 == 0);
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          encryptor.encrypt_block_b2b_mut(input.into(), output.into());
        }
      }
    }
  }

  /// r#final encrypts the last block of the input data.
  fn r#final(self, input: &[u8], output: &mut [u8]) -> Result<(), AnyError> {
    assert!(input.len() < 16);
    use Cipher::*;
    match self {
      Aes128Cbc(encryptor) => {
        let _ = (*encryptor)
          .encrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| type_error("Cannot pad the input data"))?;
        Ok(())
      }
      Aes128Ecb(encryptor) => {
        let _ = (*encryptor)
          .encrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| type_error("Cannot pad the input data"))?;
        Ok(())
      }
    }
  }
}

impl Decipher {
  fn new(
    algorithm_name: &str,
    key: &[u8],
    iv: &[u8],
  ) -> Result<Self, AnyError> {
    use Decipher::*;
    Ok(match algorithm_name {
      "aes-128-cbc" => {
        Aes128Cbc(Box::new(cbc::Decryptor::new(key.into(), iv.into())))
      }
      "aes-128-ecb" => Aes128Ecb(Box::new(ecb::Decryptor::new(key.into()))),
      _ => return Err(type_error(format!("Unknown cipher {algorithm_name}"))),
    })
  }

  /// decrypt decrypts the data in the middle of the input.
  fn decrypt(&mut self, input: &[u8], output: &mut [u8]) {
    use Decipher::*;
    match self {
      Aes128Cbc(decryptor) => {
        assert!(input.len() % 16 == 0);
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          decryptor.decrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes128Ecb(decryptor) => {
        assert!(input.len() % 16 == 0);
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          decryptor.decrypt_block_b2b_mut(input.into(), output.into());
        }
      }
    }
  }

  /// r#final decrypts the last block of the input data.
  fn r#final(self, input: &[u8], output: &mut [u8]) -> Result<(), AnyError> {
    assert!(input.len() == 16);
    use Decipher::*;
    match self {
      Aes128Cbc(decryptor) => {
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| type_error("Cannot unpad the input data"))?;
        Ok(())
      }
      Aes128Ecb(decryptor) => {
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| type_error("Cannot unpad the input data"))?;
        Ok(())
      }
    }
  }
}
