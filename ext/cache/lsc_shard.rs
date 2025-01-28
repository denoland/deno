// Copyright 2018-2025 the Deno authors. MIT license.

use std::convert::Infallible;

use bytes::Bytes;
use http::Method;
use http::Request;
use http::Response;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::BodyExt;
use http_body_util::Either;
use http_body_util::Empty;
use hyper::body::Incoming;
use hyper::header::AUTHORIZATION;
use hyper::HeaderMap;
use hyper::StatusCode;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::tokio::TokioExecutor;

use crate::CacheError;

type ClientBody =
  Either<UnsyncBoxBody<Bytes, CacheError>, UnsyncBoxBody<Bytes, Infallible>>;

pub struct CacheShard {
  client: Client<HttpConnector, ClientBody>,
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
  ) -> Result<Option<Response<Incoming>>, CacheError> {
    let body = Either::Right(UnsyncBoxBody::new(Empty::new()));
    let req = Request::builder()
      .method(Method::GET)
      .uri(format!("{}/objects/{}", self.endpoint, object_key))
      .header(&AUTHORIZATION, format!("Bearer {}", self.token))
      .header("x-ryw", "1")
      .body(body)
      .unwrap();

    let res = self.client.request(req).await?;

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

  pub async fn put_object_empty(
    &self,
    object_key: &str,
    headers: HeaderMap,
  ) -> Result<(), CacheError> {
    let body = Either::Right(UnsyncBoxBody::new(Empty::new()));
    let mut builder = Request::builder()
      .method(Method::PUT)
      .uri(format!("{}/objects/{}", self.endpoint, object_key))
      .header(&AUTHORIZATION, format!("Bearer {}", self.token));

    for (key, val) in headers.iter() {
      builder = builder.header(key, val)
    }

    let req = builder.body(body).unwrap();

    let res = self.client.request(req).await?;

    if res.status().is_success() {
      Ok(())
    } else {
      let status = res.status();
      log::debug!(
        "Response body {:#?}",
        res.into_body().collect().await?.to_bytes()
      );
      Err(CacheError::RequestFailed {
        method: "PUT",
        status,
      })
    }
  }

  pub async fn put_object(
    &self,
    object_key: &str,
    headers: HeaderMap,
    body: UnsyncBoxBody<Bytes, CacheError>,
  ) -> Result<(), CacheError> {
    let mut builder = Request::builder()
      .method(Method::PUT)
      .uri(format!("{}/objects/{}", self.endpoint, object_key))
      .header(&AUTHORIZATION, format!("Bearer {}", self.token));

    for (key, val) in headers.iter() {
      builder = builder.header(key, val)
    }

    let req = builder.body(Either::Left(body)).unwrap();

    let res = self.client.request(req).await?;

    if res.status().is_success() {
      Ok(())
    } else {
      let status = res.status();
      log::debug!(
        "Response body {:#?}",
        res.into_body().collect().await?.to_bytes()
      );
      Err(CacheError::RequestFailed {
        method: "PUT",
        status,
      })
    }
  }
}
