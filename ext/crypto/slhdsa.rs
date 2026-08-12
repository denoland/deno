// Copyright 2018-2026 the Deno authors. MIT license.

use deno_error::JsErrorBox;
use fips205::traits::SerDes;
use fips205::traits::Signer;
use fips205::traits::Verifier;
use rsa::pkcs8;
use rsa::pkcs8::der::Decode;
use spki::der::Encode;
use spki::der::asn1::BitString;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum SlhDsaError {
  #[class(generic)]
  #[error("Invalid SLH-DSA key data")]
  InvalidKeyData,
  #[class(generic)]
  #[error("Unknown SLH-DSA variant")]
  UnknownVariant,
  #[class(generic)]
  #[error("SLH-DSA operation failed")]
  OperationFailed,
  #[class(generic)]
  #[error("SLH-DSA context must be at most 255 bytes")]
  ContextTooLong,
  #[class(generic)]
  #[error(transparent)]
  Der(#[from] spki::der::Error),
}

#[derive(Clone, Copy)]
pub(crate) struct SlhDsaVariant {
  pub(crate) name: &'static str,
  oid: const_oid::ObjectIdentifier,
  pub(crate) public_key_len: usize,
  pub(crate) private_key_len: usize,
}

macro_rules! variants {
  ($callback:ident) => {
    $callback! {
      (Sha2_128s, "SLH-DSA-SHA2-128s", "2.16.840.1.101.3.4.3.20", slh_dsa_sha2_128s),
      (Sha2_128f, "SLH-DSA-SHA2-128f", "2.16.840.1.101.3.4.3.21", slh_dsa_sha2_128f),
      (Sha2_192s, "SLH-DSA-SHA2-192s", "2.16.840.1.101.3.4.3.22", slh_dsa_sha2_192s),
      (Sha2_192f, "SLH-DSA-SHA2-192f", "2.16.840.1.101.3.4.3.23", slh_dsa_sha2_192f),
      (Sha2_256s, "SLH-DSA-SHA2-256s", "2.16.840.1.101.3.4.3.24", slh_dsa_sha2_256s),
      (Sha2_256f, "SLH-DSA-SHA2-256f", "2.16.840.1.101.3.4.3.25", slh_dsa_sha2_256f),
      (Shake128s, "SLH-DSA-SHAKE-128s", "2.16.840.1.101.3.4.3.26", slh_dsa_shake_128s),
      (Shake128f, "SLH-DSA-SHAKE-128f", "2.16.840.1.101.3.4.3.27", slh_dsa_shake_128f),
      (Shake192s, "SLH-DSA-SHAKE-192s", "2.16.840.1.101.3.4.3.28", slh_dsa_shake_192s),
      (Shake192f, "SLH-DSA-SHAKE-192f", "2.16.840.1.101.3.4.3.29", slh_dsa_shake_192f),
      (Shake256s, "SLH-DSA-SHAKE-256s", "2.16.840.1.101.3.4.3.30", slh_dsa_shake_256s),
      (Shake256f, "SLH-DSA-SHAKE-256f", "2.16.840.1.101.3.4.3.31", slh_dsa_shake_256f),
    }
  };
}

macro_rules! declare_variant_id {
  ($(($id:ident, $name:literal, $oid:literal, $module:path),)+) => {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum SlhDsaVariantId {
      $($id,)+
    }
  };
}

variants!(declare_variant_id);

macro_rules! variant_from_name {
  ($(($id:ident, $name:literal, $oid:literal, $module:path),)+) => {
    pub(crate) fn variant_from_name(name: &str) -> Option<SlhDsaVariantId> {
      match name {
        $($name => Some(SlhDsaVariantId::$id),)+
        _ => None,
      }
    }
  };
}

variants!(variant_from_name);

macro_rules! variant_from_oid {
  ($(($id:ident, $name:literal, $oid:literal, $module:path),)+) => {
    fn variant_from_oid(oid: &const_oid::ObjectIdentifier) -> Option<SlhDsaVariantId> {
      $(
        if oid == &const_oid::ObjectIdentifier::new_unwrap($oid) {
          return Some(SlhDsaVariantId::$id);
        }
      )+
      None
    }
  };
}

variants!(variant_from_oid);

macro_rules! variant_params {
  ($(($id:ident, $name:literal, $oid:literal, $module:ident),)+) => {
    pub(crate) fn params(variant: SlhDsaVariantId) -> SlhDsaVariant {
      match variant {
        $(SlhDsaVariantId::$id => SlhDsaVariant {
          name: $name,
          oid: const_oid::ObjectIdentifier::new_unwrap($oid),
          public_key_len: fips205::$module::PK_LEN,
          private_key_len: fips205::$module::SK_LEN,
        },)+
      }
    }
  };
}

variants!(variant_params);

fn to_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], SlhDsaError> {
  bytes.try_into().map_err(|_| SlhDsaError::InvalidKeyData)
}

macro_rules! with_variant {
  ($(($id:ident, $name:literal, $oid:literal, $module:ident),)+) => {
    macro_rules! dispatch_variant {
      ($variant:expr, $module_ident:ident, $body:block) => {
        match $variant {
          $(
            SlhDsaVariantId::$id => {
              use fips205::$module as $module_ident;
              $body
            }
          )+
        }
      };
    }
  };
}

variants!(with_variant);

