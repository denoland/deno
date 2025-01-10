// Copyright 2018-2025 the Deno authors. MIT license.

use anyhow::Context;
use hyper::header::AUTHORIZATION;
use hyper::HeaderMap;
use hyper::StatusCode;

pub struct CacheShard {
  client: reqwest::Client,
  endpoint: String,
  token: String,
}

impl CacheShard {
  pub fn new(endpoint: String, token: String) -> Self {
    let client = reqwest::Client::builder()
      .build()
      .expect("Failed to build reqwest client");
    Self {
      client,
      endpoint,
      token,
    }
  }

  pub async fn get_object(
    &self,
    object_key: &str,
  ) -> anyhow::Result<Option<reqwest::Response>> {
    let res = self
      .client
      .get(format!("{}/objects/{}", self.endpoint, object_key))
      .header(&AUTHORIZATION, format!("Bearer {}", self.token))
      .header("x-ryw", "1")
      .send()
      .await
      .map_err(|e| e.without_url())
      .with_context(|| "failed to start cache GET request")?;

    if res.status().is_success() {
      Ok(Some(res))
    } else if res.status() == StatusCode::NOT_FOUND {
      Ok(None)
    } else {
      Err(anyhow::anyhow!(
        "cache GET request failed: {}",
        res.status()
      ))
    }
  }

  pub async fn put_object(
    &self,
    object_key: &str,
    headers: HeaderMap,
    body: reqwest::Body,
  ) -> anyhow::Result<()> {
    let res = self
      .client
      .put(format!("{}/objects/{}", self.endpoint, object_key))
      .headers(headers)
      .header(&AUTHORIZATION, format!("Bearer {}", self.token))
      .body(body)
      .send()
      .await
      .map_err(|e| e.without_url())
      .with_context(|| "failed to start cache PUT request")?;

    if res.status().is_success() {
      Ok(())
    } else {
      Err(anyhow::anyhow!(
        "cache PUT request failed: {}",
        res.status()
      ))
    }
  }
}
