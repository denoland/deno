
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecryptArg {
  key: KeyData,
  algorithm: Algorithm,
  // RSA-OAEP
  hash: Option<CryptoHash>,
  label: Option<ZeroCopyBuf>,
  // AES-CBC
  iv: Option<ZeroCopyBuf>,
  length: Option<usize>,
}

pub async fn op_crypto_decrypt_key(
  _state: Rc<RefCell<OpState>>,
  args: DecryptArg,
  zero_copy: ZeroCopyBuf,
) -> Result<ZeroCopyBuf, AnyError> {
  let data = &*zero_copy;
  let algorithm = args.algorithm;

  match algorithm {
    Algorithm::RsaOaep => {
      let private_key: RsaPrivateKey =
        RsaPrivateKey::from_pkcs1_der(&*args.key.data)?;
      let label = args.label.map(|l| String::from_utf8_lossy(&*l).to_string());
      let padding = match args
        .hash
        .ok_or_else(|| type_error("Missing argument hash".to_string()))?
      {
        CryptoHash::Sha1 => PaddingScheme::OAEP {
          digest: Box::new(Sha1::new()),
          mgf_digest: Box::new(Sha1::new()),
          label,
        },
        CryptoHash::Sha256 => PaddingScheme::OAEP {
          digest: Box::new(Sha256::new()),
          mgf_digest: Box::new(Sha256::new()),
          label,
        },
        CryptoHash::Sha384 => PaddingScheme::OAEP {
          digest: Box::new(Sha384::new()),
          mgf_digest: Box::new(Sha384::new()),
          label,
        },
        CryptoHash::Sha512 => PaddingScheme::OAEP {
          digest: Box::new(Sha512::new()),
          mgf_digest: Box::new(Sha512::new()),
          label,
        },
      };

      Ok(
        private_key
          .decrypt(padding, data)
          .map_err(|e| {
            custom_error("DOMExceptionOperationError", e.to_string())
          })?
          .into(),
      )
    }
    Algorithm::AesCbc => {
      let key = &*args.key.data;
      let length = args
        .length
        .ok_or_else(|| type_error("Missing argument length".to_string()))?;
      let iv = args
        .iv
        .ok_or_else(|| type_error("Missing argument iv".to_string()))?;

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
      Ok(plaintext.into())
    }
    _ => Err(type_error("Unsupported algorithm".to_string())),
  }
}
