// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use deno_core::OpState;
use deno_core::futures::Stream;
use deno_error::JsErrorBox;
use deno_fetch::CreateHttpClientOptions;
use deno_fetch::create_http_client;
use deno_permissions::PermissionsContainer;
use deno_tls::Proxy;
use deno_tls::RootCertStoreProvider;
use deno_tls::TlsKeys;
use deno_tls::rustls::RootCertStore;
use denokv_remote::MetadataEndpoint;
use denokv_remote::Remote;
use denokv_remote::RemoteResponse;
use denokv_remote::RemoteTransport;
use http_body_util::BodyExt;
use url::Url;

use crate::DatabaseHandler;

#[derive(Clone)]
pub struct HttpOptions {
  pub user_agent: String,
  pub root_cert_store_provider: Option<Arc<dyn RootCertStoreProvider>>,
  pub proxy: Option<Proxy>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub client_cert_chain_and_key: TlsKeys,
}

impl HttpOptions {
  pub fn root_cert_store(&self) -> Result<Option<RootCertStore>, JsErrorBox> {
    Ok(match &self.root_cert_store_provider {
      Some(provider) => Some(provider.get_or_try_init()?.clone()),
      None => None,
    })
  }
}

pub struct RemoteDbHandler {
  http_options: HttpOptions,
}

impl RemoteDbHandler {
  pub fn new(http_options: HttpOptions) -> Self {
    Self { http_options }
  }
}

pub struct PermissionChecker {
  state: Rc<RefCell<OpState>>,
}

impl Clone for PermissionChecker {
  fn clone(&self) -> Self {
    Self {
      state: self.state.clone(),
    }
  }
}

impl denokv_remote::RemotePermissions for PermissionChecker {
  fn check_net_url(&self, url: &Url) -> Result<(), JsErrorBox> {
    let mut state = self.state.borrow_mut();
    let permissions = state.borrow_mut::<PermissionsContainer>();
    permissions
      .check_net_url(url, "Deno.openKv")
      .map_err(JsErrorBox::from_err)
  }
}

#[derive(Clone)]
pub struct FetchClient(deno_fetch::Client);
pub struct FetchResponse(http::Response<deno_fetch::ResBody>);

impl RemoteTransport for FetchClient {
  type Response = FetchResponse;
  async fn post(
    &self,
    url: Url,
    headers: http::HeaderMap,
    body: Bytes,
  ) -> Result<(Url, http::StatusCode, Self::Response), JsErrorBox> {
    let body = deno_fetch::ReqBody::full(body);
    let mut req = http::Request::new(body);
    *req.method_mut() = http::Method::POST;
    *req.uri_mut() =
      url.as_str().parse().map_err(|e: http::uri::InvalidUri| {
        JsErrorBox::type_error(e.to_string())
      })?;
    *req.headers_mut() = headers;

    let res = self
      .0
      .clone()
      .send(req)
      .await
      .map_err(JsErrorBox::from_err)?;
    let status = res.status();
    Ok((url, status, FetchResponse(res)))
  }
}

impl RemoteResponse for FetchResponse {
  async fn bytes(self) -> Result<Bytes, JsErrorBox> {
    Ok(self.0.collect().await?.to_bytes())
  }
  fn stream(
    self,
  ) -> impl Stream<Item = Result<Bytes, JsErrorBox>> + Send + Sync {
    self.0.into_body().into_data_stream()
  }
  async fn text(self) -> Result<String, JsErrorBox> {
    let bytes = self.bytes().await?;
    Ok(
      std::str::from_utf8(&bytes)
        .map_err(JsErrorBox::from_err)?
        .into(),
    )
  }
}

#[async_trait(?Send)]
impl DatabaseHandler for RemoteDbHandler {
  type DB = Remote<PermissionChecker, FetchClient>;

