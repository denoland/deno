// Copyright 2018-2026 the Deno authors. MIT license.

use aes::cipher::BlockDecryptMut;
use aes::cipher::KeyIvInit;
use aes::cipher::block_padding::Pkcs7;
use aes_gcm::AeadInPlace;
use aes_gcm::KeyInit;
use aes_gcm::Nonce;
use aes_gcm::aead::generic_array::ArrayLength;
use aes_gcm::aead::generic_array::typenum::U12;
use aes_gcm::aead::generic_array::typenum::U16;
use aes_gcm::aes::Aes128;
use aes_gcm::aes::Aes192;
use aes_gcm::aes::Aes256;
use aws_lc_rs::aead::Aad;
use aws_lc_rs::aead::CHACHA20_POLY1305;
use aws_lc_rs::aead::LessSafeKey;
use aws_lc_rs::aead::Nonce as AwsNonce;
use aws_lc_rs::aead::UnboundKey;
use ctr::Ctr32BE;
use ctr::Ctr64BE;
use ctr::Ctr128BE;
use ctr::cipher::StreamCipher;
use rsa::pkcs1::DecodeRsaPrivateKey;
use serde::Deserialize;
use sha1::Sha1;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;
use sha3::Sha3_256;
use sha3::Sha3_384;
use sha3::Sha3_512;

use crate::shared::*;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum DecryptError {
  #[class(inherit)]
  #[error(transparent)]
  General(
    #[from]
    #[inherit]
    SharedError,
  ),
  #[class(generic)]
  #[error(transparent)]
  Pkcs1(#[from] rsa::pkcs1::Error),
  #[class("DOMExceptionOperationError")]
  #[error("Decryption failed")]
  Failed,
  #[class(type)]
  #[error("invalid length")]
  InvalidLength,
  #[class(type)]
  #[error("invalid counter length. Currently supported 32/64/128 bits")]
  InvalidCounterLength,
  #[class(type)]
  #[error("invalid tag length")]
  InvalidTagLength,
  #[class("DOMExceptionOperationError")]
  #[error("invalid key or iv")]
  InvalidKeyOrIv,
  #[class("DOMExceptionOperationError")]
  #[error("tried to decrypt too much data")]
  TooMuchData,
  #[class(type)]
  #[error("iv length not equal to 12 or 16")]
  InvalidIvLength,
  #[class(type)]
  #[error("invalid ChaCha20-Poly1305 nonce length: expected 12 bytes")]
  InvalidChaChaNonceLength,
  #[class(type)]
  #[error("invalid ChaCha20-Poly1305 key length: expected 32 bytes")]
  InvalidChaChaKeyLength,
  #[class("DOMExceptionOperationError")]
  #[error("{0}")]
  Rsa(rsa::Error),
}

pub(crate) fn decrypt_rsa_oaep(
  key: &RawKeyData,
  hash: ShaHash,
  label: Vec<u8>,
  data: &[u8],
) -> Result<Vec<u8>, DecryptError> {
  let key = key.as_rsa_private_key()?;

  let private_key = rsa::RsaPrivateKey::from_pkcs1_der(key)?;
  let label = Some(String::from_utf8_lossy(&label).to_string());

  let padding = match hash {
    ShaHash::Sha1 => rsa::Oaep {
      digest: Box::<Sha1>::default(),
      mgf_digest: Box::<Sha1>::default(),
      label,
    },
    ShaHash::Sha256 => rsa::Oaep {
      digest: Box::<Sha256>::default(),
      mgf_digest: Box::<Sha256>::default(),
      label,
    },
    ShaHash::Sha384 => rsa::Oaep {
      digest: Box::<Sha384>::default(),
      mgf_digest: Box::<Sha384>::default(),
      label,
    },
    ShaHash::Sha512 => rsa::Oaep {
      digest: Box::<Sha512>::default(),
      mgf_digest: Box::<Sha512>::default(),
      label,
    },
    ShaHash::Sha3_256 => rsa::Oaep {
      digest: Box::<Sha3_256>::default(),
      mgf_digest: Box::<Sha3_256>::default(),
      label,
    },
    ShaHash::Sha3_384 => rsa::Oaep {
      digest: Box::<Sha3_384>::default(),
      mgf_digest: Box::<Sha3_384>::default(),
      label,
    },
    ShaHash::Sha3_512 => rsa::Oaep {
      digest: Box::<Sha3_512>::default(),
      mgf_digest: Box::<Sha3_512>::default(),
      label,
    },
  };

  private_key
    .decrypt(padding, data)
    .map_err(DecryptError::Rsa)
}

pub(crate) fn decrypt_aes_cbc(
  key: &RawKeyData,
  length: usize,
  iv: Vec<u8>,
  data: &[u8],
) -> Result<Vec<u8>, DecryptError> {
  let key = key.as_secret_key()?;

  // 2.
  let plaintext = match length {
    128 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
      let cipher = Aes128CbcDec::new_from_slices(key, &iv)
        .map_err(|_| DecryptError::InvalidKeyOrIv)?;

      cipher
        .decrypt_padded_vec_mut::<Pkcs7>(data)
        .map_err(|_| DecryptError::Failed)?
    }
    192 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes192CbcDec = cbc::Decryptor<aes::Aes192>;
      let cipher = Aes192CbcDec::new_from_slices(key, &iv)
        .map_err(|_| DecryptError::InvalidKeyOrIv)?;

      cipher
        .decrypt_padded_vec_mut::<Pkcs7>(data)
        .map_err(|_| DecryptError::Failed)?
    }
    256 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
      let cipher = Aes256CbcDec::new_from_slices(key, &iv)
        .map_err(|_| DecryptError::InvalidKeyOrIv)?;

      cipher
        .decrypt_padded_vec_mut::<Pkcs7>(data)
        .map_err(|_| DecryptError::Failed)?
    }
    _ => unreachable!(),
  };

  // 6.
  Ok(plaintext)
}

