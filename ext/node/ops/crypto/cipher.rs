// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use aes::cipher::BlockDecryptMut;
use aes::cipher::BlockEncryptMut;
use aes::cipher::KeyIvInit;
use aes::cipher::KeySizeUser;
use aes::cipher::StreamCipher;
use aes::cipher::block_padding::Pkcs7;
use deno_core::Resource;
use deno_error::JsErrorClass;
use digest::KeyInit;
use digest::generic_array::GenericArray;

type Tag = Option<Vec<u8>>;

type Aes128Gcm = aead_gcm_stream::AesGcm<aes::Aes128>;
type Aes256Gcm = aead_gcm_stream::AesGcm<aes::Aes256>;

enum Cipher {
  Aes128Cbc(Box<cbc::Encryptor<aes::Aes128>>),
  Aes128Ecb(Box<ecb::Encryptor<aes::Aes128>>),
  Aes192Ecb(Box<ecb::Encryptor<aes::Aes192>>),
  Aes256Ecb(Box<ecb::Encryptor<aes::Aes256>>),
  Aes128Gcm(Box<Aes128Gcm>, Option<usize>),
  Aes256Gcm(Box<Aes256Gcm>, Option<usize>),
  Aes256Cbc(Box<cbc::Encryptor<aes::Aes256>>),
  Aes128Ctr(Box<ctr::Ctr128BE<aes::Aes128>>),
  Aes192Ctr(Box<ctr::Ctr128BE<aes::Aes192>>),
  Aes256Ctr(Box<ctr::Ctr128BE<aes::Aes256>>),
  // TODO(kt3k): add more algorithms Aes192Cbc, etc.
}

enum Decipher {
  Aes128Cbc(Box<cbc::Decryptor<aes::Aes128>>),
  Aes128Ecb(Box<ecb::Decryptor<aes::Aes128>>),
  Aes192Ecb(Box<ecb::Decryptor<aes::Aes192>>),
  Aes256Ecb(Box<ecb::Decryptor<aes::Aes256>>),
  Aes128Gcm(Box<Aes128Gcm>, Option<usize>),
  Aes256Gcm(Box<Aes256Gcm>, Option<usize>),
  Aes256Cbc(Box<cbc::Decryptor<aes::Aes256>>),
  Aes128Ctr(Box<ctr::Ctr128BE<aes::Aes128>>),
  Aes192Ctr(Box<ctr::Ctr128BE<aes::Aes192>>),
  Aes256Ctr(Box<ctr::Ctr128BE<aes::Aes256>>),
  // TODO(kt3k): add more algorithms Aes192Cbc, Aes128GCM, etc.
}

pub struct CipherContext {
  cipher: Rc<RefCell<Cipher>>,
}

pub struct DecipherContext {
  decipher: Rc<RefCell<Decipher>>,
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CipherContextError {
  #[class(type)]
  #[error("Cipher context is already in use")]
  ContextInUse,
  #[class(inherit)]
  #[error("{0}")]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Cipher(#[from] CipherError),
}

impl CipherContext {
  pub fn new(
    algorithm: &str,
    key: &[u8],
    iv: &[u8],
    auth_tag_length: Option<usize>,
  ) -> Result<Self, CipherContextError> {
    Ok(Self {
      cipher: Rc::new(RefCell::new(Cipher::new(
        algorithm,
        key,
        iv,
        auth_tag_length,
      )?)),
    })
  }

  pub fn set_aad(&self, aad: &[u8]) {
    self.cipher.borrow_mut().set_aad(aad);
  }

  pub fn encrypt(&self, input: &[u8], output: &mut [u8]) {
    self.cipher.borrow_mut().encrypt(input, output);
  }

  pub fn take_tag(self) -> Tag {
    Rc::try_unwrap(self.cipher).ok()?.into_inner().take_tag()
  }

