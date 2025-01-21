// Copyright 2018-2025 the Deno authors. MIT license.

use hyper::header::AUTHORIZATION;
use hyper::HeaderMap;
use hyper::StatusCode;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::tokio::TokioExecutor;

use crate::CacheError;

pub struct CacheShard {
  client: Client<HttpConnector, http_body::Body>,
  endpoint: String,
  token: String,
}

impl CacheShard {
  pub fn new(endpoint: String, token: String) -> Self {
    let client = Client::builder(TokioExecutor::new())
      .pool_idle_timeout(std::time::Duration::from_secs(30))
      .build_http();
    Self {
      client,
      endpoint,
      token,
    }
  }

  pub async fn get_object(
    &self,
    object_key: &str,
  ) -> Result<Option<reqwest::Response>, CacheError> {
    let res = self
      .client
      .get(format!("{}/objects/{}", self.endpoint, object_key))
      .header(&AUTHORIZATION, format!("Bearer {}", self.token))
      .header("x-ryw", "1")
      .send()
      .await
      .map_err(|e| e.without_url())?;

    if res.status().is_success() {
      Ok(Some(res))
    } else if res.status() == StatusCode::NOT_FOUND {
      Ok(None)
    } else {
      Err(CacheError::RequestFailed {
        method: "GET",
        status: res.status(),
      })
    }
  }

  pub async fn put_object(
    &self,
    object_key: &str,
    headers: HeaderMap,
    body: reqwest::Body,
  ) -> Result<(), CacheError> {
    let res = self
      .client
      .put(format!("{}/objects/{}", self.endpoint, object_key))
      .headers(headers)
      .header(&AUTHORIZATION, format!("Bearer {}", self.token))
      .body(body)
      .send()
      .await
      .map_err(|e| e.without_url())?;

    if res.status().is_success() {
      Ok(())
    } else {
      Err(CacheError::RequestFailed {
        method: "GET",
        status: res.status(),
      })
    }
  }
}
