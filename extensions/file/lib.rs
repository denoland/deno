// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::null_opbuf;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::op_sync;
use deno_core::url::Url;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_core::ZeroCopyBuf;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Blob {
  pub data: Vec<u8>,
  pub media_type: String,
}

pub struct Location(pub Url);

#[derive(Debug, Default, Clone)]
pub struct BlobUrlStore(Arc<Mutex<HashMap<Url, Blob>>>);

impl BlobUrlStore {
  pub fn get(&self, mut url: Url) -> Result<Option<Blob>, AnyError> {
    let blob_store = self.0.lock().unwrap();
    url.set_fragment(None);
    Ok(blob_store.get(&url).cloned())
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
  media_type: String,
  zero_copy: Option<ZeroCopyBuf>,
) -> Result<String, AnyError> {
  let data = zero_copy.ok_or_else(null_opbuf)?;
  let blob = Blob {
    data: data.to_vec(),
    media_type,
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
  _: (),
) -> Result<(), AnyError> {
  let url = Url::parse(&url)?;
  let blob_store = state.borrow::<BlobUrlStore>();
  blob_store.remove(&url);
  Ok(())
}

pub fn init(
  blob_url_store: BlobUrlStore,
  maybe_location: Option<Url>,
) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:extensions/file",
      "01_file.js",
      "02_filereader.js",
      "03_blob_url.js",
    ))
    .ops(vec![
      (
        "op_file_create_object_url",
        op_sync(op_file_create_object_url),
      ),
      (
        "op_file_revoke_object_url",
        op_sync(op_file_revoke_object_url),
      ),
    ])
    .state(move |state| {
      state.put(blob_url_store.clone());
      if let Some(location) = maybe_location.clone() {
        state.put(Location(location));
      }
      Ok(())
    })
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_file.d.ts")
}