  async fn open(
    &self,
    state: Rc<RefCell<OpState>>,
    path: Option<String>,
  ) -> Result<Self::DB, JsErrorBox> {
    const ENV_VAR_NAME: &str = "DENO_KV_ACCESS_TOKEN";

    let Some(url) = path else {
      return Err(JsErrorBox::type_error("Missing database url"));
    };

    let Ok(parsed_url) = Url::parse(&url) else {
      return Err(JsErrorBox::type_error(format!(
        "Invalid database url: {}",
        url
      )));
    };

    {
      let mut state = state.borrow_mut();
      let permissions = state.borrow_mut::<PermissionsContainer>();
      permissions
        .check_env(ENV_VAR_NAME)
        .map_err(JsErrorBox::from_err)?;
      permissions
        .check_net_url(&parsed_url, "Deno.openKv")
        .map_err(JsErrorBox::from_err)?;
    }

    let access_token = std::env::var(ENV_VAR_NAME)
      .map_err(anyhow::Error::from)
      .with_context(|| {
        "Missing DENO_KV_ACCESS_TOKEN environment variable. Please set it to your access token from https://console.deno.com"
      }).map_err(|e| JsErrorBox::generic(e.to_string()))?;

    let metadata_endpoint = MetadataEndpoint {
      url: parsed_url.clone(),
      access_token: access_token.clone(),
    };

    let options = &self.http_options;
    let client = create_http_client(
      &options.user_agent,
      CreateHttpClientOptions {
        root_cert_store: options.root_cert_store()?,
        ca_certs: vec![],
        proxy: options.proxy.clone(),
        dns_resolver: Default::default(),
        unsafely_ignore_certificate_errors: options
          .unsafely_ignore_certificate_errors
          .clone(),
        client_cert_chain_and_key: options
          .client_cert_chain_and_key
          .clone()
          .try_into()
          .unwrap(),
        pool_max_idle_per_host: None,
        pool_idle_timeout: None,
        http1: false,
        http2: true,
        local_address: None,
        client_builder_hook: None,
      },
    )
    .map_err(JsErrorBox::from_err)?;
    let fetch_client = FetchClient(client);

    let permissions = PermissionChecker {
      state: state.clone(),
    };

    // Eagerly validate that the URL points at a real Deno KV database before
    // returning the connection. Without this, `Deno.openKv("https://invalid")`
    // succeeds and only fails later — with an opaque error — when the
    // connection is first used (or hangs indefinitely on a network error).
    // See https://github.com/denoland/deno/issues/22248.
    validate_metadata_endpoint(&fetch_client, &metadata_endpoint).await?;

    let remote = Remote::new(fetch_client, permissions, metadata_endpoint);

    Ok(remote)
  }
}

/// Fetches the KV Connect metadata endpoint and verifies that the response is a
/// valid Deno KV database metadata document, so that an invalid URL fails fast
/// at `Deno.openKv` time with a clear error message.
async fn validate_metadata_endpoint(
  client: &FetchClient,
  metadata_endpoint: &MetadataEndpoint,
) -> Result<(), JsErrorBox> {
  use denokv_proto::DatabaseMetadata;
  use denokv_proto::MetadataExchangeRequest;

  let url = &metadata_endpoint.url;
  let body = serde_json::to_vec(&MetadataExchangeRequest {
    supported_versions: vec![1, 2, 3],
  })
  .unwrap();

  let (_, status, res) = client
    .post(url.clone(), metadata_endpoint.headers(), body.into())
    .await
    .map_err(|err| {
      JsErrorBox::type_error(format!(
        "Could not open Deno KV database: failed to connect to '{url}': {err}"
      ))
    })?;

  if !status.is_success() {
    let body = res.text().await.unwrap_or_default();
    let body = body.trim();
    let detail = if body.is_empty() {
      String::new()
    } else {
      format!(": {}", body.chars().take(512).collect::<String>())
    };
    return Err(JsErrorBox::type_error(format!(
      "Could not open Deno KV database: the metadata endpoint at '{url}' \
       responded with status {status}{detail}"
    )));
  }

  let body = res.text().await.map_err(|err| {
    JsErrorBox::type_error(format!(
      "Could not open Deno KV database: failed to read the metadata response \
       from '{url}': {err}"
    ))
  })?;

  let metadata: DatabaseMetadata =
    serde_json::from_str(&body).map_err(|err| {
      JsErrorBox::type_error(format!(
        "Could not open Deno KV database: '{url}' is not a valid KV Connect \
         endpoint (failed to parse metadata response: {err})"
      ))
    })?;

  if !matches!(metadata.version, 1..=3) {
    return Err(JsErrorBox::type_error(format!(
      "Could not open Deno KV database: '{url}' reported unsupported KV \
       Connect metadata version {}",
      metadata.version
    )));
  }

  Ok(())
}