fn decrypt_aes_ctr_gen<B>(
  key: &[u8],
  counter: &[u8],
  data: &[u8],
) -> Result<Vec<u8>, DecryptError>
where
  B: KeyIvInit + StreamCipher,
{
  let mut cipher = B::new(key.into(), counter.into());

  let mut plaintext = data.to_vec();
  cipher
    .try_apply_keystream(&mut plaintext)
    .map_err(|_| DecryptError::TooMuchData)?;

  Ok(plaintext)
}

fn decrypt_aes_gcm_gen<N: ArrayLength<u8>>(
  key: &[u8],
  tag: &aes_gcm::Tag,
  nonce: &[u8],
  length: usize,
  additional_data: Vec<u8>,
  plaintext: &mut [u8],
) -> Result<(), DecryptError> {
  let nonce = Nonce::from_slice(nonce);
  match length {
    128 => {
      let cipher = aes_gcm::AesGcm::<Aes128, N>::new_from_slice(key)
        .map_err(|_| DecryptError::Failed)?;
      cipher
        .decrypt_in_place_detached(
          nonce,
          additional_data.as_slice(),
          plaintext,
          tag,
        )
        .map_err(|_| DecryptError::Failed)?
    }
    192 => {
      let cipher = aes_gcm::AesGcm::<Aes192, N>::new_from_slice(key)
        .map_err(|_| DecryptError::Failed)?;
      cipher
        .decrypt_in_place_detached(
          nonce,
          additional_data.as_slice(),
          plaintext,
          tag,
        )
        .map_err(|_| DecryptError::Failed)?
    }
    256 => {
      let cipher = aes_gcm::AesGcm::<Aes256, N>::new_from_slice(key)
        .map_err(|_| DecryptError::Failed)?;
      cipher
        .decrypt_in_place_detached(
          nonce,
          additional_data.as_slice(),
          plaintext,
          tag,
        )
        .map_err(|_| DecryptError::Failed)?
    }
    _ => return Err(DecryptError::InvalidLength),
  };

  Ok(())
}

