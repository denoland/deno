// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

mod sqlite_cache;

pub use sqlite_cache::SqliteBackedCache;

use async_trait::async_trait;
use deno_core::error::AnyError;
use deno_core::include_js_files;
use deno_core::serde::Deserialize;
use deno_core::serde::Serialize;
use deno_core::Extension;
use std::path::PathBuf;

use deno_core::op;
use deno_core::OpState;
use deno_core::ResourceId;
use std::cell::RefCell;
use std::rc::Rc;

#[async_trait]
pub trait Cache: Clone {
  async fn storage_open(&self, cache_name: String) -> Result<i64, AnyError>;
  async fn storage_has(&self, cache_name: String) -> Result<bool, AnyError>;
  async fn storage_delete(&self, cache_name: String) -> Result<bool, AnyError>;

  async fn put(
    &self,
    request_response: CachePutRequest,
  ) -> Result<Option<ResourceId>, AnyError>;
  async fn r#match(
    &self,
    request: CacheMatchRequest,
  ) -> Result<Option<CacheMatchResponse>, AnyError>;
  async fn delete(&self, request: CacheDeleteRequest)
    -> Result<bool, AnyError>;
}

#[op]
pub async fn op_cache_storage_open<CA>(
  state: Rc<RefCell<OpState>>,
  cache_name: String,
) -> Result<i64, AnyError>
where
  CA: Cache + 'static,
{
  let cache = state.borrow().borrow::<CA>().clone();
  cache.storage_open(cache_name).await
}

#[op]
pub async fn op_cache_storage_has<CA>(
  state: Rc<RefCell<OpState>>,
  cache_name: String,
) -> Result<bool, AnyError>
where
  CA: Cache + 'static,
{
  let cache = state.borrow().borrow::<CA>().clone();
  cache.storage_has(cache_name).await
}

#[op]
pub async fn op_cache_storage_delete<CA>(
  state: Rc<RefCell<OpState>>,
  cache_name: String,
) -> Result<bool, AnyError>
where
  CA: Cache + 'static,
{
  let cache = state.borrow().borrow::<CA>().clone();
  cache.storage_delete(cache_name).await
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CachePutRequest {
  pub cache_id: i64,
  pub request_url: String,
  pub response_headers: Vec<(String, String)>,
  pub request_headers: Vec<(String, String)>,
  pub response_has_body: bool,
  pub response_status: u16,
  pub response_status_text: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CacheMatchRequest {
  pub cache_id: i64,
  pub request_url: String,
  pub response_headers: Vec<(String, String)>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheMatchResponse {
  pub response_status: u16,
  pub response_status_text: String,
  pub response_headers: Vec<(String, String)>,
  pub response_body_key: Option<String>,
  pub response_body_rid: Option<u32>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CacheDeleteRequest {
  pub cache_id: i64,
  pub request_url: String,
}

#[op]
pub async fn op_cache_put<CA>(
  state: Rc<RefCell<OpState>>,
  request_response: CachePutRequest,
) -> Result<Option<ResourceId>, AnyError>
where
  CA: Cache + 'static,
{
  let cache = state.borrow().borrow::<CA>().clone();
  cache.put(request_response).await
}

#[op]
pub async fn op_cache_match<CA>(
  state: Rc<RefCell<OpState>>,
  request: CacheMatchRequest,
) -> Result<Option<CacheMatchResponse>, AnyError>
where
  CA: Cache + 'static,
{
  let cache = state.borrow().borrow::<CA>().clone();
  cache.r#match(request).await
}

#[op]
pub async fn op_cache_delete<CA>(
  state: Rc<RefCell<OpState>>,
  request: CacheDeleteRequest,
) -> Result<bool, AnyError>
where
  CA: Cache + 'static,
{
  let cache = state.borrow().borrow::<CA>().clone();
  cache.delete(request).await
}

pub fn init<CA: Cache + 'static>(cache: CA) -> Extension {
  Extension::builder()
    .js(include_js_files!(
      prefix "deno:ext/cache",
      "01_cache.js",
    ))
    .ops(vec![
      op_cache_storage_open::decl::<CA>(),
      op_cache_storage_has::decl::<CA>(),
      op_cache_storage_delete::decl::<CA>(),
      op_cache_put::decl::<CA>(),
      op_cache_match::decl::<CA>(),
      op_cache_delete::decl::<CA>(),
    ])
    .state(move |state| {
      state.put(cache.clone());
      Ok(())
    })
    .build()
}

pub fn get_declaration() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lib.deno_cache.d.ts")
}