pub(crate) fn generate(
  variant: SlhDsaVariantId,
) -> Result<(Vec<u8>, Vec<u8>), SlhDsaError> {
  dispatch_variant!(variant, alg, {
    let (public_key, private_key) =
      alg::try_keygen().map_err(|_| SlhDsaError::OperationFailed)?;
    Ok((
      public_key.into_bytes().to_vec(),
      private_key.into_bytes().to_vec(),
    ))
  })
}

pub(crate) fn public_from_private(
  variant: SlhDsaVariantId,
  private_key: &[u8],
) -> Result<Vec<u8>, SlhDsaError> {
  dispatch_variant!(variant, alg, {
    let key = alg::PrivateKey::try_from_bytes(&to_array(private_key)?)
      .map_err(|_| SlhDsaError::InvalidKeyData)?;
    Ok(key.get_public_key().into_bytes().to_vec())
  })
}

pub(crate) fn sign(
  variant: SlhDsaVariantId,
  private_key: &[u8],
  data: &[u8],
  context: Option<&[u8]>,
) -> Result<Vec<u8>, SlhDsaError> {
  let context = context.unwrap_or_default();
  if context.len() > 255 {
    return Err(SlhDsaError::ContextTooLong);
  }
  dispatch_variant!(variant, alg, {
    let key = alg::PrivateKey::try_from_bytes(&to_array(private_key)?)
      .map_err(|_| SlhDsaError::InvalidKeyData)?;
    key
      .try_sign(data, context, true)
      .map(|sig| sig.to_vec())
      .map_err(|_| SlhDsaError::OperationFailed)
  })
}

pub(crate) fn verify(
  variant: SlhDsaVariantId,
  public_key: &[u8],
  data: &[u8],
  signature: &[u8],
  context: Option<&[u8]>,
) -> bool {
  let context = context.unwrap_or_default();
  if context.len() > 255 {
    return false;
  }
  dispatch_variant!(variant, alg, {
    let Ok(public_key_bytes) = to_array(public_key) else {
      return false;
    };
    let Ok(public_key) = alg::PublicKey::try_from_bytes(&public_key_bytes)
    else {
      return false;
    };
    let Ok(signature) = to_array(signature) else {
      return false;
    };
    public_key.verify(data, &signature, context)
  })
}

pub(crate) fn import_spki(
  expected: SlhDsaVariantId,
  data: &[u8],
) -> Result<Vec<u8>, SlhDsaError> {
  let info = spki::SubjectPublicKeyInfoRef::try_from(data)
    .map_err(|_| SlhDsaError::InvalidKeyData)?;
  if variant_from_oid(&info.algorithm.oid) != Some(expected) {
    return Err(SlhDsaError::InvalidKeyData);
  }
  let key = info
    .subject_public_key
    .as_bytes()
    .ok_or(SlhDsaError::InvalidKeyData)?;
  if key.len() != params(expected).public_key_len {
    return Err(SlhDsaError::InvalidKeyData);
  }
  Ok(key.to_vec())
}

pub(crate) fn import_pkcs8(
  expected: SlhDsaVariantId,
  data: &[u8],
) -> Result<Vec<u8>, SlhDsaError> {
  let info = pkcs8::PrivateKeyInfo::from_der(data)
    .map_err(|_| SlhDsaError::InvalidKeyData)?;
  if variant_from_oid(&info.algorithm.oid) != Some(expected) {
    return Err(SlhDsaError::InvalidKeyData);
  }
  let private_key = spki::der::asn1::OctetStringRef::from_der(info.private_key)
    .map_err(|_| SlhDsaError::InvalidKeyData)?;
  let private_key = private_key.as_bytes();
  if private_key.len() != params(expected).private_key_len {
    return Err(SlhDsaError::InvalidKeyData);
  }
  public_from_private(expected, private_key)?;
  Ok(private_key.to_vec())
}

pub(crate) fn export_spki(
  variant: SlhDsaVariantId,
  public_key: &[u8],
) -> Result<Vec<u8>, SlhDsaError> {
  let p = params(variant);
  if public_key.len() != p.public_key_len {
    return Err(SlhDsaError::InvalidKeyData);
  }
  let bit_string = BitString::from_bytes(public_key)
    .map_err(|_| SlhDsaError::InvalidKeyData)?;
  let info = spki::SubjectPublicKeyInfo {
    algorithm: spki::AlgorithmIdentifierOwned {
      oid: p.oid,
      parameters: None,
    },
    subject_public_key: bit_string,
  };
  info.to_der().map_err(|_| SlhDsaError::OperationFailed)
}

pub(crate) fn export_pkcs8(
  variant: SlhDsaVariantId,
  private_key: &[u8],
) -> Result<Vec<u8>, SlhDsaError> {
  let p = params(variant);
  if private_key.len() != p.private_key_len {
    return Err(SlhDsaError::InvalidKeyData);
  }
  let inner = spki::der::asn1::OctetString::new(private_key)
    .map_err(|_| SlhDsaError::InvalidKeyData)?
    .to_der()
    .map_err(|_| SlhDsaError::OperationFailed)?;
  let info = pkcs8::PrivateKeyInfo {
    algorithm: pkcs8::AlgorithmIdentifierRef {
      oid: p.oid,
      parameters: None,
    },
    private_key: &inner,
    public_key: None,
  };
  info.to_der().map_err(|_| SlhDsaError::OperationFailed)
}

impl From<SlhDsaError> for JsErrorBox {
  fn from(value: SlhDsaError) -> Self {
    JsErrorBox::from_err(value)
  }
}
