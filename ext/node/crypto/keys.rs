use deno_core::serde_json;
use deno_core::Resource;
use std::borrow::Cow;

pub trait KeyObject {
  fn symmetric_key_size(&self) -> usize {
    unimplemented!()
  }

  fn export(&self) -> &[u8] {
    unimplemented!()
  }

  fn export_jwk(&self, handle_rsa_pss: bool) -> serde_json::Value {
    unimplemented!()
  }
}

pub type KeyResource = Box<dyn KeyObject>;

impl Resource for KeyResource {
  fn name(&self) -> Cow<str> {
    "keyObject".into()
  }
}

pub fn create_key_object(alg: &str, key: &[u8]) -> KeyResource {
  match alg {
    "secret" => Box::new(SecretKeyObject { key: key.to_vec() }),
    _ => panic!("Unsupported algorithm"),
  }
}

pub struct SecretKeyObject {
  key: Vec<u8>,
}

impl KeyObject for SecretKeyObject {
  fn symmetric_key_size(&self) -> usize {
    self.key.len()
  }

  fn export(&self) -> &[u8] {
    &self.key
  }

  fn export_jwk(&self, _handle_rsa_pss: bool) -> serde_json::Value {
    serde_json::json!({
      "kty": "oct",
      "k": base64::encode_config(&self.key, base64::URL_SAFE_NO_PAD),
    })
  }
}
