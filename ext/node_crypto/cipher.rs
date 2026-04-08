// Copyright 2018-2026 the Deno authors. MIT license.

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
use poly1305::universal_hash::UniversalHash;
use subtle::ConstantTimeEq;

type Tag = Option<Vec<u8>>;

type Aes128Gcm = aead_gcm_stream::AesGcm<aes::Aes128>;
type Aes256Gcm = aead_gcm_stream::AesGcm<aes::Aes256>;

struct ChaCha20Poly1305Cipher {
  chacha: chacha20::ChaCha20,
  poly: poly1305::Poly1305,
  aad_buf: Vec<u8>,
  aad_flushed: bool,
  ct_len: u64,
  auth_tag_length: usize,
}

impl ChaCha20Poly1305Cipher {
  fn new(key: &[u8], iv: &[u8], auth_tag_length: usize) -> Self {
    let chacha_key = chacha20::Key::from_slice(key);
    let nonce = chacha20::Nonce::from_slice(iv);

    // Create ChaCha20 cipher for poly1305 key generation
    let mut chacha = chacha20::ChaCha20::new(chacha_key, nonce);

    // Generate poly1305 key from first 32 bytes of ChaCha20 keystream (block 0)
    let mut poly_key_block = [0u8; 64];
    chacha
      .try_apply_keystream(&mut poly_key_block)
      .expect("keystream");

    let poly_key = poly1305::Key::from_slice(&poly_key_block[..32]).to_owned();
    let poly = poly1305::Poly1305::new(&poly_key);

    // chacha is now at counter=1, ready for encryption

    ChaCha20Poly1305Cipher {
      chacha,
      poly,
      aad_buf: Vec::new(),
      aad_flushed: false,
      ct_len: 0,
      auth_tag_length,
    }
  }

  fn set_aad(&mut self, aad: &[u8]) {
    self.aad_buf.extend_from_slice(aad);
  }

  /// Flush buffered AAD to Poly1305 (padded once). Called lazily on first
  /// encrypt/decrypt/compute_tag so that multiple setAAD() calls are
  /// concatenated before padding.
  fn flush_aad(&mut self) {
    if !self.aad_flushed {
      self.aad_flushed = true;
      self.poly.update_padded(&self.aad_buf);
    }
  }

  fn encrypt(&mut self, input: &[u8], output: &mut [u8]) {
    self.flush_aad();
    output[..input.len()].copy_from_slice(input);
    // Keystream exhaustion only after ~256 GB; practically unreachable.
    self.chacha.try_apply_keystream(output).unwrap();
    self.ct_len += output.len() as u64;
    self.poly.update_padded(output);
  }

  fn decrypt(&mut self, input: &[u8], output: &mut [u8]) {
    self.flush_aad();
    // For decrypt: feed ciphertext to poly BEFORE decrypting
    self.ct_len += input.len() as u64;
    self.poly.update_padded(input);
    output[..input.len()].copy_from_slice(input);
    // Keystream exhaustion only after ~256 GB; practically unreachable.
    self.chacha.try_apply_keystream(output).unwrap();
  }

  fn compute_tag(mut self) -> Vec<u8> {
    self.flush_aad();
    let aad_len = self.aad_buf.len() as u64;
    let mut poly = self.poly;
    // Feed aad_len and ct_len as le64 in one 16-byte block
    let mut len_block = [0u8; 16];
    len_block[..8].copy_from_slice(&aad_len.to_le_bytes());
    len_block[8..].copy_from_slice(&self.ct_len.to_le_bytes());
    poly.update(&[poly1305::Block::clone_from_slice(&len_block)]);
    let tag_output = poly.finalize();
    let tag: &[u8] = tag_output.as_ref();
    let mut tag_vec = tag.to_vec();
    tag_vec.truncate(self.auth_tag_length);
    tag_vec
  }
}

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
  DesEde3Cbc(Box<cbc::Encryptor<des::TdesEde3>>),
  ChaCha20Poly1305(Box<ChaCha20Poly1305Cipher>),
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
  DesEde3Cbc(Box<cbc::Decryptor<des::TdesEde3>>),
  ChaCha20Poly1305(Box<ChaCha20Poly1305Cipher>, Option<usize>),
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