pub(crate) fn decrypt_aes_ctr(
  key: &RawKeyData,
  key_length: usize,
  counter: &[u8],
  ctr_length: usize,
  data: &[u8],
) -> Result<Vec<u8>, DecryptError> {
  let key = key.as_secret_key()?;

  match ctr_length {
    32 => match key_length {
      128 => decrypt_aes_ctr_gen::<Ctr32BE<aes::Aes128>>(key, counter, data),
      192 => decrypt_aes_ctr_gen::<Ctr32BE<aes::Aes192>>(key, counter, data),
      256 => decrypt_aes_ctr_gen::<Ctr32BE<aes::Aes256>>(key, counter, data),
      _ => Err(DecryptError::InvalidLength),
    },
    64 => match key_length {
      128 => decrypt_aes_ctr_gen::<Ctr64BE<aes::Aes128>>(key, counter, data),
      192 => decrypt_aes_ctr_gen::<Ctr64BE<aes::Aes192>>(key, counter, data),
      256 => decrypt_aes_ctr_gen::<Ctr64BE<aes::Aes256>>(key, counter, data),
      _ => Err(DecryptError::InvalidLength),
    },
    128 => match key_length {
      128 => decrypt_aes_ctr_gen::<Ctr128BE<aes::Aes128>>(key, counter, data),
      192 => decrypt_aes_ctr_gen::<Ctr128BE<aes::Aes192>>(key, counter, data),
      256 => decrypt_aes_ctr_gen::<Ctr128BE<aes::Aes256>>(key, counter, data),
      _ => Err(DecryptError::InvalidLength),
    },
    _ => Err(DecryptError::InvalidCounterLength),
  }
}

pub(crate) fn decrypt_aes_gcm(
  key: &RawKeyData,
  length: usize,
  tag_length: usize,
  iv: Vec<u8>,
  additional_data: Option<Vec<u8>>,
  data: &[u8],
) -> Result<Vec<u8>, DecryptError> {
  let key = key.as_secret_key()?;
  let additional_data = additional_data.unwrap_or_default();

  // The `aes_gcm` crate only supports 128 bits tag length.
  //
  // Note that encryption won't fail, it instead truncates the tag
  // to the specified tag length as specified in the spec.
  if tag_length != 128 {
    return Err(DecryptError::InvalidTagLength);
  }

  let sep = data.len() - (tag_length / 8);
  let tag = &data[sep..];

  // The actual ciphertext, called plaintext because it is reused in place.
  let mut plaintext = data[..sep].to_vec();

  // Fixed 96-bit or 128-bit nonce
  match iv.len() {
    12 => decrypt_aes_gcm_gen::<U12>(
      key,
      tag.into(),
      &iv,
      length,
      additional_data,
      &mut plaintext,
    )?,
    16 => decrypt_aes_gcm_gen::<U16>(
      key,
      tag.into(),
      &iv,
      length,
      additional_data,
      &mut plaintext,
    )?,
    _ => return Err(DecryptError::InvalidIvLength),
  }

  Ok(plaintext)
}

pub(crate) fn decrypt_chacha20_poly1305(
  key: &RawKeyData,
  nonce: &[u8],
  additional_data: Option<Vec<u8>>,
  data: &[u8],
) -> Result<Vec<u8>, DecryptError> {
  let key_bytes = key.as_secret_key()?;
  if key_bytes.len() != 32 {
    return Err(DecryptError::InvalidChaChaKeyLength);
  }
  if nonce.len() != 12 {
    return Err(DecryptError::InvalidChaChaNonceLength);
  }
  // 16-byte Poly1305 tag is appended.
  if data.len() < 16 {
    return Err(DecryptError::Failed);
  }

  let unbound_key = UnboundKey::new(&CHACHA20_POLY1305, key_bytes)
    .map_err(|_| DecryptError::Failed)?;
  let opening_key = LessSafeKey::new(unbound_key);
  let aws_nonce = AwsNonce::try_assume_unique_for_key(nonce)
    .map_err(|_| DecryptError::Failed)?;
  let aad = additional_data.unwrap_or_default();

  let mut in_out = data.to_vec();
  let plaintext = opening_key
    .open_in_place(aws_nonce, Aad::from(&aad), &mut in_out)
    .map_err(|_| DecryptError::Failed)?;

  Ok(plaintext.to_vec())
}

