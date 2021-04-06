// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_core::JsRuntime;
use deno_core::ModuleSpecifier;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct Blob {
  pub data: Vec<u8>,
  pub r#type: String,
}

pub struct Location(pub Url);

#[derive(Default, Clone)]
pub struct BlobUrlStore(Arc<Mutex<HashMap<Url, Blob>>>);

impl BlobUrlStore {
  pub fn get(&self, url: &ModuleSpecifier) -> Result<Option<Blob>, AnyError> {
    let blob_store = self.0.lock().unwrap();
    Ok(blob_store.get(url).cloned())
  }

  pub fn insert(&self, blob: Blob, maybe_location: Option<Url>) -> Url {
    let origin = if let Some(location) = maybe_location {
      location.origin().ascii_serialization()
    } else {
      "null".to_string()
    };
    let id = Uuid::new_v4();
    let url = Url::parse(&format!("blob:{}/{}", origin, id)).unwrap();

    let mut blob_store = self.0.lock().unwrap();
    blob_store.insert(url.clone(), blob);

    url
  }

  pub fn remove(&self, url: &ModuleSpecifier) {
    let mut blob_store = self.0.lock().unwrap();
    blob_store.remove(&url);
  }
}

pub fn op_file_create_object_url(
  state: &mut deno_core::OpState,
  r#type: String,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  let data = zero_copy.ok_or_else(null_opbuf)?;
  let blob = Blob {
    data: data.to_vec(),
    r#type,
  };

  let maybe_location = state.try_borrow::<Location>();
  let blob_store = state.borrow::<BlobUrlStore>();

  let url =
    blob_store.insert(blob, maybe_location.map(|location| location.0.clone()));

  Ok(url.to_string())
}

pub fn op_file_revoke_object_url(
  state: &mut deno_core::OpState,
  url: String,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<(), AnyError> {
  let url = Url::parse(&url)?;
  let blob_store = state.borrow::<BlobUrlStore>();
  blob_store.remove(&url);
  Ok(())
}

/// Load and execute the javascript code.
pub fn init(isolate: &mut JsRuntime) {
  let files = vec![
    ("deno:op_crates/file/01_file.js", include_str!("01_file.js")),
    (
      "deno:op_crates/file/02_filereader.js",
      include_str!("02_filereader.js"),
    ),
    (
      "deno:op_crates/file/03_blob_url.js",
      include_str!("03_blob_url.js"),
    ),
  ];
  for (url, source_code) in files {
    isolate.execute(url, source_code).unwrap();
  }
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_file.d.ts")
}