fn is_valid_chacha20_poly1305_tag_length(tag_len: usize) -> bool {
  (1..=16).contains(&tag_len)
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
        if key.len() != 16 {
          return Err(CipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(CipherError::InvalidInitializationVector);
        }
        Aes128Cbc(Box::new(cbc::Encryptor::new(key.into(), iv.into())))
      }
      "aes-128-ecb" => {
        if key.len() != 16 {
          return Err(CipherError::InvalidKeyLength);
        }
        Aes128Ecb(Box::new(ecb::Encryptor::new(key.into())))
      }
      "aes-192-ecb" => {
        if key.len() != 24 {
          return Err(CipherError::InvalidKeyLength);
        }
        Aes192Ecb(Box::new(ecb::Encryptor::new(key.into())))
      }
      "aes-256-ecb" => {
        if key.len() != 32 {
          return Err(CipherError::InvalidKeyLength);
        }
        Aes256Ecb(Box::new(ecb::Encryptor::new(key.into())))
      }
      "aes-128-gcm" => {
        if key.len() != aes::Aes128::key_size() {
          return Err(CipherError::InvalidKeyLength);
        }
        if iv.is_empty() {
          return Err(CipherError::InvalidInitializationVector);
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
        if iv.is_empty() {
          return Err(CipherError::InvalidInitializationVector);
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
      "des-ede3-cbc" => {
        if key.len() != 24 {
          return Err(CipherError::InvalidKeyLength);
        }
        if iv.len() != 8 {
          return Err(CipherError::InvalidInitializationVector);
        }
        DesEde3Cbc(Box::new(cbc::Encryptor::new(key.into(), iv.into())))
      }
      "chacha20-poly1305" => {
        if key.len() != 32 {
          return Err(CipherError::InvalidKeyLength);
        }
        if iv.len() != 12 {
          return Err(CipherError::InvalidInitializationVector);
        }
        let tag_len = auth_tag_length.unwrap_or(16);
        if !is_valid_chacha20_poly1305_tag_length(tag_len) {
          return Err(CipherError::InvalidAuthTag(tag_len));
        }
        ChaCha20Poly1305(Box::new(ChaCha20Poly1305Cipher::new(
          key, iv, tag_len,
        )))
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
      ChaCha20Poly1305(cipher) => {
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
      DesEde3Cbc(encryptor) => {
        assert!(input.len().is_multiple_of(8));
        for (input, output) in input.chunks(8).zip(output.chunks_mut(8)) {
          encryptor.encrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      ChaCha20Poly1305(cipher) => {
        cipher.encrypt(input, output);
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
      (ChaCha20Poly1305(cipher), _) => {
        let tag = cipher.compute_tag();
        Ok(Some(tag))
      }
      (DesEde3Cbc(encryptor), true) => {
        let _ = (*encryptor)
          .encrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| CipherError::CannotPadInputData)?;
        Ok(None)
      }
      (DesEde3Cbc(mut encryptor), false) => {
        encryptor.encrypt_block_b2b_mut(
          GenericArray::from_slice(input),
          GenericArray::from_mut_slice(output),
        );
        Ok(None)
      }
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
      ChaCha20Poly1305(cipher) => {
        let tag = cipher.compute_tag();
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
  #[error("Unsupported state or unable to authenticate data")]
  DataAuthenticationFailed,
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
      Self::CannotUnpadInputData => {
        deno_error::PropertyValue::String("ERR_OSSL_EVP_BAD_DECRYPT".into())
      }
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
        if key.len() != 16 {
          return Err(DecipherError::InvalidKeyLength);
        }
        if iv.len() != 16 {
          return Err(DecipherError::InvalidInitializationVector);
        }
        Aes128Cbc(Box::new(cbc::Decryptor::new(key.into(), iv.into())))
      }
      "aes-128-ecb" => {
        if key.len() != 16 {
          return Err(DecipherError::InvalidKeyLength);
        }
        Aes128Ecb(Box::new(ecb::Decryptor::new(key.into())))
      }
      "aes-192-ecb" => {
        if key.len() != 24 {
          return Err(DecipherError::InvalidKeyLength);
        }
        Aes192Ecb(Box::new(ecb::Decryptor::new(key.into())))
      }
      "aes-256-ecb" => {
        if key.len() != 32 {
          return Err(DecipherError::InvalidKeyLength);
        }
        Aes256Ecb(Box::new(ecb::Decryptor::new(key.into())))
      }
      "aes-128-gcm" => {
        if key.len() != aes::Aes128::key_size() {
          return Err(DecipherError::InvalidKeyLength);
        }
        if iv.is_empty() {
          return Err(DecipherError::InvalidInitializationVector);
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
        if iv.is_empty() {
          return Err(DecipherError::InvalidInitializationVector);
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
      "des-ede3-cbc" => {
        if key.len() != 24 {
          return Err(DecipherError::InvalidKeyLength);
        }
        if iv.len() != 8 {
          return Err(DecipherError::InvalidInitializationVector);
        }
        DesEde3Cbc(Box::new(cbc::Decryptor::new(key.into(), iv.into())))
      }
      "chacha20-poly1305" => {
        if key.len() != 32 {
          return Err(DecipherError::InvalidKeyLength);
        }
        if iv.len() != 12 {
          return Err(DecipherError::InvalidInitializationVector);
        }
        let tag_len = auth_tag_length.unwrap_or(16);
        if !is_valid_chacha20_poly1305_tag_length(tag_len) {
          return Err(DecipherError::InvalidAuthTag(tag_len));
        }
        ChaCha20Poly1305(
          Box::new(ChaCha20Poly1305Cipher::new(key, iv, tag_len)),
          auth_tag_length,
        )
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
      Decipher::Aes128Gcm(_, None) | Decipher::Aes256Gcm(_, None) => {
        if !is_valid_gcm_tag_length(length) {
          return Err(DecipherError::InvalidAuthTag(length));
        }
      }
      Decipher::ChaCha20Poly1305(_, Some(tag_len)) => {
        if *tag_len != length {
          return Err(DecipherError::InvalidAuthTag(length));
        }
      }
      Decipher::ChaCha20Poly1305(_, None) => {
        // Default tag length is 16; reject anything else
        if length != 16 {
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
      ChaCha20Poly1305(decipher, _) => {
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
      DesEde3Cbc(decryptor) => {
        assert!(input.len().is_multiple_of(8));
        for (input, output) in input.chunks(8).zip(output.chunks_mut(8)) {
          decryptor.decrypt_block_b2b_mut(input.into(), output.into());
        }
      }
      ChaCha20Poly1305(decipher, _) => {
        decipher.decrypt(input, output);
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
          | ChaCha20Poly1305(..)
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
        if !input.is_empty() {
          assert_block_len!(input.len(), 16);
          decryptor.decrypt_block_b2b_mut(
            GenericArray::from_slice(input),
            GenericArray::from_mut_slice(output),
          );
        }
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
        if !input.is_empty() {
          assert_block_len!(input.len(), 16);
          decryptor.decrypt_block_b2b_mut(
            GenericArray::from_slice(input),
            GenericArray::from_mut_slice(output),
          );
        }
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
        if !input.is_empty() {
          assert_block_len!(input.len(), 16);
          decryptor.decrypt_block_b2b_mut(
            GenericArray::from_slice(input),
            GenericArray::from_mut_slice(output),
          );
        }
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
        if !input.is_empty() {
          assert_block_len!(input.len(), 16);
          decryptor.decrypt_block_b2b_mut(
            GenericArray::from_slice(input),
            GenericArray::from_mut_slice(output),
          );
        }
        Ok(())
      }
      (Aes128Gcm(decipher, auth_tag_length), _) => {
        let tag = decipher.finish();
        let tag_slice = tag.as_slice();
        let truncated_tag = if let Some(len) = auth_tag_length {
          &tag_slice[..len]
        } else {
          tag_slice
        };
        if truncated_tag.ct_eq(auth_tag).into() {
          Ok(())
        } else {
          Err(DecipherError::DataAuthenticationFailed)
        }
      }
      (Aes256Gcm(decipher, auth_tag_length), _) => {
        let tag = decipher.finish();
        let tag_slice = tag.as_slice();
        let truncated_tag = if let Some(len) = auth_tag_length {
          &tag_slice[..len]
        } else {
          tag_slice
        };
        if truncated_tag.ct_eq(auth_tag).into() {
          Ok(())
        } else {
          Err(DecipherError::DataAuthenticationFailed)
        }
      }
      (ChaCha20Poly1305(decipher, _), _) => {
        let expected_tag = decipher.compute_tag();
        if auth_tag.is_empty() {
          return Err(DecipherError::DataAuthenticationFailed);
        }
        if expected_tag.ct_eq(auth_tag).into() {
          Ok(())
        } else {
          Err(DecipherError::DataAuthenticationFailed)
        }
      }
      (Aes256Cbc(decryptor), true) => {
        assert_block_len!(input.len(), 16);
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| DecipherError::CannotUnpadInputData)?;
        Ok(())
      }
      (Aes256Cbc(mut decryptor), false) => {
        if !input.is_empty() {
          assert_block_len!(input.len(), 16);
          decryptor.decrypt_block_b2b_mut(
            GenericArray::from_slice(input),
            GenericArray::from_mut_slice(output),
          );
        }
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
      (DesEde3Cbc(decryptor), true) => {
        assert_block_len!(input.len(), 8);
        let _ = (*decryptor)
          .decrypt_padded_b2b_mut::<Pkcs7>(input, output)
          .map_err(|_| DecipherError::CannotUnpadInputData)?;
        Ok(())
      }
      (DesEde3Cbc(mut decryptor), false) => {
        if !input.is_empty() {
          assert_block_len!(input.len(), 8);
          decryptor.decrypt_block_b2b_mut(
            GenericArray::from_slice(input),
            GenericArray::from_mut_slice(output),
          );
        }
        Ok(())
      }
    }
  }
}