pub(crate) fn decrypt_aes_ocb(
  key: &RawKeyData,
  length: usize,
  tag_length: usize,
  iv: Vec<u8>,
  additional_data: Option<Vec<u8>>,
  data: &[u8],
) -> Result<Vec<u8>, DecryptError> {
  use aes_gcm::aead::generic_array::GenericArray;
  use aes_gcm::aead::generic_array::typenum::U6;
  use aes_gcm::aead::generic_array::typenum::U7;
  use aes_gcm::aead::generic_array::typenum::U8;
  use aes_gcm::aead::generic_array::typenum::U9;
  use aes_gcm::aead::generic_array::typenum::U10;
  use aes_gcm::aead::generic_array::typenum::U11;
  use aes_gcm::aead::generic_array::typenum::U12;
  use aes_gcm::aead::generic_array::typenum::U13;
  use aes_gcm::aead::generic_array::typenum::U14;
  use aes_gcm::aead::generic_array::typenum::U15;
  use aes_gcm::aead::generic_array::typenum::U16;
  use ocb3::Ocb3;
  use ocb3::aead::AeadInPlace as Ocb3AeadInPlace;
  use ocb3::aead::KeyInit as Ocb3KeyInit;

  let key = key.as_secret_key()?;
  let additional_data = additional_data.unwrap_or_default();

  // The WICG spec permits a 64-, 96- or 128-bit tag for AES-OCB. Map each
  // permitted length to its `ocb3` TagSize typenum (U8/U12/U16); anything
  // else is an OperationError.
  let tag_size = match tag_length {
    64 => OcbTagSize::U8,
    96 => OcbTagSize::U12,
    128 => OcbTagSize::U16,
    _ => return Err(DecryptError::InvalidTagLength),
  };

  // RFC 7253 permits nonces up to 15 bytes; the `ocb3` crate supports 6..=15.
  // The NonceSize is a compile-time type parameter, so dispatch the runtime
  // length to the matching typenum. Lengths outside 6..=15 are unsupported.
  if iv.len() < 6 || iv.len() > 15 {
    return Err(DecryptError::InvalidIvLength);
  }

  // The trailing `tag_length / 8` bytes are the detached tag. Guard against
  // ciphertext shorter than the tag (the JS layer already rejects this, but
  // avoid an underflowing subtraction here).
  let tag_len = tag_length / 8;
  if data.len() < tag_len {
    return Err(DecryptError::Failed);
  }
  let sep = data.len() - tag_len;
  let tag_bytes = &data[sep..];

  // The actual ciphertext, called plaintext because it is reused in place.
  let mut plaintext = data[..sep].to_vec();

  // The tag GenericArray length must match the `ocb3` TagSize, so build it
  // inside the per-tag macro from `tag_bytes` (which is exactly `tag_len`).
  macro_rules! ocb_decrypt {
    ($aes:ty, $nonce:ty, $tag:ty) => {{
      let cipher = Ocb3::<$aes, $nonce, $tag>::new_from_slice(key)
        .map_err(|_| DecryptError::Failed)?;
      let nonce = GenericArray::<u8, $nonce>::from_slice(&iv);
      let tag = GenericArray::<u8, $tag>::from_slice(tag_bytes);
      cipher
        .decrypt_in_place_detached(nonce, &additional_data, &mut plaintext, tag)
        .map_err(|_| DecryptError::Failed)?;
    }};
  }
  macro_rules! ocb_decrypt_for_tag {
    ($nonce:ty, $tag:ty) => {
      match length {
        128 => ocb_decrypt!(aes::Aes128, $nonce, $tag),
        192 => ocb_decrypt!(aes::Aes192, $nonce, $tag),
        256 => ocb_decrypt!(aes::Aes256, $nonce, $tag),
        _ => return Err(DecryptError::InvalidLength),
      }
    };
  }
  macro_rules! ocb_decrypt_for_key {
    ($nonce:ty) => {
      match tag_size {
        OcbTagSize::U8 => ocb_decrypt_for_tag!($nonce, U8),
        OcbTagSize::U12 => ocb_decrypt_for_tag!($nonce, U12),
        OcbTagSize::U16 => ocb_decrypt_for_tag!($nonce, U16),
      }
    };
  }

  match iv.len() {
    6 => ocb_decrypt_for_key!(U6),
    7 => ocb_decrypt_for_key!(U7),
    8 => ocb_decrypt_for_key!(U8),
    9 => ocb_decrypt_for_key!(U9),
    10 => ocb_decrypt_for_key!(U10),
    11 => ocb_decrypt_for_key!(U11),
    12 => ocb_decrypt_for_key!(U12),
    13 => ocb_decrypt_for_key!(U13),
    14 => ocb_decrypt_for_key!(U14),
    15 => ocb_decrypt_for_key!(U15),
    _ => return Err(DecryptError::InvalidIvLength),
  }

  Ok(plaintext)
}

/// The AES-OCB tag sizes permitted by the WICG WebCrypto extension, mapping
/// each WICG `tagLength` to the `ocb3` TagSize typenum used at instantiation.
#[derive(Clone, Copy)]
enum OcbTagSize {
  U8,
  U12,
  U16,
}
