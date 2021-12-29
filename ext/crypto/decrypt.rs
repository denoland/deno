use std::cell::RefCell;
use std::rc::Rc;

use crate::shared::*;
use aes::BlockEncrypt;
use aes::NewBlockCipher;
use block_modes::BlockMode;
use ctr::cipher::NewCipher;
use ctr::cipher::StreamCipher;
use ctr::flavors::Ctr128BE;
use ctr::flavors::Ctr32BE;
use ctr::flavors::Ctr64BE;
use ctr::flavors::CtrFlavor;
use ctr::Ctr;
use deno_core::error::custom_error;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use rsa::pkcs1::FromRsaPrivateKey;
use rsa::PaddingScheme;
use serde::Deserialize;
use sha1::Digest;
use sha1::Sha1;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptOptions {
  key: RawKeyData,
  #[serde(flatten)]
  algorithm: DecryptAlgorithm,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "algorithm")]
pub enum DecryptAlgorithm {
  #[serde(rename = "RSA-OAEP")]
  RsaOaep {
    hash: ShaHash,
    #[serde(with = "serde_bytes")]
    label: Vec<u8>,
  },
  #[serde(rename = "AES-CBC", rename_all = "camelCase")]
  AesCbc {
    #[serde(with = "serde_bytes")]
    iv: Vec<u8>,
    length: usize,
  },
  #[serde(rename = "AES-CTR", rename_all = "camelCase")]
  AesCtr {
    #[serde(with = "serde_bytes")]
    counter: Vec<u8>,
    ctr_length: usize,
    key_length: usize,
  },
}

pub async fn op_crypto_decrypt(
  _state: Rc<RefCell<OpState>>,
  opts: DecryptOptions,
  data: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let key = opts.key;
  let fun = move || match opts.algorithm {
    DecryptAlgorithm::RsaOaep { hash, label } => {
      decrypt_rsa_oaep(key, hash, label, &data)
    }
    DecryptAlgorithm::AesCbc { iv, length } => {
      decrypt_aes_cbc(key, length, iv, &data)
    }
    DecryptAlgorithm::AesCtr {
      counter,
      ctr_length,
      key_length,
    } => decrypt_aes_ctr(key, key_length, &counter, ctr_length, &data),
  };
  let buf = tokio::task::spawn_blocking(fun).await.unwrap()?;
  Ok(buf.into())
}

fn decrypt_rsa_oaep(
  key: RawKeyData,
  hash: ShaHash,
  label: Vec<u8>,
  data: &[u8],
) -> Result<Vec<u8>, deno_core::anyhow::Error> {
  let key = key.as_rsa_private_key()?;

  let private_key = rsa::RsaPrivateKey::from_pkcs1_der(key)?;
  let label = Some(String::from_utf8_lossy(&label).to_string());

  let padding = match hash {
    ShaHash::Sha1 => PaddingScheme::OAEP {
      digest: Box::new(Sha1::new()),
      mgf_digest: Box::new(Sha1::new()),
      label,
    },
    ShaHash::Sha256 => PaddingScheme::OAEP {
      digest: Box::new(Sha256::new()),
      mgf_digest: Box::new(Sha256::new()),
      label,
    },
    ShaHash::Sha384 => PaddingScheme::OAEP {
      digest: Box::new(Sha384::new()),
      mgf_digest: Box::new(Sha384::new()),
      label,
    },
    ShaHash::Sha512 => PaddingScheme::OAEP {
      digest: Box::new(Sha512::new()),
      mgf_digest: Box::new(Sha512::new()),
      label,
    },
  };

  private_key
    .decrypt(padding, data)
    .map_err(|e| custom_error("DOMExceptionOperationError", e.to_string()))
}

fn decrypt_aes_cbc(
  key: RawKeyData,
  length: usize,
  iv: Vec<u8>,
  data: &[u8],
) -> Result<Vec<u8>, deno_core::anyhow::Error> {
  let key = key.as_secret_key()?;

  // 2.
  let plaintext = match length {
    128 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes128Cbc =
        block_modes::Cbc<aes::Aes128, block_modes::block_padding::Pkcs7>;
      let cipher = Aes128Cbc::new_from_slices(key, &iv)?;

      cipher.decrypt_vec(data).map_err(|_| {
        custom_error(
          "DOMExceptionOperationError",
          "Decryption failed".to_string(),
        )
      })?
    }
    192 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes192Cbc =
        block_modes::Cbc<aes::Aes192, block_modes::block_padding::Pkcs7>;
      let cipher = Aes192Cbc::new_from_slices(key, &iv)?;

      cipher.decrypt_vec(data).map_err(|_| {
        custom_error(
          "DOMExceptionOperationError",
          "Decryption failed".to_string(),
        )
      })?
    }
    256 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes256Cbc =
        block_modes::Cbc<aes::Aes256, block_modes::block_padding::Pkcs7>;
      let cipher = Aes256Cbc::new_from_slices(key, &iv)?;

      cipher.decrypt_vec(data).map_err(|_| {
        custom_error(
          "DOMExceptionOperationError",
          "Decryption failed".to_string(),
        )
      })?
    }
    _ => unreachable!(),
  };

  // 6.
  Ok(plaintext)
}

fn decrypt_aes_ctr_gen<B, F>(
  key: &[u8],
  counter: &[u8],
  data: &[u8],
) -> Result<Vec<u8>, AnyError>
where
  B: BlockEncrypt + NewBlockCipher,
  F: CtrFlavor<B::BlockSize>,
{
  let mut cipher = Ctr::<B, F>::new(key.into(), counter.into());

  let mut plaintext = data.to_vec();
  cipher
    .try_apply_keystream(&mut plaintext)
    .map_err(|_| operation_error("tried to decrypt too much data"))?;

  Ok(plaintext)
}

fn decrypt_aes_ctr(
  key: RawKeyData,
  key_length: usize,
  counter: &[u8],
  ctr_length: usize,
  data: &[u8],
) -> Result<Vec<u8>, deno_core::anyhow::Error> {
  let key = key.as_secret_key()?;

  match ctr_length {
    32 => match key_length {
      128 => decrypt_aes_ctr_gen::<aes::Aes128, Ctr32BE>(key, counter, data),
      192 => decrypt_aes_ctr_gen::<aes::Aes192, Ctr32BE>(key, counter, data),
      256 => decrypt_aes_ctr_gen::<aes::Aes256, Ctr32BE>(key, counter, data),
      _ => Err(type_error("invalid length")),
    },
    64 => match key_length {
      128 => decrypt_aes_ctr_gen::<aes::Aes128, Ctr64BE>(key, counter, data),
      192 => decrypt_aes_ctr_gen::<aes::Aes192, Ctr64BE>(key, counter, data),
      256 => decrypt_aes_ctr_gen::<aes::Aes256, Ctr64BE>(key, counter, data),
      _ => Err(type_error("invalid length")),
    },
    128 => match key_length {
      128 => decrypt_aes_ctr_gen::<aes::Aes128, Ctr128BE>(key, counter, data),
      192 => decrypt_aes_ctr_gen::<aes::Aes192, Ctr128BE>(key, counter, data),
      256 => decrypt_aes_ctr_gen::<aes::Aes256, Ctr128BE>(key, counter, data),
      _ => Err(type_error("invalid length")),
    },
    _ => Err(type_error(
      "invalid counter length. Currently supported 32/64/128 bits",
    )),
  }
}
