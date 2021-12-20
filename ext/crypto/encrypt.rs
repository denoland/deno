use std::cell::RefCell;
use std::rc::Rc;

use crate::shared::*;
use block_modes::BlockMode;
use deno_core::error::type_error;
use deno_core::error::AnyError;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use rand::rngs::OsRng;
use rsa::pkcs1::FromRsaPublicKey;
use rsa::PaddingScheme;
use rsa::PublicKey;
use serde::Deserialize;
use sha1::Digest;
use sha1::Sha1;
use sha2::Sha256;
use sha2::Sha384;
use sha2::Sha512;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptOptions {
  key: RawKeyData,
  #[serde(flatten)]
  algorithm: EncryptAlgorithm,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", tag = "algorithm")]
pub enum EncryptAlgorithm {
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
}
pub async fn op_crypto_encrypt(
  _state: Rc<RefCell<OpState>>,
  opts: EncryptOptions,
  data: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let key = opts.key;
  let fun = move || match opts.algorithm {
    EncryptAlgorithm::RsaOaep { hash, label } => {
      encrypt_rsa_oaep(key, hash, label, &data)
    }
    EncryptAlgorithm::AesCbc { iv, length } => {
      encrypt_aes_cbc(key, length, iv, &data)
    }
  };
  let buf = tokio::task::spawn_blocking(fun).await.unwrap()?;
  Ok(buf.into())
}

fn encrypt_rsa_oaep(
  key: RawKeyData,
  hash: ShaHash,
  label: Vec<u8>,
  data: &[u8],
) -> Result<Vec<u8>, deno_core::anyhow::Error> {
  let label = String::from_utf8_lossy(&label).to_string();

  let public_key = key.as_rsa_public_key()?;
  let public_key = rsa::RsaPublicKey::from_pkcs1_der(&public_key)
    .map_err(|_| operation_error("failed to decode public key"))?;
  let mut rng = OsRng;
  let padding = match hash {
    ShaHash::Sha1 => PaddingScheme::OAEP {
      digest: Box::new(Sha1::new()),
      mgf_digest: Box::new(Sha1::new()),
      label: Some(label),
    },
    ShaHash::Sha256 => PaddingScheme::OAEP {
      digest: Box::new(Sha256::new()),
      mgf_digest: Box::new(Sha256::new()),
      label: Some(label),
    },
    ShaHash::Sha384 => PaddingScheme::OAEP {
      digest: Box::new(Sha384::new()),
      mgf_digest: Box::new(Sha384::new()),
      label: Some(label),
    },
    ShaHash::Sha512 => PaddingScheme::OAEP {
      digest: Box::new(Sha512::new()),
      mgf_digest: Box::new(Sha512::new()),
      label: Some(label),
    },
  };
  let encrypted = public_key
    .encrypt(&mut rng, padding, data)
    .map_err(|_| operation_error("Encryption failed"))?;
  Ok(encrypted)
}

fn encrypt_aes_cbc(
  key: RawKeyData,
  length: usize,
  iv: Vec<u8>,
  data: &[u8],
) -> Result<Vec<u8>, deno_core::anyhow::Error> {
  let key = key.as_secret_key()?;
  let ciphertext = match length {
    128 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes128Cbc =
        block_modes::Cbc<aes::Aes128, block_modes::block_padding::Pkcs7>;

      let cipher = Aes128Cbc::new_from_slices(key, &iv)?;
      cipher.encrypt_vec(data)
    }
    192 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes192Cbc =
        block_modes::Cbc<aes::Aes192, block_modes::block_padding::Pkcs7>;

      let cipher = Aes192Cbc::new_from_slices(key, &iv)?;
      cipher.encrypt_vec(data)
    }
    256 => {
      // Section 10.3 Step 2 of RFC 2315 https://www.rfc-editor.org/rfc/rfc2315
      type Aes256Cbc =
        block_modes::Cbc<aes::Aes256, block_modes::block_padding::Pkcs7>;

      let cipher = Aes256Cbc::new_from_slices(key, &iv)?;
      cipher.encrypt_vec(data)
    }
    _ => return Err(type_error("invalid length")),
  };
  Ok(ciphertext)
}