  pub fn r#final(
    self,
    auto_pad: bool,
    input: &[u8],
    output: &mut [u8],
  ) -> Result<Tag, CipherContextError> {
    Rc::try_unwrap(self.cipher)
      .map_err(|_| CipherContextError::ContextInUse)?
      .into_inner()
      .r#final(auto_pad, input, output)
      .map_err(Into::into)
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum DecipherContextError {
  #[class(type)]
  #[error("Decipher context is already in use")]
  ContextInUse,
  #[class(inherit)]
  #[error("{0}")]
  Resource(#[from] deno_core::error::ResourceError),
  #[class(inherit)]
  #[error(transparent)]
  Decipher(#[from] DecipherError),
}

impl DecipherContext {
  pub fn new(
    algorithm: &str,
    key: &[u8],
    iv: &[u8],
    auth_tag_length: Option<usize>,
  ) -> Result<Self, DecipherContextError> {
    Ok(Self {
      decipher: Rc::new(RefCell::new(Decipher::new(
        algorithm,
        key,
        iv,
        auth_tag_length,
      )?)),
    })
  }

  pub fn validate_auth_tag(
    &self,
    length: usize,
  ) -> Result<(), DecipherContextError> {
    self.decipher.borrow().validate_auth_tag(length)?;

    Ok(())
  }

  pub fn set_aad(&self, aad: &[u8]) {
    self.decipher.borrow_mut().set_aad(aad);
  }

  pub fn decrypt(&self, input: &[u8], output: &mut [u8]) {
    self.decipher.borrow_mut().decrypt(input, output);
  }

  pub fn r#final(
    self,
    auto_pad: bool,
    input: &[u8],
    output: &mut [u8],
    auth_tag: &[u8],
  ) -> Result<(), DecipherContextError> {
    Rc::try_unwrap(self.decipher)
      .map_err(|_| DecipherContextError::ContextInUse)?
      .into_inner()
      .r#final(auto_pad, input, output, auth_tag)
      .map_err(Into::into)
  }
}

impl Resource for CipherContext {
  fn name(&self) -> Cow<'_, str> {
    "cryptoCipher".into()
  }
}

impl Resource for DecipherContext {
  fn name(&self) -> Cow<'_, str> {
    "cryptoDecipher".into()
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum CipherError {
  #[class(type)]
  #[error("IV length must be 12 bytes")]
  InvalidIvLength,
  #[class(range)]
  #[error("Invalid key length")]
  InvalidKeyLength,
  #[class(type)]
  #[error("Invalid initialization vector")]
  InvalidInitializationVector,
  #[class(type)]
  #[error("bad decrypt")]
  CannotPadInputData,
  #[class(type)]
  #[error("Unknown cipher {0}")]
  UnknownCipher(String),
  #[class(type)]
  #[error("Invalid authentication tag length: {0}")]
  InvalidAuthTag(usize),
}

impl Cipher {
  fn new(
    algorithm_name: &str,
    key: &[u8],
    iv: &[u8],
    auth_tag_length: Option<usize>,
  ) -> Result<Self, CipherError> {
    use Cipher::*;
    Ok(match algorithm_name {
      "aes128" | "aes-128-cbc" => {
        Aes128Cbc(Box::new(cbc::Encryptor::new(key.into(), iv.into())))
      }
      "aes-128-ecb" => Aes128Ecb(Box::new(ecb::Encryptor::new(key.into()))),
      "aes-192-ecb" => Aes192Ecb(Box::new(ecb::Encryptor::new(key.into()))),
      "aes-256-ecb" => Aes256Ecb(Box::new(ecb::Encryptor::new(key.into()))),
      "aes-128-gcm" => {
        if key.len() != aes::Aes128::key_size() {
          return Err(CipherError::InvalidKeyLength);
        }

        if let Some(tag_len) = auth_tag_length
          && !is_valid_gcm_tag_length(tag_len)
        {
          return Err(CipherError::InvalidAuthTag(tag_len));
        }

        let cipher =
          aead_gcm_stream::AesGcm::<aes::Aes128>::new(key.into(), iv);

        Aes128Gcm(Box::new(cipher), auth_tag_length)
      }
      "aes-256-gcm" => {
        if key.len() != aes::Aes256::key_size() {
          return Err(CipherError::InvalidKeyLength);
        }

        if let Some(tag_len) = auth_tag_length
          && !is_valid_gcm_tag_length(tag_len)
        {
          return Err(CipherError::InvalidAuthTag(tag_len));
        }

        let cipher =
          aead_gcm_stream::AesGcm::<aes::Aes256>::new(key.into(), iv);

        Aes256Gcm(Box::new(cipher), auth_tag_length)
      }
      "aes256" | "aes-256-cbc" => {
        if key.len() != 32 {
          return Err(CipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(CipherError::InvalidInitializationVector);
        }

        Aes256Cbc(Box::new(cbc::Encryptor::new(key.into(), iv.into())))
      }
      "aes-256-ctr" => {
        if key.len() != 32 {
          return Err(CipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(CipherError::InvalidInitializationVector);
        }
        Aes256Ctr(Box::new(ctr::Ctr128BE::new(key.into(), iv.into())))
      }
      "aes-192-ctr" => {
        if key.len() != 24 {
          return Err(CipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(CipherError::InvalidInitializationVector);
        }
        Aes192Ctr(Box::new(ctr::Ctr128BE::new(key.into(), iv.into())))
      }
      "aes-128-ctr" => {
        if key.len() != 16 {
          return Err(CipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(CipherError::InvalidInitializationVector);
        }
        Aes128Ctr(Box::new(ctr::Ctr128BE::new(key.into(), iv.into())))
      }
      _ => return Err(CipherError::UnknownCipher(algorithm_name.to_string())),
    })
  }

  fn set_aad(&mut self, aad: &[u8]) {
    use Cipher::*;
    match self {
      Aes128Gcm(cipher, _) => {
        cipher.set_aad(aad);
      }
      Aes256Gcm(cipher, _) => {
        cipher.set_aad(aad);
      }
      _ => {}
    }
  }

  /// encrypt encrypts the data in the middle of the input.
  fn encrypt(&mut self, input: &[u8], output: &mut [u8]) {
    use Cipher::*;
    match self {
      Aes128Cbc(encryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          encryptor.encrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes128Ecb(encryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          encryptor.encrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes192Ecb(encryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          encryptor.encrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes256Ecb(encryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          encryptor.encrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes128Gcm(cipher, _) => {
        output[..input.len()].copy_from_slice(input);
        cipher.encrypt(output);
      }
      Aes256Gcm(cipher, _) => {
        output[..input.len()].copy_from_slice(input);
        cipher.encrypt(output);
      }
      Aes256Cbc(encryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          encryptor.encrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes256Ctr(encryptor) => {
        encryptor.apply_keystream_b2b(input, output).unwrap();
      }
      Aes192Ctr(encryptor) => {
        encryptor.apply_keystream_b2b(input, output).unwrap();
      }
      Aes128Ctr(encryptor) => {
        encryptor.apply_keystream_b2b(input, output).unwrap();
      }
    }
  }

  /// r#final encrypts the last block of the input data.
  fn r#final(
    self,
    auto_pad: bool,
    input: &[u8],
    output: &mut [u8],
  ) -> Result<Tag, CipherError> {
    assert!(input.len() < 16);
    use Cipher::*;
    match (self, auto_pad) {
      (Aes128Cbc(encryptor), true) => {
        let _ = (*encryptor)
          .encrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| CipherError::CannotPadInputData)?;
        Ok(None)
      }
      (Aes128Cbc(mut encryptor), false) => {
        encryptor.encrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(None)
      }
      (Aes128Ecb(encryptor), true) => {
        let _ = (*encryptor)
          .encrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| CipherError::CannotPadInputData)?;
        Ok(None)
      }
      (Aes128Ecb(mut encryptor), false) => {
        encryptor.encrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(None)
      }
      (Aes192Ecb(encryptor), true) => {
        let _ = (*encryptor)
          .encrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| CipherError::CannotPadInputData)?;
        Ok(None)
      }
      (Aes192Ecb(mut encryptor), false) => {
        encryptor.encrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(None)
      }
      (Aes256Ecb(encryptor), true) => {
        let _ = (*encryptor)
          .encrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| CipherError::CannotPadInputData)?;
        Ok(None)
      }
      (Aes256Ecb(mut encryptor), false) => {
        encryptor.encrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(None)
      }
      (Aes128Gcm(cipher, auth_tag_length), _) => {
        let mut tag = cipher.finish().to_vec();
        if let Some(tag_len) = auth_tag_length {
          tag.truncate(tag_len);
        }
        Ok(Some(tag))
      }
      (Aes256Gcm(cipher, auth_tag_length), _) => {
        let mut tag = cipher.finish().to_vec();
        if let Some(tag_len) = auth_tag_length {
          tag.truncate(tag_len);
        }
        Ok(Some(tag))
      }
      (Aes256Cbc(encryptor), true) => {
        let _ = (*encryptor)
          .encrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| CipherError::CannotPadInputData)?;
        Ok(None)
      }
      (Aes256Cbc(mut encryptor), false) => {
        encryptor.encrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(None)
      }
      (Aes256Ctr(_) | Aes128Ctr(_) | Aes192Ctr(_), _) => Ok(None),
    }
  }

  fn take_tag(self) -> Tag {
    use Cipher::*;
    match self {
      Aes128Gcm(cipher, auth_tag_length) => {
        let mut tag = cipher.finish().to_vec();
        if let Some(tag_len) = auth_tag_length {
          tag.truncate(tag_len);
        }
        Some(tag)
      }
      Aes256Gcm(cipher, auth_tag_length) => {
        let mut tag = cipher.finish().to_vec();
        if let Some(tag_len) = auth_tag_length {
          tag.truncate(tag_len);
        }
        Some(tag)
      }
      _ => None,
    }
  }
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[property("library" = "Provider routines")]
#[property("reason" = self.reason())]
#[property("code" = self.code())]
pub enum DecipherError {
  #[class(type)]
  #[error("IV length must be 12 bytes")]
  InvalidIvLength,
  #[class(range)]
  #[error("Invalid key length")]
  InvalidKeyLength,
  #[class(type)]
  #[error("Invalid authentication tag length: {0}")]
  InvalidAuthTag(usize),
  #[class(range)]
  #[error("error:1C80006B:Provider routines::wrong final block length")]
  InvalidFinalBlockLength,
  #[class(type)]
  #[error("Invalid initialization vector")]
  InvalidInitializationVector,
  #[class(type)]
  #[error("bad decrypt")]
  CannotUnpadInputData,
  #[class(type)]
  #[error("Failed to authenticate data")]
  DataAuthenticationFailed,
  #[class(type)]
  #[error("setAutoPadding(false) not supported for Aes128Gcm yet")]
  SetAutoPaddingFalseAes128GcmUnsupported,
  #[class(type)]
  #[error("setAutoPadding(false) not supported for Aes256Gcm yet")]
  SetAutoPaddingFalseAes256GcmUnsupported,
  #[class(type)]
  #[error("Unknown cipher {0}")]
  UnknownCipher(String),
}

impl DecipherError {
  fn code(&self) -> deno_error::PropertyValue {
    match self {
      Self::InvalidIvLength => {
        deno_error::PropertyValue::String("ERR_CRYPTO_INVALID_IV_LENGTH".into())
      }
      Self::InvalidKeyLength => deno_error::PropertyValue::String(
        "ERR_CRYPTO_INVALID_KEY_LENGTH".into(),
      ),
      Self::InvalidAuthTag(_) => {
        deno_error::PropertyValue::String("ERR_CRYPTO_INVALID_AUTH_TAG".into())
      }
      Self::InvalidFinalBlockLength => deno_error::PropertyValue::String(
        "ERR_OSSL_WRONG_FINAL_BLOCK_LENGTH".into(),
      ),
      _ => deno_error::PropertyValue::String("ERR_CRYPTO_DECIPHER".into()),
    }
  }

  fn reason(&self) -> deno_error::PropertyValue {
    match self {
      Self::InvalidFinalBlockLength => {
        deno_error::PropertyValue::String("wrong final block length".into())
      }
      _ => deno_error::PropertyValue::String(self.get_message()),
    }
  }
}

macro_rules! assert_block_len {
  ($input:expr, $len:expr) => {
    if $input != $len {
      return Err(DecipherError::InvalidFinalBlockLength);
    }
  };
}

fn is_valid_gcm_tag_length(tag_len: usize) -> bool {
  tag_len == 4 || tag_len == 8 || (12..=16).contains(&tag_len)
}

impl Decipher {
  fn new(
    algorithm_name: &str,
    key: &[u8],
    iv: &[u8],
    auth_tag_length: Option<usize>,
  ) -> Result<Self, DecipherError> {
    use Decipher::*;
    Ok(match algorithm_name {
      "aes-128-cbc" => {
        Aes128Cbc(Box::new(cbc::Decryptor::new(key.into(), iv.into())))
      }
      "aes-128-ecb" => Aes128Ecb(Box::new(ecb::Decryptor::new(key.into()))),
      "aes-192-ecb" => Aes192Ecb(Box::new(ecb::Decryptor::new(key.into()))),
      "aes-256-ecb" => Aes256Ecb(Box::new(ecb::Decryptor::new(key.into()))),
      "aes-128-gcm" => {
        if key.len() != aes::Aes128::key_size() {
          return Err(DecipherError::InvalidKeyLength);
        }

        if let Some(tag_len) = auth_tag_length
          && !is_valid_gcm_tag_length(tag_len)
        {
          return Err(DecipherError::InvalidAuthTag(tag_len));
        }

        let decipher =
          aead_gcm_stream::AesGcm::<aes::Aes128>::new(key.into(), iv);

        Aes128Gcm(Box::new(decipher), auth_tag_length)
      }
      "aes-256-gcm" => {
        if key.len() != aes::Aes256::key_size() {
          return Err(DecipherError::InvalidKeyLength);
        }

        if let Some(tag_len) = auth_tag_length
          && !is_valid_gcm_tag_length(tag_len)
        {
          return Err(DecipherError::InvalidAuthTag(tag_len));
        }

        let decipher =
          aead_gcm_stream::AesGcm::<aes::Aes256>::new(key.into(), iv);

        Aes256Gcm(Box::new(decipher), auth_tag_length)
      }
      "aes256" | "aes-256-cbc" => {
        if key.len() != 32 {
          return Err(DecipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(DecipherError::InvalidInitializationVector);
        }

        Aes256Cbc(Box::new(cbc::Decryptor::new(key.into(), iv.into())))
      }
      "aes-256-ctr" => {
        if key.len() != 32 {
          return Err(DecipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(DecipherError::InvalidInitializationVector);
        }
        Aes256Ctr(Box::new(ctr::Ctr128BE::new(key.into(), iv.into())))
      }
      "aes-192-ctr" => {
        if key.len() != 24 {
          return Err(DecipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(DecipherError::InvalidInitializationVector);
        }
        Aes192Ctr(Box::new(ctr::Ctr128BE::new(key.into(), iv.into())))
      }
      "aes-128-ctr" => {
        if key.len() != 16 {
          return Err(DecipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(DecipherError::InvalidInitializationVector);
        }
        Aes128Ctr(Box::new(ctr::Ctr128BE::new(key.into(), iv.into())))
      }
      _ => {
        return Err(DecipherError::UnknownCipher(algorithm_name.to_string()));
      }
    })
  }

  fn validate_auth_tag(&self, length: usize) -> Result<(), DecipherError> {
    match self {
      Decipher::Aes128Gcm(_, Some(tag_len))
      | Decipher::Aes256Gcm(_, Some(tag_len)) => {
        if *tag_len != length {
          return Err(DecipherError::InvalidAuthTag(length));
        }
      }
      _ => {}
    }
    Ok(())
  }

  fn set_aad(&mut self, aad: &[u8]) {
    use Decipher::*;
    match self {
      Aes128Gcm(decipher, _) => {
        decipher.set_aad(aad);
      }
      Aes256Gcm(decipher, _) => {
        decipher.set_aad(aad);
      }
      _ => {}
    }
  }

  /// decrypt decrypts the data in the middle of the input.
  fn decrypt(&mut self, input: &[u8], output: &mut [u8]) {
    use Decipher::*;
    match self {
      Aes128Cbc(decryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          decryptor.decrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes128Ecb(decryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          decryptor.decrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes192Ecb(decryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          decryptor.decrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes256Ecb(decryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          decryptor.decrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes128Gcm(decipher, _) => {
        output[..input.len()].copy_from_slice(input);
        decipher.decrypt(output);
      }
      Aes256Gcm(decipher, _) => {
        output[..input.len()].copy_from_slice(input);
        decipher.decrypt(output);
      }
      Aes256Cbc(decryptor) => {
        assert!(input.len().is_multiple_of(16));
        for (input, output) in input.chunks(16).zip(output.chunks_mut(16)) {
          decryptor.decrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      Aes256Ctr(decryptor) => {
        decryptor.apply_keystream_b2b(input, output).unwrap();
      }
      Aes192Ctr(decryptor) => {
        decryptor.apply_keystream_b2b(input, output).unwrap();
      }
      Aes128Ctr(decryptor) => {
        decryptor.apply_keystream_b2b(input, output).unwrap();
      }
    }
  }

  /// r#final decrypts the last block of the input data.
  fn r#final(
    self,
    auto_pad: bool,
    input: &[u8],
    output: &mut [u8],
    auth_tag: &[u8],
  ) -> Result<(), DecipherError> {
    use Decipher::*;

    if input.is_empty()
      && !matches!(
        self,
        Aes128Ecb(..)
          | Aes192Ecb(..)
          | Aes256Ecb(..)
          | Aes128Gcm(..)
          | Aes256Gcm(..)
      )
    {
      return Ok(());
    }

    match (self, auto_pad) {
      (Aes128Cbc(decryptor), true) => {
        assert_block_len!(input.len(), 16);
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| DecipherError::CannotUnpadInputData)?;
        Ok(())
      }
      (Aes128Cbc(mut decryptor), false) => {
        decryptor.decrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(())
      }
      (Aes128Ecb(decryptor), true) => {
        assert_block_len!(input.len(), 16);
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| DecipherError::CannotUnpadInputData)?;
        Ok(())
      }
      (Aes128Ecb(mut decryptor), false) => {
        decryptor.decrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(())
      }
      (Aes192Ecb(decryptor), true) => {
        assert_block_len!(input.len(), 16);
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| DecipherError::CannotUnpadInputData)?;
        Ok(())
      }
      (Aes192Ecb(mut decryptor), false) => {
        decryptor.decrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(())
      }
      (Aes256Ecb(decryptor), true) => {
        assert_block_len!(input.len(), 16);
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| DecipherError::CannotUnpadInputData)?;
        Ok(())
      }
      (Aes256Ecb(mut decryptor), false) => {
        decryptor.decrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(())
      }
      (Aes128Gcm(decipher, auth_tag_length), true) => {
        let tag = decipher.finish();
        let tag_slice = tag.as_slice();
        let truncated_tag = if let Some(len) = auth_tag_length {
          &tag_slice[..len]
        } else {
          tag_slice
        };
        if truncated_tag == auth_tag {
          Ok(())
        } else {
          Err(DecipherError::DataAuthenticationFailed)
        }
      }
      (Aes128Gcm(..), false) => {
        Err(DecipherError::SetAutoPaddingFalseAes128GcmUnsupported)
      }
      (Aes256Gcm(decipher, auth_tag_length), true) => {
        let tag = decipher.finish();
        let tag_slice = tag.as_slice();
        let truncated_tag = if let Some(len) = auth_tag_length {
          &tag_slice[..len]
        } else {
          tag_slice
        };
        if truncated_tag == auth_tag {
          Ok(())
        } else {
          Err(DecipherError::DataAuthenticationFailed)
        }
      }
      (Aes256Gcm(..), false) => {
        Err(DecipherError::SetAutoPaddingFalseAes256GcmUnsupported)
      }
      (Aes256Cbc(decryptor), true) => {
        assert_block_len!(input.len(), 16);
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| DecipherError::CannotUnpadInputData)?;
        Ok(())
      }
      (Aes256Cbc(mut decryptor), false) => {
        decryptor.decrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(())
      }
      (Aes256Ctr(mut decryptor), _) => {
        decryptor.apply_keystream_b2b(input, output).unwrap();
        Ok(())
      }
      (Aes192Ctr(mut decryptor), _) => {
        decryptor.apply_keystream_b2b(input, output).unwrap();
        Ok(())
      }
      (Aes128Ctr(mut decryptor), _) => {
        decryptor.apply_keystream_b2b(input, output).unwrap();
        Ok(())
      }
    }
  }
}
